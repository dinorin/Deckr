import { getCurrentWindow } from '@tauri-apps/api/window';
import { ArrowLeft, Copy, Minus, Settings, Square, X } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { ThemePicker } from './ThemePicker';

interface TitleBarProps {
  title?: string;
  slideCount?: number;
  themeId: string;
  onBack?: () => void;
  onChangeTheme: (id: string) => void;
  onOpenSettings: () => void;
}

export function TitleBar({ title, slideCount, themeId, onBack, onChangeTheme, onOpenSettings }: TitleBarProps) {
  const win = getCurrentWindow();
  const [isMaximized, setIsMaximized] = useState(false);
  const minimize = useCallback(() => win.minimize(), [win]);
  const maximize = useCallback(() => win.toggleMaximize(), [win]);
  const close = useCallback(() => win.close(), [win]);

  useEffect(() => {
    win.isMaximized().then(setIsMaximized);
    const unlisten = win.onResized(() => win.isMaximized().then(setIsMaximized));
    return () => { unlisten.then(f => f()); };
  }, [win]);

  return (
    <div
      data-tauri-drag-region
      className="flex items-center h-11 bg-surface border-b border-line pl-4 gap-3 select-none shrink-0"
    >
      {/* Left side */}
      <div className="flex items-center gap-2.5">
        {onBack ? (
          <button
            onClick={onBack}
            className="flex items-center gap-2 px-2 py-1.5 rounded-md text-fg-4 hover:text-fg-2 hover:bg-input transition-colors"
          >
            <ArrowLeft size={14} strokeWidth={2} />
            <span className="text-[13px] font-medium text-fg-3">Home</span>
          </button>
        ) : (
          <>
            <div className="w-6 h-6 rounded-md bg-cta flex items-center justify-center shrink-0">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--cta-fg)" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                <rect x="2" y="3" width="20" height="14" rx="2"/>
                <line x1="8" y1="21" x2="16" y2="21"/>
                <line x1="12" y1="17" x2="12" y2="21"/>
              </svg>
            </div>
            <span className="text-[15px] font-semibold text-fg tracking-tight">Deckr</span>
          </>
        )}

        {onBack && title && (
          <>
            <div className="w-px h-4 bg-line" />
            <span className="text-[13px] text-fg-4 truncate max-w-[240px]">{title}</span>
            {slideCount ? (
              <span className="text-[12px] text-fg-4 bg-input border border-line px-2 py-0.5 rounded-full font-medium shrink-0">
                {slideCount} slides
              </span>
            ) : null}
          </>
        )}
      </div>

      <div className="flex-1" data-tauri-drag-region />

      <div className="flex items-stretch h-full gap-0.5 pr-1">
        <ThemePicker themeId={themeId} onSelect={onChangeTheme} />

        <button
          onClick={onOpenSettings}
          className="p-2.5 self-center rounded-md text-fg-4 hover:text-fg-3 hover:bg-input transition-colors"
          title="Settings"
        >
          <Settings size={16} />
        </button>

        <div className="w-px h-4 bg-line mx-2 self-center" />

        <button onClick={minimize} className="w-11 h-full flex items-center justify-center text-fg-4 hover:text-fg-2 hover:bg-input transition-colors">
          <Minus className="w-3.5 h-3.5" strokeWidth={1.5} />
        </button>
        <button onClick={maximize} className="w-11 h-full flex items-center justify-center text-fg-4 hover:text-fg-2 hover:bg-input transition-colors">
          {isMaximized
            ? <Copy className="w-[13px] h-[13px]" strokeWidth={1.5} />
            : <Square className="w-[13px] h-[13px]" strokeWidth={1.5} />
          }
        </button>
        <button onClick={close} className="w-11 h-full flex items-center justify-center text-fg-4 hover:text-white hover:bg-[#c42b1c] transition-colors">
          <X className="w-[15px] h-[15px]" strokeWidth={1.5} />
        </button>
      </div>
    </div>
  );
}
