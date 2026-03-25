use futures::StreamExt;
use serde::Serialize;
use serde_json::json;
use tauri::Emitter;

use super::{AgentContext, AgentMessage, DeckTheme, GeneratedDeck, GeneratedSlide, SlideOutline, call_llm};
use super::design_agent::SlideDesignSpec;
use crate::settings::AppSettings;
use crate::tools::ImageResult;

#[derive(Serialize, Clone)]
pub struct SlideReadyPayload {
    pub index: usize,
    pub id: String,
    pub slide_type: String,
    pub html: String,
}

// ── Merged: Layout Planner + Slide Builder ────────────────────────────────────

const SLIDE_SYSTEM_PROMPT: &str = r##"You are a world-class slide designer. Output ONLY raw HTML for one 960×540 slide. No markdown, no explanation, no code fences.

## Canvas: 960 × 540 px
Safe zone: x ≥ 40, y ≥ 30, x+w ≤ 920, y+h ≤ 510.
STRICT non-overlap: element B must have top_B ≥ top_A + height_A + 20.
Side-by-side columns: left x+w ≤ 440, right x ≥ 520.

## BEFORE writing HTML — compute a position table:
  el | top | height | bottom(top+h)
  ---|-----|--------|-------------
  (fill this mentally, verify bottom_N + 20 ≤ top_N+1 for every pair)

## Layout Templates — use EXACT pixel values below

### title
  bg       x=0   y=0   w=960 h=540
  title    x=40  y=175 w=880 h=80  (font-size:56px) click=0 anim=fly-in-bottom
  subtitle x=40  y=275 w=880 h=48  (font-size:28px) click=1 anim=fade-in
  → bottom of subtitle = 323 ✓ (fits in 510)

### content (text-left / image-right)
  title  x=40  y=32  w=540 h=56  (font-size:40px) click=0 anim=wipe-left
  body_1 x=40  y=108 w=520 h=48  (font-size:20px) click=1 anim=float-in
  body_2 x=40  y=166 w=520 h=48  click=2 anim=float-in
  body_3 x=40  y=224 w=520 h=48  click=3 anim=float-in
  body_4 x=40  y=282 w=520 h=48  click=4 anim=float-in
  image  x=590 y=32  w=330 h=400 click=0 anim=fade-in
  → gaps: 108-88=20 ✓  166-156=10 ✗ → use exact values above

### bullets (5 max)
  title    x=40 y=32  w=880 h=56  (font-size:40px) click=0 anim=wipe-left
  bullet_1 x=56 y=108 w=860 h=44  (font-size:20px) click=1 anim=fly-in-left
  bullet_2 x=56 y=162 w=860 h=44  click=2 anim=fly-in-left
  bullet_3 x=56 y=216 w=860 h=44  click=3 anim=fly-in-left
  bullet_4 x=56 y=270 w=860 h=44  click=4 anim=fly-in-left
  bullet_5 x=56 y=324 w=860 h=44  click=5 anim=fly-in-left

### icon-grid (4 icons with labels)
  title  x=40  y=32  w=880 h=56  click=0 anim=fade-in
  icon_1 x=80  y=140 w=180 h=180 click=1 anim=zoom-in   (deco layer with Lucide icon centered)
  icon_2 x=300 y=140 w=180 h=180 click=1 anim=zoom-in
  icon_3 x=520 y=140 w=180 h=180 click=1 anim=zoom-in
  icon_4 x=740 y=140 w=180 h=180 click=1 anim=zoom-in
  label_1 x=80  y=328 w=180 h=40 click=1 anim=fade-in
  label_2 x=300 y=328 w=180 h=40 click=1 anim=fade-in
  label_3 x=520 y=328 w=180 h=40 click=1 anim=fade-in
  label_4 x=740 y=328 w=180 h=40 click=1 anim=fade-in

