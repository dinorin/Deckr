import { useCallback, useEffect, useRef, useState } from 'react';
import { ChatInputArea } from './components/ChatInputArea';
import { MessageList } from './components/MessageList';
import { SettingsModal } from './components/SettingsModal';
import { StartScreen } from './components/StartScreen';
import { TitleBar } from './components/TitleBar';
import { useSession } from './hooks/useSession';
import { useLlm } from './hooks/useLlm';
import { getSettings, saveSettings } from './services/settings';
import { SlidePreview } from './slide/Preview';
import { exportToPptx } from './slide/utils';
import { invoke } from '@tauri-apps/api/core';
import { applyTheme, getTheme } from './themes';

export default function App() {
  const [input, setInput] = useState('');
  const [view, setView] = useState<'start' | 'app'>('start');
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isConfigured, setIsConfigured] = useState(true);
  const [themeId, setThemeId] = useState(() => localStorage.getItem('theme') ?? 'material');
  const [model, setModel] = useState('Missing');
  const [availableModels, setAvailableModels] = useState<Record<string, string[]>>({});
  const [numSlides, setNumSlides] = useState(() => Number(localStorage.getItem('numSlides') ?? '8'));
  const [language, setLanguage] = useState(() => localStorage.getItem('language') ?? '');

  const pendingSend = useRef<string | null>(null);

  const { session, sessions, messages, deckData, notes, setMessages, setDeckData, setNotes, resetSession, switchToSession, removeSession } = useSession();

  const fetchAllModels = useCallback(async () => {
    const s = await getSettings();
    const providers = Object.entries(s.llm.configs);
    const results: Record<string, string[]> = {};
    
    await Promise.all(providers.map(async ([id, cfg]) => {
      if (cfg.api_key || id === 'ollama' || id === 'lmstudio') {
        try {
          const list = await invoke<string[]>('fetch_models', { 
            provider: id, 
            baseUrl: cfg.base_url, 
            apiKey: cfg.api_key 
          });
          if (list.length > 0) results[id] = list;
        } catch (e) {
          console.error(`Failed to fetch models for ${id}:`, e);
        }
      }
    }));
    setAvailableModels(results);
  }, []);

  const { isLoading, agentStatus, agentLog, imageStatus, handleSend, handleStop } = useLlm({
    messages,
    deckData,
    notes,
    onMessages: setMessages,
    onDeckData: setDeckData,
    onNotes: setNotes,
  });

  useEffect(() => {
    applyTheme(getTheme(themeId));
    localStorage.setItem('theme', themeId);
  }, [themeId]);

  useEffect(() => {
    applyTheme(getTheme(themeId));
    getSettings().then(s => {
      const hasKey = !!s.llm.api_key;
      setIsConfigured(hasKey);
      
      if (!hasKey) setModel('Missing');
      else if (!s.llm.model) setModel('Select Model');
      else setModel(s.llm.model);
    });
    fetchAllModels();
    invoke('app_ready').catch(() => {});
  }, [fetchAllModels]);

  const handleCopy = useCallback((text: string) => {
    navigator.clipboard.writeText(text);
  }, []);

  const [isExporting, setIsExporting] = useState(false);
  const [exportProgress, setExportProgress] = useState(0);

  const handleExport = useCallback(async () => {
    if (!deckData || isExporting) return;
    setIsExporting(true);
    setExportProgress(0);
    try {
      await exportToPptx(deckData, (current, total) => {
        setExportProgress(total > 0 ? Math.round((current / total) * 100) : 0);
      });
    }
    catch (e) { console.error('Export failed:', e); }
    finally { setIsExporting(false); setExportProgress(0); }
  }, [deckData, isExporting]);

  const handleSettingsSaved = useCallback((_p: string, m: string) => {
    getSettings().then(s => {
      const hasKey = !!s.llm.api_key;
      setIsConfigured(hasKey);
      
      if (!hasKey) setModel('Missing');
      else if (!m) setModel('Select Model');
      else setModel(m);
    });
    fetchAllModels();
  }, [fetchAllModels]);

  const handleModelChange = useCallback(async (p: string, m: string) => {
    const s = await getSettings();
    const cfg = s.llm.configs[p];
    if (cfg) {
      const updated = {
        ...s,
        llm: {
          ...s.llm,
          provider: p,
          base_url: cfg.base_url,
          api_key: cfg.api_key,
          model: m,
        }
      };
      await saveSettings(updated);
      setModel(m);
      setIsConfigured(!!cfg.api_key);
    }
  }, []);

  // When session resets (new id) and there's a pending message, fire it
  useEffect(() => {
    if (pendingSend.current && messages.length === 0) {
      const text = pendingSend.current;
      pendingSend.current = null;
      setView('app');
      handleSend(text, numSlides, language || 'auto');
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session.id]);

  const handleNumSlidesChange = useCallback((n: number) => {
    setNumSlides(n);
    localStorage.setItem('numSlides', String(n));
  }, []);

  const handleLanguageChange = useCallback((l: string) => {
    setLanguage(l);
    localStorage.setItem('language', l);
  }, []);

  // From start screen: always open a fresh new chat
  const handleStartSend = useCallback((text: string) => {
    setInput('');
    pendingSend.current = text;
    resetSession(); // creates new session id → triggers useEffect above
  }, [resetSession]);

  // From start screen: open existing session
  const handleOpenSession = useCallback(async (id: string) => {
    handleStop(); // kill any in-flight generation before switching session
    await switchToSession(id);
    setView('app');
  }, [switchToSession, handleStop]);

  // Back to home
  const handleBack = useCallback(() => {
    setView('start');
  }, []);

  // Send from app view
  const handleAppSend = useCallback((text: string) => {
    setInput('');

    // Resume intent: "resume", "resume [keyword]", "tiếp tục", etc.
    const resumeMatch = text.trim().match(/^(?:resume|tiếp tục|tải lại|load|open|mở lại)(?:\s+(.+))?$/i);
    if (resumeMatch) {
      const keyword = resumeMatch[1]?.toLowerCase().trim();
      const others = sessions.filter(s => s.id !== session.id);
      const target = keyword
        ? (others.find(s => s.title.toLowerCase().includes(keyword)) ?? others[0])
        : others[0];
      if (target) {
        handleStop();
        switchToSession(target.id);
        return;
      }
      // No other sessions — fall through to AI
    }

    handleSend(text, numSlides, language || 'auto');
  }, [handleSend, numSlides, language, sessions, session.id, switchToSession]);

  return (
    <div className="flex flex-col h-screen bg-bg text-fg overflow-hidden">
      <TitleBar
        themeId={themeId}
        title={view === 'app' ? session.title : undefined}
        slideCount={view === 'app' ? (deckData?.slides.length || 0) : undefined}
        onBack={view === 'app' ? handleBack : undefined}
        onChangeTheme={setThemeId}
        onOpenSettings={() => setIsSettingsOpen(true)}
      />

      {view === 'start' ? (
        <StartScreen
          sessions={sessions}
          activeSessionId={session.messages.length > 0 || session.deckData ? session.id : undefined}
          input={input}
          isLoading={isLoading}
          currentModel={model}
          isConfigured={isConfigured}
          availableModels={availableModels}
          numSlides={numSlides}
          language={language}
          onChange={setInput}
          onSend={handleStartSend}
          onStop={handleStop}
          onOpenSession={handleOpenSession}
          onOpenSettings={() => setIsSettingsOpen(true)}
          onModelChange={handleModelChange}
          onNumSlidesChange={handleNumSlidesChange}
          onLanguageChange={handleLanguageChange}
        />
      ) : (
        <div className="flex flex-1 overflow-hidden">
          {/* Chat — 1/3 */}
          <div className="flex flex-col border-r border-line overflow-hidden bg-surface" style={{ width: '33.333%' }}>
            <MessageList
              messages={messages}
              isLoading={isLoading}
              agentStatus={agentStatus}
              agentLog={agentLog}
              onCopy={handleCopy}
            />
            <div className="shrink-0 px-4 pb-4 pt-2">
              <ChatInputArea
                value={input}
                onChange={setInput}
                onSend={handleAppSend}
                onStop={handleStop}
                isLoading={isLoading}
                isConfigured={isConfigured}
                currentModel={model}
                availableModels={availableModels}
                onModelChange={handleModelChange}
                onOpenSettings={() => setIsSettingsOpen(true)}
              />
            </div>
          </div>

          {/* Preview — 2/3 */}
          <div className="flex-1 overflow-hidden">
            <SlidePreview
              deckData={deckData}
              isOpen={true}
              onClose={handleBack}
              onExport={handleExport}
              isExporting={isExporting}
              exportProgress={exportProgress}
              imageStatus={imageStatus}
            />
          </div>
        </div>
      )}

      <SettingsModal
        open={isSettingsOpen}
        onClose={() => setIsSettingsOpen(false)}
        onSaved={handleSettingsSaved}
      />
    </div>
  );
}
