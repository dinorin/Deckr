#![recursion_limit = "512"]
mod agents;
mod llm;
mod pptx;
mod settings;
mod slide;
mod storage;
mod tools;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{Emitter, Manager};

#[tauri::command]
async fn app_ready(app: tauri::AppHandle) {
    if let Some(splash) = app.get_webview_window("splashscreen") {
        let _ = splash.close();
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.maximize();
    }
}

// ─── Multi-Agent Generate ─────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentLogEntry {
    pub agent: String,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MultiAgentResult {
    #[serde(rename = "deckData")]
    pub deck_data: Option<Value>,
    #[serde(rename = "slideEdits")]
    pub slide_edits: Vec<Value>,
    #[serde(rename = "coachMessage")]
    pub coach_message: String,
    pub notes: String,
    #[serde(rename = "agentLog")]
    pub agent_log: Vec<AgentLogEntry>,
}

#[tauri::command]
async fn generate_deck_v2(
    app: tauri::AppHandle,
    history: Vec<agents::AgentMessage>,
    current_deck: Option<Value>,
    notes: Option<String>,
    language: String,
    num_slides: u32,
) -> Result<MultiAgentResult, String> {
    let mut settings = settings::load_settings_raw(&app);

    if settings.llm.api_key.trim() == settings::MASKED_SENTINEL {
        settings.llm.api_key = settings::resolve_api_key(
            &app, &settings.llm.provider, &settings.llm.api_key
        );
    }

    if settings.llm.model.trim().is_empty() {
        return Err("No model selected. Open Settings and configure an AI provider.".into());
    }

    let provider = settings.llm.provider.to_lowercase();
    if provider == "gemini" && settings.llm.api_key.is_empty() {
        return Err("Gemini API key is missing.".into());
    }
    if provider != "ollama" && provider != "lmstudio" && settings.llm.api_key.is_empty() {
        return Err(format!("API key for {} is missing.", settings.llm.provider));
    }

    let notes_str = notes.as_deref().unwrap_or("").to_string();
    let mut log: Vec<AgentLogEntry> = Vec::new();

    macro_rules! emit {
        ($agent:expr, $status:expr, $msg:expr) => {{
            let entry = AgentLogEntry {
                agent: $agent.into(),
                status: $status.into(),
                message: $msg.into(),
            };
            let _ = app.emit("agent-status", &entry);
            log.push(entry);
        }};
    }

    // Resolve language: if user set something explicit, use it; else auto-detect from message
    let effective_language = if language.trim().is_empty() || language.trim() == "auto" {
        let last_msg = history.iter().rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("");
        agents::orchestrator::detect_language_pub(last_msg)
    } else {
        language.clone()
    };

    let slide_count = num_slides.max(3).min(30) as usize;

    // ── Orchestrator ──────────────────────────────────────────────────────────
    emit!("orchestrator", "thinking", "Analyzing request…");

    let last_user_msg = history.iter().rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_str())
        .unwrap_or("");

    let orch = agents::orchestrator::run(&settings, last_user_msg, &history, &current_deck).await
        .map_err(|e| format!("Orchestrator: {}", e))?;

    match orch.action {
        // ── Just a message ─────────────────────────────────────────────────
        agents::orchestrator::OrchestratorAction::Message(msg) => {
            emit!("orchestrator", "done", msg.clone());
            return Ok(MultiAgentResult {
                deck_data: current_deck,
                slide_edits: Vec::new(),
                coach_message: msg,
                notes: notes_str,
                agent_log: log,
            });
        }

        // ── Create new deck ────────────────────────────────────────────────
        agents::orchestrator::OrchestratorAction::CreateDeck(params) => {
            emit!("orchestrator", "done", format!("Creating \"{}\"", params.topic));

            // ── Phase 1: Parallel search workers ──────────────────────────
            let keywords_display = params.keywords.iter()
                .map(|k| format!("\"{}\"", k))
                .collect::<Vec<_>>()
                .join(", ");
            emit!("search", "thinking", format!("Searching: {}", keywords_display));

            let tavily_key = settings.search.get("tavily").cloned().unwrap_or_default();
            let keywords = params.keywords.clone();
            // Use the first generated keyword for image search to get relevant results, instead of the long user prompt
            let topic_for_img = params.keywords.first().cloned().unwrap_or_else(|| params.topic.clone());

            let (web_research, tavily_imgs) = tools::parallel_tavily_search(keywords, &tavily_key).await;

            let mut images: Vec<tools::ImageResult> = Vec::new();
            for (i, t_url) in tavily_imgs.into_iter().enumerate() {
                images.push(tools::ImageResult {
                    url: t_url,
                    width: 1280,
                    height: 720,
                    aspect_ratio: 1280.0 / 720.0,
                    description: format!("Tavily research image {}", i + 1),
                    avg_color: "rgb(100,100,100)".to_string(),
                    text_color: "#ffffff".to_string(),
                });
            }

            let image_count = images.len();
            let research_len = web_research.chars().count();

            emit!("search", "done", format!(
                "Found {} images · {} chars of research",
                image_count, research_len
            ));

            // ── Phase 2: Content planning ──────────────────────────────────
            emit!("content", "thinking", format!(
                "Planning {} slides in {}…",
                slide_count, effective_language
            ));

            let ctx = agents::AgentContext {
                topic: params.topic.clone(),
                intent: params.intent.clone(),
                audience: params.audience.clone(),
                slide_count,
                language: effective_language.clone(),
                style_hint: params.style_hint.clone(),
                web_research: web_research.clone(),
                image_refs: images.iter().map(|i| i.url.clone()).collect(),
                ..Default::default()
            };

            let outline = agents::content_agent::run(&settings, &ctx).await
                .map_err(|e| format!("Content agent: {}", e))?;

            emit!("content", "done", format!("{} slides planned", outline.len()));

            // ── Phase 3: Theme & animation design ─────────────────────────
            emit!("design", "thinking", "Designing layout & animations…");

            let (theme, anim_plan) = agents::animation_agent::run(&settings, &ctx, &outline).await
                .map_err(|e| format!("Animation agent: {}", e))?;

            emit!("design", "done", format!("{} theme · {} slide plans", theme.style, anim_plan.len()));

            // Signal frontend to open the preview panel immediately
            let early_title = outline.first()
                .map(|s| s.title.clone())
                .unwrap_or_else(|| params.topic.clone());

            let _ = app.emit("deck-started", serde_json::json!({
                "title": early_title,
                "slideCount": outline.len(),
                "theme": {
                    "primaryColor": theme.primary_color,
                    "secondaryColor": theme.secondary_color,
                    "backgroundColor": theme.bg_color,
                    "textColor": theme.text_color,
                    "fontFamily": theme.font_family,
                    "style": theme.style,
                }
            }));

            // ── Phase 4: Parallel slide generation ────────────────────────
            emit!("slides", "thinking", format!(
                "Rendering {} slides in parallel…",
                outline.len()
            ));

            let mut html_ctx = ctx.clone();
            html_ctx.slide_outline = outline.clone();
            html_ctx.theme = Some(theme.clone());
            html_ctx.animation_plan = anim_plan.clone();

            let deck = agents::html_agent::run(
                &app, &settings, &html_ctx, &outline, &theme, &anim_plan, &images
            ).await.map_err(|e| format!("HTML agent: {}", e))?;

            let valid_slides: Vec<_> = deck.slides.iter()
                .filter(|s| !s.html.trim().is_empty())
                .collect();

            emit!("slides", "done", format!("{} slides ready", valid_slides.len()));

            if valid_slides.is_empty() {
                return Err("No slides generated. Try again.".into());
            }

            let slide_count_final = valid_slides.len();
            let slides_json: Vec<Value> = valid_slides.iter().map(|s| serde_json::json!({
                "id": s.id, "type": s.slide_type, "html": s.html
            })).collect();

            let deck_data = serde_json::json!({
                "title": deck.title,
                "theme": deck.theme,
                "slides": slides_json,
                "masterHtml": deck.master_html,
                "metadata": {
                    "slideCount": slide_count_final,
                    "generatedAt": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default().as_millis() as u64,
                    "topic": params.topic,
                    "language": effective_language,
                }
            });

            Ok(MultiAgentResult {
                deck_data: Some(deck_data),
                slide_edits: Vec::new(),
                coach_message: format!(
                    "\"{}\" is ready — {} slides! Click to reveal animations.",
                    deck.title, slide_count_final
                ),
                notes: notes_str,
                agent_log: log,
            })
        }

        // ── Edit existing deck ─────────────────────────────────────────────
        agents::orchestrator::OrchestratorAction::EditDeck(params) => {
            let deck = current_deck.as_ref()
                .ok_or_else(|| "No existing deck to edit. Create one first.".to_string())?;

            // Resolve slide_index → slide_id (e.g. index 2 → id "s3")
            let target_slide_id: Option<String> = params.slide_index.and_then(|idx| {
                deck["slides"].as_array()
                    .and_then(|arr| arr.get(idx as usize))
                    .and_then(|s| s["id"].as_str())
                    .map(|id| id.to_string())
            });

            let scope_desc = match target_slide_id.as_deref() {
                Some(id) => format!("Editing slide {}…", id),
                None => "Editing presentation…".to_string(),
            };
            emit!("orchestrator", "done", scope_desc);
            emit!("edit", "thinking", format!("Instructions: {}", &params.instructions));

            let edit_result = agents::edit_agent::run(
                &settings,
                deck,
                &params.instructions,
                target_slide_id.as_deref(),
            ).await.map_err(|e| format!("Edit agent: {}", e))?;

            emit!("edit", "done", edit_result.coach_message.clone());

            // Apply edits to deck and rebuild masterHtml
            let updated_deck = agents::edit_agent::apply_edits(deck, &edit_result);

            Ok(MultiAgentResult {
                deck_data: Some(updated_deck),
                slide_edits: edit_result.updated_slides.iter().map(|(id, html)| {
                    serde_json::json!({ "slideId": id, "html": html })
                }).collect(),
                coach_message: edit_result.coach_message,
                notes: notes_str,
                agent_log: log,
            })
        }
    }
}

