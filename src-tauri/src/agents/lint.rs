use scraper::{Html, Selector};

/// A lint issue found in a slide's HTML.
#[derive(Debug, Clone)]
pub struct LintIssue {
    pub slide_id: String,
    pub message: String,
}

/// Lint a single slide's HTML. Returns a list of issues found.
pub fn lint_slide(slide_id: &str, html: &str) -> Vec<LintIssue> {
    let mut issues: Vec<LintIssue> = Vec::new();
    let doc = Html::parse_fragment(html);

    macro_rules! issue {
        ($msg:expr) => {
            issues.push(LintIssue { slide_id: slide_id.to_string(), message: $msg.to_string() });
        };
        ($fmt:literal, $($arg:tt)*) => {
            issues.push(LintIssue { slide_id: slide_id.to_string(), message: format!($fmt, $($arg)*) });
        };
    }

    // ── 1. Outer wrapper must be <div class="ppt-slide"> ─────────────────────
    let ppt_slide_sel = Selector::parse(".ppt-slide").unwrap();
    if doc.select(&ppt_slide_sel).next().is_none() {
        issue!("Missing outer <div class=\"ppt-slide\">. The entire slide content must be wrapped in it.");
    }

    // ── 2. ppt-hidden elements must have data-click ───────────────────────────
    let hidden_sel = Selector::parse(".ppt-hidden").unwrap();
    for el in doc.select(&hidden_sel) {
        if el.value().attr("data-click").is_none() {
            let tag = el.value().name();
            let classes = el.value().attr("class").unwrap_or("");
            issue!(
                "Element <{} class=\"{}\"> has class ppt-hidden but no data-click attribute — it will be hidden forever.",
                tag, classes
            );
        }
    }

    // ── 3. data-click elements should have data-ppt-animation ────────────────
    let data_click_sel = Selector::parse("[data-click]").unwrap();
    for el in doc.select(&data_click_sel) {
        let click = el.value().attr("data-click").unwrap_or("0");
        if click != "0" && el.value().attr("data-ppt-animation").is_none() {
            let tag = el.value().name();
            issue!(
                "Element <{}> has data-click=\"{}\" but missing data-ppt-animation — animation type unknown.",
                tag, click
            );
        }
    }

    // ── 4. data-ppt-animation with unknown value ──────────────────────────────
    let known_anims = [
        "appear", "fade-in", "fly-in-bottom", "fly-in-top",
        "fly-in-left", "fly-in-right", "zoom-in", "bounce-in",
        "float-in", "wipe-left", "split", "swivel",
    ];
    let anim_sel = Selector::parse("[data-ppt-animation]").unwrap();
    for el in doc.select(&anim_sel) {
        let anim = el.value().attr("data-ppt-animation").unwrap_or("");
        if !known_anims.contains(&anim) {
            issue!(
                "Unknown animation \"{}\" — use one of: {}",
                anim, known_anims.join(", ")
            );
        }
    }

    // ── 5. Non-AI img with empty src ─────────────────────────────────────────
    let img_sel = Selector::parse("img").unwrap();
    for el in doc.select(&img_sel) {
        let src = el.value().attr("src").unwrap_or("").trim().to_string();
        let classes = el.value().attr("class").unwrap_or("");
        let is_ai = classes.contains("ai-gen-image");
        if src.is_empty() && !is_ai {
            issue!("Regular <img> has empty src and is not an ai-gen-image — it will show a broken image icon. Use a real URL or add class=\"ai-gen-image\" with data-prompt.");
        }
    }

    // ── 6. Truncated HTML ────────────────────────────────────────────────────
    let trimmed = html.trim();
    if !trimmed.ends_with("</div>") {
        issue!("Slide HTML appears to be truncated or is missing the final </div> tag.");
    }

    // ── 7. Tag Balance (Simple heuristic) ────────────────────────────────────
    let div_open = html.matches("<div").count();
    let div_close = html.matches("</div>").count();
    if div_open != div_close {
        issue!(
            "Mismatched <div> tags: {} opening vs {} closing. This usually breaks the layout.",
            div_open, div_close
        );
    }

    issues
}

/// Lint all slides in a generated deck, returning all issues grouped by slide.
pub fn lint_deck(slides: &[(String, String)]) -> Vec<LintIssue> {
    slides.iter()
        .flat_map(|(id, html)| lint_slide(id, html))
        .collect()
}

/// Format lint issues as a concise instruction string for the edit agent.
pub fn issues_to_instructions(issues: &[LintIssue]) -> String {
    issues.iter()
        .map(|i| format!("[{}] {}", i.slide_id, i.message))
        .collect::<Vec<_>>()
        .join("\n")
}
