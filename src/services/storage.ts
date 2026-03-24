import { invoke } from '@tauri-apps/api/core';
import type { Session, SessionSummary } from '../types';

export async function saveSession(session: Session): Promise<void> {
  await invoke('save_session', { session });
}

export async function listSessions(): Promise<SessionSummary[]> {
  try {
    return await invoke<SessionSummary[]>('list_sessions');
  } catch {
    return [];
  }
}

export async function loadSession(id: string): Promise<Session | null> {
  try {
    return await invoke<Session>('load_session', { id });
  } catch {
    return null;
  }
}

export async function deleteSession(id: string): Promise<void> {
  await invoke('delete_session', { id });
}
