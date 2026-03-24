import { invoke } from '@tauri-apps/api/core';
import type { Settings } from '../types';

export async function getSettings(): Promise<Settings> {
  try {
    return await invoke<Settings>('get_settings');
  } catch {
    return {
      llm: { provider: 'gemini', configs: {}, base_url: '', api_key: '', model: '' },
      image: {} as Record<string, { api_key: string; model: string }>,
      search: {},
      dark_mode: true,
    };
  }
}

export async function saveSettings(settings: Settings): Promise<void> {
  await invoke('save_settings', { settings });
}
