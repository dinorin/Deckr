export type Role = 'user' | 'assistant';

export interface Message {
  id: string;
  role: Role;
  content: string;
  timestamp: number;
}

export interface DeckTheme {
  primaryColor: string;
  secondaryColor: string;
  backgroundColor: string;
  textColor: string;
  fontFamily: string;
  style: 'modern' | 'minimal' | 'bold' | 'corporate' | 'creative';
}

export interface Slide {
  id: string;
  type: 'title' | 'content' | 'two-column' | 'quote' | 'bullets' | 'closing' | 'image';
  html: string;
}

export interface SlideEdit {
  slideId: string;
  html: string;
}

export interface DeckData {
  title: string;
  theme: DeckTheme;
  slides: Slide[];
  /** Single self-contained HTML file with all slides + CSS + JS */
  masterHtml?: string;
  metadata: {
    slideCount: number;
    generatedAt: number;
    topic?: string;
  };
}

export interface Session {
  id: string;
  title: string;
  messages: Message[];
  deckData: DeckData | null;
  notes: string;
  createdAt: number;
  updatedAt: number;
}

export interface SessionSummary {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  slideCount: number;
}

export interface AgentStatus {
  type: 'thinking' | 'generating' | 'idle';
  message: string;
}

export interface Settings {
  llm: {
    provider: string;
    configs: Record<string, {
      base_url: string;
      api_key: string;
      model: string;
    }>;
    base_url: string;
    api_key: string;
    model: string;
  };
  image: Record<string, { api_key: string; model: string }>;
  search: Record<string, string>;
  dark_mode: boolean;
}
