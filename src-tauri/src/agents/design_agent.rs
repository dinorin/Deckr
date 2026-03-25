use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::{AgentMessage, DeckTheme, SlideOutline, call_llm, safe_trunc};
use crate::settings::AppSettings;

// ── Public types ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecoSpec {
    pub kind: String,       // "circle" | "rect" | "line" | "stripe" | "dots"
    #[serde(default)] pub x: i32,
    #[serde(default)] pub y: i32,
    #[serde(default)] pub w: i32,
    #[serde(default)] pub h: i32,
    #[serde(default = "default_color")] pub color: String,
    #[serde(default = "default_angle")] pub angle: i32,
}

fn default_color() -> String { "rgba(255,255,255,0.1)".into() }
fn default_angle() -> i32 { 45 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideDesignSpec {
    /// One of: title-hero | title-split | bullets | bullets-icon |
    ///         content-right | content-left | two-column | quote |
    ///         icon-grid | image-full | stat-cards | closing
    pub layout: String,
    /// Full CSS background value (gradient or solid)
    pub bg_css: String,
    /// Solid hex for data-bg-color fallback
    pub bg_hex: String,
    pub accent: String,
    pub text_primary: String,
    pub text_secondary: String,
    pub font: String,
    /// "none" | "dark" | "light"
    pub overlay: String,
    pub deco: Vec<DecoSpec>,
    pub mood: String,
}

// ── System prompt ──────────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = r##"You are a world-class presentation visual designer.
Output ONLY a JSON array of per-slide design specs. No markdown, no explanation, no code fences.

## Available layouts — choose based on slide type
"title-hero"    → title slides: full-canvas hero, centered title + subtitle, dramatic
"title-split"   → title slides: title on left half, solid accent panel on right half
"bullets"       → bullets/content: title + up to 5 plain text bullets
"bullets-icon"  → bullets/content: title + up to 5 bullets each with a Lucide icon
"content-right" → content/image: text left half, image right half
"content-left"  → content/image: image left half, text right half
"two-column"    → two-column: title + two equal columns
"quote"         → quote: large italic quote centered, attribution below
"icon-grid"     → any: title + 4 icon+label cards in a row
"image-full"    → image/content: title + large image + caption
"stat-cards"    → any: title + 3 large stat numbers side by side
"closing"       → closing slides: large centered message + subtitle

## Deco element kinds (for the deco array)
"circle"  — blurred circle: x,y=center  w=h=diameter
"rect"    — solid rectangle: x,y=top-left  w,h=size
"line"    — horizontal bar: x,y=position  w=length  h=thickness(1-4)
"stripe"  — diagonal bar: x,y=top-left  w,h=bounding box  angle=degrees
"dots"    — dot grid: x,y=top-left  w,h=coverage area

## Output format — ONLY the JSON array, N objects
[
  {
    "index": 0,
    "layout": "title-hero",
    "bg_css": "linear-gradient(135deg,#0f172a 0%,#1e293b 100%)",
    "bg_hex": "#0f172a",
    "accent": "#6366f1",
    "text_primary": "#ffffff",
    "text_secondary": "#94a3b8",
    "font": "Montserrat",
    "overlay": "none",
    "deco": [
      {"kind":"circle","x":820,"y":-40,"w":320,"h":320,"color":"rgba(99,102,241,0.15)"},
      {"kind":"circle","x":-30,"y":500,"w":200,"h":200,"color":"rgba(139,92,246,0.1)"},
      {"kind":"line","x":40,"y":520,"w":880,"h":2,"color":"#6366f1"}
    ],
    "mood": "dramatic"
  }
]

## Design rules
- Exactly N objects, one per slide, matching index order
- Match layout to slide type (see table above) — vary between similar options
- Make each slide visually DISTINCT: vary bg gradients, accent colors, deco shapes
- bg_hex must be the darkest/most representative hex in bg_css
- text_primary must contrast clearly with bg_hex (dark bg → #ffffff or light color; light bg → #1e293b or dark)
- text_secondary is a muted/dimmer version of text_primary
- accent must pop against the background — high contrast
- Deco should be subtle: use low-opacity colors (rgba with 0.05–0.2 alpha)
- 2–4 deco elements per slide is ideal — avoid clutter
- overlay: use "dark" only when a full-bleed image needs text readable over it; otherwise "none""##;

// ── Run ────────────────────────────────────────────────────────────────────────

pub async fn run(
    settings: &AppSettings,
    outline: &[SlideOutline],
    theme: &DeckTheme,
    image_count: usize,
) -> Result<Vec<SlideDesignSpec>, String> {
    let slides_summary: Vec<String> = outline.iter().enumerate().map(|(i, s)| {
        format!("{}: [{}] \"{}\"", i, s.slide_type, s.title)
    }).collect();

    // Tell design agent how many images are available so it picks image layouts wisely.
    // Exclude title/closing/quote from the eligible count to reflect actual distribution.
    let eligible_for_image = outline.iter()
        .filter(|s| !matches!(s.slide_type.as_str(), "title" | "closing" | "quote"))
        .count();
    let image_hint = if image_count == 0 {
        "\nImages: 0 real images — for slides that need an image, builder will use ai-gen-image.".to_string()
    } else {
        format!(
            "\nImages: {img} real image(s) available for {elig} eligible slides (non-title/closing/quote). \
             Assign image-compatible layouts (content-right, content-left, image-full) to the first {img} eligible slides. \
             Remaining eligible slides can use bullets/bullets-icon/two-column (builder will use ai-gen-image for them if needed).",
            img = image_count, elig = eligible_for_image
        )
    };

    let user_msg = format!(
        "Design {total} slides.\n\nTheme: {style} | font:{font}\nBase colors: bg={bg} primary={pri} accent={acc}{image_hint}\n\nSlides:\n{slides}\n\nReturn ONLY a JSON array of {total} objects.",
        total  = outline.len(),
        style  = theme.style,
        font   = theme.font_family,
        bg     = theme.bg_color,
        pri    = theme.primary_color,
        acc    = theme.accent_color,
        image_hint = image_hint,
        slides = slides_summary.join("\n"),
    );

    let history = vec![AgentMessage { role: "user".into(), content: user_msg }];
    let resp = call_llm(settings, SYSTEM_PROMPT, &history, &json!([])).await
        .map_err(|e| format!("Design agent: {}", e))?;

    let raw = resp.text.unwrap_or_default();
    parse_design_specs(&raw, outline.len())
}

// ── Parse ──────────────────────────────────────────────────────────────────────

fn parse_design_specs(raw: &str, expected: usize) -> Result<Vec<SlideDesignSpec>, String> {
    let s = raw.trim();
    let s = if s.starts_with("```") {
        let after = s.find('\n').map(|i| &s[i+1..]).unwrap_or(s);
        after.trim_end_matches("```").trim()
    } else { s };

    let cleaned = if let (Some(start), Some(end)) = (s.find('['), s.rfind(']')) {
        &s[start..=end]
    } else { s };

    let arr: Value = serde_json::from_str(cleaned)
        .map_err(|e| format!("Design agent parse: {}. Raw: {}", e, safe_trunc(raw, 300)))?;

    let items = arr.as_array().ok_or("Design agent: expected JSON array")?;

    let parse_deco = |v: &Value| -> Vec<DecoSpec> {
        v.as_array().map(|arr| {
            arr.iter().filter_map(|d| serde_json::from_value(d.clone()).ok()).collect()
        }).unwrap_or_default()
    };

    let mut specs: Vec<SlideDesignSpec> = items.iter().map(|v| SlideDesignSpec {
        layout:         v["layout"].as_str().unwrap_or("bullets").to_string(),
        bg_css:         v["bg_css"].as_str().unwrap_or("#0f0f1f").to_string(),
        bg_hex:         v["bg_hex"].as_str().unwrap_or("#0f0f1f").to_string(),
        accent:         v["accent"].as_str().unwrap_or("#6366f1").to_string(),
        text_primary:   v["text_primary"].as_str().unwrap_or("#ffffff").to_string(),
        text_secondary: v["text_secondary"].as_str().unwrap_or("#94a3b8").to_string(),
        font:           v["font"].as_str().unwrap_or("Inter").to_string(),
        overlay:        v["overlay"].as_str().unwrap_or("none").to_string(),
        deco:           parse_deco(&v["deco"]),
        mood:           v["mood"].as_str().unwrap_or("bold").to_string(),
    }).collect();

    while specs.len() < expected {
        specs.push(SlideDesignSpec {
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
        });
    }
    specs.truncate(expected);
    Ok(specs)
}
