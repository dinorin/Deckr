# Deckr

> Type a topic. Get a presentation. Export to PowerPoint.

## About

Deckr is an open-source desktop app that turns a plain-text prompt into a fully animated, PowerPoint-compatible slide deck — in seconds, not minutes.

Most AI presentation tools paste a wall of text onto a generic template. Deckr takes a different approach: a multi-agent pipeline where each agent owns exactly one job — research, narrative structure, visual design, HTML rendering, and surgical edits. The result is a deck that actually looks designed, with live animations and editable text that survive the trip into PowerPoint.

Built with Tauri (Rust + React), Deckr runs entirely on your machine. Your API keys stay local. No subscription, no cloud upload, no lock-in.

**Key features:**
- Multi-agent pipeline: Orchestrator → Content → Design → HTML → Edit
- Parallel slide generation (3 slides at a time)
- Web research via Tavily — real data, not hallucinations
- AI image generation (Together AI / Fal.ai / OpenAI)
- Export to `.pptx` with live animations, transitions, and editable text boxes
- Supports Gemini and OpenAI as the LLM backend

---

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
└───────────────────────────────┬─────────────────────────────┘
                                │
          ┌─────────────────────┼─────────────────────┐
          ▼                     ▼                     ▼
   Web Research          Content Agent          Design Agent
   (Tavily API)          Outline + copy         Visual spec per slide
   Real facts, data      Slide types,           Layout variant,
   sourced live          speaker notes,         color palette,
                         transitions            decorative elements
          └─────────────────────┬─────────────────────┘
                                │
                                ▼
                    ┌───────────────────────┐
                    │  HTML Agent           │
                    │  3 slides at a time   │
                    │  (parallel LLM calls) │
                    │  960×540 HTML/CSS     │
                    │  per slide            │
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

**Design Agent** — thinks visually. For each slide it independently chooses a layout variant (hero-centered, split-diagonal, full-bleed-image…), accent colors, background gradients, and decorative elements. No two slides look the same.

**HTML Agent** — the workhorse. Takes the content outline + design spec + research context and writes raw `960×540` HTML/CSS for each slide. Runs **3 slides concurrently** so a 10-slide deck generates in roughly the time of 4 sequential calls. Each slide is a self-contained layer system: `bg → overlay → deco → image → chart → text`.

**Edit Agent** — surgical edits. When you ask to "change the title on slide 3" or "make it more corporate", it reads the existing HTML and rewrites only what needs to change.

---

## Export to PowerPoint

Deckr doesn't just screenshot slides. The export pipeline reconstructs the deck in native OOXML so animations, transitions, and text remain live in PowerPoint.

### What happens on Export PPTX

```
For each slide:

1. Mount slide HTML in an off-screen 960×540 DOM node
   └── Inline all external images (fetched via Tauri's Rust backend to bypass CORS)
   └── Bake computed colors (oklch → hex, safe for canvas)
   └── Auto-fit text: detect overflow, scale fonts down proportionally

2. Render background PNG
   └── Hide text/image/chart layers
   └── html2canvas → bg + deco + overlay baked into one PNG
   └── This PNG becomes shape ID 2 in PptxGenJS

3. Extract image/chart layers (shape IDs 3, 4, …)
   └── <img src="data:…">  → pass data URL directly
   └── <canvas data-chart> → Chart.js renders, export toDataURL

4. Extract text layer backgrounds (nền)
   └── For each text wrapper with a non-transparent background:
       hide inner content → html2canvas → PNG image layer
   └── Placed under the text box so color/border-radius shows in PowerPoint

5. Extract text layers as editable text boxes
   └── Reads data-ppt-font-size, data-ppt-bold, data-ppt-color, data-ppt-align
   └── PptxGenJS addText() with transparent fill → stays editable in PowerPoint

6. Capture FA icon layers
   └── Text layers containing Font Awesome icons → html2canvas
       (webfont already loaded, canvas renders glyphs correctly)
   └── Added on top of text boxes so icons are visible above transparent text

7. Build animation timing XML (OOXML p:timing)
   └── Shape IDs match the exact order PptxGenJS assigned them
   └── Fly-in directions use correct presetSubtype bitmask (left=2, right=1, bottom=4, top=8)
   └── Click groups → delay="indefinite" sequences
   └── Entry animations → delay="0" parallel group

8. Build slide transition XML (p:transition)
   └── fade / push / wipe / cover

9. PptxGenJS writes .pptx buffer
   └── JSZip injects p:timing + p:transition into each slide XML
   └── writeFile via Tauri fs plugin → saved to disk
```

### Shape ID sequence

PptxGenJS assigns shape IDs sequentially in the order shapes are added. The animation XML references these IDs directly, so the order is load-bearing:

| ID | Shape |
|----|-------|
| 1  | nvGrpSpPr (group, implicit) |
| 2  | Background PNG |
| 3… | Image / chart layers |
| …  | Text background PNGs |
| …  | Editable text boxes |
| …  | FA icon PNGs |

---

## Tech stack

| Layer | Technology |
|-------|-----------|
| UI | React 19 + TypeScript + TailwindCSS |
| Desktop shell | Tauri 2 (Rust) |
| Slide rendering | Custom HTML/CSS, Web Animations API |
| Charts | Chart.js |
| Icons | Font Awesome 6 (CDN), Lucide React |
| PPTX generation | PptxGenJS + JSZip |
| Canvas capture | html2canvas |
| LLM | Gemini or OpenAI (configurable) |
| Web research | Tavily API |
| Image generation | Together AI / Fal.ai / OpenAI |

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

- **LLM** — Gemini or OpenAI key (content generation)
- **Search** — Tavily key (web research)
- **Images** — Together AI / Fal.ai / OpenAI key (AI image generation)

---

## License

MIT