### two-column
  title    x=40  y=32  w=880 h=56  click=0 anim=fade-in
  lhead    x=40  y=108 w=400 h=40  (font-size:22px bold) click=1 anim=fly-in-left
  lbody_1  x=40  y=158 w=400 h=44  click=2 anim=fly-in-left
  lbody_2  x=40  y=212 w=400 h=44  click=3 anim=fly-in-left
  lbody_3  x=40  y=266 w=400 h=44  click=4 anim=fly-in-left
  rhead    x=520 y=108 w=400 h=40  click=1 anim=fly-in-right
  rbody_1  x=520 y=158 w=400 h=44  click=2 anim=fly-in-right
  rbody_2  x=520 y=212 w=400 h=44  click=3 anim=fly-in-right
  rbody_3  x=520 y=266 w=400 h=44  click=4 anim=fly-in-right

### quote
  bar    x=40 y=130 w=6   h=160
  quote  x=74 y=140 w=822 h=160 (font-size:30px italic center) click=0 anim=fade-in
  author x=74 y=318 w=822 h=40  (font-size:18px right)         click=1 anim=float-in

### image-full
  title   x=40 y=32  w=880 h=56  click=0 anim=fly-in-top
  image   x=40 y=108 w=880 h=360 click=0 anim=fade-in
  caption x=40 y=480 w=880 h=26  (font-size:14px center) click=0 anim=appear

### closing
  title    x=40 y=185 w=880 h=80  (font-size:56px bold center) click=0 anim=zoom-in
  subtitle x=40 y=285 w=880 h=48  (font-size:28px center)      click=1 anim=fade-in

## Click / Reveal
- click=0: appears on slide entry (titles, images, structural)
- click=1,2,3…: revealed on Nth click
- Max 6 clicks per slide.

## Slide wrapper
<div class="ppt-slide" data-slide-index="N" data-bg-color="#hex" data-transition="fade|push|wipe|none"
     style="position:relative;width:960px;height:540px;overflow:hidden;box-sizing:border-box;font-family:'FONT',sans-serif;">

## Layer system — every direct child MUST have id="layer-TYPE-N"
Types: bg | overlay | deco | image | chart | text
Render order: bg → overlay → deco → image → chart → text

## Text layer — CRITICAL overflow rules
EVERY layer-text-* wrapper MUST include overflow:hidden to prevent text bleeding into other elements:
  style="position:absolute;left:Xpx;top:Ypx;width:Wpx;height:Hpx;overflow:hidden;"

Single-line elements (titles, bullets, labels) MUST also add: white-space:nowrap;text-overflow:ellipsis;
Multi-line body text: use overflow:hidden only (allow wrapping).

NO other styles on wrapper: no bg-*, rounded-*, border-*, shadow-*, backdrop-blur-*, padding.

