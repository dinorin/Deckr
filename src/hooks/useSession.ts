import { useCallback, useEffect, useRef, useState } from 'react';
import { generateId } from '../lib/utils';
import { deleteSession, listSessions, loadSession, saveSession } from '../services/storage';
import type { DeckData, Message, Session, SessionSummary } from '../types';

const STORAGE_KEY = 'deckr_current_session';

function newSession(): Session {
  return {
    id: generateId(),
    title: 'New Presentation',
    messages: [],
    deckData: null,
    notes: '',
    createdAt: Date.now(),
    updatedAt: Date.now(),
  };
}

export function useSession() {
  const [session, setSession] = useState<Session>(newSession);
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    listSessions().then(setSessions);
    const savedId = localStorage.getItem(STORAGE_KEY);
    if (savedId) {
      loadSession(savedId).then(s => { if (s) setSession(s); });
    }
  }, []);

  useEffect(() => {
    if (saveTimer.current) clearTimeout(saveTimer.current);
    saveTimer.current = setTimeout(() => {
      if (session.messages.length === 0 && !session.deckData) return;
      saveSession(session);
      localStorage.setItem(STORAGE_KEY, session.id);
      listSessions().then(setSessions);
    }, 1000);
  }, [session]);

  const setMessages = useCallback((messages: Message[]) => {
    setSession(s => ({ ...s, messages, updatedAt: Date.now() }));
  }, []);

  const setDeckData = useCallback((deckData: DeckData | null) => {
    setSession(s => ({
      ...s,
      deckData,
      title: deckData?.title || s.title,
      updatedAt: Date.now(),
    }));
  }, []);

  const setNotes = useCallback((notes: string) => {
    setSession(s => ({ ...s, notes, updatedAt: Date.now() }));
  }, []);

  const resetSession = useCallback(() => {
    const s = newSession();
    setSession(s);
    localStorage.setItem(STORAGE_KEY, s.id);
  }, []);

  const switchToSession = useCallback(async (id: string) => {
    const s = await loadSession(id);
    if (s) {
      setSession(s);
      localStorage.setItem(STORAGE_KEY, s.id);
    }
  }, []);

  const removeSession = useCallback(async (id: string) => {
    await deleteSession(id);
    setSessions(prev => prev.filter(s => s.id !== id));
    if (session.id === id) resetSession();
  }, [session.id, resetSession]);

  const removeAllSessions = useCallback(async () => {
    const all = await listSessions();
    await Promise.all(all.map(s => deleteSession(s.id)));
    setSessions([]);
    resetSession();
  }, [resetSession]);

  return {
    session,
    sessions,
    messages: session.messages,
    deckData: session.deckData,
    notes: session.notes,
    setMessages,
    setDeckData,
    setNotes,
    resetSession,
    switchToSession,
    removeSession,
    removeAllSessions,
  };
}
