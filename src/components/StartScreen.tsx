import { ChevronDown, FileText } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { ChatInputArea } from './ChatInputArea';
import { cn } from '../lib/utils';
import type { SessionSummary } from '../types';

const SLIDE_OPTIONS = [5, 8, 10, 12, 15, 20];

interface StartScreenProps {
  sessions: SessionSummary[];
  input: string;
  isLoading: boolean;
  currentModel: string;
  isConfigured: boolean;
  availableModels: Record<string, string[]>;
  numSlides: number;
  language: string;
  onChange: (v: string) => void;
  onSend: (text: string) => void;
  onStop: () => void;
  onOpenSession: (id: string) => void;
  onOpenSettings: () => void;
  onModelChange: (provider: string, model: string) => void;
  onNumSlidesChange: (n: number) => void;
  onLanguageChange: (l: string) => void;
}

export function StartScreen({
  sessions, input, isLoading, currentModel, isConfigured,
  availableModels, numSlides, language,
  onChange, onSend, onStop, onOpenSession, onOpenSettings,
  onModelChange, onNumSlidesChange, onLanguageChange,
}: StartScreenProps) {
  const recent = sessions.slice(0, 8);
  const [slideDropdownOpen, setSlideDropdownOpen] = useState(false);
  const slideRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (slideRef.current && !slideRef.current.contains(e.target as Node))
        setSlideDropdownOpen(false);
    }
    document.addEventListener('mousedown', handleClick);
    return () => document.removeEventListener('mousedown', handleClick);
  }, []);

  return (
    <div
      className="flex-1 flex flex-col items-center justify-center px-6 overflow-y-auto"
      style={{ background: 'radial-gradient(ellipse 80% 60% at 50% 35%, var(--card) 0%, var(--bg) 70%)' }}
    >
      <div className="w-full max-w-[560px] py-10">

        {/* Logo */}
        <div className="flex flex-col items-center mb-10">
          <div className="w-14 h-14 bg-cta rounded-2xl flex items-center justify-center mb-5">
            <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="var(--cta-fg)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <rect x="2" y="3" width="20" height="14" rx="2"/>
              <line x1="8" y1="21" x2="16" y2="21"/>
              <line x1="12" y1="17" x2="12" y2="21"/>
            </svg>
          </div>
          <h1 className="text-[22px] font-semibold text-fg mb-1.5">What are we building?</h1>
          <p className="text-[13px] text-fg-4">Describe a topic and I'll craft professional slides.</p>
        </div>

        {/* Input */}
        <ChatInputArea
          value={input}
          onChange={onChange}
          onSend={onSend}
          onStop={onStop}
          isLoading={isLoading}
          isConfigured={isConfigured}
          currentModel={currentModel}
          availableModels={availableModels}
          onModelChange={onModelChange}
          onOpenSettings={onOpenSettings}
        />

        {/* Slide count + Language */}
        <div className="flex items-center gap-2 mt-2">

          {/* Number of slides — custom dropdown */}
          <div className="relative" ref={slideRef}>
            <button
              onClick={() => setSlideDropdownOpen(o => !o)}
              className="flex items-center gap-2 bg-bg border border-line hover:border-line-hi rounded-xl px-3 py-2 transition-colors"
            >
              <span className="text-[12px] text-fg-4">Number of slides</span>
              <span className="text-[13px] font-medium text-fg">{numSlides}</span>
              <ChevronDown size={11} className={cn('text-fg-5 transition-transform duration-150', slideDropdownOpen && 'rotate-180')} />
            </button>

            {slideDropdownOpen && (
              <div className="absolute top-full left-0 mt-1 bg-surface border border-line rounded-lg shadow-xl overflow-hidden z-50">
                {SLIDE_OPTIONS.map(n => (
                  <button
                    key={n}
                    onClick={() => { onNumSlidesChange(n); setSlideDropdownOpen(false); }}
                    className={cn(
                      'w-full text-left px-4 py-2 text-[13px] transition-colors',
                      n === numSlides
                        ? 'bg-cta/10 text-cta font-medium'
                        : 'text-fg-2 hover:bg-card'
                    )}
                  >
                    {n} slides
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Language */}
          <div className="flex items-center gap-2 bg-bg border border-line hover:border-line-hi rounded-xl px-3 py-2 flex-1 transition-colors">
            <span className="text-[12px] text-fg-4 shrink-0">Language</span>
            <input
              type="text"
              value={language}
              onChange={e => onLanguageChange(e.target.value)}
              placeholder="auto-detect"
              className="flex-1 min-w-0 bg-transparent text-[13px] text-fg-2 placeholder-fg-5 outline-none"
              style={{ userSelect: 'text' }}
            />
          </div>
        </div>

        {/* Recent sessions */}
        {recent.length > 0 && (
          <div className="mt-8">
            <p className="text-[11px] text-fg-4 font-medium uppercase tracking-wider mb-2">Recent</p>
            <div className="space-y-0.5 max-h-48 overflow-y-auto">
              {recent.map(s => (
                <button
                  key={s.id}
                  onClick={() => onOpenSession(s.id)}
                  className="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-left hover:bg-card transition-colors group"
                >
                  <FileText size={13} className="text-fg-5 shrink-0" />
                  <span className="flex-1 text-[13px] text-fg-3 group-hover:text-fg-2 truncate transition-colors">{s.title}</span>
                  {s.slideCount > 0 && (
                    <span className="text-[11px] text-fg-5 shrink-0">{s.slideCount} slides</span>
                  )}
                </button>
              ))}
            </div>
          </div>
        )}

      </div>
    </div>
  );
}
