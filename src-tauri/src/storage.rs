use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StoredSession {
    pub id: String,
    pub title: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    pub messages: Vec<StoredMessage>,
    #[serde(rename = "deckData")]
    pub deck_data: Option<Value>,
    pub notes: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    #[serde(rename = "slideCount")]
    pub slide_count: usize,
}

fn sessions_dir(app: &AppHandle) -> PathBuf {
    let dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("sessions");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn session_path(app: &AppHandle, id: &str) -> PathBuf {
    sessions_dir(app).join(format!("{}.json", id))
}

fn index_path(app: &AppHandle) -> PathBuf {
    sessions_dir(app).join("index.json")
}

fn read_index(app: &AppHandle) -> Vec<SessionSummary> {
    fs::read_to_string(index_path(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_index(app: &AppHandle, index: &[SessionSummary]) -> Result<(), String> {
    let content = serde_json::to_string_pretty(index).map_err(|e| e.to_string())?;
    fs::write(index_path(app), content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_session(app: AppHandle, session: StoredSession) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    fs::write(session_path(&app, &session.id), content).map_err(|e| e.to_string())?;

    let slide_count = session.deck_data.as_ref()
        .and_then(|d| d["slides"].as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let summary = SessionSummary {
        id: session.id.clone(),
        title: session.title.clone(),
        created_at: session.created_at,
        updated_at: session.updated_at,
        slide_count,
    };

    let mut index = read_index(&app);
    if let Some(pos) = index.iter().position(|s| s.id == session.id) {
        index[pos] = summary;
    } else {
        index.insert(0, summary);
    }
    write_index(&app, &index)
}

#[tauri::command]
pub fn list_sessions(app: AppHandle) -> Vec<SessionSummary> {
    let mut index = read_index(&app);
    index.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    index
}

#[tauri::command]
pub fn load_session(app: AppHandle, id: String) -> Result<StoredSession, String> {
    let content = fs::read_to_string(session_path(&app, &id))
        .map_err(|e| format!("Session not found: {e}"))?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_session(app: AppHandle, id: String) -> Result<(), String> {
    let _ = fs::remove_file(session_path(&app, &id));
    let mut index = read_index(&app);
    index.retain(|s| s.id != id);
    write_index(&app, &index)
}