Inner tags use Tailwind typography only:
  text-sm text-base text-lg text-xl text-2xl text-3xl text-4xl text-5xl text-6xl
  font-bold font-black font-semibold · leading-tight leading-snug · tracking-wide uppercase italic
  text-white text-[#hex] · text-center text-left text-right

Font size guide: 56px→text-6xl  48px→text-5xl  40px→text-4xl  30px→text-3xl  24px→text-2xl  20px→text-xl  18px→text-lg  14px→text-sm

ALWAYS include data-ppt-* on every inner text tag.

## Text layer pattern
<div id="layer-text-N" class="ppt-element ppt-hidden ppt-ANIM"
     data-click="N" data-duration="500ms" data-ppt-animation="ANIM"
     style="position:absolute;left:Xpx;top:Ypx;width:Wpx;height:Hpx;overflow:hidden;white-space:nowrap;text-overflow:ellipsis;">
  <h2 class="text-4xl font-bold text-white leading-tight"
      data-ppt-font-size="40" data-ppt-bold="true" data-ppt-color="#fff" data-ppt-align="left" data-ppt-font="Inter">Title text</h2>
</div>

## Icons (Lucide) — available everywhere
Use Lucide icons freely in any layer: deco, text, or standalone icon blocks.
Syntax: <i data-lucide="icon-name" style="width:Npx;height:Npx;color:#hex;flex-shrink:0;"></i>
Size: width/height in px (NOT font-size). Color: set via color:#hex on the element.

### Bullet with icon (inside layer-text-*):
<div style="display:flex;align-items:center;gap:12px;height:100%;">
  <i data-lucide="rocket" style="width:20px;height:20px;color:#6366f1;flex-shrink:0;"></i>
  <span class="text-lg text-white leading-tight" data-ppt-font-size="18" data-ppt-bold="false" data-ppt-color="#fff" data-ppt-align="left" data-ppt-font="Inter">Bullet text here</span>
</div>

### Icon block (inside layer-deco-* or layer-text-*):
<div style="display:flex;flex-direction:column;align-items:center;justify-content:center;height:100%;gap:12px;">
  <i data-lucide="trending-up" style="width:56px;height:56px;color:#f59e0b;"></i>
  <span class="text-base font-semibold text-white text-center" data-ppt-font-size="16" data-ppt-bold="true" data-ppt-color="#fff" data-ppt-align="center" data-ppt-font="Inter">Growth</span>
</div>

Useful icons: rocket trending-up shield zap star globe users code lightbulb trophy settings lock cloud smartphone check arrow-right circle-check flame leaf coins search handshake brain cpu database layers target award badge medal gift heart mail bell clock calendar bar-chart pie-chart activity

## Animations
ppt-appear | ppt-fade-in | ppt-fly-in-bottom | ppt-fly-in-top | ppt-fly-in-left | ppt-fly-in-right | ppt-zoom-in | ppt-bounce-in | ppt-float-in | ppt-wipe-left | ppt-split | ppt-swivel
data-click="0" = entry · data-click="N" = revealed on Nth click

## Images
≥1 image OR icon per slide. Pick ONE — never mix src URL + data-prompt:
- URL available → <img id="layer-image-N" src="URL" data-ppt-animation="ANIM" data-click="N" style="position:absolute;left:Xpx;top:Ypx;width:Wpx;height:Hpx;object-fit:cover;">
- No URL        → <img id="layer-image-N" class="ai-gen-image" src="" data-prompt="short English description" data-ppt-animation="ANIM" data-click="N" style="position:absolute;...">

## Charts (Chart.js)
<div id="layer-chart-N" class="ppt-element ppt-hidden ppt-ANIM"
     data-click="N" data-duration="500ms" data-ppt-animation="ANIM"
     style="position:absolute;left:Xpx;top:Ypx;width:Wpx;height:Hpx;overflow:hidden;">
  <canvas style="width:100%;height:100%;display:block;"
    data-chart='{"type":"bar","data":{"labels":["Q1","Q2","Q3","Q4"],"datasets":[{"label":"Revenue","data":[42,68,55,81],"backgroundColor":["#6366f1","#8b5cf6","#a78bfa","#c4b5fd"],"borderRadius":4}]},"options":{"responsive":false,"animation":false,"plugins":{"legend":{"labels":{"color":"#fff","font":{"size":11}}}},"scales":{"x":{"ticks":{"color":"#ccc"},"grid":{"color":"rgba(255,255,255,.1)"}},"y":{"ticks":{"color":"#ccc"},"grid":{"color":"rgba(255,255,255,.1)"}}}}}'></canvas>
</div>
max 7 data points · "animation":false · "responsive":false · types: bar/line/pie/doughnut/radar

## Hard rules
- NO transform for layout (breaks animations) — use flex/grid/absolute
- No vw/vh — absolute px only
- overflow:hidden on ALL text wrappers — NO EXCEPTIONS
- Stay within safe zone: x ≥ 40, y ≥ 30, x+w ≤ 920, y+h ≤ 510
- Make each slide visually unique and distinct from others"##;

// ── Theme derivation (no LLM call needed) ─────────────────────────────────────

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
            // default: modern dark
            ("#6366f1", "#8b5cf6", "#0f0f1f", "#ffffff", "#f59e0b", "Montserrat", "modern")
        };

    DeckTheme {
        primary_color: primary.to_string(),
        secondary_color: secondary.to_string(),
        bg_color: bg.to_string(),
        text_color: text.to_string(),
        accent_color: accent.to_string(),
        font_family: font.to_string(),
        style: style.to_string(),
    }
}

