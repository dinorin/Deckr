use serde_json::{json, Value};

use super::{AgentMessage, DeckTheme, SlideOutline, call_llm, safe_trunc};
use crate::settings::AppSettings;

#[derive(Debug, Clone)]
pub struct SlideDesignSpec {
    pub layout_variant: String,
    pub accent_hex: String,
    pub bg_css: String,
    pub deco: String,
    pub mood: String,
}

const SYSTEM_PROMPT: &str = r##"You are a world-class presentation visual designer. Given slide outlines and a theme, output a JSON array of per-slide design specs.

Return EXACTLY this format - no markdown, no explanation, ONLY the JSON array:
[
  {
    "index": 0,
    "layout_variant": "hero-centered | hero-image-right | hero-image-left | stat-center | grid-icons | timeline | quote-large | split-diagonal | full-bleed-image | minimal-text",
    "accent_hex": "a hex color e.g. #f59e0b",
    "bg_css": "CSS background value e.g. linear-gradient(135deg,#0f0f2e 0%,#1a1a3e 100%)",
    "deco": "1-sentence desc of decorative elements e.g. large blurred circle top-right, thin rule below title",
    "mood": "bold | calm | dramatic | playful | minimal"
  }
]

Rules:
- MUST have exactly N objects (one per slide)
- Make each slide visually DISTINCT - vary colors, layouts, moods
- Title slide: dramatic, large, impactful
- Closing slide: warm, memorable
- Vary accent_hex across slides - don't repeat the same color twice in a row
- bg_css can be gradient or solid - mix both for variety
- deco should be concrete and diverse: geometric shapes, blurred orbs, diagonal lines, dot grids, icon watermarks
- Keep text readable: if bg is light, accent should be dark and vice versa"##;

pub async fn run(
    settings: &AppSettings,
    outline: &[SlideOutline],
    theme: &DeckTheme,
) -> Result<Vec<SlideDesignSpec>, String> {
    let slides_summary: Vec<String> = outline.iter().enumerate().map(|(i, s)| {
        format!("{}: [{}] \"{}\"", i, s.slide_type, s.title)
    }).collect();

    let user_msg = format!(
        "Design specs for {total} slides.\n\nTheme: {style} | bg:{bg} | primary:{pri} | secondary:{sec} | accent:{acc} | font:{font}\n\nSlide outline:\n{slides}\n\nReturn ONLY a JSON array of {total} design objects.",
        total  = outline.len(),
        style  = theme.style,
        bg     = theme.bg_color,
        pri    = theme.primary_color,
        sec    = theme.secondary_color,
        acc    = theme.accent_color,
        font   = theme.font_family,
        slides = slides_summary.join("\n"),
    );

    let history = vec![AgentMessage { role: "user".into(), content: user_msg }];
    let resp = call_llm(settings, SYSTEM_PROMPT, &history, &json!([])).await
        .map_err(|e| format!("Design agent: {}", e))?;

    let raw = resp.text.unwrap_or_default();
    parse_design_specs(&raw, outline.len())
}

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
        .map_err(|e| format!("Design agent parse error: {}. Raw: {}", e, safe_trunc(&raw, 300)))?;

    let items = arr.as_array()
        .ok_or("Design agent: expected JSON array")?;

    let mut specs: Vec<SlideDesignSpec> = items.iter().map(|v| SlideDesignSpec {
        layout_variant: v["layout_variant"].as_str().unwrap_or("hero-centered").to_string(),
        accent_hex:     v["accent_hex"].as_str().unwrap_or("#6366f1").to_string(),
        bg_css:         v["bg_css"].as_str().unwrap_or("").to_string(),
        deco:           v["deco"].as_str().unwrap_or("").to_string(),
        mood:           v["mood"].as_str().unwrap_or("bold").to_string(),
    }).collect();

    while specs.len() < expected {
        specs.push(SlideDesignSpec {
            layout_variant: "hero-centered".into(),
            accent_hex: "#6366f1".into(),
            bg_css: String::new(),
            deco: String::new(),
            mood: "bold".into(),
        });
    }
    specs.truncate(expected);

    Ok(specs)
}
