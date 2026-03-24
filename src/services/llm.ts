import { invoke } from '@tauri-apps/api/core';
import type { DeckData } from '../types';

export interface GenerateResult {
  deckData: DeckData | null;
  slideEdits: { slideId: string; html: string }[];
  coachMessage: string;
  notes: string;
}

export interface AgentLogEntry {
  agent: string;
  status: 'thinking' | 'done' | 'error';
  message: string;
}

export interface MultiAgentResult extends GenerateResult {
  agentLog: AgentLogEntry[];
}

export async function generateDeckV2(params: {
  history: { role: string; content: string }[];
  currentDeck: DeckData | null;
  notes: string;
  language: string;
  numSlides: number;
}): Promise<MultiAgentResult> {
  return await invoke<MultiAgentResult>('generate_deck_v2', {
    history: params.history,
    currentDeck: params.currentDeck,
    notes: params.notes,
    language: params.language,
    numSlides: params.numSlides,
  });
}

export async function generateDeck(params: {
  history: { role: string; content: string }[];
  currentDeck: DeckData | null;
  notes: string;
  language: string;
}): Promise<GenerateResult> {
  return await invoke<GenerateResult>('generate_deck', {
    history: params.history,
    currentDeck: params.currentDeck,
    notes: params.notes,
    language: params.language,
  });
}

export async function generateAiImage(prompt: string): Promise<string> {
  return await invoke<string>('generate_ai_image', { prompt });
}

export async function fetchModels(provider: string, baseUrl: string, apiKey: string): Promise<string[]> {
  try {
    return await invoke<string[]>('fetch_models', { provider, baseUrl, apiKey });
  } catch {
    return [];
  }
}
