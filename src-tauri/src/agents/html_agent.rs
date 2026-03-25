use futures::StreamExt;
use serde::Serialize;
use serde_json::json;
use tauri::Emitter;

use super::{AgentContext, AgentMessage, DeckTheme, GeneratedDeck, GeneratedSlide, SlideOutline, call_llm};
use super::design_agent::{DecoSpec, SlideDesignSpec};
use crate::settings::AppSettings;
use crate::tools::ImageResult;

#[derive(Serialize, Clone)]
pub struct SlideReadyPayload {
    pub index: usize,
    pub id: String,
    pub slide_type: String,
    pub html: String,
}

// ── Deco layer renderer ────────────────────────────────────────────────────────
// Deterministic Rust → HTML. Builder never needs to generate deco.

fn render_deco_layer(deco: &[DecoSpec]) -> String {
    if deco.is_empty() { return String::new(); }
    let mut inner = String::new();
    for d in deco {
        let style = match d.kind.as_str() {
            "circle" => {
                let r = d.w / 2;
                format!(
                    "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;\
                     border-radius:50%;background:{};pointer-events:none;",
                    d.x - r, d.y - r, d.w, d.h.max(d.w), d.color
                )
            }
            "rect" => format!(
                "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;\
                 background:{};pointer-events:none;",
                d.x, d.y, d.w, d.h, d.color
            ),
            "line" => format!(
                "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;\
                 background:{};pointer-events:none;",
                d.x, d.y, d.w, d.h.max(1), d.color
            ),
            "stripe" => format!(
                "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;\
                 background:{};transform:rotate({}deg);transform-origin:center;\
                 pointer-events:none;",
                d.x, d.y, d.w, d.h.max(4), d.color, d.angle
            ),
            "dots" => format!(
                "position:absolute;left:{}px;top:{}px;width:{}px;height:{}px;\
                 background-image:radial-gradient({} 1.5px,transparent 1.5px);\
                 background-size:24px 24px;pointer-events:none;",
                d.x, d.y, d.w, d.h, d.color
            ),
            _ => continue,
        };
        inner.push_str(&format!("<div style=\"{}\"></div>", style));
    }
    format!(
        "<div id=\"layer-deco-1\" style=\"position:absolute;inset:0;z-index:3;overflow:hidden;pointer-events:none;\">{}</div>",
        inner
    )
}

fn render_overlay(overlay: &str) -> String {
    match overlay {
        "dark"  => "<div id=\"layer-overlay-1\" style=\"position:absolute;inset:0;z-index:2;background:rgba(0,0,0,0.38);pointer-events:none;\"></div>".into(),
        "light" => "<div id=\"layer-overlay-1\" style=\"position:absolute;inset:0;z-index:2;background:rgba(255,255,255,0.18);pointer-events:none;\"></div>".into(),
        _       => String::new(),
    }
}

// ── Per-layout position tables ─────────────────────────────────────────────────
// Each layout returns focused instructions with exact pixel values and colors.

