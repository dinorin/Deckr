use serde_json::{json, Value};

use super::{AgentContext, AgentMessage, DeckTheme, ElementAnimationPlan, SlideAnimationPlan, SlideOutline, call_llm};
use crate::settings::AppSettings;

const SYSTEM_PROMPT: &str = r##"You are the Layout & Animation Planner for Deckr. Output ONLY valid JSON — no markdown, no explanation.

## Canvas: 960 × 540 px
Safe zone: x ≥ 40, y ≥ 30, x+w ≤ 920, y+h ≤ 510.

## Non-overlap Rule
Stack elements vertically: next.y ≥ prev.y + prev.h + 20 (min 20px gap).
Side-by-side columns: left ends at x+w ≤ 440, right starts at x ≥ 520 (80px gutter between).
Decoration/bg layers may overlap text — they render behind.

## Layout Templates (use these exact zones, adapt element count)

### type: title
  title     x=40  y=175 w=880 h=88  font_size=56 bold=true  align=center click=0 anim=fly-in-bottom
  subtitle  x=40  y=283 w=880 h=52  font_size=28 bold=false align=center click=1 anim=fade-in

### type: content  (text left, image area right)
  title     x=40  y=32  w=560 h=62  font_size=40 bold=true  align=left click=0 anim=wipe-left
  body×1    x=40  y=114 w=540 h=52  font_size=20 bold=false align=left click=1 anim=float-in
  body×2    x=40  y=186 w=540 h=52  font_size=20 bold=false align=left click=2 anim=float-in
  body×3    x=40  y=258 w=540 h=52  font_size=20 bold=false align=left click=3 anim=float-in
  image     x=600 y=50  w=320 h=380 font_size=0  bold=false align=left click=0 anim=fade-in

### type: bullets
  title     x=40  y=32  w=880 h=62  font_size=40 bold=true  align=left click=0 anim=wipe-left
  bullet×1  x=64  y=114 w=852 h=40  font_size=19 bold=false align=left click=1 anim=fly-in-left
  bullet×2  x=64  y=162 w=852 h=40  font_size=19 bold=false align=left click=2 anim=fly-in-left
  bullet×3  x=64  y=210 w=852 h=40  font_size=19 bold=false align=left click=3 anim=fly-in-left
  bullet×4  x=64  y=258 w=852 h=40  font_size=19 bold=false align=left click=4 anim=fly-in-left
  bullet×5  x=64  y=306 w=852 h=40  font_size=19 bold=false align=left click=5 anim=fly-in-left
  (if >5 bullets: reduce h to 36, recompute y for each; max 6 bullets)

### type: two-column
  title     x=40  y=32  w=880 h=62  font_size=40 bold=true  align=left  click=0 anim=fade-in
  lheading  x=40  y=114 w=400 h=44  font_size=22 bold=true  align=left  click=1 anim=fly-in-left
  lbody×1   x=40  y=168 w=400 h=46  font_size=18 bold=false align=left  click=2 anim=fly-in-left
  lbody×2   x=40  y=224 w=400 h=46  font_size=18 bold=false align=left  click=3 anim=fly-in-left
  rheading  x=520 y=114 w=400 h=44  font_size=22 bold=true  align=left  click=1 anim=fly-in-right
  rbody×1   x=520 y=168 w=400 h=46  font_size=18 bold=false align=left  click=2 anim=fly-in-right
  rbody×2   x=520 y=224 w=400 h=46  font_size=18 bold=false align=left  click=3 anim=fly-in-right

### type: quote
  accent    x=40  y=130 w=6   h=160 font_size=0  bold=false align=left click=0 anim=appear
  quote     x=74  y=140 w=822 h=172 font_size=30 bold=false align=center italic=true click=0 anim=fade-in
  author    x=74  y=326 w=822 h=46  font_size=18 bold=false align=right click=1 anim=float-in

### type: image
  title     x=40  y=32  w=880 h=62  font_size=40 bold=true  align=left  click=0 anim=fly-in-top
  image     x=40  y=114 w=880 h=360 font_size=0  bold=false align=left  click=0 anim=fade-in
  caption   x=40  y=480 w=880 h=28  font_size=14 bold=false align=center click=0 anim=appear

