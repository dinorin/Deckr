pub mod orchestrator;
pub mod content_agent;
pub mod animation_agent;
pub mod html_agent;
pub mod edit_agent;
pub mod lint;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use crate::settings::AppSettings;

// ─── Shared Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SlideOutline {
    pub index: usize,
    #[serde(rename = "type")]
    pub slide_type: String,
    pub title: String,
    pub bullets: Vec<String>,
    pub notes: String,
    pub transition: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ElementAnimationPlan {
    pub element_type: String,
    pub content: String,
    pub animation: String,
    pub click_order: u32,
    pub duration_ms: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub font_size: u32,
    pub bold: bool,
    pub italic: bool,
    pub color: String,
    pub align: String,
    pub font_family: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SlideAnimationPlan {
    pub index: usize,
    pub bg_color: String,
    pub transition: String,
    pub elements: Vec<ElementAnimationPlan>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeckTheme {
    pub primary_color: String,
    pub secondary_color: String,
    pub bg_color: String,
    pub text_color: String,
    pub accent_color: String,
    pub font_family: String,
    pub style: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct AgentContext {
    pub topic: String,
    pub language: String,
    pub intent: String,
    pub audience: String,
    pub slide_count: usize,
    pub style_hint: String,
    pub web_research: String,
    pub image_refs: Vec<String>,
    pub slide_outline: Vec<SlideOutline>,
    pub theme: Option<DeckTheme>,
    pub animation_plan: Vec<SlideAnimationPlan>,
    pub current_deck: Option<Value>,
    pub edit_instructions: String,
    pub is_edit: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneratedDeck {
    pub title: String,
    pub theme: Value,
    pub slides: Vec<GeneratedSlide>,
    /// Single self-contained HTML file with all slides + CSS + JS
    pub master_html: String,
    pub coach_message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneratedSlide {
    pub id: String,
    #[serde(rename = "type")]
    pub slide_type: String,
    pub html: String,
}

// ─── Shared LLM Client ────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlmFunctionCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TokenUsage {
    pub prompt: i64,
    pub completion: i64,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlmResponse {
    pub text: Option<String>,
    pub function_calls: Vec<LlmFunctionCall>,
    pub token_usage: TokenUsage,
}

pub async fn call_llm(
    settings: &AppSettings,
    system_prompt: &str,
    history: &[AgentMessage],
    tools: &Value,
) -> Result<LlmResponse, String> {
    let provider = settings.llm.provider.to_lowercase();
    match provider.as_str() {
        "gemini" => call_gemini(settings, system_prompt, history, tools).await,
        _ => call_openai_compat(settings, system_prompt, history, tools).await,
    }
}

fn make_client(timeout_secs: u64) -> Result<reqwest::Client, String> {
    // Use default client — reqwest handles decompression automatically
    // Do NOT use no_gzip/no_deflate/no_brotli; those prevent auto-decompression
    // which causes "error decoding response body" on large responses.
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .pool_max_idle_per_host(0)
        .build()
        .map_err(|e: reqwest::Error| e.to_string())
}

async fn call_gemini(
    settings: &AppSettings,
    sys: &str,
    history: &[AgentMessage],
    tools: &Value,
) -> Result<LlmResponse, String> {
    let client = make_client(300)?;

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        settings.llm.model, settings.llm.api_key
    );

    let contents: Vec<Value> = history.iter().map(|m| {
        let role = if m.role == "user" { "user" } else { "model" };
        json!({ "role": role, "parts": [{"text": &m.content}] })
    }).collect();

    // Only include function declarations when tools is non-empty array
    let has_tools = tools.as_array().map(|a| !a.is_empty()).unwrap_or(false);
    let body = if has_tools {
        json!({
            "contents": contents,
            "system_instruction": { "parts": [{"text": sys}] },
            "tools": [{ "function_declarations": tools }],
            "generationConfig": { "temperature": 0.8, "maxOutputTokens": 8192 }
        })
    } else {
        json!({
            "contents": contents,
            "system_instruction": { "parts": [{"text": sys}] },
            "generationConfig": { "temperature": 0.8, "maxOutputTokens": 8192 }
        })
    };

    let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;

    let resp = client.post(&url)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send().await
        .map_err(|e| format!("Network: {}", e))?;

    let status = resp.status();
    let text = resp.text().await
        .map_err(|e| format!("Read error: {}", e))?;

    if !status.is_success() {
        return Err(format!("Gemini HTTP {}: {}", status, &text[..text.len().min(500)]));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Gemini parse error: {}. Body: {}", e, &text[..text.len().min(300)]))?;

    if let Some(err) = json.get("error") {
        return Err(format!("Gemini error: {}", err["message"].as_str().unwrap_or("Unknown")));
    }

    let candidate = &json["candidates"][0];
    if candidate.is_null() {
        return Err(format!("Gemini no candidates. Body: {}", &text[..text.len().min(400)]));
    }

    let mut text_out = None;
    let mut function_calls = Vec::new();

    if let Some(parts) = candidate["content"]["parts"].as_array() {
        for part in parts {
            if let Some(t) = part["text"].as_str() {
                if !t.is_empty() { text_out = Some(t.to_string()); }
            }
            if let Some(fc) = part.get("functionCall") {
                function_calls.push(LlmFunctionCall {
                    name: fc["name"].as_str().unwrap_or("").to_string(),
                    args: fc["args"].clone(),
                });
            }
        }
    }

    let mut token_usage = TokenUsage::default();
    if let Some(usage) = json.get("usageMetadata") {
        token_usage.prompt = usage["promptTokenCount"].as_i64().unwrap_or(0);
        token_usage.completion = usage["candidatesTokenCount"].as_i64().unwrap_or(0);
        token_usage.total = usage["totalTokenCount"].as_i64().unwrap_or(0);
        println!("\n📊 [Gemini Token Usage] Prompt: {}, Completion: {}, Total: {}\n", token_usage.prompt, token_usage.completion, token_usage.total);
    }

    Ok(LlmResponse { text: text_out, function_calls, token_usage })
}

async fn call_openai_compat(
    settings: &AppSettings,
    sys: &str,
    history: &[AgentMessage],
    tools: &Value,
) -> Result<LlmResponse, String> {
    let client = make_client(300)?;

    let url = format!("{}/chat/completions", settings.llm.base_url.trim_end_matches('/'));

    let mut messages = vec![json!({"role": "system", "content": sys})];
    for m in history {
        messages.push(json!({"role": m.role, "content": m.content}));
    }

    let has_tools = tools.as_array().map(|a| !a.is_empty()).unwrap_or(false);
    let body = if has_tools {
        json!({
            "model": settings.llm.model,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto",
            "max_tokens": 8192
        })
    } else {
        json!({
            "model": settings.llm.model,
            "messages": messages,
            "max_tokens": 8192
        })
    };

    let body_str = serde_json::to_string(&body).map_err(|e| e.to_string())?;

    let resp = client.post(&url)
        .header("Authorization", format!("Bearer {}", settings.llm.api_key))
        .header("Content-Type", "application/json")
        .body(body_str)
        .send().await
        .map_err(|e| format!("Network: {}", e))?;

    let status = resp.status();
    let text = resp.text().await
        .map_err(|e| format!("Read error: {}", e))?;

    if !status.is_success() {
        return Err(format!("HTTP {}: {}", status, &text[..text.len().min(500)]));
    }

    let json: Value = serde_json::from_str(&text)
        .map_err(|e| format!("Parse: {}. Body: {}", e, &text[..text.len().min(300)]))?;

    if let Some(err) = json.get("error") {
        return Err(format!("API error: {}", err["message"].as_str().unwrap_or("Unknown")));
    }

    let msg = &json["choices"][0]["message"];
    if msg.is_null() {
        return Err(format!("No choices in response. Body: {}", &text[..text.len().min(400)]));
    }

    let text_content = msg["content"].as_str().map(|s| s.to_string());
    let mut function_calls = Vec::new();

    if let Some(tool_calls) = msg["tool_calls"].as_array() {
        for tc in tool_calls {
            let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
            let args: Value = serde_json::from_str(
                tc["function"]["arguments"].as_str().unwrap_or("{}")
            ).unwrap_or(json!({}));
            function_calls.push(LlmFunctionCall { name, args });
        }
    }

    let mut token_usage = TokenUsage::default();
    if let Some(usage) = json.get("usage") {
        token_usage.prompt = usage["prompt_tokens"].as_i64().unwrap_or(0);
        token_usage.completion = usage["completion_tokens"].as_i64().unwrap_or(0);
        token_usage.total = usage["total_tokens"].as_i64().unwrap_or(0);
        println!("\n📊 [OpenAI Token Usage] Prompt: {}, Completion: {}, Total: {}\n", token_usage.prompt, token_usage.completion, token_usage.total);
    }

    Ok(LlmResponse { text: text_content, function_calls, token_usage })
}
