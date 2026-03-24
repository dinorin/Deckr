use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use crate::settings::load_settings_raw;
use crate::slide;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HistoryMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FunctionCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TokenUsage {
    pub prompt: i64,
    pub completion: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct LlmResponse {
    pub text: Option<String>,
    pub function_calls: Vec<FunctionCall>,
    pub token_usage: TokenUsage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateResult {
    #[serde(rename = "deckData")]
    pub deck_data: Option<Value>,
    #[serde(rename = "slideEdits")]
    pub slide_edits: Vec<SlideEdit>,
    #[serde(rename = "coachMessage")]
    pub coach_message: String,
    pub notes: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SlideEdit {
    #[serde(rename = "slideId")]
    pub slide_id: String,
    pub html: String,
}

use crate::settings::AppSettings;

// ─── Gemini API ───────────────────────────────────────────────────────────────

async fn call_gemini(settings: &AppSettings, sys: &str, history: &[HistoryMessage]) -> Result<LlmResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .no_gzip()
        .no_deflate()
        .no_brotli()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        settings.llm.model, settings.llm.api_key
    );

    let contents: Vec<Value> = history.iter().map(|m| {
        let role = if m.role == "user" { "user" } else { "model" };
        json!({ "role": role, "parts": [{"text": &m.content}] })
    }).collect();

    let body = json!({
        "contents": contents,
        "system_instruction": { "parts": [{"text": sys}] },
        "tools": [{ "function_declarations": slide::tools_gemini() }],
        "generationConfig": { "temperature": 0.7, "maxOutputTokens": 32768 }
    });

    let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    let content_length = body_str.len();

    let resp = client.post(&url)
        .header("Content-Type", "application/json")
        .header("Content-Length", content_length)
        .header("User-Agent", "Deckr/1.0")
        .body(body_str)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let bytes = resp.bytes().await.map_err(|_| "Failed to read response".to_string())?;
    let text = String::from_utf8_lossy(&bytes);

    let json: Value = serde_json::from_str(&text).map_err(|e| {
        format!("JSON parse error: {}. Body: {}", e, &text[..text.len().min(500)])
    })?;

    if let Some(err) = json.get("error") {
        return Err(format!("Gemini API error: {}", err["message"].as_str().unwrap_or("Unknown")));
    }

    let candidate = &json["candidates"][0];
    if candidate.is_null() {
        return Err("Gemini returned no candidates. Request may have been blocked.".into());
    }

    let mut text_out = None;
    let mut function_calls = Vec::new();

    if let Some(parts) = candidate["content"]["parts"].as_array() {
        for part in parts {
            if let Some(t) = part["text"].as_str() {
                if !t.is_empty() { text_out = Some(t.to_string()); }
            }
            if let Some(fc) = part.get("functionCall") {
                let name = fc["name"].as_str().unwrap_or("").to_string();
                function_calls.push(FunctionCall { name, args: fc["args"].clone() });
            }
        }
    }

    let usage = &json["usageMetadata"];
    Ok(LlmResponse {
        text: text_out,
        function_calls,
        token_usage: TokenUsage {
            prompt: usage["promptTokenCount"].as_i64().unwrap_or(0),
            completion: usage["candidatesTokenCount"].as_i64().unwrap_or(0),
            total: usage["totalTokenCount"].as_i64().unwrap_or(0),
        },
    })
}

// ─── OpenAI-compatible API ────────────────────────────────────────────────────

async fn call_openai_compat(settings: &AppSettings, sys: &str, history: &[HistoryMessage]) -> Result<LlmResponse, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .pool_max_idle_per_host(0)
        .no_gzip()
        .no_deflate()
        .no_brotli()
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("{}/chat/completions", settings.llm.base_url.trim_end_matches('/'));

    let mut messages = vec![json!({"role": "system", "content": sys})];
    for m in history {
        let role = if m.role == "user" { "user" } else { "assistant" };
        messages.push(json!({"role": role, "content": m.content}));
    }

    let body = json!({
        "model": settings.llm.model,
        "messages": messages,
        "tools": slide::tools_openai(),
        "tool_choice": "auto",
        "max_tokens": 8192
    });

    let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    let content_length = body_str.len();

    let resp = client.post(&url)
        .header("Authorization", format!("Bearer {}", settings.llm.api_key))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .header("Content-Length", content_length)
        .body(body_str)
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;

    let bytes = resp.bytes().await.map_err(|_| "Failed to read response".to_string())?;
    let text = String::from_utf8_lossy(&bytes).to_string();

    let json: Value = serde_json::from_str(&text).map_err(|e| {
        format!("JSON parse error: {}. Body: {}", e, &text[..text.len().min(500)])
    })?;

    if let Some(err) = json.get("error") {
        let msg = err["message"].as_str().or(err.as_str()).unwrap_or("Unknown");
        return Err(format!("API error: {}. Check your API key or balance.", msg));
    }

    let choice = &json["choices"][0];
    if choice.is_null() {
        return Err(format!("API returned no choices. Body: {}", &text[..text.len().min(300)]));
    }

    let msg = &choice["message"];
    let text_content = msg["content"].as_str().map(|s| s.to_string());
    let mut function_calls = Vec::new();

    if let Some(tool_calls) = msg["tool_calls"].as_array() {
        for tc in tool_calls {
            let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
            let args: Value = serde_json::from_str(
                tc["function"]["arguments"].as_str().unwrap_or("{}")
            ).unwrap_or(json!({}));
            function_calls.push(FunctionCall { name, args });
        }
    }

    let usage = &json["usage"];
    Ok(LlmResponse {
        text: text_content,
        function_calls,
        token_usage: TokenUsage {
            prompt: usage["prompt_tokens"].as_i64().unwrap_or(0),
            completion: usage["completion_tokens"].as_i64().unwrap_or(0),
            total: usage["total_tokens"].as_i64().unwrap_or(0),
        },
    })
}

