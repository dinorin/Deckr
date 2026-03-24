# Deckr — AI-Powered Presentation Builder

Deckr is a desktop application that allows you to build stunning, PowerPoint-compatible presentations in minutes using AI. By combining web research, intelligent design agents, and modern web technologies, Deckr transforms a simple topic into a full-bleed, animated slide deck.

## 🚀 Key Features

- **AI Orchestration**: Multi-agent system that handles research, content planning, and slide design independently.
- **Deep Web Research**: Integrated with Tavily API to fetch real-time facts, data, and context for your topic.
- **Smart Image Sourcing**: Automatically finds relevant images via Tavily research and high-quality stock sources.
- **AI Image Generation**: Built-in support for Together AI, Fal.ai, and OpenAI to generate custom illustrations.
- **PowerPoint Animations**: Native-like "click-to-reveal" animations and professional slide transitions.
- **PPTX Export**: Export your generated slides to standard `.pptx` format while preserving layout and images.
- **Modern UI**: Built with React and Tauri for a fast, native desktop experience.

## 🛠️ Tech Stack

- **Frontend**: React, TypeScript, Vite, TailwindCSS
- **Backend**: Rust, Tauri
- **AI Models**: Gemini 1.5 Pro/Flash, OpenAI GPT-4o
- **APIs**: Tavily (Research), DuckDuckGo (Image Search)

## 📦 Installation

### Prerequisites
- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/) (v1.70+)
- [Tauri Dependencies](https://tauri.app/v1/guides/getting-started/prerequisites)

### Setup
1. Clone the repository:
   ```bash
   git clone https://github.com/dinorin/Deckr.git
   cd Deckr
   ```
2. Install dependencies:
   ```bash
   npm install
   ```
3. Run in development mode:
   ```bash
   npm run tauri dev
   ```

## 🔑 Configuration

Open the **Settings** modal in the app to configure your API keys:
- **LLM**: Gemini or OpenAI key for content generation.
- **Search**: Tavily API key for web research.
- **Image**: API keys for Together, Fal, or OpenAI for AI image generation.

## 📄 License

MIT License - feel free to use and contribute!
