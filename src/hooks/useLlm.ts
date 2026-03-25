import { listen } from '@tauri-apps/api/event';
import { type MutableRefObject, useCallback, useRef, useState } from 'react';
import { generateId } from '../lib/utils';
import { generateDeckV2, generateAiImage, type AgentLogEntry } from '../services/llm';
import type { AgentStatus, DeckData, Message, Slide } from '../types';

// ── AI image injection helpers ─────────────────────────────────────────────

function injectImageUrl(html: string, prompt: string, url: string): string {
  const ep = prompt.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const dp = `data-prompt="${ep}"`;
  // Match src="anything" (empty or existing URL) that appears alongside data-prompt
  const srcAny = `src="[^"]*"`;
  const srcAnyS = `src='[^']*'`;
  return html
    // src="..." ... data-prompt="PROMPT"
    .replace(new RegExp(`${srcAny}([^>]*?)${dp}`, 'g'), `src="${url}"$1${dp}`)
    .replace(new RegExp(`${srcAnyS}([^>]*?)${dp}`, 'g'), `src="${url}"$1${dp}`)
    // data-prompt="PROMPT" ... src="..."
    .replace(new RegExp(`${dp}([^>]*?)${srcAny}`, 'g'), `${dp}$1src="${url}"`)
    .replace(new RegExp(`${dp}([^>]*?)${srcAnyS}`, 'g'), `${dp}$1src="${url}"`)
    // background-image
    .replace(new RegExp(`background-image:url\\([^)]*\\)([^>]*?)${dp}`, 'g'), `background-image:url('${url}')$1${dp}`)
    .replace(new RegExp(`${dp}([^>]*?)background-image:url\\([^)]*\\)`, 'g'), `${dp}$1background-image:url('${url}')`);
}

function fillAiGenImages(
  deck: DeckData,
  onDeckData: (d: DeckData | null) => void,
  streamingDeck: React.MutableRefObject<DeckData | null>,
  onImageStatus?: (status: string | null) => void,
) {
  const prompts = new Set<string>();
  const promptRe = /data-prompt="([^"]+)"/g;

  // Scan individual slides + masterHtml
  for (const slide of deck.slides) {
    let m;
    while ((m = promptRe.exec(slide.html)) !== null) prompts.add(m[1]);
    promptRe.lastIndex = 0;
  }
  if (deck.masterHtml) {
    let m;
    while ((m = promptRe.exec(deck.masterHtml)) !== null) prompts.add(m[1]);
    promptRe.lastIndex = 0;
  }
  if (prompts.size === 0) return;

  // Run sequentially so each update reads the latest ref — no race condition
  (async () => {
    let done = 0;
    const total = prompts.size;
    for (const prompt of prompts) {
      onImageStatus?.(`Generating image ${done + 1}/${total}…`);
      try {
        const url = await generateAiImage(prompt);
        done++;
        // Notify iframe for live update
        window.dispatchEvent(new CustomEvent('deck-image-ready', { detail: { prompt, url } }));
        // Inject into current deck state (reads latest ref each time)
        const current = streamingDeck.current;
        if (!current) return;
        const updated: DeckData = {
          ...current,
          slides: current.slides.map(s => ({ ...s, html: injectImageUrl(s.html, prompt, url) })),
          masterHtml: current.masterHtml ? injectImageUrl(current.masterHtml, prompt, url) : current.masterHtml,
        };
        streamingDeck.current = updated;
        onDeckData(updated);
      } catch (e) {
        const msg = String(e);
        console.warn('[Deckr] AI image generation failed:', msg);
        onImageStatus?.(`Image failed: ${msg}`);
        // Surface "no provider" specifically so user knows to configure Settings → Image
        if (msg.includes('No image provider')) {
          onImageStatus?.('No image provider — configure one in Settings → Image');
          return; // all prompts will fail the same way, stop early
        }
      }
    }
    onImageStatus?.(null);
  })();
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
  const [imageStatus, setImageStatus] = useState<string | null>(null);
  const abortRef = useRef(false);
  // Track latest deck during streaming so slide-ready can append to it
  const streamingDeck = useRef<DeckData | null>(null);
  // Monotonic counter — incremented at start of each generation and on stop.
  // Async callbacks (event listeners, fillAiGenImages) capture the token at
  // creation time; if the current token has moved on they are stale and bail.
  const genToken = useRef(0);

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
    const myToken = ++genToken.current;
    const isAlive = () => genToken.current === myToken;

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
      if (!isAlive()) return;
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
      if (!isAlive()) return;
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

      if (!isAlive()) return;

      if (result.deckData) {
        const finalDeck = result.deckData as DeckData;
        streamingDeck.current = finalDeck;
        onDeckData(finalDeck);
        // Guard onDeckData so stale image callbacks don't write to wrong session
        fillAiGenImages(finalDeck, (d) => { if (isAlive()) onDeckData(d); }, streamingDeck, setImageStatus);
      } else if (result.slideEdits?.length && deckData) {
        const updatedSlides: Slide[] = deckData.slides.map(slide => {
          const edit = result.slideEdits.find(e => e.slideId === slide.id);
          return edit ? { ...slide, html: edit.html } : slide;
        });
        onDeckData({ ...deckData, slides: updatedSlides });
      }

      if (result.notes) onNotes(result.notes);

      // Unblock the UI (input, agent pipeline) before streaming the reply text.
      // The pipeline must disappear as soon as generation is done, not after typing.
      setIsLoading(false);
      setAgentStatus(null);

      const aiMsg: Message = {
        id: generateId(),
        role: 'assistant',
        content: '',
        timestamp: Date.now(),
      };
      const withAi = [...newMessages, aiMsg];
      onMessages(withAi);

      await simulateStreaming(result.coachMessage || 'Done!', (text) => {
        if (!isAlive()) return; // a new request started — stop orphaned streaming
        onMessages(withAi.map(m => m.id === aiMsg.id ? { ...m, content: text } : m));
      });

    } catch (e) {
      setIsLoading(false);
      setAgentStatus(null);
      setError(String(e));
      onMessages([...newMessages, {
        id: generateId(),
        role: 'assistant',
        content: `Error: ${e}`,
        timestamp: Date.now(),
      }]);
    } finally {
      // Cleanup event listeners; ensure loading is cleared even on unexpected exits.
      Promise.all([unlistenStatus, unlistenDeckStarted, unlistenSlide])
        .then(fns => fns.forEach(f => f()));
      setIsLoading(false);
      setAgentStatus(null);
    }
  }, [isLoading, messages, deckData, notes, onMessages, onDeckData, onNotes, simulateStreaming]);

  const handleStop = useCallback(() => {
    genToken.current++; // invalidate any in-flight callbacks
    abortRef.current = true;
    setIsLoading(false);
    setAgentStatus(null);
    streamingDeck.current = null;
  }, []);

  return { isLoading, agentStatus, agentLog, error, imageStatus, handleSend, handleStop };
}