// ─── AI Image Generation ──────────────────────────────────────────────────────

#[tauri::command]
async fn generate_ai_image(app: tauri::AppHandle, prompt: String) -> Result<String, String> {
    let settings = settings::load_settings_raw(&app);
    let order = ["together", "fal", "openai_img", "getimg"];
    for provider_id in &order {
        if let Some(cfg) = settings.image.get(*provider_id) {
            if !cfg.api_key.trim().is_empty() {
                return tools::generate_ai_image_with_provider(
                    &prompt, provider_id, &cfg.api_key, &cfg.model
                ).await;
            }
        }
    }
    Err("No image provider configured. Add an API key in Settings → Image.".to_string())
}

// ─── PPTX Export ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct PptxSlideInput {
    pub html: String,
    pub index: usize,
}

#[tauri::command]
async fn export_pptx(title: String, slides: Vec<PptxSlideInput>) -> Result<Vec<u8>, String> {
    let pptx_slides: Vec<pptx::builder::PptxSlide> = slides.iter()
        .map(|s| pptx::builder::parse_slide_html(&s.html, s.index))
        .collect();
    pptx::build_pptx(&title, &pptx_slides)
}

// ─── Runner ───────────────────────────────────────────────────────────────────

pub fn run() {
    let builder = tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            app_ready,
            generate_deck_v2,
            generate_ai_image,
            export_pptx,
            llm::generate_deck,
            llm::fetch_models,
            settings::get_settings,
            settings::save_settings,
            storage::save_session,
            storage::list_sessions,
            storage::load_session,
            storage::delete_session,
        ]);

    #[cfg(debug_assertions)]
    let builder = builder.plugin(tauri_plugin_mcp_bridge::init());

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
