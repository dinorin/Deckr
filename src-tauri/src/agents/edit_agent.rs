use serde_json::{json, Value};

use super::{AgentMessage, call_llm, html_agent::build_master_html, GeneratedSlide, safe_trunc};
use crate::settings::AppSettings;

// ─── Base64 stripping ─────────────────────────────────────────────────────────

/// Strip base64 data URIs, returning the cleaned HTML and a map of placeholders → originals.
fn strip_base64(html: &str) -> (String, Vec<(String, String)>) {
    let tag = ";base64,";
    if !html.contains(tag) {
        return (html.to_string(), vec![]);
    }

    let mut result = String::with_capacity(html.len());
    let mut map: Vec<(String, String)> = Vec::new();
    let mut remaining = html;

    while let Some(b64_pos) = remaining.find(tag) {
        // Walk back to find "data:"
        let prefix = &remaining[..b64_pos];
        let data_start = prefix.rfind("data:").unwrap_or(b64_pos);

        result.push_str(&remaining[..data_start]);

        // Walk forward past the base64 data (end at quote / paren / whitespace)
        let after_b64 = &remaining[b64_pos + tag.len()..];
        let rel_end = after_b64
            .find(|c: char| c == '"' || c == '\'' || c == ')' || c == ' ' || c == '\n')
            .unwrap_or(after_b64.len());

        let original = &remaining[data_start..b64_pos + tag.len() + rel_end];
        let placeholder = format!("[b64-{}]", map.len());
        map.push((placeholder.clone(), original.to_string()));
        result.push_str(&placeholder);

        remaining = &remaining[b64_pos + tag.len() + rel_end..];
    }
    result.push_str(remaining);
    (result, map)
}

fn restore_base64(html: &str, map: &[(String, String)]) -> String {
    let mut result = html.to_string();
    for (placeholder, original) in map {
        result = result.replace(placeholder.as_str(), original.as_str());
    }
    result
}

// ─── Edit Agent ───────────────────────────────────────────────────────────────

const SYSTEM_PROMPT: &str = r#"You are the Deckr slide editor. You receive the current presentation slides (base64 images stripped) and fix instructions.

Your task: output ONLY a JSON array of slide updates. Each entry is a slide that needs changes:
[
  {
    "slide_id": "s3",
    "html": "<!-- complete new <div class=\"ppt-slide\"...>...</div> -->"
  }
]

Rules for HTML Structure:
1. Outer wrapper MUST be <div class="ppt-slide" ...> with style="position:relative;width:960px;height:540px;overflow:hidden;..."
2. Animation: Elements revealed on click MUST be wrapped in a div with class "ppt-element ppt-hidden ppt-ANIMATION" AND "data-click" attribute (1, 2, 3...). Do NOT strip this wrapper when editing text inside it!
3. Animation: Elements with "data-click" > 0 MUST have "data-ppt-animation" attribute (e.g., "fade-in", "fly-in-bottom").
4. Layout: STRICTLY FORBIDDEN to use `transform` for centering (e.g. no `translateX(-50%)`). Use Flexbox, Grid, or absolute `left/top` only. Animations will overwrite any layout `transform`. Everything must fit 960x540.
5. Typography — canvas is 960×540px, fixed sizes (DO NOT deviate):
   <h1>=60px(text-6xl, ~20 chars/line) · <h2>=36px(text-4xl, ~32 chars) · <h3>=30px(text-3xl) · <p>=20px(text-xl, ~72 chars) · <li>=18px(text-lg) · small label=14px(text-sm)
6. Export: All text tags (<p>, <span>, <h1>, etc.) MUST have data-ppt-* attributes (data-ppt-font-size, data-ppt-bold, data-ppt-color, data-ppt-align, data-ppt-font).
7. Colors: All text tags MUST have explicit color in inline style. Keep text readable against its background.
8. Images: Standard <img> tags must have a valid src. For AI-generated images, use class="ai-gen-image" and data-prompt.
9. Placeholders: Keep [b64-N] placeholders exactly as they are if you are not replacing the image.

Output raw JSON only, no markdown fences."#;

pub struct EditResult {
    pub updated_slides: Vec<(String, String)>, // (slide_id, new_html)
    pub coach_message: String,
}

