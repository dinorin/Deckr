use serde_json::{json, Value};

use super::{AgentContext, AgentMessage, SlideOutline, call_llm, safe_trunc};
use crate::settings::AppSettings;

const SYSTEM_PROMPT: &str = r#"You are the Content Agent for Deckr. Output ONLY valid JSON — no markdown, no explanation.

Return a JSON array of exactly the requested number of slide outlines:
[
  {
    "index": 0,
    "type": "title|content|bullets|two-column|quote|closing|image",
    "title": "Slide title",
    "bullets": ["Point 1 (max 10 words)", "Point 2"],
    "notes": "Speaker notes (1–2 sentences)",
    "transition": "fade|push|wipe|none"
  }
]

## Rules
- EXACTLY match the requested slide count — no more, no less.
- First slide: always "title" type
- Last slide: always "closing" type
- Bullets: max 5 per slide, STRICT max 10 words each — be extremely concise. Viewers do not read paragraphs.
- Distribute research content evenly across slides — don't front-load information
- Use the provided research to add facts, numbers, examples — not generic filler
- All text in the requested language
- Vary slide types for visual interest (mix bullets, quotes, two-column, image slides)
- Output ONLY a JSON array of slide objects. No markdown formatting, no code fences."#;

pub async fn run(
    settings: &AppSettings,
    ctx: &AgentContext,
) -> Result<Vec<SlideOutline>, String> {
    let empty_tools = json!([]);

    let research_section = if ctx.web_research.is_empty() {
        String::new()
    } else {
        format!("\n\n## Research (use this to enrich slides with facts/data):\n{}", safe_trunc(&ctx.web_research, 3000))
    };

    let image_hint = if ctx.image_refs.is_empty() {
        String::new()
    } else {
        format!("\n\n## Available images ({} found — plan image slides where appropriate).", ctx.image_refs.len())
    };

    let user_msg = format!(
        "Topic: {}\nIntent: {}\nAudience: {}\nSLIDE COUNT: {} (MUST be exactly this many)\nLanguage: {}\nStyle: {}{}{}\n\nReturn ONLY the JSON array with exactly {} slides.",
        ctx.topic,
        ctx.intent,
        ctx.audience,
        ctx.slide_count,
        ctx.language,
        ctx.style_hint,
        research_section,
        image_hint,
        ctx.slide_count,
    );

    let history = vec![AgentMessage { role: "user".to_string(), content: user_msg }];
    let resp = call_llm(settings, SYSTEM_PROMPT, &history, &empty_tools).await?;

    let raw = resp.text
        .or_else(|| resp.function_calls.first().map(|fc| fc.args.to_string()))
        .ok_or_else(|| "Content agent returned no content".to_string())?;

    let json_str = extract_json_array(&raw);
    let arr: Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Content agent parse error: {}. Raw: {}", e, safe_trunc(&raw, 400)))?;

    let slides_arr = arr.as_array()
        .ok_or_else(|| format!("Content agent: expected JSON array, got: {}", safe_trunc(&raw, 200)))?;

    let mut outlines = Vec::new();
    for (i, s) in slides_arr.iter().enumerate() {
        let bullets: Vec<String> = s["bullets"].as_array()
            .map(|a| a.iter().filter_map(|b| b.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        outlines.push(SlideOutline {
            index: s["index"].as_u64().unwrap_or(i as u64) as usize,
            slide_type: s["type"].as_str().unwrap_or("content").to_string(),
            title: s["title"].as_str().unwrap_or("").to_string(),
            bullets,
            notes: s["notes"].as_str().unwrap_or("").to_string(),
            transition: s["transition"].as_str().unwrap_or("fade").to_string(),
        });
    }

    if outlines.is_empty() {
        return Err("Content agent returned empty outline".to_string());
    }

    Ok(outlines)
}

fn extract_json_array(text: &str) -> String {
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
    if let (Some(start), Some(end)) = (text.find('['), text.rfind(']')) {
        if start <= end {
            return text[start..=end].to_string();
        }
    }
    text.to_string()
}
