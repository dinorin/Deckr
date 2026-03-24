import { AlertCircle, ArrowUp, ChevronDown, Search, Square, Settings2 } from 'lucide-react';
import { useCallback, useRef, useState, useEffect } from 'react';
import { cn } from '../lib/utils';
import { motion, AnimatePresence } from 'motion/react';
import { AI_PROVIDERS } from '../constants';

interface ChatInputAreaProps {
  value: string;
  onChange: (v: string) => void;
  onSend: (text: string) => void;
  onStop: () => void;
  isLoading: boolean;
  isConfigured?: boolean;
  currentModel: string;
  availableModels?: Record<string, string[]>;
  onModelChange?: (provider: string, model: string) => void;
  onOpenSettings: () => void;
}

export function ChatInputArea({
  value, onChange, onSend, onStop,
  isLoading, isConfigured = true, currentModel,
  availableModels = {}, onModelChange, onOpenSettings,
}: ChatInputAreaProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);
  const [showWarning, setShowWarning] = useState(false);
  const [shake, setShake] = useState(0);
  const [isDropdownOpen, setIsDropdownOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsDropdownOpen(false);
      }
    }
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, []);

  const handleSend = useCallback(() => {
    if (!value.trim() || isLoading) return;
    if (!isConfigured || currentModel === 'Select Model') {
      setShake(s => s + 1);
      setShowWarning(true);
      setTimeout(() => setShowWarning(false), 3000);
      return;
    }
    onSend(value);
    onChange('');
    if (textareaRef.current) textareaRef.current.style.height = 'auto';
  }, [value, isLoading, isConfigured, currentModel, onSend, onChange]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }, [handleSend]);

  const handleInput = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    onChange(e.target.value);
    e.target.style.height = 'auto';
    e.target.style.height = `${Math.min(e.target.scrollHeight, 120)}px`;
  }, [onChange]);

  const filteredModels = Object.entries(availableModels).map(([providerId, models]) => {
    const provider = AI_PROVIDERS.find(p => p.id === providerId);
    const matches = models.filter(m => m.toLowerCase().includes(searchQuery.toLowerCase()));
    return { id: providerId, name: provider?.name || providerId, models: matches };
  }).filter(group => group.models.length > 0);

  const modelShort = currentModel.length > 24 ? currentModel.slice(0, 24) + '…' : currentModel;

  return (
    <div className="relative">
      <AnimatePresence>
        {showWarning && (
          <motion.div
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 5 }}
            className="absolute -top-10 left-0 right-0 flex justify-center z-10"
          >
            <div className="bg-cta text-cta-fg px-3 py-1.5 rounded-lg text-[12px] font-medium shadow-lg flex items-center gap-2 border border-white/10">
              <AlertCircle size={14} />
              {currentModel === 'Select Model' ? 'Please select a model first!' : 'Please configure your API Key first!'}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <motion.div
        animate={shake ? { x: [0, -4, 4, -4, 4, 0] } : {}}
        transition={{ duration: 0.4 }}
        key={shake}
        className="bg-bg border border-line hover:border-line-hi rounded-xl transition-all duration-300"
      >
        {/* Textarea */}
        <div className="px-4 pt-3 pb-0">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={handleInput}
            onKeyDown={handleKeyDown}
            placeholder="Describe your presentation…"
            rows={2}
            disabled={isLoading}
            className="w-full bg-transparent text-[14px] text-fg placeholder-fg-5 resize-none outline-none leading-relaxed min-h-[44px] max-h-[120px] disabled:opacity-40"
            style={{ userSelect: 'text' }}
          />
        </div>

        {/* Toolbar */}
        <div className="flex items-center justify-between px-3 pb-3 pt-1">
          <div className="relative" ref={dropdownRef}>
            <button
              onClick={() => setIsDropdownOpen(!isDropdownOpen)}
              className={cn(
                "flex items-center gap-1.5 px-2.5 py-1.5 bg-surface border border-line hover:bg-card hover:border-line-hi rounded-lg transition-colors group",
                (currentModel === 'Missing' || currentModel === 'Select Model') && "border-cta/20 bg-cta/5"
              )}
            >
              <span className={cn(
                "text-[13px] transition-colors truncate max-w-[140px]",
                (currentModel === 'Missing' || currentModel === 'Select Model') ? "text-cta font-medium" : "text-fg-3 group-hover:text-fg-2"
              )}>
                {currentModel === 'Missing' ? 'Missing API Key' :
                 currentModel === 'Select Model' ? 'Select Model' : modelShort}
              </span>
              <ChevronDown size={11} className={cn(
                "shrink-0 transition-transform duration-200",
                isDropdownOpen && "rotate-180",
                (currentModel === 'Missing' || currentModel === 'Select Model') ? "text-cta" : "text-fg-5 group-hover:text-fg-3"
              )} />
            </button>

            <AnimatePresence>
              {isDropdownOpen && (
                <motion.div
                  initial={{ opacity: 0, scale: 0.95, y: 10 }}
                  animate={{ opacity: 1, scale: 1, y: 0 }}
                  exit={{ opacity: 0, scale: 0.95, y: 10 }}
                  className="absolute bottom-full left-0 mb-2 w-64 bg-surface border border-line rounded-xl shadow-2xl overflow-hidden z-50 flex flex-col max-h-[400px]"
                >
                  <div className="p-2 border-b border-line bg-bg/50">
                    <div className="relative">
                      <Search size={12} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-fg-5" />
                      <input
                        autoFocus
                        type="text"
                        placeholder="Search models..."
                        value={searchQuery}
                        onChange={e => setSearchQuery(e.target.value)}
                        className="w-full bg-card border border-line rounded-lg pl-8 pr-3 py-1.5 text-[12px] text-fg outline-none focus:border-cta/50 transition-colors"
                      />
                    </div>
                  </div>

                  <div className="flex-1 overflow-y-auto py-1">
                    {filteredModels.length > 0 ? filteredModels.map(group => (
                      <div key={group.id} className="mb-1 last:mb-0">
                        <div className="px-3 py-1 text-[10px] font-bold text-fg-5 uppercase tracking-wider bg-bg/30">
                          {group.name}
                        </div>
                        {group.models.map(m => (
                          <button
                            key={`${group.id}-${m}`}
                            onClick={() => {
                              onModelChange?.(group.id, m);
                              setIsDropdownOpen(false);
                              setSearchQuery('');
                            }}
                            className={cn(
                              "w-full text-left px-3 py-2 text-[12px] transition-colors",
                              currentModel === m
                                ? "bg-cta/10 text-cta font-medium border-r-2 border-cta"
                                : "text-fg-3 hover:bg-card hover:text-fg-2"
                            )}
                          >
                            <div className="truncate">{m}</div>
                          </button>
                        ))}
                      </div>
                    )) : (
                      <div className="px-3 py-8 text-center">
                        <p className="text-[12px] text-fg-5">No models found</p>
                        <p className="text-[11px] text-fg-5 mt-1">Open Settings to configure</p>
                      </div>
                    )}
                  </div>

                  <button
                    onClick={() => { onOpenSettings(); setIsDropdownOpen(false); }}
                    className="p-2.5 text-[12px] text-fg-4 hover:text-fg-2 hover:bg-card border-t border-line flex items-center justify-center gap-2 transition-colors"
                  >
                    <Settings2 size={13} />
                    Open Settings
                  </button>
                </motion.div>
              )}
            </AnimatePresence>
          </div>

          <div className="flex items-center gap-2">
            {!isLoading && isConfigured && value.trim() === '' && (
              <span className="text-[13px] text-fg-4 hidden sm:block">Enter ↵</span>
            )}
            {isLoading ? (
              <button
                onClick={onStop}
                className="flex items-center gap-2 px-3 py-1.5 bg-input border border-line-hi text-fg-3 hover:text-fg hover:border-fg-5 text-[13px] rounded-lg transition-colors"
              >
                <Square size={11} />
                Stop
              </button>
            ) : (
              <button
                onClick={handleSend}
                disabled={!value.trim()}
                className={cn(
                  'flex items-center justify-center w-8 h-8 rounded-lg transition-all',
                  value.trim()
                    ? 'bg-cta text-cta-fg hover:bg-cta-hv'
                    : 'bg-input border border-line text-fg-5 cursor-not-allowed opacity-40'
                )}
              >
                <ArrowUp size={14} strokeWidth={2.5} />
              </button>
            )}
          </div>
        </div>
      </motion.div>
    </div>
  );
}
