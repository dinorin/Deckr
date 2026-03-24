import { Check, Palette } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { THEMES } from '../themes';

interface ThemePickerProps {
  themeId: string;
  onSelect: (id: string) => void;
}

export function ThemePicker({ themeId, onSelect }: ThemePickerProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [open]);

  return (
    <div ref={ref} className="relative self-center">
      <button
        onClick={() => setOpen(v => !v)}
        className="p-2.5 rounded-md text-fg-4 hover:text-fg-3 hover:bg-input transition-colors"
        title="Color theme"
      >
        <Palette size={16} />
      </button>

      {open && (
        <div className="absolute right-0 top-full mt-1 w-52 bg-card border border-line rounded-xl overflow-hidden z-50" style={{ boxShadow: '0 8px 32px var(--bg)' }}>
          <div className="px-3 py-2 border-b border-line">
            <p className="text-[13px] text-fg-3 uppercase tracking-widest">Color Theme</p>
          </div>
          <div className="overflow-y-auto max-h-72 py-1">
            {THEMES.map(t => (
              <button
                key={t.id}
                onClick={() => { onSelect(t.id); setOpen(false); }}
                className="w-full flex items-center gap-3 px-3 py-2 hover:bg-input transition-colors text-left"
              >
                {/* Swatch */}
                <div className="flex gap-0.5 shrink-0">
                  <div className="w-3 h-5 rounded-l-sm" style={{ background: t.vars['--bg'] }} />
                  <div className="w-3 h-5" style={{ background: t.vars['--cta'] }} />
                  <div className="w-3 h-5 rounded-r-sm" style={{ background: t.vars['--fg'] }} />
                </div>
                <span className="text-[13px] text-fg-2 flex-1 truncate">{t.name}</span>
                {t.id === themeId && <Check size={12} className="text-fg-3 shrink-0" />}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
