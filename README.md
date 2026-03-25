# Deckr

> Type a topic. Get a presentation. Export to PowerPoint.

Deckr is a desktop app that turns a prompt into a fully animated, PowerPoint-compatible slide deck using a pipeline of specialized AI agents — each responsible for one part of the job.

---

## Features

- **Multi-agent pipeline** — Orchestrator, Content, Design, HTML, and Edit agents work in sequence and parallel
- **Per-slide visual design** — Design agent outputs unique layout, color palette, gradients, and decorative elements for each slide
- **Parallel slide generation** — HTML agent renders multiple slides concurrently via separate LLM calls with unique per-slide system prompts
- **Web research** — Tavily integration fetches real facts, data, and images for the topic before writing any content
- **AI image generation** — Falls back to Together AI / Fal.ai / OpenAI when no real photos are found
- **Image validation** — Filters broken URLs, checks content-type and file size before using images
- **Animations** — Web Animations API with click-reveal sequences; preserved in PPTX export
- **PPTX export** — Reconstructed in native OOXML: editable text, animations, slide transitions, embedded images
- **Auto-open** — Exported file opens immediately in PowerPoint (or system default)
- **Edit existing decks** — Ask in natural language to change a slide, rewrite a section, or restyle the whole deck
- **Session history** — Conversations and decks persist locally; resume any previous session
- **Delete sessions** — Remove individual sessions or clear all history from the start screen

---

## How it works

Most AI presentation tools generate a wall of text and slap it on a template. Deckr doesn't. Instead, the moment you hit send, **five agents fire in parallel or sequence**, each doing exactly one thing.

### Agent pipeline

```
User prompt
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  Orchestrator                                               │
│  Reads intent → routes to Create, Edit, or Chat            │
│  Extracts search keywords for the Research worker          │
└───────────────────────┬─────────────────────────────────────┘
                        │
          ┌─────────────┼─────────────┐
          ▼             ▼             ▼
   Web Research    Content Agent   Design Agent
   (Tavily API)    Outline + copy  Visual spec per slide
   Real facts,     Slide types,    Layout variant,
   images          speaker notes,  color palette,
                   transitions     deco elements
          └─────────────┬─────────────┘
                        │
                        ▼
            ┌───────────────────────┐
            │  HTML Agent           │
            │  Parallel LLM calls   │
            │  Per-slide unique     │
            │  system prompt        │
            │  960×540 HTML/CSS     │
            └───────────┬───────────┘
                        │
                        ▼
            ┌───────────────────────┐
            │  Edit Agent           │
            │  Targeted diffs on    │
            │  existing slides      │
            └───────────────────────┘
```

**Orchestrator** — a single LLM call with tool-use. Decides whether to create a new deck, edit an existing one, or just reply. Extracts search keywords so research can run in parallel while the content agent is already working.

**Content Agent** — structures the narrative. Picks slide types (`title`, `bullets`, `two-column`, `quote`, `image`, `closing`), writes concise copy from the research, and assigns slide transitions.

**Design Agent** — thinks visually. Outputs a structured JSON spec per slide: layout variant, background gradient, accent color, text colors, font, overlay mode, and decorative elements (`circle`, `rect`, `line`, `stripe`, `dots`). No two slides look the same.

**HTML Agent** — the workhorse. Takes the content outline + design spec and generates raw `960×540` HTML/CSS per slide. Each slide gets a **unique system prompt** built from its design spec — exact colors, pre-rendered deco HTML, and a pixel-precise layout table. Runs concurrently so a 10-slide deck generates in roughly the time of 4 sequential calls. Each slide is a self-contained layer system: `bg → overlay → deco → image → chart → text`.

**Edit Agent** — surgical edits. When you ask to "change the title on slide 3" or "make it more corporate", it reads the existing HTML and rewrites only what needs to change.

---

## Export to PowerPoint

Deckr doesn't just screenshot slides. The export pipeline reconstructs the deck in native OOXML so animations, transitions, and text remain live in PowerPoint.

### What happens on Export PPTX

```
For each slide:

1. Mount slide HTML in an off-screen 960×540 DOM node
   └── Inline all external images (fetched via Tauri backend to bypass CORS)
   └── Bake computed colors (oklch → hex, safe for canvas)
   └── Auto-fit text: detect overflow, scale fonts down proportionally

2. Render background PNG
   └── Hide text/image/chart layers
   └── html2canvas → bg + deco + overlay baked into one PNG
   └── This PNG becomes shape ID 2 in PptxGenJS

3. Extract image/chart layers (shape IDs 3, 4, …)
   └── <img src="data:…">  → pass data URL directly
   └── <canvas data-chart> → Chart.js renders, export toDataURL

4. Extract text layer backgrounds
   └── For each text wrapper with a non-transparent background:
       hide inner content → html2canvas → PNG image layer

5. Extract text layers as editable text boxes
   └── Reads data-ppt-font-size, data-ppt-bold, data-ppt-color, data-ppt-align
   └── PptxGenJS addText() with transparent fill → stays editable in PowerPoint

6. Build animation timing XML (OOXML p:timing)
   └── Shape IDs match the exact order PptxGenJS assigned them
   └── Click groups → delay="indefinite" sequences
   └── Entry animations → delay="0" parallel group

7. Build slide transition XML (p:transition)
   └── fade / push / wipe / cover

8. PptxGenJS writes .pptx buffer
   └── JSZip injects p:timing + p:transition into each slide XML
   └── writeFile via Tauri fs plugin → saved to disk
   └── openPath via Tauri opener plugin → opens in PowerPoint
```

---

## Tech stack

| Layer | Technology |
|-------|-----------|
| UI | React 19 + TypeScript + TailwindCSS |
| Desktop shell | Tauri 2 (Rust) |
| Slide rendering | Custom HTML/CSS, Web Animations API |
| Charts | Chart.js |
| Icons | Lucide React |
| PPTX generation | PptxGenJS + JSZip |
| Canvas capture | html2canvas |
| LLM | Any OpenAI-compatible API (Gemini, OpenAI, Ollama, LM Studio…) |
| Web research | Tavily API |
| Image search | Tavily Images |
| Image generation | Together AI / Fal.ai / OpenAI / GetImg |

---

## Setup

**Prerequisites:** Node.js 18+, Rust 1.70+, [Tauri system deps](https://tauri.app/start/prerequisites/)

```bash
git clone https://github.com/dinorin/Deckr.git
cd Deckr
npm install
npm run tauri dev
```

Open **Settings** to add your API keys:

- **LLM** — any OpenAI-compatible key (Gemini, OpenAI, Ollama, LM Studio)
- **Search** — Tavily key (web research + image search)
- **Images** — Together AI / Fal.ai / OpenAI key (AI image generation, optional)

---

## License

MIT