fn layout_template(spec: &SlideDesignSpec, has_image: bool) -> String {
    let acc = &spec.accent;
    let tp  = &spec.text_primary;
    let ts  = &spec.text_secondary;
    let fnt = &spec.font;

    match spec.layout.as_str() {

        "title-hero" => format!(r#"## Layout: title-hero — centered hero, no image
Position table:
  layer-text-1 (title)    left=80  top=160 w=800 h=90   font=60px bold center   click=0 anim=fly-in-bottom
  layer-text-2 (subtitle) left=80  top=268 w=800 h=52   font=28px center        click=1 anim=fade-in
  → bottom subtitle = 320 ✓

Title:    class="text-6xl font-black text-center leading-tight"  style="color:{tp};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Subtitle: class="text-2xl text-center leading-snug"              style="color:{ts};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
data-ppt-color must match: title={tp}  subtitle={ts}
data-ppt-align="center" on both."#, tp=tp, ts=ts),

        "title-split" => format!(r#"## Layout: title-split — left text, right solid panel
Position table:
  layer-deco-2 (panel)    left=520 top=0   w=440 h=540  solid accent panel   z-index:3
  layer-text-1 (title)    left=40  top=180 w=440 h=80   font=52px bold       click=0 anim=fly-in-right
  layer-text-2 (subtitle) left=40  top=278 w=440 h=48   font=24px            click=1 anim=fade-in

Add panel as a SECOND deco div immediately after layer-deco-1:
<div id="layer-deco-2" style="position:absolute;left:520px;top:0;width:440px;height:540px;z-index:3;background:{acc};opacity:0.92;"></div>

Title:    class="text-5xl font-black leading-tight"  style="color:{tp};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Subtitle: class="text-2xl leading-snug"              style="color:{ts};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
data-ppt-color must match: title={tp}  subtitle={ts}
data-ppt-align="left" on both."#, acc=acc, tp=tp, ts=ts),

        "bullets" => format!(r#"## Layout: bullets — title + up to 5 text bullets
Position table:
  layer-text-1 (title)    left=40 top=32  w=880 h=56  font=40px bold   click=0 anim=wipe-left
  layer-text-2 (bullet 1) left=56 top=108 w=860 h=44  font=20px        click=1 anim=fly-in-left
  layer-text-3 (bullet 2) left=56 top=162 w=860 h=44                   click=2 anim=fly-in-left
  layer-text-4 (bullet 3) left=56 top=216 w=860 h=44                   click=3 anim=fly-in-left
  layer-text-5 (bullet 4) left=56 top=270 w=860 h=44                   click=4 anim=fly-in-left
  layer-text-6 (bullet 5) left=56 top=324 w=860 h=44                   click=5 anim=fly-in-left
  → bottom bullet-5 = 368 ✓

Title inner:  class="text-4xl font-bold leading-tight"
              style="color:{tp};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Bullet inner: class="text-xl leading-snug" style="color:{tp};"
              Prefix each bullet with <span style="color:{acc};margin-right:10px;">•</span>
data-ppt-color must match: title={tp}  bullets={tp}"#,
              tp=tp, acc=acc),

        "bullets-icon" => format!(r#"## Layout: bullets-icon — title + up to 5 bullets each with a Lucide icon
Position table (same as bullets):
  layer-text-1 (title)    left=40 top=32  w=880 h=56   click=0 anim=wipe-left
  layer-text-2..6 (bullets) left=40 top=108..324 w=880 h=44  click=1..5 anim=fly-in-left
  Row spacing: 54px (top: 108, 162, 216, 270, 324)

Each bullet inner content uses flex row:
<div style="display:flex;align-items:center;gap:12px;height:100%;">
  <i data-lucide="ICON_NAME" style="width:20px;height:20px;color:{acc};flex-shrink:0;"></i>
  <span class="text-xl leading-snug" style="color:{tp};"
        data-ppt-font-size="20" data-ppt-bold="false" data-ppt-color="{tp}" data-ppt-align="left" data-ppt-font="{fnt}">Bullet text</span>
</div>
Choose relevant icons from: rocket trending-up shield zap star globe users code lightbulb trophy check arrow-right flame brain cpu database target award"#,
              acc=acc, tp=tp, fnt=fnt),

        "content-right" | "content" => format!(r#"## Layout: content-right — text left, image right
Position table:
  layer-text-1 (title)  left=40  top=32  w=520 h=56  font=40px bold   click=0 anim=wipe-left
  layer-text-2 (body 1) left=40  top=108 w=500 h=48  font=20px        click=1 anim=float-in
  layer-text-3 (body 2) left=40  top=176 w=500 h=48                   click=2 anim=float-in
  layer-text-4 (body 3) left=40  top=244 w=500 h=48                   click=3 anim=float-in
  layer-text-5 (body 4) left=40  top=312 w=500 h=48                   click=4 anim=float-in
  layer-image-1 (image) left=560 top=32  w=360 h=476                  click=0 anim=fade-in
  → TEXT ZONE: x=40..540   IMAGE ZONE: x=560..920   NO overlap between zones.
  → bottom body-4 = 360 ✓{img_note}

Title inner:  class="text-4xl font-bold leading-tight"
              style="color:{tp};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Body inner:   class="text-xl leading-snug" style="color:{tp};"
data-ppt-color must match: {tp}"#,
              tp=tp,
              img_note = if has_image { "" } else { "\n  No image available → use ai-gen-image placeholder." }),

        "content-left" => format!(r#"## Layout: content-left — image left, text right
Position table:
  layer-image-1 (image) left=40  top=32  w=360 h=476                  click=0 anim=fade-in
  layer-text-1 (title)  left=440 top=32  w=480 h=56  font=40px bold   click=0 anim=wipe-left
  layer-text-2 (body 1) left=440 top=108 w=480 h=48  font=20px        click=1 anim=float-in
  layer-text-3 (body 2) left=440 top=176 w=480 h=48                   click=2 anim=float-in
  layer-text-4 (body 3) left=440 top=244 w=480 h=48                   click=3 anim=float-in
  layer-text-5 (body 4) left=440 top=312 w=480 h=48                   click=4 anim=float-in
  → IMAGE ZONE: x=40..400   TEXT ZONE: x=440..920   NO overlap.{img_note}

Title inner:  class="text-4xl font-bold leading-tight"
              style="color:{tp};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Body inner:   class="text-xl leading-snug" style="color:{tp};"
data-ppt-color must match: {tp}"#,
              tp=tp,
              img_note = if has_image { "" } else { "\n  No image → use ai-gen-image placeholder." }),

        "two-column" => format!(r#"## Layout: two-column — title + two equal columns
Position table:
  layer-text-1  (title)   left=40  top=32  w=880 h=56  font=40px bold  click=0 anim=fade-in
  layer-deco-2  (divider) left=480 top=100 w=2   h=360 background:{acc}
  layer-text-2  (lhead)   left=40  top=108 w=400 h=40  font=22px bold  click=1 anim=fly-in-left
  layer-text-3  (lbody 1) left=40  top=158 w=400 h=44  font=19px       click=2 anim=fly-in-left
  layer-text-4  (lbody 2) left=40  top=212 w=400 h=44                  click=3 anim=fly-in-left
  layer-text-5  (lbody 3) left=40  top=266 w=400 h=44                  click=4 anim=fly-in-left
  layer-text-6  (rhead)   left=520 top=108 w=400 h=40  font=22px bold  click=1 anim=fly-in-right
  layer-text-7  (rbody 1) left=520 top=158 w=400 h=44  font=19px       click=2 anim=fly-in-right
  layer-text-8  (rbody 2) left=520 top=212 w=400 h=44                  click=3 anim=fly-in-right
  layer-text-9  (rbody 3) left=520 top=266 w=400 h=44                  click=4 anim=fly-in-right

Add divider after layer-deco-1:
<div id="layer-deco-2" style="position:absolute;left:480px;top:100px;width:2px;height:360px;z-index:3;background:{acc};"></div>

Title:  class="text-4xl font-bold leading-tight"  style="color:{tp};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Heads:  class="text-2xl font-bold leading-tight"  style="color:{acc};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Bodies: class="text-lg leading-snug"              style="color:{tp};"
data-ppt-color must match: title/bodies={tp}  heads={acc}"#,
              acc=acc, tp=tp),

        "quote" => format!(r#"## Layout: quote — large centered quote
Position table:
  layer-deco-2  (bar)    left=40 top=130 w=6   h=160  solid accent bar
  layer-text-1  (quote)  left=74 top=140 w=822 h=160  font=30px italic center  click=0 anim=fade-in
  layer-text-2  (author) left=74 top=318 w=822 h=40   font=18px right          click=1 anim=float-in

Add accent bar after layer-deco-1:
<div id="layer-deco-2" style="position:absolute;left:40px;top:130px;width:6px;height:160px;z-index:3;background:{acc};"></div>

Quote inner:  class="text-3xl italic leading-snug text-center"
              style="color:{tp};text-align:center;" (multi-line — no white-space:nowrap)
Author inner: class="text-lg"
              style="color:{ts};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;text-align:right;"
              Prefix author with "— "
data-ppt-color must match: quote={tp}  author={ts}"#,
              acc=acc, tp=tp, ts=ts),

        "icon-grid" => format!(r#"## Layout: icon-grid — title + 4 icon+label cards
Position table:
  layer-text-1 (title)     left=40  top=32  w=880 h=56   click=0 anim=fade-in
  layer-text-2 (card 1)    left=80  top=140 w=180 h=220  click=1 anim=zoom-in
  layer-text-3 (card 2)    left=300 top=140 w=180 h=220  click=1 anim=zoom-in
  layer-text-4 (card 3)    left=520 top=140 w=180 h=220  click=1 anim=zoom-in
  layer-text-5 (card 4)    left=740 top=140 w=180 h=220  click=1 anim=zoom-in

Each card layer inner content:
<div style="display:flex;flex-direction:column;align-items:center;justify-content:center;gap:14px;height:100%;">
  <i data-lucide="ICON" style="width:56px;height:56px;color:{acc};"></i>
  <span class="text-base font-semibold text-center" style="color:{tp};"
        data-ppt-font-size="15" data-ppt-bold="true" data-ppt-color="{tp}" data-ppt-align="center" data-ppt-font="{fnt}">Label text</span>
</div>
Icon color: {acc}. Choose distinct relevant icons for each card."#,
              acc=acc, tp=tp, fnt=fnt),

        "image-full" => format!(r#"## Layout: image-full — title + large image + caption
Position table:
  layer-text-1  (title)   left=40 top=32  w=880 h=56   font=40px bold   click=0 anim=fly-in-top
  layer-image-1 (image)   left=40 top=108 w=880 h=360                   click=0 anim=fade-in
  layer-text-2  (caption) left=40 top=482 w=880 h=24   font=14px center click=0 anim=appear
  → TITLE zone: y=32..88  IMAGE zone: y=108..468  CAPTION zone: y=482..506 ✓{img_note}

Title inner:   class="text-4xl font-bold leading-tight"
               style="color:{tp};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Caption inner: class="text-sm text-center"
               style="color:{ts};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
data-ppt-color must match: title={tp}  caption={ts}"#,
               tp=tp, ts=ts,
               img_note = if has_image { "" } else { "\n  No image available → use ai-gen-image placeholder." }),

        "stat-cards" => format!(r#"## Layout: stat-cards — title + 3 large stat numbers
Position table:
  layer-text-1  (title)   left=40  top=32  w=880 h=56   font=40px bold        click=0 anim=fade-in
  layer-deco-2  (card bg1) left=40  top=120 w=260 h=260  card background
  layer-deco-3  (card bg2) left=350 top=120 w=260 h=260
  layer-deco-4  (card bg3) left=660 top=120 w=260 h=260
  layer-text-2  (stat 1)  left=40  top=150 w=260 h=110  font=64px bold center click=1 anim=zoom-in
  layer-text-3  (label 1) left=40  top=268 w=260 h=36   font=15px center      click=1 anim=fade-in
  layer-text-4  (stat 2)  left=350 top=150 w=260 h=110  font=64px bold center click=2 anim=zoom-in
  layer-text-5  (label 2) left=350 top=268 w=260 h=36   font=15px center      click=2 anim=fade-in
  layer-text-6  (stat 3)  left=660 top=150 w=260 h=110  font=64px bold center click=3 anim=zoom-in
  layer-text-7  (label 3) left=660 top=268 w=260 h=36   font=15px center      click=3 anim=fade-in

Card backgrounds (add after layer-deco-1):
<div id="layer-deco-2" style="position:absolute;left:40px;top:120px;width:260px;height:260px;z-index:3;background:rgba(255,255,255,0.05);border:1px solid {acc};border-radius:12px;"></div>
<div id="layer-deco-3" style="position:absolute;left:350px;top:120px;width:260px;height:260px;z-index:3;background:rgba(255,255,255,0.05);border:1px solid {acc};border-radius:12px;"></div>
<div id="layer-deco-4" style="position:absolute;left:660px;top:120px;width:260px;height:260px;z-index:3;background:rgba(255,255,255,0.05);border:1px solid {acc};border-radius:12px;"></div>

Stat numbers: class="text-6xl font-black text-center leading-none" style="color:{acc};"
Labels:       class="text-sm font-semibold text-center"            style="color:{ts};"
data-ppt-color must match: stats={acc}  labels={ts}"#,
              acc=acc, ts=ts),

        "closing" => format!(r#"## Layout: closing — large centered message
Position table:
  layer-text-1 (title)    left=40 top=185 w=880 h=80   font=56px bold center  click=0 anim=zoom-in
  layer-text-2 (subtitle) left=40 top=285 w=880 h=48   font=28px center       click=1 anim=fade-in
  → bottom subtitle = 333 ✓

Title:    class="text-6xl font-black text-center leading-tight"  style="color:{tp};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Subtitle: class="text-2xl text-center leading-snug"              style="color:{ts};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
data-ppt-color must match: title={tp}  subtitle={ts}
data-ppt-align="center" on both."#, tp=tp, ts=ts),

        // fallback
        _ => format!(r#"## Layout: bullets (default)
Position table:
  layer-text-1 (title)    left=40 top=32  w=880 h=56  font=40px bold  click=0 anim=wipe-left
  layer-text-2 (bullet 1) left=56 top=108 w=860 h=44  font=20px       click=1 anim=fly-in-left
  layer-text-3 (bullet 2) left=56 top=162 w=860 h=44                  click=2 anim=fly-in-left
  layer-text-4 (bullet 3) left=56 top=216 w=860 h=44                  click=3 anim=fly-in-left
  layer-text-5 (bullet 4) left=56 top=270 w=860 h=44                  click=4 anim=fly-in-left
  layer-text-6 (bullet 5) left=56 top=324 w=860 h=44                  click=5 anim=fly-in-left

Title inner:  class="text-4xl font-bold leading-tight"  style="color:{tp};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
Bullet inner: class="text-xl leading-snug"              style="color:{tp};"
data-ppt-color must match: {tp}"#, tp=tp),
    }
}

// ── Per-slide system prompt builder ───────────────────────────────────────────
// Each builder call gets a unique system prompt with exact colors, pre-rendered
// deco, and only the relevant layout template — no design guesswork needed.

fn build_slide_system_prompt(spec: &SlideDesignSpec, has_image: bool) -> String {
    let deco_html    = render_deco_layer(&spec.deco);
    let overlay_html = render_overlay(&spec.overlay);
    let layout_sec   = layout_template(spec, has_image);

    format!(r##"You are a world-class slide builder. Output ONLY raw HTML for one 960×540 slide.
No markdown, no code fences, no explanation — start directly with <div class="ppt-slide"

## Canvas: 960×540px
Safe zone: x≥40  y≥30  x+w≤920  y+h≤510
Vertical gap between any two elements: ≥20px  (top_next ≥ bottom_prev + 20)

{layout_sec}

## Slide colors
Background : {bg_css}
Accent     : {accent}
Text       : {text_primary}  (secondary: {text_secondary})
Font       : {font}

## Slide wrapper — copy EXACTLY, substitute INDEX and TRANSITION
<div class="ppt-slide" data-slide-index="INDEX" data-bg-color="{bg_hex}" data-transition="TRANSITION"
     style="position:relative;width:960px;height:540px;overflow:hidden;box-sizing:border-box;font-family:'{font}',sans-serif;">

## First children — REQUIRED, copy verbatim, do NOT modify
<div id="layer-bg-1" style="position:absolute;inset:0;z-index:1;background:{bg_css};"></div>
{overlay_html}{deco_html}

## Layer z-index rules
bg=1  overlay=2  deco=3  image=4  chart=5  text=10
Text layers ALWAYS z-index:10. Never place image/deco above text.

## Text layer wrapper — ONLY these exact styles, no extras
style="position:absolute;left:Xpx;top:Ypx;width:Wpx;height:Hpx;overflow:hidden;z-index:10;"
- Single-line inner tags: add  style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
- Multi-line inner tags: NO white-space:nowrap

## Text tag — REQUIRED attributes on every inner text element
style="color:#hex;"  ← MANDATORY — always set inline color, NEVER rely on inheritance
data-ppt-font-size="N" data-ppt-bold="true|false" data-ppt-color="#hex"
data-ppt-align="left|center|right" data-ppt-font="{font}"
RULE: data-ppt-color and style="color:..." MUST always match. Both are required.

## Animation
class="ppt-element ppt-hidden ppt-ANIM"  data-click="N"  data-duration="500"  data-ppt-animation="ANIM"
click=0=entry  click=N=Nth click reveal  max 6 clicks per slide
Anims: ppt-fade-in  ppt-fly-in-bottom  ppt-fly-in-top  ppt-fly-in-left  ppt-fly-in-right
       ppt-zoom-in  ppt-float-in  ppt-wipe-left  ppt-bounce-in

## Image pattern
<img id="layer-image-1" src="URL"
     data-ppt-animation="fade-in" data-click="0"
     style="position:absolute;left:Xpx;top:Ypx;width:Wpx;height:Hpx;object-fit:cover;z-index:4;">
No URL → add class="ai-gen-image" src="" and data-prompt="short English description"

## Hard rules
- NO CSS transform for layout (breaks animations)
- Absolute px only — no %, vw, vh
- overflow:hidden + z-index:10 on ALL layer-text-* wrappers
- Images must NOT overlap text bounding boxes
- bg-1, overlay-1, deco-1 children must be copied from above VERBATIM"##,
        layout_sec   = layout_sec,
        bg_css       = spec.bg_css,
        bg_hex       = spec.bg_hex,
        accent       = spec.accent,
        text_primary = spec.text_primary,
        text_secondary = spec.text_secondary,
        font         = spec.font,
        overlay_html = overlay_html,
        deco_html    = deco_html,
    )
}

// ── Theme derivation ───────────────────────────────────────────────────────────

pub fn derive_theme(style_hint: &str, topic: &str) -> DeckTheme {
    let h = format!("{} {}", style_hint, topic).to_lowercase();
    let (primary, secondary, bg, text, accent, font, style) =
        if h.contains("minimal") || h.contains("clean") || h.contains("light") || h.contains("white") {
            ("#334155", "#475569", "#f8fafc", "#1e293b", "#6366f1", "Inter", "minimal")
        } else if h.contains("bold") || h.contains("vibrant") || h.contains("colorful") {
            ("#f59e0b", "#ef4444", "#18181b", "#ffffff", "#f59e0b", "Montserrat", "bold")
        } else if h.contains("corporate") || h.contains("business") || h.contains("professional") {
            ("#1d4ed8", "#1e40af", "#0f172a", "#f1f5f9", "#3b82f6", "Inter", "corporate")
        } else if h.contains("creative") || h.contains("art") || h.contains("design") {
            ("#a855f7", "#ec4899", "#09090b", "#fafafa", "#a855f7", "Poppins", "creative")
        } else {
            ("#6366f1", "#8b5cf6", "#0f0f1f", "#ffffff", "#f59e0b", "Montserrat", "modern")
        };
    DeckTheme {
        primary_color:   primary.into(),
        secondary_color: secondary.into(),
        bg_color:        bg.into(),
        text_color:      text.into(),
        accent_color:    accent.into(),
        font_family:     font.into(),
        style:           style.into(),
    }
}

/// Slide types/layouts that should NEVER get an image slot.
fn slide_excludes_image(slide_type: &str, layout: &str) -> bool {
    matches!(slide_type, "title" | "closing" | "quote")
        || matches!(layout, "title-hero" | "title-split" | "closing" | "quote" | "stat-cards" | "icon-grid")
}

/// Layouts where image is a primary element (must be placed prominently).
fn layout_needs_image(layout: &str) -> bool {
    matches!(layout, "content-right" | "content-left" | "content" | "image-full")
}

// ── Run ────────────────────────────────────────────────────────────────────────

pub async fn run(
    app: &tauri::AppHandle,
    settings: &AppSettings,
    ctx: &AgentContext,
    outline: &[SlideOutline],
    theme: &DeckTheme,
    images: &[ImageResult],
    design_specs: &[SlideDesignSpec],
) -> Result<GeneratedDeck, String> {
    let total = outline.len();

    // Pre-assign images: one image per eligible slide, no repeats.
    // Eligible = not title/closing/quote/icon-grid/stat-cards.
    // When pool runs out, remaining eligible slides get None → ai-gen-image.
    let mut image_pool = images.iter();
    let assigned_images: Vec<Option<ImageResult>> = (0..total).map(|i| {
        let stype  = outline.get(i).map(|o| o.slide_type.as_str()).unwrap_or("");
        let layout = design_specs.get(i).map(|d| d.layout.as_str()).unwrap_or("");
        if slide_excludes_image(stype, layout) {
            None
        } else {
            image_pool.next().cloned()
        }
    }).collect();

    let settings_arc = std::sync::Arc::new(settings.clone());
    let specs_arc    = std::sync::Arc::new(design_specs.to_vec());
    let assigned_arc = std::sync::Arc::new(assigned_images);
    let language     = ctx.language.clone();

    let tasks = outline.to_vec().into_iter().enumerate().map(|(i, slide)| {
        let s    = settings_arc.clone();
        let lang = language.clone();
        let spec = specs_arc.get(i).cloned();
        let img  = assigned_arc.get(i).and_then(|o| o.clone());
        async move {
            let result = generate_single_slide(&s, i, total, &slide, &lang, img.as_ref(), spec.as_ref()).await?;
            Ok::<(usize, GeneratedSlide), String>((i, result))
        }
    });

    let mut results: Vec<(usize, GeneratedSlide)> = futures::stream::iter(tasks)
        .buffer_unordered(3)
        .collect::<Vec<Result<_, _>>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    results.sort_by_key(|(i, _)| *i);

    let mut slides: Vec<GeneratedSlide> = Vec::with_capacity(total);
    for (i, slide) in results {
        let _ = app.emit("slide-ready", SlideReadyPayload {
            index: i,
            id:    slide.id.clone(),
            slide_type: slide.slide_type.clone(),
            html:  slide.html.clone(),
        });
        slides.push(slide);
    }

    let title = outline.first()
        .map(|s| s.title.clone())
        .unwrap_or_else(|| ctx.topic.clone());

    let master_html = build_master_html(&title, &slides);

    let theme_json = json!({
        "primaryColor":   theme.primary_color,
        "secondaryColor": theme.secondary_color,
        "backgroundColor": theme.bg_color,
        "textColor":      theme.text_color,
        "fontFamily":     theme.font_family,
        "style":          theme.style,
    });

    Ok(GeneratedDeck { title, theme: theme_json, slides, master_html, coach_message: String::new() })
}

// ── Single slide generation ────────────────────────────────────────────────────

async fn generate_single_slide(
    settings: &AppSettings,
    index: usize,
    total: usize,
    outline: &SlideOutline,
    language: &str,
    image: Option<&ImageResult>,   // at most ONE image per slide, pre-assigned
    design: Option<&SlideDesignSpec>,
) -> Result<GeneratedSlide, String> {

    let has_image = image.is_some();

    // Build per-slide system prompt from design spec
    let system_prompt = if let Some(spec) = design {
        build_slide_system_prompt(spec, has_image)
    } else {
        build_slide_system_prompt(&SlideDesignSpec {
            layout:         "bullets".into(),
            bg_css:         "#0f0f1f".into(),
            bg_hex:         "#0f0f1f".into(),
            accent:         "#6366f1".into(),
            text_primary:   "#ffffff".into(),
            text_secondary: "#94a3b8".into(),
            font:           "Inter".into(),
            overlay:        "none".into(),
            deco:           vec![],
            mood:           "bold".into(),
        }, has_image)
    };

    let bullets_str = if outline.bullets.is_empty() {
        String::new()
    } else {
        format!("\nContent points: {}", outline.bullets.join(" | "))
    };

    let layout_str = design.map(|d| d.layout.as_str()).unwrap_or("bullets");
    let needs_img  = layout_needs_image(layout_str);

    // Each slide gets exactly ONE image (pre-assigned, no repeats across slides).
    // Layout that needs image + no URL → MUST use ai-gen-image.
    let image_block = match image {
        Some(img) => format!(
            "\nImage assigned to this slide (MUST include it at the position shown in the layout):\
             \n  URL: {}\n  avg-color: {}  text-overlay-color: {}\
             \n  Use <img id=\"layer-image-1\" src=\"{}\" ...> with the coordinates from the layout table above.",
            img.url, img.avg_color, img.text_color, img.url
        ),
        None if needs_img => concat!(
            "\nNo Tavily image available — MUST add an ai-gen-image placeholder at the image position:\n",
            "<img id=\"layer-image-1\" class=\"ai-gen-image\" src=\"\" ",
            "data-prompt=\"vivid English description of an ideal photo for this slide\" ",
            "data-ppt-animation=\"fade-in\" data-click=\"0\" ",
            "style=\"position:absolute;left:Xpx;top:Ypx;width:Wpx;height:Hpx;object-fit:cover;z-index:4;\">"
        ).into(),
        None => String::new(),
    };

    let user_msg = format!(
        "Slide {idx}/{total} | type:{stype} | transition:{tr} | language:{lang}\nTitle: \"{title}\"{bullets}{images}\n\nOutput ONLY raw HTML starting with <div class=\"ppt-slide\"",
        idx     = index + 1,
        total   = total,
        stype   = outline.slide_type,
        tr      = outline.transition,
        lang    = language,
        title   = outline.title,
        bullets = bullets_str,
        images  = image_block,
    );

    let history = vec![AgentMessage { role: "user".into(), content: user_msg }];
    let resp = call_llm(settings, &system_prompt, &history, &json!([])).await
        .map_err(|e| e.to_string())?;

    let raw = resp.text.unwrap_or_default();
    let raw = raw.trim();

    let mut html = if raw.contains("ppt-slide") {
        raw.to_string()
    } else {
        let extracted = extract_html_div(raw);
        if extracted.contains("ppt-slide") { extracted }
        else {
            build_fallback_slide(outline, design, index)
        }
    };

    // Stamp transition attribute
    let tr = &outline.transition;
    if !tr.is_empty() && tr != "none" && !html.contains("data-transition") {
        html = html.replacen(
            "class=\"ppt-slide\"",
            &format!("class=\"ppt-slide\" data-transition=\"{}\"", tr),
            1,
        );
    }

    Ok(GeneratedSlide {
        id:         format!("s{}", index + 1),
        slide_type: outline.slide_type.clone(),
        html,
    })
}

// ── Utilities ──────────────────────────────────────────────────────────────────

fn extract_html_div(text: &str) -> String {
    if let Some(start) = text.find("<div") {
        if let Some(end) = text.rfind("</div>") {
            return text[start..end + 6].to_string();
        }
        return text[start..].to_string();
    }
    if let Some(start) = text.find("```html") {
        let after = &text[start + 7..];
        let end = after.find("```").unwrap_or(after.len());
        return after[..end].trim().to_string();
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        let end = after.find("```").unwrap_or(after.len());
        return after[..end].trim().to_string();
    }
    text.to_string()
}

fn build_fallback_slide(outline: &SlideOutline, design: Option<&SlideDesignSpec>, index: usize) -> String {
    let bg     = design.map(|d| d.bg_css.as_str()).unwrap_or("#0f0f1f");
    let bg_hex = design.map(|d| d.bg_hex.as_str()).unwrap_or("#0f0f1f");
    let acc    = design.map(|d| d.accent.as_str()).unwrap_or("#6366f1");
    let text   = design.map(|d| d.text_primary.as_str()).unwrap_or("#ffffff");
    let font   = design.map(|d| d.font.as_str()).unwrap_or("Inter");
    let tr     = &outline.transition;
    let title  = html_escape(&outline.title);

    let bullets_html = outline.bullets.iter().enumerate().map(|(i, b)| {
        format!(
            r#"<div id="layer-text-{n}" class="ppt-element ppt-hidden ppt-fly-in-left"
     data-click="{click}" data-duration="500" data-ppt-animation="fly-in-left"
     style="position:absolute;left:56px;top:{y}px;width:860px;height:44px;overflow:hidden;z-index:10;">
  <p class="text-xl leading-snug" style="color:{text};"
     data-ppt-font-size="20" data-ppt-bold="false" data-ppt-color="{text}" data-ppt-align="left" data-ppt-font="{font}">• {bullet}</p>
</div>"#,
            n = i + 2, click = i + 1, y = 108 + i * 54,
            text = text, font = font, bullet = html_escape(b),
        )
    }).collect::<Vec<_>>().join("\n");

    format!(
        r#"<div class="ppt-slide" data-slide-index="{idx}" data-bg-color="{bg_hex}" data-transition="{tr}"
     style="position:relative;width:960px;height:540px;overflow:hidden;box-sizing:border-box;font-family:'{font}',sans-serif;">
  <div id="layer-bg-1" style="position:absolute;inset:0;z-index:1;background:{bg};"></div>
  <div id="layer-deco-1" style="position:absolute;left:0;bottom:0;right:0;height:3px;z-index:3;background:{acc};"></div>
  <div id="layer-text-1" class="ppt-element ppt-hidden ppt-wipe-left"
       data-click="0" data-duration="500" data-ppt-animation="wipe-left"
       style="position:absolute;left:40px;top:32px;width:880px;height:56px;overflow:hidden;z-index:10;">
    <h2 class="text-4xl font-bold leading-tight" style="color:{text};white-space:nowrap;overflow:hidden;text-overflow:ellipsis;"
        data-ppt-font-size="40" data-ppt-bold="true" data-ppt-color="{text}" data-ppt-align="left" data-ppt-font="{font}">{title}</h2>
  </div>
{bullets}
</div>"#,
        idx = index, bg_hex = bg_hex, tr = tr, font = font,
        bg = bg, acc = acc, text = text, title = title, bullets = bullets_html,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ── Master HTML ────────────────────────────────────────────────────────────────

pub fn build_master_html(title: &str, slides: &[GeneratedSlide]) -> String {
    let slides_html: String = slides.iter().enumerate().map(|(i, s)| {
        if s.html.contains("data-slide-index") {
            s.html.clone()
        } else {
            s.html.replacen("<div", &format!("<div data-slide-index=\"{}\"", i), 1)
        }
    }).collect::<Vec<_>>().join("\n");

    format!(r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<script src="https://cdn.jsdelivr.net/npm/lucide@0.400.0/dist/umd/lucide.min.js"></script>
<script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
<script src="https://cdn.tailwindcss.com"></script>
<style>
*,*::before,*::after{{box-sizing:border-box;margin:0;padding:0}}
html,body{{width:100%;height:100%;overflow:hidden;background:#111;color:#fff;display:flex;align-items:center;justify-content:center}}
#deck{{position:relative;width:960px;height:540px;transform-origin:center center;}}
.ppt-slide{{position:absolute;inset:0;width:960px;height:540px;overflow:hidden;display:none;}}
.ppt-slide.active{{display:block;z-index:10;}}
.ppt-slide.exiting{{display:block;z-index:5;}}
.ppt-element{{display:block;position:relative;box-sizing:border-box;}}
.ppt-element.ppt-hidden{{opacity:0!important;visibility:hidden!important;pointer-events:none;}}
.ppt-slide.tr-fade-enter{{animation:deckFadeIn .4s ease forwards;}}
.ppt-slide.tr-fade-exit{{animation:deckFadeOut .4s ease forwards;}}
@keyframes deckFadeIn{{from{{opacity:0;}}to{{opacity:1;}}}}
@keyframes deckFadeOut{{from{{opacity:1;}}to{{opacity:0;}}}}
.ppt-slide.tr-push-enter{{animation:deckPushEnter .45s cubic-bezier(.4,0,.2,1) forwards;}}
.ppt-slide.tr-push-exit{{animation:deckPushExit .45s cubic-bezier(.4,0,.2,1) forwards;}}
@keyframes deckPushEnter{{from{{transform:translateX(100%);}}to{{transform:translateX(0);}}}}
@keyframes deckPushExit{{from{{transform:translateX(0);}}to{{transform:translateX(-100%);}}}}
.ppt-slide.tr-push-back-enter{{animation:deckPushBackEnter .45s cubic-bezier(.4,0,.2,1) forwards;}}
.ppt-slide.tr-push-back-exit{{animation:deckPushBackExit .45s cubic-bezier(.4,0,.2,1) forwards;}}
@keyframes deckPushBackEnter{{from{{transform:translateX(-100%);}}to{{transform:translateX(0);}}}}
@keyframes deckPushBackExit{{from{{transform:translateX(0);}}to{{transform:translateX(100%);}}}}
.ppt-slide.tr-wipe-enter{{animation:deckWipeEnter .5s ease-out forwards;clip-path:inset(0 100% 0 0);}}
@keyframes deckWipeEnter{{from{{clip-path:inset(0 100% 0 0);}}to{{clip-path:inset(0 0 0 0);}}}}
#progress{{position:fixed;bottom:0;left:0;height:2px;background:rgba(255,255,255,.35);transition:width .3s ease;z-index:100;}}
</style>
</head>
<body>
<div id="deck">
{slides_html}
</div>
<div id="progress"></div>
<script>
(function(){{
  var ANIMS = {{
    'appear':        function(e,d){{return e.animate([{{opacity:0}},{{opacity:1}}],{{duration:Math.min(d,100),fill:'forwards'}});}},
    'fade-in':       function(e,d){{return e.animate([{{opacity:0}},{{opacity:1}}],{{duration:d,easing:'ease',fill:'forwards'}});}},
    'fly-in-bottom': function(e,d){{return e.animate([{{opacity:0,transform:'translateY(110%)'}},{{opacity:1,transform:'translateY(0)'}}],{{duration:d,easing:'cubic-bezier(.25,.46,.45,.94)',fill:'forwards'}});}},
    'fly-in-top':    function(e,d){{return e.animate([{{opacity:0,transform:'translateY(-110%)'}},{{opacity:1,transform:'translateY(0)'}}],{{duration:d,easing:'cubic-bezier(.25,.46,.45,.94)',fill:'forwards'}});}},
    'fly-in-left':   function(e,d){{return e.animate([{{opacity:0,transform:'translateX(-110%)'}},{{opacity:1,transform:'translateX(0)'}}],{{duration:d,easing:'cubic-bezier(.25,.46,.45,.94)',fill:'forwards'}});}},
    'fly-in-right':  function(e,d){{return e.animate([{{opacity:0,transform:'translateX(110%)'}},{{opacity:1,transform:'translateX(0)'}}],{{duration:d,easing:'cubic-bezier(.25,.46,.45,.94)',fill:'forwards'}});}},
    'zoom-in':       function(e,d){{return e.animate([{{opacity:0,transform:'scale(0.3)'}},{{opacity:1,transform:'scale(1)'}}],{{duration:d,easing:'cubic-bezier(.175,.885,.32,1.275)',fill:'forwards'}});}},
    'bounce-in':     function(e,d){{return e.animate([{{opacity:0,transform:'scale(0.3)'}},{{opacity:1,transform:'scale(1.08)'}},{{opacity:1,transform:'scale(0.94)'}},{{opacity:1,transform:'scale(1)'}}],{{duration:d,fill:'forwards'}});}},
    'float-in':      function(e,d){{return e.animate([{{opacity:0,transform:'translateY(36px)'}},{{opacity:1,transform:'translateY(0)'}}],{{duration:d,easing:'cubic-bezier(.22,1,.36,1)',fill:'forwards'}});}},
    'wipe-left':     function(e,d){{e.style.opacity='1';return e.animate([{{clipPath:'inset(0 100% 0 0)'}},{{clipPath:'inset(0 0% 0 0)'}}],{{duration:d,easing:'ease-out',fill:'forwards'}});}},
    'split':         function(e,d){{e.style.opacity='1';return e.animate([{{clipPath:'inset(50% 0)'}},{{clipPath:'inset(0% 0)'}}],{{duration:d,easing:'ease-out',fill:'forwards'}});}},
    'swivel':        function(e,d){{return e.animate([{{opacity:0,transform:'perspective(800px) rotateY(-90deg)'}},{{opacity:1,transform:'perspective(800px) rotateY(0)'}}],{{duration:d,easing:'cubic-bezier(.4,0,.2,1)',fill:'forwards'}});}}
  }};
  function revealEl(el){{
    var anim=el.getAttribute('data-ppt-animation')||'fade-in';
    var dur=parseInt(el.getAttribute('data-duration')||'500',10);
    el.getAnimations().forEach(function(a){{a.cancel();}});
    el.style.removeProperty('opacity');el.style.removeProperty('transform');el.style.removeProperty('clip-path');el.style.removeProperty('visibility');
    el.classList.remove('ppt-hidden');
    var a=(ANIMS[anim]||ANIMS['fade-in'])(el,dur);
    if(a&&a.finished){{a.finished.then(function(){{
      try{{a.commitStyles();}}catch(e){{}}
      a.cancel();
      var t=el.style.transform;
      if(!t||t==='none'||/^(translateX|translateY|translateZ)\(0/.test(t)||/^scale\(1\)/.test(t)){{el.style.removeProperty('transform');}}
      el.style.removeProperty('clip-path');
    }}).catch(function(){{}});}}
  }}
  function hideEl(el){{
    el.getAnimations().forEach(function(a){{a.cancel();}});
    el.style.removeProperty('opacity');el.style.removeProperty('transform');el.style.removeProperty('clip-path');el.style.removeProperty('visibility');
    el.classList.add('ppt-hidden');
  }}
  var deck=document.getElementById('deck');
  var progress=document.getElementById('progress');
  var slides=Array.from(deck.querySelectorAll('.ppt-slide'));
  var total=slides.length,cur=0,clickStep=0,clickSequence=[],transitioning=false;
  function getSequence(slide){{var s=new Set();slide.querySelectorAll('[data-click]').forEach(function(el){{var c=parseInt(el.getAttribute('data-click'),10);if(c>0)s.add(c);}});return Array.from(s).sort(function(a,b){{return a-b;}});}}
  function updateProgress(){{progress.style.width=((cur+1)/total*100)+'%';}}
  function showSlide(n,dir){{
    if(transitioning||n===cur)return;
    var prev=slides[cur],next=slides[n];
    var tr=next.getAttribute('data-transition')||'fade';
    transitioning=true;
    next.querySelectorAll('[data-click]').forEach(function(el){{var c=parseInt(el.getAttribute('data-click'),10);if(c>0)hideEl(el);else{{el.classList.remove('ppt-hidden');el.style.removeProperty('opacity');el.style.removeProperty('visibility');revealEl(el);}}}});
    if(tr==='none'){{prev.classList.remove('active');next.classList.add('active');transitioning=false;}}
    else{{var ec=dir>=0?'tr-'+tr+'-enter':'tr-'+tr+'-back-enter',xc=dir>=0?'tr-'+tr+'-exit':'tr-'+tr+'-back-exit';
      prev.classList.add('exiting',xc);next.classList.add('active',ec);
      setTimeout(function(){{prev.classList.remove('active','exiting',xc);next.classList.remove(ec);transitioning=false;}},480);
    }}
    cur=n;clickSequence=getSequence(next);clickStep=0;updateProgress();notifyParent();
    setTimeout(function(){{initChartsInSlide(next);}},50);
  }}
  function revealNext(){{if(clickStep<clickSequence.length){{var n=clickSequence[clickStep];slides[cur].querySelectorAll('[data-click="'+n+'"]').forEach(function(el){{revealEl(el);}});clickStep++;return true;}}return false;}}
  function advance(){{if(transitioning)return;if(!revealNext()){{if(cur<total-1)showSlide(cur+1,1);}}}}
  function retreat(){{if(transitioning||cur<=0)return;showSlide(cur-1,-1);}}
  function notifyParent(){{try{{window.parent.postMessage({{type:'slideChange',index:cur,total:total}},'*');}}catch(e){{}}}}
  window.addEventListener('message',function(e){{
    if(!e.data)return;var d=e.data;
    if(d.type==='goto')showSlide(d.index,d.index>cur?1:-1);
    if(d.type==='advance')advance();
    if(d.type==='retreat')retreat();
    if(d.type==='updateImage'){{var p=d.prompt,u=d.url;deck.querySelectorAll('img.ai-gen-image').forEach(function(el){{if(el.getAttribute('data-prompt')===p)el.src=u;}});}}
  }});
  deck.addEventListener('click',advance);
  document.addEventListener('keydown',function(e){{
    if(e.key==='ArrowRight'||e.key===' '||e.key==='PageDown')advance();
    if(e.key==='ArrowLeft'||e.key==='PageUp')retreat();
    if(e.key==='Home')showSlide(0,-1);if(e.key==='End')showSlide(total-1,1);
  }});
  var chartInstances=new Map();
  function initChartsInSlide(slide){{
    if(typeof Chart==='undefined')return;
    slide.querySelectorAll('canvas[data-chart]').forEach(function(canvas){{
      var ex=chartInstances.get(canvas);if(ex){{try{{ex.destroy();}}catch(e){{}}}}
      try{{var cfg=JSON.parse(canvas.getAttribute('data-chart'));var p=canvas.parentElement;
        canvas.width=p?p.offsetWidth:(parseInt(canvas.style.width)||400);
        canvas.height=p?p.offsetHeight:(parseInt(canvas.style.height)||300);
        chartInstances.set(canvas,new Chart(canvas,cfg));}}catch(e){{console.warn('[Deckr] Chart:',e);}}
    }});
  }}
  function scaleToFit(){{var s=Math.min(window.innerWidth/960,window.innerHeight/540);deck.style.transform='scale('+s+')';}}
  window.addEventListener('resize',scaleToFit);scaleToFit();
  slides.forEach(function(slide){{slide.querySelectorAll('[data-click]').forEach(function(el){{var c=parseInt(el.getAttribute('data-click'),10);if(c>0)hideEl(el);}});}});
  if(slides[0]){{slides[0].classList.add('active');clickSequence=getSequence(slides[0]);slides[0].querySelectorAll('[data-click="0"]').forEach(function(el){{revealEl(el);}});initChartsInSlide(slides[0]);}}
  if(typeof lucide!=='undefined')lucide.createIcons();
  updateProgress();notifyParent();
}})();
</script>
</body>
</html>"##,
        title = title,
        slides_html = slides_html,
    )
}
