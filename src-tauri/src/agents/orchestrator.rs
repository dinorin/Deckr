use serde_json::Value;

use super::{AgentMessage, call_llm};
use crate::settings::AppSettings;
use crate::tools;

// ─── Orchestrator: single call to decide action + extract keywords ─────────────
// No search loop here — search runs externally in lib.rs (parallel workers).

const SYSTEM_PROMPT: &str = r#"You are the Deckr orchestrator. Analyze the user's request and call EXACTLY ONE tool.

## Tools:
- `create_deck` — user wants a NEW presentation (any topic, "make/create/build"). Extract 3–5 specific search keywords.
- `edit_deck` — user wants to MODIFY existing slides ("change/fix/update/redo/add"). 
- `send_message` — user is ONLY chatting or asking questions, NOT requesting a presentation or modifications.

## Rules:
- ALWAYS call a tool. Never respond with plain text.
- Be decisive. If the user asks to "make", "create", or just gives a topic (e.g. "Rolls-Royce") and there is NO existing deck, call `create_deck`.
- If there IS an existing deck and the user gives an instruction or topic, assume they want to `edit_deck` unless they explicitly ask to create a new one. Do not just reply with a chat message saying "I see you have a deck... what do you want to do?". ACT ON IT.
- For create_deck, `keywords` should be 3-5 specific terms to search."#;

pub struct CreateDeckParams {
    pub topic: String,
    pub intent: String,
    pub audience: String,
    pub style_hint: String,
    pub keywords: Vec<String>,
}

pub struct EditDeckParams {
    pub instructions: String,
    pub scope: String,
    pub slide_index: Option<u32>,
    pub language: String,
}

pub enum OrchestratorAction {
    CreateDeck(CreateDeckParams),
    EditDeck(EditDeckParams),
    Message(String),
}

pub struct OrchestratorResult {
    pub action: OrchestratorAction,
    pub coach_message: String,
}

pub async fn run(
    settings: &AppSettings,
    user_message: &str,
    history: &[AgentMessage],
    current_deck: &Option<Value>,
) -> Result<OrchestratorResult, String> {
    let provider = settings.llm.provider.to_lowercase();
    let tools = if provider == "gemini" {
        tools_gemini()
    } else {
        tools_openai()
    };

    // Inject deck context
    let mut history_with_ctx = history.to_vec();
    if let Some(last) = history_with_ctx.last_mut() {
        if last.role == "user" {
            if let Some(d) = current_deck {
                let title = d["title"].as_str().unwrap_or("untitled");
                let count = d["slides"].as_array().map(|a| a.len()).unwrap_or(0);
                last.content = format!(
                    "{}\n\n[SYSTEM CONTEXT: The user currently has an active presentation open titled \"{}\" with {} slides. Apply edits to this deck.]",
                    last.content, title, count
                );
            } else {
                last.content = format!(
                    "{}\n\n[SYSTEM CONTEXT: The user currently DOES NOT have any presentation open. You must create one if they are asking for slides.]",
                    last.content
                );
            }
        }
    }

    let resp = call_llm(settings, SYSTEM_PROMPT, &history_with_ctx, &tools).await?;

    // No function call → treat as message
    if resp.function_calls.is_empty() {
        let msg = resp.text.unwrap_or_else(|| "How can I help you?".to_string());
        return Ok(OrchestratorResult {
            action: OrchestratorAction::Message(msg.clone()),
            coach_message: msg,
        });
    }

    let fc = &resp.function_calls[0];
    let args = &fc.args;

    match fc.name.as_str() {
        "create_deck" => {
            let keywords: Vec<String> = args["keywords"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_else(|| vec![user_message.to_string()]);

            Ok(OrchestratorResult {
                action: OrchestratorAction::CreateDeck(CreateDeckParams {
                    topic: args["topic"].as_str().unwrap_or(user_message).to_string(),
                    intent: args["intent"].as_str().unwrap_or("").to_string(),
                    audience: args["audience"].as_str().unwrap_or("general").to_string(),
                    style_hint: args["style_hint"].as_str().unwrap_or("").to_string(),
                    keywords,
                }),
                coach_message: String::new(),
            })
        }

        "edit_deck" => Ok(OrchestratorResult {
            action: OrchestratorAction::EditDeck(EditDeckParams {
                instructions: args["instructions"].as_str().unwrap_or("").to_string(),
                scope: args["scope"].as_str().unwrap_or("full").to_string(),
                slide_index: args["slide_index"].as_u64().map(|n| n as u32),
                language: detect_language(user_message),
            }),
            coach_message: String::new(),
        }),

        "send_message" => {
            let msg = args["message"].as_str().unwrap_or("").to_string();
            Ok(OrchestratorResult {
                action: OrchestratorAction::Message(msg.clone()),
                coach_message: msg,
            })
        }

        _ => Ok(OrchestratorResult {
            action: OrchestratorAction::CreateDeck(CreateDeckParams {
                topic: user_message.to_string(),
                intent: String::new(),
                audience: "general".to_string(),
                style_hint: String::new(),
                keywords: vec![user_message.to_string()],
            }),
            coach_message: String::new(),
        }),
    }
}

pub fn detect_language_pub(text: &str) -> String {
    detect_language(text)
}

fn detect_language(text: &str) -> String {
    if text.chars().any(|c| matches!(c,
        'à'|'á'|'ả'|'ã'|'ạ'|'ă'|'â'|'è'|'é'|'ê'|'ì'|'í'|'ò'|'ó'|'ô'|'ơ'|'ù'|'ú'|'ư'|'ý'|'đ'
    )) {
        return "Vietnamese".to_string();
    }
    if text.chars().any(|c| c as u32 >= 0x4E00 && c as u32 <= 0x9FFF) {
        return "Chinese".to_string();
    }
    "English".to_string()
}

// ─── Tool definitions ─────────────────────────────────────────────────────────

fn tools_openai() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "create_deck",
                "description": "Create a new presentation. Extract search keywords for research.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "topic": { "type": "string", "description": "Main presentation topic" },
                        "intent": { "type": "string", "description": "What the user wants to achieve" },
                        "audience": { "type": "string" },
                        "style_hint": { "type": "string", "description": "Visual style (dark/minimal/corporate/bold...)" },
                        "keywords": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "3–5 specific search terms to gather research"
                        }
                    },
                    "required": ["topic", "intent", "keywords"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "edit_deck",
                "description": "Edit the existing presentation",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "instructions": { "type": "string" },
                        "scope": { "type": "string", "enum": ["full", "style", "content", "slide"] },
                        "slide_index": { "type": "integer" }
                    },
                    "required": ["instructions", "scope"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "send_message",
                "description": "Respond to user directly, no slide generation",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"]
                }
            }
        }
    ])
}

fn tools_gemini() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "create_deck",
            "description": "Create a new presentation. Extract search keywords for research.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "topic": { "type": "STRING" },
                    "intent": { "type": "STRING" },
                    "audience": { "type": "STRING" },
                    "style_hint": { "type": "STRING" },
                    "keywords": { "type": "ARRAY", "items": { "type": "STRING" } }
                },
                "required": ["topic", "intent", "keywords"]
            }
        },
        {
            "name": "edit_deck",
            "description": "Edit the existing presentation",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "instructions": { "type": "STRING" },
                    "scope": { "type": "STRING" },
                    "slide_index": { "type": "INTEGER" }
                },
                "required": ["instructions", "scope"]
            }
        },
        {
            "name": "send_message",
            "description": "Respond to user directly",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "message": { "type": "STRING" }
                },
                "required": ["message"]
            }
        }
    ])
}