pub async fn run(
    settings: &AppSettings,
    current_deck: &Value,
    instructions: &str,
    target_slide_id: Option<&str>,
) -> Result<EditResult, String> {
    let empty_tools = json!([]);

    // Build slide context (strip base64 first)
    let slides_arr = current_deck["slides"].as_array()
        .ok_or("No slides in deck")?;

    let theme = &current_deck["theme"];
    let title = current_deck["title"].as_str().unwrap_or("Presentation");

    // Collect stripped slides (all or just the target)
    let mut stripped_map: Vec<(String, String, Vec<(String, String)>)> = Vec::new(); // (id, stripped_html, b64_map)
    for slide in slides_arr {
        let id = slide["id"].as_str().unwrap_or("").to_string();
        if let Some(target) = target_slide_id {
            if id != target { continue; }
        }
        let raw_html = slide["html"].as_str().unwrap_or("");
        let (stripped, b64_map) = strip_base64(raw_html);
        stripped_map.push((id, stripped, b64_map));
    }

    if stripped_map.is_empty() {
        return Err("No matching slides found for edit".to_string());
    }

    // Build prompt
    let slides_text: String = stripped_map.iter()
        .map(|(id, html, _)| format!("=== Slide {} ===\n{}\n", id, safe_trunc(html, 4000)))
        .collect::<Vec<_>>()
        .join("\n");

    let theme_hint = format!(
        "Theme: {} | primary:{} | accent:{} | bg:{} | font:{}",
        theme["style"].as_str().unwrap_or("modern"),
        theme["primaryColor"].as_str().unwrap_or("#6366f1"),
        theme["secondaryColor"].as_str().unwrap_or("#8b5cf6"),
        theme["backgroundColor"].as_str().unwrap_or("#0f0f1f"),
        theme["fontFamily"].as_str().unwrap_or("Inter"),
    );

    let scope_note = match target_slide_id {
        Some(id) => format!("Focus on slide {}.", id),
        None => "Fix any slides that need it.".to_string(),
    };

    let user_msg = format!(
        "Presentation: \"{title}\"\n{theme_hint}\n\nInstructions: {instructions}\n{scope_note}\n\nCurrent slides:\n{slides_text}\n\nOutput JSON array of updates:"
    );

    let history = vec![AgentMessage { role: "user".to_string(), content: user_msg }];
    let resp = call_llm(settings, SYSTEM_PROMPT, &history, &empty_tools).await?;

    let raw = resp.text.unwrap_or_default();
    let json_str = extract_json_array(&raw);
    let updates: Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Edit agent parse error: {}. Raw: {}", e, safe_trunc(&raw, 300)))?;

    let arr = updates.as_array()
        .ok_or("Edit agent did not return an array")?;

    // Re-inject base64 into updated HTML
    let mut result: Vec<(String, String)> = Vec::new();
    for update in arr {
        let slide_id = update["slide_id"].as_str().unwrap_or("").to_string();
        let new_html = update["html"].as_str().unwrap_or("").to_string();
        if slide_id.is_empty() || new_html.is_empty() { continue; }

        // Find the b64 map for this slide
        let restored = if let Some((_, _, b64_map)) = stripped_map.iter().find(|(id, _, _)| id == &slide_id) {
            restore_base64(&new_html, b64_map)
        } else {
            new_html
        };
        result.push((slide_id, restored));
    }

    if result.is_empty() {
        return Err("Edit agent returned no changes".to_string());
    }

    let count = result.len();
    Ok(EditResult {
        updated_slides: result,
        coach_message: format!("Fixed {} slide{}.", count, if count == 1 { "" } else { "s" }),
    })
}

/// Apply edit results to an existing deck Value, returning updated slides + rebuilt masterHtml.
pub fn apply_edits(deck: &Value, edits: &EditResult) -> Value {
    let mut slides_arr = deck["slides"].as_array()
        .cloned()
        .unwrap_or_default();

    for (slide_id, new_html) in &edits.updated_slides {
        if let Some(slide) = slides_arr.iter_mut().find(|s| s["id"].as_str() == Some(slide_id)) {
            *slide = json!({
                "id": slide_id,
                "type": slide["type"].as_str().unwrap_or("content"),
                "html": new_html,
            });
        }
    }

    // Rebuild masterHtml
    let gen_slides: Vec<GeneratedSlide> = slides_arr.iter().map(|s| GeneratedSlide {
        id: s["id"].as_str().unwrap_or("").to_string(),
        slide_type: s["type"].as_str().unwrap_or("content").to_string(),
        html: s["html"].as_str().unwrap_or("").to_string(),
    }).collect();

    let title = deck["title"].as_str().unwrap_or("Presentation");
    let master_html = build_master_html(title, &gen_slides);

    let mut updated = deck.clone();
    updated["slides"] = Value::Array(slides_arr);
    updated["masterHtml"] = Value::String(master_html);
    updated
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