### type: closing
  title     x=40  y=192 w=880 h=88  font_size=56 bold=true  align=center click=0 anim=zoom-in
  subtitle  x=40  y=304 w=880 h=52  font_size=28 bold=false align=center click=1 anim=fade-in

## Click / Reveal Strategy
- click=0: visible on slide entry (title, images, structural elements)
- click=1,2,3…: sequential reveal — use for bullets, body paragraphs, key points
- Limit: max 6 clicks per slide
- Title/closing: title=0, subtitle=1
- Bullets/content: title=0, each bullet/body = sequential starting at 1

## Animation Variety
- Vary animations across slides — do not repeat the same animation for every slide
- Dark dramatic slides → swivel/zoom-in for title
- Clean minimal → wipe-left/float-in
- Bullets → fly-in-left/fly-in-bottom
- Title slides → fly-in-bottom/zoom-in

## Output Format
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
          "content": "text content (empty string for image/accent/decoration)",
          "x": 40,
          "y": 175,
          "w": 880,
          "h": 88,
          "font_size": 56,
          "bold": true,
          "italic": false,
          "color": "#ffffff",
          "align": "left|center|right",
          "animation": "appear|fade-in|fly-in-bottom|fly-in-top|fly-in-left|fly-in-right|zoom-in|bounce-in|float-in|wipe-left|split|swivel",
          "click_order": 0,
          "duration_ms": 600
        }
      ]
    }
  ]
}"##;

pub async fn run(
    settings: &AppSettings,
    ctx: &AgentContext,
    outline: &[SlideOutline],
) -> Result<(DeckTheme, Vec<SlideAnimationPlan>), String> {
    let empty_tools = json!([]);

    let outline_text: String = outline.iter().map(|s| {
        format!(
            "Slide {} [{}]: {}\nBullets: {}",
            s.index + 1, s.slide_type, s.title,
            if s.bullets.is_empty() { "(none)".to_string() } else { s.bullets.join(" | ") }
        )
    }).collect::<Vec<_>>().join("\n\n");

    let user_msg = format!(
        "Style: {}\nLanguage: {}\n\nOutline:\n{}\n\nOutput ONLY the JSON object with precise pixel positions for every element.",
        if ctx.style_hint.is_empty() { "modern dark" } else { &ctx.style_hint },
        ctx.language,
        outline_text
    );

    let history = vec![AgentMessage { role: "user".to_string(), content: user_msg }];
    let resp = call_llm(settings, SYSTEM_PROMPT, &history, &empty_tools).await?;

    let raw = match &resp.text {
        Some(t) => t.clone(),
        None => {
            if let Some(fc) = resp.function_calls.first() {
                fc.args.to_string()
            } else {
                return Err("Animation agent returned no content".to_string());
            }
        }
    };

    let json_str = extract_json(&raw);
    let parsed: Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Animation agent JSON parse error: {}. Raw: {}", e, &raw[..raw.len().min(400)]))?;

    parse_animation_plan(&parsed)
}

fn extract_json(text: &str) -> String {
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
                        x: el["x"].as_u64().unwrap_or(40) as u32,
                        y: el["y"].as_u64().unwrap_or(40) as u32,
                        width:  el["w"].as_u64().or_else(|| el["width"].as_u64()).unwrap_or(880) as u32,
                        height: el["h"].as_u64().or_else(|| el["height"].as_u64()).unwrap_or(60) as u32,
                        font_size: el["font_size"].as_u64().unwrap_or(20) as u32,
                        bold:   el["bold"].as_bool().unwrap_or(false),
                        italic: el["italic"].as_bool().unwrap_or(false),
                        color:  el["color"].as_str().unwrap_or("").to_string(),
                        align:  el["align"].as_str().unwrap_or("left").to_string(),
                        font_family: el["font_family"].as_str().unwrap_or("").to_string(),
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

    slides.sort_by_key(|s| s.index);
    Ok((theme, slides))
}
