import { Eye, EyeOff, ExternalLink, RefreshCw, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { AI_PROVIDERS, IMAGE_PROVIDERS, SEARCH_PROVIDERS } from '../constants';
import { cn } from '../lib/utils';
import { fetchModels } from '../services/llm';
import { getSettings, saveSettings } from '../services/settings';
import type { Settings } from '../types';

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
  onSaved: (provider: string, model: string) => void;
}

const MASKED_SENTINEL = '__MASKED__';

type Section = 'ai' | 'image' | 'search';

function defaultSettings(): Settings {
  return {
    llm: { provider: 'gemini', configs: {}, base_url: '', api_key: '', model: '' },
    image: {},
    search: {},
    dark_mode: true,
  };
}

const IMAGE_PROVIDER_DOCS: Record<string, string> = {
  together: 'https://api.together.ai',
  fal: 'https://fal.ai/dashboard/keys',
  openai_img: 'https://platform.openai.com/api-keys',
  google_img: 'https://console.cloud.google.com',
  getimg: 'https://docs.getimg.ai/reference/introduction',
  unsplash: 'https://unsplash.com/developers',
};

export function SettingsModal({ open, onClose, onSaved }: SettingsModalProps) {
  const [settings, setSettings] = useState<Settings>(defaultSettings());
  const [section, setSection] = useState<Section>('ai');
  const [activeTab, setActiveTab] = useState('gemini');
  const [activeImageProvider, setActiveImageProvider] = useState(IMAGE_PROVIDERS[0].id);
  const [models, setModels] = useState<Record<string, string[]>>({});
  const [loadingModels, setLoadingModels] = useState<string | null>(null);
  const [showKeys, setShowKeys] = useState<Record<string, boolean>>({});
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) getSettings().then(setSettings);
  }, [open]);

  // Auto-load AI models when key/base_url ready
  useEffect(() => {
    if (section !== 'ai') return;
    const cfg = getAiConfig(activeTab);
    const hasKey = cfg.api_key && cfg.api_key !== MASKED_SENTINEL;
    const isLocal = activeTab === 'ollama' || activeTab === 'lmstudio';
    if ((hasKey || isLocal) && cfg.base_url && !models[activeTab] && !loadingModels) {
      loadModels(activeTab);
    }
  }, [activeTab, section, settings.llm.configs, models, loadingModels]);

  const getAiConfig = (provider: string) =>
    settings.llm.configs[provider] || {
      base_url: AI_PROVIDERS.find(p => p.id === provider)?.defaultBase || '',
      api_key: '',
      model: '',
    };

  const updateAiConfig = (provider: string, key: string, value: string) => {
    setSettings(s => ({
      ...s,
      llm: {
        ...s.llm,
        configs: { ...s.llm.configs, [provider]: { ...getAiConfig(provider), [key]: value } },
      },
    }));
  };

  const getImageConfig = (id: string) =>
    settings.image[id] || {
      api_key: '',
      model: IMAGE_PROVIDERS.find(p => p.id === id)?.defaultModel || '',
    };

  const updateImageConfig = (id: string, field: 'api_key' | 'model', value: string) => {
    setSettings(s => ({
      ...s,
      image: { ...s.image, [id]: { ...getImageConfig(id), [field]: value } },
    }));
  };

  const updateSearchKey = (id: string, value: string) => {
    setSettings(s => ({ ...s, search: { ...s.search, [id]: value } }));
  };

  const loadModels = async (provider: string) => {
    const cfg = getAiConfig(provider);
    setLoadingModels(provider);
    try {
      const list = await fetchModels(provider, cfg.base_url, cfg.api_key);
      setModels(prev => ({ ...prev, [provider]: list }));
    } finally {
      setLoadingModels(null);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const active = getAiConfig(settings.llm.provider);
      await saveSettings({
        ...settings,
        llm: { ...settings.llm, base_url: active.base_url, api_key: active.api_key, model: active.model },
      });
      onSaved(settings.llm.provider, active.model);
      onClose();
    } catch (e) {
      console.error('Failed to save settings:', e);
    } finally {
      setSaving(false);
    }
  };

  if (!open) return null;

  const activeAiConfig = getAiConfig(activeTab);
  const providerModels = models[activeTab] || [];
  const imgProvider = IMAGE_PROVIDERS.find(p => p.id === activeImageProvider)!;
  const imgConfig = getImageConfig(activeImageProvider);
  const imgKeyMasked = imgConfig.api_key === MASKED_SENTINEL;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.75)' }}>
      <div className="w-[560px] h-[520px] bg-surface border border-line rounded-xl flex flex-col overflow-hidden">

        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-line">
          <div>
            <h2 className="text-[14px] font-semibold text-fg">Settings</h2>
            <div className="flex items-center gap-1 mt-1.5">
              {(['ai', 'image', 'search'] as Section[]).map(s => (
                <button
                  key={s}
                  onClick={() => setSection(s)}
                  className={cn(
                    'px-2.5 py-0.5 text-[11px] font-medium rounded-md transition-colors uppercase tracking-wider',
                    section === s ? 'bg-cta text-cta-fg' : 'text-fg-4 hover:text-fg-3'
                  )}
                >
                  {s === 'ai' ? 'AI Model' : s === 'image' ? 'Image' : 'Search'}
                </button>
              ))}
            </div>
          </div>
          <button onClick={onClose} className="p-2.5 rounded-md text-fg-5 hover:text-fg-3 transition-colors">
            <X size={16} />
          </button>
        </div>

        {/* ── AI Model ── */}
        {section === 'ai' && (
          <div className="flex flex-1 overflow-hidden">
            <div className="w-36 border-r border-line overflow-y-auto shrink-0 bg-bg">
              {AI_PROVIDERS.map(p => (
                <button
                  key={p.id}
                  onClick={() => setActiveTab(p.id)}
                  className={cn(
                    'w-full text-left px-3 py-2.5 text-[12px] transition-colors border-l-2',
                    activeTab === p.id
                      ? 'bg-tint border-cta text-fg'
                      : 'border-transparent text-fg-3 hover:bg-card hover:text-fg-2'
                  )}
                >
                  <div className="font-medium truncate">{p.name}</div>
                  {settings.llm.provider === p.id && (
                    <div className="text-[11px] text-fg-3 uppercase tracking-wider mt-0.5">active</div>
                  )}
                </button>
              ))}
            </div>

            <div className="flex-1 p-4 overflow-y-auto space-y-4">
              <div>
                <label className="text-[12px] text-fg-3 uppercase tracking-widest mb-1.5 block">Base URL</label>
                <input
                  type="text"
                  value={activeAiConfig.base_url}
                  onChange={e => updateAiConfig(activeTab, 'base_url', e.target.value)}
                  className="w-full bg-card border border-line rounded-lg px-3 py-2 text-[12px] text-fg-2 outline-none focus:border-line-hi transition-colors font-mono"
                  style={{ userSelect: 'text' }}
                />
              </div>

              <div>
                <label className="text-[12px] text-fg-3 uppercase tracking-widest mb-1.5 block">API Key</label>
                <div className="relative">
                  <input
                    type={showKeys[activeTab] ? 'text' : 'password'}
                    value={activeAiConfig.api_key === MASKED_SENTINEL ? '' : activeAiConfig.api_key}
                    onChange={e => updateAiConfig(activeTab, 'api_key', e.target.value)}
                    placeholder={activeAiConfig.api_key === MASKED_SENTINEL ? '•••••••• (Enter new to change)' : 'sk-...'}
                    className="w-full bg-card border border-line rounded-lg px-3 py-2 pr-10 text-[12px] text-fg-2 outline-none focus:border-line-hi transition-colors font-mono"
                    style={{ userSelect: 'text' }}
                  />
                  <button
                    onClick={() => setShowKeys(s => ({ ...s, [activeTab]: !s[activeTab] }))}
                    className="absolute right-1.5 top-1/2 -translate-y-1/2 p-1.5 rounded text-fg-5 hover:text-fg-3 transition-colors"
                  >
                    {showKeys[activeTab] ? <EyeOff size={14} /> : <Eye size={14} />}
                  </button>
                </div>
              </div>

              <div>
                <div className="flex items-center justify-between mb-1.5">
                  <label className="text-[12px] text-fg-3 uppercase tracking-widest">Model</label>
                  <button
                    onClick={() => loadModels(activeTab)}
                    disabled={loadingModels === activeTab}
                    className="flex items-center gap-1 text-[12px] text-fg-4 hover:text-fg-3 uppercase tracking-wider transition-colors"
                  >
                    <RefreshCw size={9} className={loadingModels === activeTab ? 'animate-spin' : ''} />
                    Fetch
                  </button>
                </div>
                {providerModels.length > 0 ? (
                  <select
                    value={activeAiConfig.model}
                    onChange={e => updateAiConfig(activeTab, 'model', e.target.value)}
                    className="w-full bg-card border border-line rounded-lg px-3 py-2 text-[12px] text-fg-2 outline-none focus:border-line-hi font-mono"
                  >
                    {providerModels.map(m => <option key={m} value={m}>{m}</option>)}
                  </select>
                ) : (
                  <input
                    type="text"
                    value={activeAiConfig.model}
                    onChange={e => updateAiConfig(activeTab, 'model', e.target.value)}
                    className="w-full bg-card border border-line rounded-lg px-3 py-2 text-[12px] text-fg-2 outline-none focus:border-line-hi transition-colors font-mono"
                    style={{ userSelect: 'text' }}
                  />
                )}
              </div>

              <div className="flex items-center justify-between pt-3 border-t border-line">
                <div>
                  <p className="text-[12px] text-fg-2 font-medium">Use as default</p>
                  <p className="text-[12px] text-fg-4 mt-0.5">{AI_PROVIDERS.find(p => p.id === activeTab)?.name}</p>
                </div>
                <button
                  onClick={() => {
                    const cfg = getAiConfig(activeTab);
                    setSettings(s => ({
                      ...s,
                      llm: { ...s.llm, provider: activeTab, base_url: cfg.base_url, api_key: cfg.api_key, model: cfg.model }
                    }));
                  }}
                  className={cn(
                    'px-3 py-1.5 text-[12px] font-medium rounded-lg transition-colors',
                    settings.llm.provider === activeTab
                      ? 'bg-tint border border-tint-bd text-fg-3 cursor-default'
                      : 'bg-cta text-cta-fg hover:bg-cta-hv'
                  )}
                >
                  {settings.llm.provider === activeTab ? 'Active' : 'Use this'}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* ── Image Generation ── */}
        {section === 'image' && (
          <div className="flex-1 p-4 overflow-y-auto space-y-4">
            {/* Provider select */}
            <div>
              <label className="text-[12px] text-fg-3 uppercase tracking-widest mb-1.5 block">Provider</label>
              <div className="flex items-center gap-2">
                <select
                  value={activeImageProvider}
                  onChange={e => setActiveImageProvider(e.target.value)}
                  className="flex-1 bg-card border border-line rounded-lg px-3 py-2 text-[12px] text-fg-2 outline-none focus:border-line-hi font-mono"
                >
                  {IMAGE_PROVIDERS.map(p => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                      {settings.image[p.id]?.api_key ? ' ✓' : ''}
                    </option>
                  ))}
                </select>
                {IMAGE_PROVIDER_DOCS[activeImageProvider] && (
                  <a
                    href={IMAGE_PROVIDER_DOCS[activeImageProvider]}
                    target="_blank"
                    rel="noreferrer"
                    className="p-2 rounded-lg border border-line text-fg-4 hover:text-fg-2 hover:border-line-hi transition-colors"
                    title="View docs"
                  >
                    <ExternalLink size={13} />
                  </a>
                )}
              </div>
              <p className="text-[11px] text-fg-4 mt-1">{imgProvider.hint}</p>
            </div>

            {/* API Key */}
            <div>
              <label className="text-[12px] text-fg-3 uppercase tracking-widest mb-1.5 block">API Key</label>
              <div className="relative">
                <input
                  type={showKeys[`img_${activeImageProvider}`] ? 'text' : 'password'}
                  value={imgKeyMasked ? '' : imgConfig.api_key}
                  onChange={e => updateImageConfig(activeImageProvider, 'api_key', e.target.value)}
                  placeholder={imgKeyMasked ? '•••••••• (Enter new to change)' : activeImageProvider === 'unsplash' ? 'Access Key...' : 'API key...'}
                  className="w-full bg-card border border-line rounded-lg px-3 py-2 pr-10 text-[12px] text-fg-2 outline-none focus:border-line-hi transition-colors font-mono"
                  style={{ userSelect: 'text' }}
                />
                <button
                  onClick={() => setShowKeys(s => ({ ...s, [`img_${activeImageProvider}`]: !s[`img_${activeImageProvider}`] }))}
                  className="absolute right-1.5 top-1/2 -translate-y-1/2 p-1.5 rounded text-fg-5 hover:text-fg-3 transition-colors"
                >
                  {showKeys[`img_${activeImageProvider}`] ? <EyeOff size={14} /> : <Eye size={14} />}
                </button>
              </div>
              {imgKeyMasked && (
                <p className="text-[11px] text-green-500 mt-1">Key saved</p>
              )}
            </div>

            {/* Model — hide for Unsplash */}
            {activeImageProvider !== 'unsplash' && (
              <div>
                <label className="text-[12px] text-fg-3 uppercase tracking-widest mb-1.5 block">Model</label>
                <input
                  type="text"
                  value={imgConfig.model}
                  onChange={e => updateImageConfig(activeImageProvider, 'model', e.target.value)}
                  placeholder={imgProvider.defaultModel || 'model-id...'}
                  className="w-full bg-card border border-line rounded-lg px-3 py-2 text-[12px] text-fg-2 outline-none focus:border-line-hi transition-colors font-mono"
                  style={{ userSelect: 'text' }}
                />
                <p className="text-[11px] text-fg-4 mt-1">Default: {imgProvider.defaultModel}</p>
              </div>
            )}
          </div>
        )}

        {/* ── Web Search ── */}
        {section === 'search' && (
          <div className="flex-1 p-4 overflow-y-auto space-y-4">
            <p className="text-[12px] text-fg-4">API keys for web search used during slide generation.</p>
            {SEARCH_PROVIDERS.map(p => {
              const val = settings.search[p.id] ?? '';
              const isMasked = val === MASKED_SENTINEL;
              const showKey = showKeys[`search_${p.id}`];
              return (
                <div key={p.id} className="space-y-3">
                  <div>
                    <div className="flex items-center justify-between mb-1.5">
                      <label className="text-[12px] text-fg-3 uppercase tracking-widest">{p.name}</label>
                      <a
                        href="https://app.tavily.com/home"
                        target="_blank"
                        rel="noreferrer"
                        className="flex items-center gap-1 text-[11px] text-fg-4 hover:text-fg-2 transition-colors"
                      >
                        <ExternalLink size={11} />
                        Get key
                      </a>
                    </div>
                    <div className="relative">
                      <input
                        type={showKey ? 'text' : 'password'}
                        value={isMasked ? '' : val}
                        onChange={e => updateSearchKey(p.id, e.target.value)}
                        placeholder={isMasked ? '•••••••• (Enter new to change)' : 'tvly-...'}
                        className="w-full bg-card border border-line rounded-lg px-3 py-2 pr-10 text-[12px] text-fg-2 outline-none focus:border-line-hi transition-colors font-mono"
                        style={{ userSelect: 'text' }}
                      />
                      <button
                        onClick={() => setShowKeys(s => ({ ...s, [`search_${p.id}`]: !s[`search_${p.id}`] }))}
                        className="absolute right-1.5 top-1/2 -translate-y-1/2 p-1.5 rounded text-fg-5 hover:text-fg-3 transition-colors"
                      >
                        {showKey ? <EyeOff size={14} /> : <Eye size={14} />}
                      </button>
                    </div>
                    {isMasked && <p className="text-[11px] text-green-500 mt-1">Key saved</p>}
                    <p className="text-[11px] text-fg-4 mt-1">{p.hint}</p>
                  </div>
                </div>
              );
            })}
          </div>
        )}

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 px-4 py-3 border-t border-line">
          <button onClick={onClose} className="px-4 py-1.5 text-[12px] text-fg-4 hover:text-fg-3 rounded-lg transition-colors">
            Cancel
          </button>
          <button
            onClick={handleSave}
            disabled={saving}
            className="px-4 py-1.5 text-[12px] font-medium bg-cta text-cta-fg hover:bg-cta-hv rounded-lg transition-colors disabled:opacity-40"
          >
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      </div>
    </div>
  );
}