// ── Run ───────────────────────────────────────────────────────────────────────

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

    // Run up to 3 slide LLM calls concurrently — same token cost, ~3× faster wall-clock.
    // Each task gets cloned copies so it can be sent across await points.
    let settings_arc = std::sync::Arc::new(settings.clone());
    let theme_arc = std::sync::Arc::new(theme.clone());
    let images_arc = std::sync::Arc::new(images.to_vec());
    let specs_arc = std::sync::Arc::new(design_specs.to_vec());

    let outline_cloned: Vec<SlideOutline> = outline.to_vec();
    let language = ctx.language.clone();

    let tasks = outline_cloned.into_iter().enumerate().map(|(i, outline_slide)| {
        let s    = settings_arc.clone();
        let t    = theme_arc.clone();
        let imgs = images_arc.clone();
        let lang = language.clone();
        let spec = specs_arc.get(i).cloned();
        async move {
            let slide = generate_single_slide(&s, i, total, &outline_slide, &t, &lang, &imgs, spec.as_ref()).await?;
            Ok::<(usize, GeneratedSlide), String>((i, slide))
        }
    });

    let mut results: Vec<(usize, GeneratedSlide)> = futures::stream::iter(tasks)
        .buffer_unordered(3)
        .collect::<Vec<Result<(usize, GeneratedSlide), String>>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    // Sort by index so slides are in the correct order
    results.sort_by_key(|(i, _)| *i);

    // Emit slide-ready events in order and collect slides
    let mut slides: Vec<GeneratedSlide> = Vec::with_capacity(total);
    for (i, slide) in results {
        let _ = app.emit("slide-ready", SlideReadyPayload {
            index: i,
            id: slide.id.clone(),
            slide_type: slide.slide_type.clone(),
            html: slide.html.clone(),
        });
        slides.push(slide);
    }

    let title = outline.first()
        .map(|s| s.title.clone())
        .unwrap_or_else(|| ctx.topic.clone());

    let master_html = build_master_html(&title, &slides);

    let theme_json = json!({
        "primaryColor": theme.primary_color,
        "secondaryColor": theme.secondary_color,
        "backgroundColor": theme.bg_color,
        "textColor": theme.text_color,
        "fontFamily": theme.font_family,
        "style": theme.style,
    });

    Ok(GeneratedDeck {
        title,
        theme: theme_json,
        slides,
        master_html,
        coach_message: String::new(),
    })
}

