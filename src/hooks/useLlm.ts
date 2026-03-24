import { listen } from '@tauri-apps/api/event';
import { type MutableRefObject, useCallback, useRef, useState } from 'react';
import { generateId } from '../lib/utils';
import { generateDeckV2, generateAiImage, type AgentLogEntry } from '../services/llm';
import type { AgentStatus, DeckData, Message, Slide } from '../types';

// ── AI image injection helpers ─────────────────────────────────────────────

function injectImageUrl(html: string, prompt: string, url: string): string {
  const ep = prompt.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return html
    // img src="" ... data-prompt="PROMPT"
    .replace(new RegExp(`src=""([^>]{0,300}?)data-prompt="${ep}"`, 'g'), `src="${url}"$1data-prompt="${prompt}"`)
    // data-prompt="PROMPT" ... src=""
    .replace(new RegExp(`data-prompt="${ep}"([^>]{0,300}?)src=""`, 'g'), `data-prompt="${prompt}"$1src="${url}"`)
    // background-image:url('') ... data-prompt="PROMPT"
    .replace(new RegExp(`background-image:url\\(''\\)([^>]{0,300}?)data-prompt="${ep}"`, 'g'), `background-image:url('${url}')$1data-prompt="${prompt}"`)
    // data-prompt="PROMPT" ... background-image:url('')
    .replace(new RegExp(`data-prompt="${ep}"([^>]{0,300}?)background-image:url\\(''\\)`, 'g'), `data-prompt="${prompt}"$1background-image:url('${url}')`);
}

function fillAiGenImages(
  deck: DeckData,
  onDeckData: (d: DeckData | null) => void,
  streamingDeck: React.MutableRefObject<DeckData | null>,
) {
  const prompts = new Set<string>();
  const promptRe = /data-prompt="([^"]+)"/g;
  for (const slide of deck.slides) {
    let m;
    while ((m = promptRe.exec(slide.html)) !== null) prompts.add(m[1]);
    promptRe.lastIndex = 0;
  }
  if (prompts.size === 0) return;

  for (const prompt of prompts) {
    generateAiImage(prompt).then(url => {
      // Notify iframe directly via custom event
      window.dispatchEvent(new CustomEvent('deck-image-ready', { detail: { prompt, url } }));
      // Update slide HTML for thumbnails
      const current = streamingDeck.current;
      if (!current) return;
      const updated: DeckData = {
        ...current,
        slides: current.slides.map(s => ({ ...s, html: injectImageUrl(s.html, prompt, url) })),
      };
      streamingDeck.current = updated;
      onDeckData(updated);
    }).catch(() => { /* silent: image generation is optional */ });
  }
}

interface UseLlmProps {
  messages: Message[];
  deckData: DeckData | null;
  notes: string;
  onMessages: (msgs: Message[]) => void;
  onDeckData: (data: DeckData | null) => void;
  onNotes: (notes: string) => void;
}