// ─── Main generate command ────────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_deck(
    app: tauri::AppHandle,
    history: Vec<HistoryMessage>,
    current_deck: Option<Value>,
    notes: Option<String>,
    _language: String,
) -> Result<GenerateResult, String> {
    let mut settings = load_settings_raw(&app);
    
    // Safety check: resolve masked key if any
    if settings.llm.api_key.trim() == crate::settings::MASKED_SENTINEL {
        settings.llm.api_key = crate::settings::resolve_api_key(
            &app, &settings.llm.provider, &settings.llm.api_key
        );
    }

    let notes_str = notes.as_deref().unwrap_or("No notes yet.");
    let sys = slide::system_prompt(notes_str);

    if settings.llm.model.trim().is_empty() {
        return Err("No model selected. Open Settings and configure an AI provider.".into());
    }

    let provider = settings.llm.provider.to_lowercase();
    let api_key = settings.llm.api_key.trim();

    if provider == "gemini" && api_key.is_empty() {
        return Err("Gemini API key is missing. Open Settings to add it.".into());
    }
    if provider != "ollama" && provider != "lmstudio" && api_key.is_empty() {
        return Err(format!("API key for {} is missing.", settings.llm.provider));
    }
    if provider != "gemini" && settings.llm.base_url.trim().is_empty() {
        return Err("Base URL is missing. Open Settings to configure it.".into());
    }

    // Agent loop: call LLM, process tool calls
    let mut result = GenerateResult {
        deck_data: current_deck,
        slide_edits: Vec::new(),
        coach_message: String::new(),
        notes: notes_str.to_string(),
    };

    let mut loop_history = history.clone();
    let mut iterations = 0;

    loop {
        iterations += 1;
        if iterations > 5 { break; }

        let resp = match provider.as_str() {
            "gemini" => call_gemini(&settings, &sys, &loop_history).await?,
            _ => call_openai_compat(&settings, &sys, &loop_history).await?,
        };

        // Process tool calls
        if resp.function_calls.is_empty() {
            // Plain text response
            if let Some(text) = resp.text {
                result.coach_message = text;
            }
            break;
        }

        let mut tool_results = Vec::new();
        let mut should_continue = false;

        for fc in &resp.function_calls {
            match fc.name.as_str() {
                "render_deck" => {
                    // Full deck replacement
                    let slides_raw = fc.args["slides"].as_array().cloned().unwrap_or_default();
                    let slide_count = slides_raw.len();

                    // Inject metadata
                    let mut deck = fc.args.clone();
                    if let Some(meta) = deck.as_object_mut() {
                        meta.insert("metadata".to_string(), json!({
                            "slideCount": slide_count,
                            "generatedAt": std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64,
                        }));
                    }

                    result.deck_data = Some(deck);
                    result.slide_edits = Vec::new();
                    tool_results.push(format!("render_deck: Created {} slides successfully.", slide_count));
                    should_continue = true;
                }
                "edit_slide" => {
                    let slide_id = fc.args["slideId"].as_str().unwrap_or("").to_string();
                    let html = fc.args["html"].as_str().unwrap_or("").to_string();
                    if !slide_id.is_empty() {
                        result.slide_edits.push(SlideEdit { slide_id: slide_id.clone(), html });
                        tool_results.push(format!("edit_slide: Slide {} updated.", slide_id));
                    }
                    should_continue = true;
                }
                "send_message" => {
                    result.coach_message = fc.args["message"].as_str().unwrap_or("").to_string();
                    // send_message ends the loop
                }
                _ => {
                    tool_results.push(format!("Unknown tool: {}", fc.name));
                }
            }
        }

        if !should_continue || !result.coach_message.is_empty() {
            break;
        }

        // Feed tool results back for next iteration
        if !tool_results.is_empty() {
            loop_history.push(HistoryMessage {
                role: "assistant".to_string(),
                content: format!("[Tool calls executed: {}]", tool_results.join("; ")),
            });
            loop_history.push(HistoryMessage {
                role: "user".to_string(),
                content: tool_results.join("\n"),
            });
        }
    }

    if result.coach_message.is_empty() {
        result.coach_message = if result.deck_data.is_some() {
            let count = result.deck_data.as_ref()
                .and_then(|d| d["slides"].as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            format!("Your presentation is ready with {} slides! You can ask me to adjust the design, add or remove slides, or change any content.", count)
        } else {
            "Done! Let me know if you'd like any changes.".to_string()
        };
    }

    Ok(result)
}

// ─── Fetch models ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn fetch_models(
    app: tauri::AppHandle,
    provider: String,
    base_url: String,
    api_key: String,
) -> Result<Vec<String>, String> {
    let api_key = crate::settings::resolve_api_key(&app, &provider, &api_key);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .no_gzip()
        .no_deflate()
        .no_brotli()
        .build()
        .map_err(|e| e.to_string())?;

    match provider.as_str() {
        "gemini" => {
            if api_key.is_empty() { return Ok(vec![]); }
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={}&pageSize=50",
                api_key
            );
            let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
            let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
            let json: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;

            let models: Vec<String> = json["models"].as_array().unwrap_or(&vec![]).iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.trim_start_matches("models/").to_string()))
                .filter(|n| !n.contains("embedding") && !n.contains("aqa"))
                .collect();
            Ok(models)
        }
        _ => {
            if base_url.is_empty() { return Ok(vec![]); }
            let mut req = client.get(format!("{}/models", base_url.trim_end_matches('/')));
            if !api_key.is_empty() {
                req = req.header("Authorization", format!("Bearer {}", api_key));
            }
            let resp = req.send().await.map_err(|e| e.to_string())?;
            let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
            let json: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;

            let arr = json["data"].as_array().or_else(|| json["models"].as_array());
            let models: Vec<String> = arr.unwrap_or(&vec![]).iter()
                .filter_map(|m| m["id"].as_str().or_else(|| m["name"].as_str()).map(|s| s.to_string()))
                .collect();
            Ok(models)
        }
    }
}
