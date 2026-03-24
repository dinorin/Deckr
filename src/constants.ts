export const AI_PROVIDERS = [
  { id: 'gemini', name: 'Google Gemini', defaultBase: 'https://generativelanguage.googleapis.com/v1beta' },
  { id: 'openai', name: 'OpenAI', defaultBase: 'https://api.openai.com/v1' },
  { id: 'deepseek', name: 'DeepSeek', defaultBase: 'https://api.deepseek.com/v1' },
  { id: 'groq', name: 'Groq', defaultBase: 'https://api.groq.com/openai/v1' },
  { id: 'openrouter', name: 'OpenRouter', defaultBase: 'https://openrouter.ai/api/v1' },
  { id: 'ollama', name: 'Ollama (Local)', defaultBase: 'http://localhost:11434/v1' },
];

export const IMAGE_PROVIDERS = [
  { id: 'together', name: 'Together AI', hint: 'FLUX · black-forest-labs/FLUX.1-schnell-Free', defaultModel: 'black-forest-labs/FLUX.1-schnell-Free' },
  { id: 'fal', name: 'Fal.ai', hint: 'FLUX, SD models · fal-ai/flux/schnell', defaultModel: 'fal-ai/flux/schnell' },
  { id: 'openai_img', name: 'OpenAI', hint: 'DALL-E · dall-e-3', defaultModel: 'dall-e-3' },
  { id: 'google_img', name: 'Google Imagen', hint: 'Imagen · imagen-3.0-generate-002', defaultModel: 'imagen-3.0-generate-002' },
  { id: 'getimg', name: 'GetImg.ai', hint: 'Stable Diffusion, FLUX · docs.getimg.ai', defaultModel: 'flux-schnell' },
  { id: 'unsplash', name: 'Unsplash', hint: 'Stock photos (free tier available)', defaultModel: '' },
];

export const SEARCH_PROVIDERS = [
  { id: 'tavily', name: 'Tavily', hint: 'AI-optimized search' },
];

export const SLIDE_WIDTH = 960;
export const SLIDE_HEIGHT = 540;