async fn generate_single_slide(
    settings: &AppSettings,
    index: usize,
    total: usize,
    outline_slide: &SlideOutline,
    theme: &DeckTheme,
    language: &str,
    images: &[ImageResult],
    design: Option<&SlideDesignSpec>,
) -> Result<GeneratedSlide, String> {
    let empty_tools = json!([]);

    let bullets = if outline_slide.bullets.is_empty() {
        String::new()
    } else {
        format!("\nKey points: {}", outline_slide.bullets.join(" | "))
    };

    let image_block = if images.is_empty() {
        String::new()
    } else {
        let len = images.len();
        let start = (index * 3) % len;
        let lines: Vec<String> = (0..len.min(4))
            .map(|i| {
                let img = &images[(start + i) % len];
                format!("  - \"{desc}\" {w}×{h} avg:{avg} text:{txt} | {url}",
                    desc = img.description.replace('"', "'"),
                    w = img.width, h = img.height,
                    avg = img.avg_color, txt = img.text_color,
                    url = img.url)
            })
            .collect();
        format!("\nAvailable images (pick different ones per slide, place in <img> tags):\n{}", lines.join("\n"))
    };

    // Design spec block — inject visual direction from design agent
    let design_block = if let Some(d) = design {
        format!(
            "\nDesign spec (FOLLOW THESE EXACTLY):\n  Layout: {layout}\n  Accent: {accent}\n  Background: {bg}\n  Mood: {mood}\n  Decorative elements: {deco}",
            layout = d.layout_variant,
            accent = d.accent_hex,
            bg     = if d.bg_css.is_empty() { theme.bg_color.as_str() } else { d.bg_css.as_str() },
            mood   = d.mood,
            deco   = d.deco,
        )
    } else {
        String::new()
    };

    let user_msg = format!(
        r#"Slide {idx}/{total} — type: {stype}, title: "{title}"{bullets}
Language: {lang}

Theme: {style} | bg:{bg} | primary:{pri} | accent:{acc} | font:{font}
Transition: {tr}{design}{images}
Output ONLY raw HTML starting with <div class="ppt-slide""#,
        idx     = index + 1,
        total   = total,
        stype   = outline_slide.slide_type,
        title   = outline_slide.title,
        bullets = bullets,
        lang    = language,
        style   = theme.style,
        bg      = theme.bg_color,
        pri     = theme.primary_color,
        acc     = theme.accent_color,
        font    = theme.font_family,
        tr      = outline_slide.transition,
        design  = design_block,
        images  = image_block,
    );

    let history = vec![AgentMessage { role: "user".to_string(), content: user_msg }];
    let resp = call_llm(settings, SLIDE_SYSTEM_PROMPT, &history, &empty_tools).await
        .map_err(|e| e.to_string())?;

    let raw = resp.text.unwrap_or_default();
    let raw = raw.trim();

    let mut html = if raw.contains("ppt-slide") {
        raw.to_string()
    } else {
        let extracted = extract_html_div(raw);
        if extracted.contains("ppt-slide") {
            extracted
        } else {
            build_fallback_slide(outline_slide, theme, index)
        }
    };

    // Stamp the slide transition so the PPTX exporter can inject <p:transition>
    let tr = &outline_slide.transition;
    if !tr.is_empty() && tr != "none" {
        html = html.replacen(
            "class=\"ppt-slide\"",
            &format!("class=\"ppt-slide\" data-transition=\"{}\"", tr),
            1,
        );
    }

    let slide_id = format!("s{}", index + 1);
    Ok(GeneratedSlide {
        id: slide_id,
        slide_type: outline_slide.slide_type.clone(),
        html,
    })
}

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

