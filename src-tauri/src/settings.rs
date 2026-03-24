use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const XOR_SEED: &[u8] = b"deckr\xde\xad\xbe\xef\x13\x37\xc0\xde\xfa\xce\xba\xbe\x00\xff\x42\x69";
pub const MASKED_SENTINEL: &str = "__MASKED__";

fn obfuscate(s: &str) -> String {
    s.bytes()
        .enumerate()
        .map(|(i, b)| format!("{:02x}", b ^ XOR_SEED[i % XOR_SEED.len()]))
        .collect()
}

fn deobfuscate(hex: &str) -> String {
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .enumerate()
        .filter_map(|(i, pos)| {
            if pos + 2 > hex.len() { return None; }
            u8::from_str_radix(&hex[pos..pos + 2], 16)
                .ok()
                .map(|b| b ^ XOR_SEED[i % XOR_SEED.len()])
        })
        .collect();
    String::from_utf8(bytes).unwrap_or_default()
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct LlmProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlmSettings {
    pub provider: String,
    pub configs: HashMap<String, LlmProviderConfig>,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ImageProviderConfig {
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub llm: LlmSettings,
    pub image: HashMap<String, ImageProviderConfig>,
    pub search: HashMap<String, String>,
    pub dark_mode: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        AppSettings {
            llm: LlmSettings {
                provider: "gemini".to_string(),
                configs: HashMap::new(),
                base_url: String::new(),
                api_key: String::new(),
                model: String::new(),
            },
            image: HashMap::new(),
            search: HashMap::new(),
            dark_mode: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredConfig {
    provider: String,
    configs: HashMap<String, LlmProviderConfigSave>,
    #[serde(default)]
    image_configs: HashMap<String, ImageProviderConfigSave>,
    #[serde(default)]
    search_providers: Vec<String>,
    dark_mode: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct LlmProviderConfigSave {
    base_url: String,
    model: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ImageProviderConfigSave {
    model: String,
}

pub fn resolve_api_key(app: &AppHandle, provider: &str, received: &str) -> String {
    if received.trim() == MASKED_SENTINEL {
        keys_read(app).get(provider).cloned().unwrap_or_default()
    } else {
        received.to_string()
    }
}

fn config_dir(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_config_dir().unwrap_or_else(|_| PathBuf::from("."));
    let _ = fs::create_dir_all(&dir);
    dir
}

fn settings_path(app: &AppHandle) -> PathBuf {
    config_dir(app).join("settings.v1.json")
}

fn keys_path(app: &AppHandle) -> PathBuf {
    config_dir(app).join("api.keys")
}

fn keys_read(app: &AppHandle) -> HashMap<String, String> {
    let content = fs::read_to_string(keys_path(app)).unwrap_or_default();
    if content.is_empty() { return HashMap::new(); }
    let deob = deobfuscate(content.trim());
    serde_json::from_str(&deob).unwrap_or_default()
}

fn keys_write(app: &AppHandle, keys: &HashMap<String, String>) -> Result<(), String> {
    let json = serde_json::to_string(keys).map_err(|e| e.to_string())?;
    fs::write(keys_path(app), obfuscate(&json)).map_err(|e| format!("Failed to save API keys: {e}"))
}

pub fn load_settings_raw(app: &AppHandle) -> AppSettings {
    let keys = keys_read(app);
    let stored: Option<StoredConfig> = fs::read_to_string(settings_path(app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    match stored {
        Some(s) => {
            let mut configs = HashMap::new();
            for (id, cfg) in s.configs {
                let real_key = keys.get(&id).cloned().unwrap_or_default();
                configs.insert(id.clone(), LlmProviderConfig {
                    base_url: cfg.base_url,
                    model: cfg.model,
                    api_key: real_key,
                });
            }
            let active = configs.get(&s.provider).cloned().unwrap_or_default();

            let mut image = HashMap::new();
            for (id, cfg) in &s.image_configs {
                let key = keys.get(&format!("img_{id}")).cloned().unwrap_or_default();
                image.insert(id.clone(), ImageProviderConfig {
                    api_key: key,
                    model: cfg.model.clone(),
                });
            }

            let mut search = HashMap::new();
            for id in &s.search_providers {
                let key = keys.get(&format!("search_{id}")).cloned().unwrap_or_default();
                search.insert(id.clone(), key);
            }

            AppSettings {
                llm: LlmSettings {
                    provider: s.provider,
                    configs,
                    base_url: active.base_url,
                    api_key: active.api_key,
                    model: active.model,
                },
                image,
                search,
                dark_mode: s.dark_mode,
            }
        }
        None => AppSettings::default(),
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> AppSettings {
    let keys = keys_read(&app);
    let stored: Option<StoredConfig> = fs::read_to_string(settings_path(&app))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    match stored {
        Some(s) => {
            let mut configs = HashMap::new();
            for (id, cfg) in s.configs {
                let real_key = keys.get(&id).cloned().unwrap_or_default();
                configs.insert(id.clone(), LlmProviderConfig {
                    base_url: cfg.base_url,
                    model: cfg.model,
                    api_key: if real_key.is_empty() { String::new() } else { MASKED_SENTINEL.to_string() },
                });
            }
            let active = configs.get(&s.provider).cloned().unwrap_or_default();

            let mut image = HashMap::new();
            for (id, cfg) in &s.image_configs {
                let real_key = keys.get(&format!("img_{id}")).cloned().unwrap_or_default();
                image.insert(id.clone(), ImageProviderConfig {
                    api_key: if real_key.is_empty() { String::new() } else { MASKED_SENTINEL.to_string() },
                    model: cfg.model.clone(),
                });
            }

            let mut search = HashMap::new();
            for id in &s.search_providers {
                let real_key = keys.get(&format!("search_{id}")).cloned().unwrap_or_default();
                search.insert(id.clone(), if real_key.is_empty() { String::new() } else { MASKED_SENTINEL.to_string() });
            }

            AppSettings {
                llm: LlmSettings { provider: s.provider, configs, base_url: active.base_url, api_key: active.api_key, model: active.model },
                image,
                search,
                dark_mode: s.dark_mode,
            }
        }
        None => AppSettings::default(),
    }
}

#[tauri::command]
pub fn save_settings(app: AppHandle, mut settings: AppSettings) -> Result<(), String> {
    let mut keys = keys_read(&app);
    let mut configs = HashMap::new();

    // 1. Resolve masked LLM keys
    for (id, cfg) in &mut settings.llm.configs {
        if cfg.api_key.trim() == MASKED_SENTINEL {
            if let Some(real_key) = keys.get(id) {
                cfg.api_key = real_key.clone();
            }
        }
    }

    // 2. Sync active provider config
    let active_provider = settings.llm.provider.clone();
    let active_key = if settings.llm.api_key.trim() == MASKED_SENTINEL {
        keys.get(&active_provider).cloned().unwrap_or_default()
    } else {
        settings.llm.api_key.clone()
    };
    settings.llm.configs.insert(active_provider.clone(), LlmProviderConfig {
        base_url: settings.llm.base_url.clone(),
        api_key: active_key,
        model: settings.llm.model.clone(),
    });

    // 3. Process LLM configs
    for (id, cfg) in &settings.llm.configs {
        let key = cfg.api_key.trim();
        if !key.is_empty() && key != MASKED_SENTINEL {
            keys.insert(id.clone(), key.to_string());
        }
        configs.insert(id.clone(), LlmProviderConfigSave {
            base_url: cfg.base_url.clone(),
            model: cfg.model.clone(),
        });
    }

    // 4. Save image provider configs
    let mut image_configs: HashMap<String, ImageProviderConfigSave> = HashMap::new();
    for (id, cfg) in &settings.image {
        let k = cfg.api_key.trim();
        if !k.is_empty() && k != MASKED_SENTINEL {
            keys.insert(format!("img_{id}"), k.to_string());
        }
        // Keep entry if key exists in store or model is set
        let has_stored_key = keys.contains_key(&format!("img_{id}"));
        if has_stored_key || !cfg.model.is_empty() {
            image_configs.insert(id.clone(), ImageProviderConfigSave {
                model: cfg.model.clone(),
            });
        }
    }

    // 5. Save search provider keys
    let mut search_providers: Vec<String> = Vec::new();
    for (id, key) in &settings.search {
        let k = key.trim();
        if !k.is_empty() && k != MASKED_SENTINEL {
            keys.insert(format!("search_{id}"), k.to_string());
        }
        if keys.contains_key(&format!("search_{id}")) {
            search_providers.push(id.clone());
        }
    }

    // 6. Persist
    keys_write(&app, &keys)?;

    let stored = StoredConfig {
        provider: active_provider,
        configs,
        image_configs,
        search_providers,
        dark_mode: settings.dark_mode,
    };

    let content = serde_json::to_string_pretty(&stored).map_err(|e| e.to_string())?;
    fs::write(settings_path(&app), content).map_err(|e| e.to_string())
}