export function useLlm({ messages, deckData, notes, onMessages, onDeckData, onNotes }: UseLlmProps) {
  const [isLoading, setIsLoading] = useState(false);
  const [agentStatus, setAgentStatus] = useState<AgentStatus | null>(null);
  const [agentLog, setAgentLog] = useState<AgentLogEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const abortRef = useRef(false);
  // Track latest deck during streaming so slide-ready can append to it
  const streamingDeck = useRef<DeckData | null>(null);

  const simulateStreaming = useCallback(async (text: string, onChunk: (t: string) => void) => {
    const words = text.split(' ');
    let current = '';
    for (const word of words) {
      if (abortRef.current) break;
      current += (current ? ' ' : '') + word;
      onChunk(current);
      await new Promise(r => setTimeout(r, 18));
    }
  }, []);

  const handleSend = useCallback(async (input: string, numSlides: number = 8, language: string = 'auto') => {
    if (!input.trim() || isLoading) return;
    setError(null);
    setAgentLog([]);
    streamingDeck.current = null;
    abortRef.current = false;

    const meta = [
      `${numSlides} slides`,
      language && language !== 'auto' ? `language: ${language}` : '',
    ].filter(Boolean).join(', ');

    const userMsg: Message = {
      id: generateId(),
      role: 'user',
      content: meta ? `${input.trim()}\n[${meta}]` : input.trim(),
      timestamp: Date.now(),
    };

    const newMessages = [...messages, userMsg];
    onMessages(newMessages);
    setIsLoading(true);
    setAgentStatus({ type: 'thinking', message: 'Starting...' });

    // ── Realtime agent status events ──────────────────────────────────────────
    const unlistenStatus = listen<AgentLogEntry>('agent-status', (event) => {
      const entry = event.payload;
      setAgentLog(prev => {
        let idx = -1;
      for (let j = prev.length - 1; j >= 0; j--) { if (prev[j].agent === entry.agent) { idx = j; break; } }
        if (idx >= 0 && prev[idx].status === 'thinking' && entry.status === 'done') {
          const next = [...prev];
          next[idx] = entry;
          return next;
        }
        return [...prev, entry];
      });
      setAgentStatus({ type: entry.status === 'done' ? 'generating' : 'thinking', message: entry.message });
    });

    // ── deck-started: initialize empty preview as soon as theme is known ──────
    const unlistenDeckStarted = listen<{ title: string; slideCount: number; theme: DeckData['theme'] }>('deck-started', (event) => {
      const { title, slideCount, theme } = event.payload;
      const initial: DeckData = {
        title,
        theme,
        slides: [],
        metadata: { slideCount, generatedAt: Date.now() },
      };
      streamingDeck.current = initial;
      onDeckData(initial);
    });

    // ── slide-ready: append each slide as it finishes rendering ───────────────
    const unlistenSlide = listen<{ index: number; id: string; slide_type: string; html: string }>('slide-ready', (event) => {
      const { index, id, slide_type, html } = event.payload;
      const newSlide: Slide = { id, type: slide_type as Slide['type'], html };

      const current = streamingDeck.current;
      if (current) {
        let slides = [...current.slides];
        const existingIdx = slides.findIndex(s => s.id === id);
        if (existingIdx >= 0) {
          slides[existingIdx] = newSlide;
        } else {
          slides.push(newSlide);
        }
        // Insert in order by index (id = "s1", "s2", ...)
        slides.sort((a, b) => parseInt(a.id.slice(1)) - parseInt(b.id.slice(1)));
        
        const updated = { ...current, slides };
        streamingDeck.current = updated;
        onDeckData(updated);
      }
    });

    try {
      const history = newMessages.map(m => ({ role: m.role, content: m.content }));

      const result = await generateDeckV2({
        history,
        currentDeck: deckData,
        notes,
        language: language || 'auto',
        numSlides,
      });

      // Sync final state from result (authoritative)
      if (result.agentLog?.length) setAgentLog(result.agentLog);

      if (result.deckData) {
        const finalDeck = result.deckData as DeckData;
        streamingDeck.current = finalDeck;
        onDeckData(finalDeck);
        // Fill ai-gen-image placeholders asynchronously
        fillAiGenImages(finalDeck, onDeckData, streamingDeck);
      } else if (result.slideEdits?.length && deckData) {
        const updatedSlides: Slide[] = deckData.slides.map(slide => {
          const edit = result.slideEdits.find(e => e.slideId === slide.id);
          return edit ? { ...slide, html: edit.html } : slide;
        });
        onDeckData({ ...deckData, slides: updatedSlides });
      }

      if (result.notes) onNotes(result.notes);

      const aiMsg: Message = {
        id: generateId(),
        role: 'assistant',
        content: '',
        timestamp: Date.now(),
      };
      const withAi = [...newMessages, aiMsg];
      onMessages(withAi);

      await simulateStreaming(result.coachMessage || 'Done!', (text) => {
        if (abortRef.current) return;
        onMessages(withAi.map(m => m.id === aiMsg.id ? { ...m, content: text } : m));
      });

    } catch (e) {
      setError(String(e));
      onMessages([...newMessages, {
        id: generateId(),
        role: 'assistant',
        content: `Error: ${e}`,
        timestamp: Date.now(),
      }]);
    } finally {
      // Cleanup all listeners
      Promise.all([unlistenStatus, unlistenDeckStarted, unlistenSlide])
        .then(fns => fns.forEach(f => f()));
      setIsLoading(false);
      setAgentStatus(null);
    }
  }, [isLoading, messages, deckData, notes, onMessages, onDeckData, onNotes, simulateStreaming]);

  const handleStop = useCallback(() => {
    abortRef.current = true;
    setIsLoading(false);
    setAgentStatus(null);
  }, []);

  return { isLoading, agentStatus, agentLog, error, handleSend, handleStop };
}