fn build_fallback_slide(outline: &SlideOutline, theme: &DeckTheme, index: usize) -> String {
    let title = html_escape(&outline.title);
    let bullets_html = outline.bullets.iter().enumerate().map(|(i, b)| {
        format!(
            r#"<div id="layer-text-{n}" class="ppt-element ppt-hidden ppt-fly-in-left" data-click="{click}" data-duration="500" data-ppt-animation="fly-in-left"
     style="position:absolute;left:64px;top:{y}px;width:852px;height:40px;">
  <p data-ppt-font-size="19" data-ppt-bold="false" data-ppt-color="{color}" data-ppt-align="left" data-ppt-font="{font}"
     class="text-lg text-white">• {text}</p>
</div>"#,
            n = i + 2,
            click = i + 1,
            y = 114 + i * 52,
            color = theme.text_color,
            font = theme.font_family,
            text = html_escape(b),
        )
    }).collect::<Vec<_>>().join("\n");

    format!(
        r#"<div class="ppt-slide" data-slide-index="{idx}" data-bg-color="{bg}" data-transition="{tr}"
     style="position:relative;width:960px;height:540px;overflow:hidden;font-family:'{font}',sans-serif;">
  <div id="layer-bg-1" style="position:absolute;inset:0;background:{bg};"></div>
  <div id="layer-deco-1" style="position:absolute;bottom:0;left:0;right:0;height:3px;background:{accent};"></div>
  <div id="layer-text-1" class="ppt-element ppt-hidden ppt-wipe-left" data-click="0" data-duration="500" data-ppt-animation="wipe-left"
       style="position:absolute;left:40px;top:32px;width:880px;height:62px;">
    <h2 data-ppt-font-size="40" data-ppt-bold="true" data-ppt-color="{text}" data-ppt-align="left" data-ppt-font="{font}"
        class="text-4xl font-bold leading-tight" style="color:{text};">{title}</h2>
  </div>
{bullets}
</div>"#,
        idx = index,
        bg = theme.bg_color,
        tr = outline.transition,
        font = theme.font_family,
        accent = theme.accent_color,
        text = theme.text_color,
        title = title,
        bullets = bullets_html,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ── Master HTML ────────────────────────────────────────────────────────────────

pub fn build_master_html(title: &str, slides: &[GeneratedSlide]) -> String {
    let slides_html: String = slides.iter().enumerate().map(|(i, s)| {
        let html = if s.html.contains("data-slide-index") {
            s.html.clone()
        } else {
            s.html.replacen("<div", &format!("<div data-slide-index=\"{}\"", i), 1)
        };
        html
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

.ppt-slide{{position:absolute;inset:0;width:960px;height:540px;overflow:hidden;display:none;color:#fff;}}
.ppt-slide.active{{display:block;z-index:10;}}
.ppt-slide.exiting{{display:block;z-index:5;}}

.ppt-element{{display:block;position:relative;box-sizing:border-box;}}

/* JS animations: elements start hidden via this class; JS removes it and calls element.animate() */
.ppt-element.ppt-hidden{{opacity:0!important;visibility:hidden!important;pointer-events:none;}}

/* Slide transitions */
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
  /* ── Web Animations API — entrance animations ─────────────────────────── */
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
    var anim = el.getAttribute('data-ppt-animation') || 'fade-in';
    var dur  = parseInt(el.getAttribute('data-duration') || '500', 10);
    el.getAnimations().forEach(function(a){{a.cancel();}});
    el.style.opacity = '';
    el.style.transform = '';
    el.style.clipPath = '';
    el.classList.remove('ppt-hidden');
    el.style.visibility = 'visible';
    var a = (ANIMS[anim] || ANIMS['fade-in'])(el, dur);
    if (a && a.finished) {{
      a.finished.then(function(){{
        try {{ a.commitStyles(); }} catch(e) {{}}
        a.cancel();
      }}).catch(function(){{}});
    }}
  }}

  function hideEl(el){{
    el.getAnimations().forEach(function(a){{a.cancel();}});
    el.style.opacity = '0';
    el.style.visibility = 'hidden';
    el.style.transform = '';
    el.style.clipPath = '';
    el.classList.add('ppt-hidden');
  }}

  /* ── Deck state ─────────────────────────────────────────────────────────── */
  var deck = document.getElementById('deck');
  var progress = document.getElementById('progress');
  var slides = Array.from(deck.querySelectorAll('.ppt-slide'));
  var total = slides.length;
  var cur = 0;
  var clickStep = 0;
  var clickSequence = [];
  var transitioning = false;

  function getSequence(slide) {{
    var s = new Set();
    slide.querySelectorAll('[data-click]').forEach(function(el){{
      var c = parseInt(el.getAttribute('data-click'), 10);
      if(c > 0) s.add(c);
    }});
    return Array.from(s).sort(function(a,b){{return a-b;}});
  }}

  function updateProgress(){{
    progress.style.width = ((cur + 1) / total * 100) + '%';
  }}

  function showSlide(n, dir){{
    if(transitioning || n === cur) return;
    var prev = slides[cur];
    var next = slides[n];
    var tr = next.getAttribute('data-transition') || 'fade';
    transitioning = true;

    // Reset animated elements on incoming slide
    next.querySelectorAll('[data-click]').forEach(function(el){{
      var c = parseInt(el.getAttribute('data-click'),10);
      if(c > 0) {{
         hideEl(el);
      }} else if (c === 0) {{
         el.classList.remove('ppt-hidden');
         el.style.opacity = '';
         el.style.visibility = '';
         revealEl(el);
      }}
    }});

    if(tr === 'none'){{
      prev.classList.remove('active');
      next.classList.add('active');
      transitioning = false;
    }} else {{
      var enterClass = dir >= 0 ? 'tr-'+tr+'-enter' : 'tr-'+tr+'-back-enter';
      var exitClass  = dir >= 0 ? 'tr-'+tr+'-exit'  : 'tr-'+tr+'-back-exit';
      prev.classList.add('exiting', exitClass);
      next.classList.add('active', enterClass);
      setTimeout(function(){{
        prev.classList.remove('active','exiting',exitClass);
        next.classList.remove(enterClass);
        transitioning = false;
      }}, 480);
    }}

    cur = n;
    clickSequence = getSequence(next);
    clickStep = 0;
    updateProgress(); notifyParent();
    setTimeout(function(){{ initChartsInSlide(next); }}, 50);
  }}

  function revealNext(){{
    if (clickStep < clickSequence.length) {{
      var nextNum = clickSequence[clickStep];
      var targets = slides[cur].querySelectorAll('[data-click="'+nextNum+'"]');
      targets.forEach(function(el){{ revealEl(el); }});
      clickStep++;
      return true;
    }}
    return false;
  }}

  function advance(){{
    if(transitioning) return;
    if(!revealNext()) {{ if(cur < total-1) showSlide(cur+1, 1); }}
  }}
  function retreat(){{
    if(transitioning || cur <= 0) return;
    showSlide(cur-1, -1);
  }}
  function notifyParent(){{
    try{{ window.parent.postMessage({{type:'slideChange',index:cur,total:total}},'*'); }}catch(e){{}}
  }}

  /* ── Message bus ──────────────────────────────────────────────────────── */
  window.addEventListener('message', function(e){{
    if(!e.data) return;
    var d = e.data;
    if(d.type==='goto')       showSlide(d.index, d.index>cur ? 1 : -1);
    if(d.type==='advance')    advance();
    if(d.type==='retreat')    retreat();
    if(d.type==='updateImage'){{
      var p = d.prompt, u = d.url;
      deck.querySelectorAll('img.ai-gen-image').forEach(function(el){{
        if(el.getAttribute('data-prompt')===p) el.src = u;
      }});
      deck.querySelectorAll('div.ai-gen-image').forEach(function(el){{
        if(el.getAttribute('data-prompt')===p) el.style.backgroundImage='url('+u+')';
      }});
    }}
  }});

  deck.addEventListener('click', advance);
  document.addEventListener('keydown', function(e){{
    if(e.key==='ArrowRight'||e.key===' '||e.key==='PageDown') advance();
    if(e.key==='ArrowLeft' ||e.key==='PageUp') retreat();
    if(e.key==='Home') showSlide(0,-1);
    if(e.key==='End')  showSlide(total-1,1);
  }});

  /* ── Chart.js ─────────────────────────────────────────────────────────── */
  var chartInstances = new Map();
  function initChartsInSlide(slide){{
    if(typeof Chart === 'undefined') return;
    slide.querySelectorAll('canvas[data-chart]').forEach(function(canvas){{
      var existing = chartInstances.get(canvas);
      if(existing){{ try{{existing.destroy();}}catch(e){{}} }}
      try{{
        var cfg = JSON.parse(canvas.getAttribute('data-chart'));
        var parent = canvas.parentElement;
        canvas.width  = parent ? parent.offsetWidth  : (parseInt(canvas.style.width)  || 400);
        canvas.height = parent ? parent.offsetHeight : (parseInt(canvas.style.height) || 300);
        chartInstances.set(canvas, new Chart(canvas, cfg));
      }}catch(e){{ console.warn('[Deckr] Chart error:', e); }}
    }});
  }}

  function scaleToFit(){{
    var s = Math.min(window.innerWidth/960, window.innerHeight/540);
    deck.style.transform = 'scale('+s+')';
  }}
  window.addEventListener('resize', scaleToFit);
  scaleToFit();

  /* ── Init ─────────────────────────────────────────────────────────────── */
  slides.forEach(function(slide){{
    slide.querySelectorAll('[data-click]').forEach(function(el){{
      var c = parseInt(el.getAttribute('data-click'),10);
      if(c > 0) hideEl(el);
    }});
  }});
  if(slides[0]) {{
     slides[0].classList.add('active');
     clickSequence = getSequence(slides[0]);
     slides[0].querySelectorAll('[data-click="0"]').forEach(function(el) {{ revealEl(el); }});
     initChartsInSlide(slides[0]);
  }}
  if(typeof lucide !== 'undefined') lucide.createIcons();
  updateProgress(); notifyParent();
}})();
</script>
</body>
</html>"##,
        title = title,
        slides_html = slides_html,
    )
}
