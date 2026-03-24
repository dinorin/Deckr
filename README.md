# Deckr — AI-Powered Presentation Builder

Deckr is a desktop application that allows you to build stunning, PowerPoint-compatible presentations in minutes using AI. By combining web research, intelligent design agents, and modern web technologies, Deckr transforms a simple topic into a full-bleed, animated slide deck.

## 📖 About

Deckr was born from the frustration of spending hours on slide layouts and searching for relevant content. Unlike traditional AI presentation tools that just generate static text on a template, Deckr uses a **Multi-Agent Orchestration** approach:
1.  **Research Agent**: Scours the web for real-time facts and data.
2.  **Content Agent**: Plans a coherent narrative across slides.
3.  **Animation Agent**: Designs high-level visual sequences.
4.  **Designer Agent**: Writes custom, unique HTML/CSS for every single slide.

The result is a presentation that feels hand-crafted, dynamic, and data-driven.

## 🚀 Key Features

- **AI Orchestration**: Multi-agent system that handles research, content planning, and slide design independently.
- **Deep Web Research**: Integrated with Tavily API to fetch real-time facts, data, and context for your topic.
- **Smart Image Sourcing**: Automatically finds relevant images via Tavily research and high-quality stock sources.
- **AI Image Generation**: Built-in support for Together AI, Fal.ai, and OpenAI to generate custom illustrations.
- **PowerPoint Animations**: Native-like "click-to-reveal" animations and professional slide transitions.
- **Modern UI**: Built with React and Tauri for a fast, native desktop experience.

## 🛠️ Tech Stack

- **Frontend**: React, TypeScript, Vite, TailwindCSS
- **Backend**: Rust, Tauri
- **APIs**: Tavily (Research Engine)

## 🗺️ Roadmap (Task Todo List)

### 🔴 High Priority
- [ ] **PPTX Export**: Implement native `.pptx` generation preserving layouts and animations.
- [ ] **PDF Export**: Single-click "Save as PDF" for static distribution.
- [ ] **Chat UI/UX Overhaul**: Multi-modal input, improved message bubbles, and better agent status visualization.

### 🟡 Improved Design
- [ ] **Icon Library**: Integration with Lucide/FontAwesome for richer visuals.
- [ ] **Charts & Data Viz**: Auto-generation of Chart.js/SVG charts based on research data.
- [ ] **Smart Templates**: Dynamic layouts based on slide intent (Team, Financials, Timeline).

### 🟢 Advanced Features
- [ ] **Local LLM Support**: Native connection to Ollama/Localhost for private generation.
- [ ] **Voice Commands**: Command the presentation builder via voice-to-action.
- [ ] **Version History**: Save and restore previous versions of your deck.

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
