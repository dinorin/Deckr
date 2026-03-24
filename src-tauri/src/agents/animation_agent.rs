use serde_json::{json, Value};

use super::{AgentContext, AgentMessage, DeckTheme, ElementAnimationPlan, SlideAnimationPlan, SlideOutline, call_llm};
use crate::settings::AppSettings;

const SYSTEM_PROMPT: &str = r##"You are the Animation & Design Agent for Deckr. Output ONLY valid JSON — no markdown, no explanation.

Given a slide outline, return a JSON object with this exact structure:
{
  "theme": {
    "primary_color": "#hex",
    "secondary_color": "#hex",
    "bg_color": "#hex",
    "text_color": "#hex",
    "accent_color": "#hex",
    "font_family": "FontName",
    "style": "modern|minimal|bold|corporate|creative"
  },
  "slides": [
    {
      "index": 0,
      "bg_color": "#hex",
      "transition": "fade|push|wipe|none",
      "elements": [
        {
          "element_type": "title|subtitle|heading|body|bullet|accent|decoration",
          "content": "text content",
          "animation": "appear|fade-in|fly-in-bottom|fly-in-top|fly-in-left|fly-in-right|zoom-in|bounce-in|float-in|wipe-left|split|swivel",
          "click_order": 1,
          "duration_ms": 600
        }
      ]
    }
  ]
}

## Design Rules
- Dark themes (dark bg + vibrant accent) feel premium
- click_order=0 → always visible; click_order>0 → revealed on that Nth click
- Title slides: title fly-in-bottom click=1, subtitle fade-in click=2
- Bullets: each bullet its own click_order (1,2,3...)
- Two-column: left fly-in-left click=1, right fly-in-right click=2
- Max 5 text elements per slide — fewer is cleaner
- Vary transitions and animations across slides"##;

pub async fn run(
    settings: &AppSettings,
    ctx: &AgentContext,
    outline: &[SlideOutline],
) -> Result<(DeckTheme, Vec<SlideAnimationPlan>), String> {
    // Use no tools — ask for raw JSON in text (works with all providers)
    let empty_tools = json!([]);

    let outline_text: String = outline.iter().map(|s| {
        format!(
            "Slide {} [{}]: {}\nBullets: {}",
            s.index + 1, s.slide_type, s.title,
            if s.bullets.is_empty() { "(none)".to_string() } else { s.bullets.join(" | ") }
        )
    }).collect::<Vec<_>>().join("\n\n");

    let user_msg = format!(
        "Style: {}\nLanguage: {}\n\nOutline:\n{}\n\nRespond with ONLY the JSON object.",
        if ctx.style_hint.is_empty() { "modern dark" } else { &ctx.style_hint },
        ctx.language,
        outline_text
    );

    let history = vec![AgentMessage { role: "user".to_string(), content: user_msg }];
    let resp = call_llm(settings, SYSTEM_PROMPT, &history, &empty_tools).await?;

    // Parse from text response (JSON-in-text mode)
    let raw = match &resp.text {
        Some(t) => t.clone(),
        None => {
            // Fallback: try function call args
            if let Some(fc) = resp.function_calls.first() {
                fc.args.to_string()
            } else {
                return Err("Animation agent returned no content".to_string());
            }
        }
    };

    // Extract JSON from possible markdown code block
    let json_str = extract_json(&raw);
    let parsed: Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Animation agent JSON parse error: {}. Raw: {}", e, &raw[..raw.len().min(400)]))?;

    parse_animation_plan(&parsed)
}

fn extract_json(text: &str) -> String {
    // Strip markdown code fences if present
    let text = text.trim();
    if let Some(start) = text.find("```json") {
        let after = &text[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    // Find first { to last }
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        if start <= end {
            return text[start..=end].to_string();
        }
    }
    text.to_string()
}

fn parse_animation_plan(parsed: &Value) -> Result<(DeckTheme, Vec<SlideAnimationPlan>), String> {
    let theme_val = &parsed["theme"];
    if theme_val.is_null() {
        return Err("Animation agent JSON missing 'theme' field".to_string());
    }

    let theme = DeckTheme {
        primary_color: theme_val["primary_color"].as_str().unwrap_or("#6366f1").to_string(),
        secondary_color: theme_val["secondary_color"].as_str().unwrap_or("#8b5cf6").to_string(),
        bg_color: theme_val["bg_color"].as_str().unwrap_or("#0f0f1f").to_string(),
        text_color: theme_val["text_color"].as_str().unwrap_or("#ffffff").to_string(),
        accent_color: theme_val["accent_color"].as_str().unwrap_or("#f59e0b").to_string(),
        font_family: theme_val["font_family"].as_str().unwrap_or("Montserrat").to_string(),
        style: theme_val["style"].as_str().unwrap_or("modern").to_string(),
    };

    let mut slides = Vec::new();
    if let Some(slides_arr) = parsed["slides"].as_array() {
        for (i, s) in slides_arr.iter().enumerate() {
            let mut elements = Vec::new();
            if let Some(elems) = s["elements"].as_array() {
                for el in elems {
                    elements.push(ElementAnimationPlan {
                        element_type: el["element_type"].as_str().unwrap_or("body").to_string(),
                        content: el["content"].as_str().unwrap_or("").to_string(),
                        animation: el["animation"].as_str().unwrap_or("fade-in").to_string(),
                        click_order: el["click_order"].as_u64().unwrap_or(1) as u32,
                        duration_ms: el["duration_ms"].as_u64().unwrap_or(500) as u32,
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                        font_size: Default::default(),
                        bold: false,
                        italic: false,
                        color: String::new(),
                        align: String::new(),
                        font_family: String::new(),
                    });
                }
            }
            slides.push(SlideAnimationPlan {
                index: s["index"].as_u64().unwrap_or(i as u64) as usize,
                bg_color: s["bg_color"].as_str().unwrap_or("#0f0f1f").to_string(),
                transition: s["transition"].as_str().unwrap_or("fade").to_string(),
                elements,
            });
        }
    }

    if slides.is_empty() {
        return Err("Animation agent returned no slides".to_string());
    }

    // CRITICAL: Sort slides by index to ensure they are in the correct order,
    // in case the LLM outputs them out of sequence.
    slides.sort_by_key(|s| s.index);

    Ok((theme, slides))
}
