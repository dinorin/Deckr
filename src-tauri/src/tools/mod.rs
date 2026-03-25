use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

fn safe_trunc(s: &str, max_bytes: usize) -> &str {
    let mut end = s.len().min(max_bytes);
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}

// ─── Image Result ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageResult {
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: f32,
    pub description: String,
    /// Average color of the image as hex, e.g. "#1a2b3c"
    pub avg_color: String,
    /// Recommended text color for contrast: "#ffffff" or "#000000"
    pub text_color: String,
}

impl ImageResult {
    fn new(url: String, width: u32, height: u32, description: String) -> Self {
        Self::new_with_color(url, width, height, description, None)
    }

    fn new_with_color(
        url: String,
        width: u32,
        height: u32,
        description: String,
        avg: Option<(u8, u8, u8)>,
    ) -> Self {
        let aspect_ratio = width as f32 / height as f32;
        let (avg_color, text_color) = match avg {
            Some((r, g, b)) => {
                let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
                // Perceived luminance
                let lum = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
                let txt = if lum > 140.0 { "#000000" } else { "#ffffff" };
                (hex, txt.to_string())
            }
            None => ("#111111".to_string(), "#ffffff".to_string()),
        };
        Self { url, width, height, aspect_ratio, description, avg_color, text_color }
    }
}

// ─── Average Color ────────────────────────────────────────────────────────────

fn average_color(bytes: &[u8]) -> Option<(u8, u8, u8)> {
    use image::GenericImageView;
    let img = image::load_from_memory(bytes).ok()?;
    let (w, h) = img.dimensions();
    let total = w * h;
    let step = (total / 600).max(1);
    let (mut rs, mut gs, mut bs, mut n) = (0u64, 0u64, 0u64, 0u64);
    for (x, y, p) in img.pixels() {
        if (y * w + x) % step == 0 {
            rs += p[0] as u64;
            gs += p[1] as u64;
            bs += p[2] as u64;
            n  += 1;
        }
    }
    if n == 0 { return None; }
    Some(((rs / n) as u8, (gs / n) as u8, (bs / n) as u8))
}

async fn fetch_avg_color(client: &reqwest::Client, url: &str) -> Option<(u8, u8, u8)> {
    let bytes = client.get(url)
        .timeout(Duration::from_secs(6))
        .send().await.ok()?
        .bytes().await.ok()?;
    average_color(&bytes)
}

// ─── Tavily Search ────────────────────────────────────────────────────────────

/// (text_summary, image_urls, web_links as (title, url))
pub async fn tavily_search(query: &str, api_key: &str) -> Result<(String, Vec<String>, Vec<(String, String)>), String> {
    if api_key.is_empty() {
        return Ok((format!("(No API key for research on: {})", query), vec![], vec![]));
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let body = serde_json::json!({
        "api_key": api_key,
        "query": query,
        "search_depth": "basic",
        "max_results": 3,
        "include_answer": true,
        "include_images": true
    });

    let resp = match client
        .post("https://api.tavily.com/search")
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body).unwrap())
        .send().await {
            Ok(r) => r,
            Err(e) => return Ok((format!("(Search timeout/error for: {}. Error: {})", query, e), vec![], vec![])),
        };

    let text = resp.text().await.unwrap_or_default();
    let json: Value = serde_json::from_str(&text).unwrap_or(json!({}));

    let mut result = String::new();
    let mut links: Vec<(String, String)> = Vec::new();

    if let Some(answer) = json["answer"].as_str() {
        if !answer.is_empty() {
            result.push_str(&format!("Answer: {}\n\n", answer));
        }
    }

    if let Some(results) = json["results"].as_array() {
        for (i, r) in results.iter().take(2).enumerate() {
            let title = r["title"].as_str().unwrap_or("");
            let url   = r["url"].as_str().unwrap_or("");
            let content = r["content"].as_str().unwrap_or("");
            if !content.is_empty() {
                result.push_str(&format!("{}. {}\n{}\n\n", i + 1, title, safe_trunc(content, 150)));
            }
            if !url.is_empty() && !title.is_empty() {
                links.push((title.to_string(), url.to_string()));
            }
        }
    }

    let mut image_urls = Vec::new();
    if let Some(images) = json["images"].as_array() {
        for img_val in images {
            if let Some(url) = img_val.as_str() {
                 image_urls.push(url.to_string());
            } else if let Some(url) = img_val["url"].as_str() {
                 image_urls.push(url.to_string());
            }
        }
    }

    if result.is_empty() {
        return Ok((format!("(No results found for: {})", query), image_urls, links));
    }
    Ok((result, image_urls, links))
}

/// Run multiple Tavily searches in parallel, return aggregated (text, images, links).
pub async fn parallel_tavily_search(queries: Vec<String>, api_key: &str) -> (String, Vec<String>, Vec<(String, String)>) {
    if api_key.is_empty() || queries.is_empty() {
        return (String::new(), vec![], vec![]);
    }
    let futs: Vec<_> = queries.iter().take(2).map(|q| {
        let q = q.clone();
        let key = api_key.to_string();
        async move {
            match tavily_search(&q, &key).await {
                Ok((r, imgs, lnks)) => (format!("### {}\n{}", q, r), imgs, lnks),
                Err(_) => (String::new(), vec![], vec![]),
            }
        }
    }).collect();

    let results = futures::future::join_all(futs).await;
    let mut combined_text = String::new();
    let mut combined_images = Vec::new();
    let mut combined_links: Vec<(String, String)> = Vec::new();

    for (txt, mut imgs, mut lnks) in results {
        if !txt.is_empty() {
            if !combined_text.is_empty() { combined_text.push('\n'); }
            combined_text.push_str(&txt);
        }
        combined_images.append(&mut imgs);
        combined_links.append(&mut lnks);
    }
    combined_images.sort();
    combined_images.dedup();

    (combined_text, combined_images, combined_links)
}

// ─── Web Search ───────────────────────────────────────────────────────────────
// DuckDuckGo removed per request.

// ─── Image Search Fallback ───────────────────────────────────────────────────

pub fn picsum_fallback(query: &str) -> Result<Vec<ImageResult>, String> {
    let seed: u32 = query.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    let images = (0u32..6).map(|i| {
        let s = seed.wrapping_add(i * 137) % 1000;
        let (w, h) = if i % 2 == 0 { (1920u32, 1080u32) } else { (1280u32, 720u32) };
        let url = format!("https://picsum.photos/seed/{}/{}/{}", s, w, h);
        ImageResult::new(url, w, h, format!("{} photo {}", query, i + 1))
    }).collect();
    Ok(images)
}

// ─── AI Image Generation ─────────────────────────────────────────────────────

pub async fn generate_ai_image_with_provider(
    prompt: &str,
    provider: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    match provider {
        "together"   => generate_image_together(prompt, api_key, model).await,
        "fal"        => generate_image_fal(prompt, api_key, model).await,
        "openai_img" => generate_image_openai(prompt, api_key, model).await,
        "getimg"     => generate_image_getimg(prompt, api_key, model).await,
        _            => Err(format!("Unsupported image provider: {}", provider)),
    }
}

async fn generate_image_together(prompt: &str, api_key: &str, model: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60)).build().map_err(|e| e.to_string())?;
    let model = if model.is_empty() { "black-forest-labs/FLUX.1-schnell-Free" } else { model };
    let body = serde_json::to_string(&json!({"model": model, "prompt": prompt, "n": 1, "width": 1024, "height": 576})).unwrap();
    let text = client
        .post("https://api.together.xyz/v1/images/generations")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .body(body)
        .send().await.map_err(|e| format!("Together AI: {}", e))?
        .text().await.map_err(|e| format!("Together AI read: {}", e))?;
    let resp: Value = serde_json::from_str(&text).map_err(|e| format!("Together AI parse: {}", e))?;
    resp["data"][0]["url"].as_str().map(|s| s.to_string())
        .ok_or_else(|| format!("Together AI no URL. Response: {}", resp))
}

async fn generate_image_fal(prompt: &str, api_key: &str, model: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90)).build().map_err(|e| e.to_string())?;
    let model = if model.is_empty() { "fal-ai/flux/schnell" } else { model };
    let url = format!("https://fal.run/{}", model);
    let body = serde_json::to_string(&json!({"prompt": prompt, "image_size": "landscape_16_9", "num_images": 1})).unwrap();
    let text = client
        .post(&url)
        .header("Authorization", format!("Key {}", api_key))
        .header("Content-Type", "application/json")
        .body(body)
        .send().await.map_err(|e| format!("Fal.ai: {}", e))?
        .text().await.map_err(|e| format!("Fal.ai read: {}", e))?;
    let resp: Value = serde_json::from_str(&text).map_err(|e| format!("Fal.ai parse: {}", e))?;
    resp["images"][0]["url"].as_str().map(|s| s.to_string())
        .ok_or_else(|| format!("Fal.ai no URL. Response: {}", resp))
}

async fn generate_image_openai(prompt: &str, api_key: &str, model: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60)).build().map_err(|e| e.to_string())?;
    let model = if model.is_empty() { "dall-e-3" } else { model };
    let body = serde_json::to_string(&json!({"model": model, "prompt": prompt, "n": 1, "size": "1792x1024", "response_format": "url"})).unwrap();
    let text = client
        .post("https://api.openai.com/v1/images/generations")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .body(body)
        .send().await.map_err(|e| format!("OpenAI: {}", e))?
        .text().await.map_err(|e| format!("OpenAI read: {}", e))?;
    let resp: Value = serde_json::from_str(&text).map_err(|e| format!("OpenAI parse: {}", e))?;
    resp["data"][0]["url"].as_str().map(|s| s.to_string())
        .ok_or_else(|| format!("OpenAI no URL. Response: {}", resp))
}

async fn generate_image_getimg(prompt: &str, api_key: &str, _model: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60)).build().map_err(|e| e.to_string())?;
    let body = serde_json::to_string(&json!({"prompt": prompt, "width": 1024, "height": 576, "output_format": "jpeg"})).unwrap();
    let text = client
        .post("https://api.getimg.ai/v1/flux-schnell/text-to-image")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .body(body)
        .send().await.map_err(|e| format!("GetImg: {}", e))?
        .text().await.map_err(|e| format!("GetImg read: {}", e))?;
    let resp: Value = serde_json::from_str(&text).map_err(|e| format!("GetImg parse: {}", e))?;
    if let Some(b64) = resp["image"].as_str() {
        return Ok(format!("data:image/jpeg;base64,{}", b64));
    }
    Err(format!("GetImg no image. Response: {}", resp))
}

// ─── Image Validation + Color Analysis ───────────────────────────────────────

/// Fetch each URL in parallel, skip broken ones, compute real avg_color/text_color.
/// Returns only valid images, preserving order of successful fetches.
pub async fn validate_and_build_images(urls: Vec<String>) -> Vec<ImageResult> {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(6))
        .build() {
            Ok(c) => std::sync::Arc::new(c),
            Err(_) => return vec![],
        };

    let futs: Vec<_> = urls.into_iter().enumerate().map(|(i, url)| {
        let client = client.clone();
        async move {
            let resp = client.get(&url).send().await.ok()?;
            // Filter non-image content types and error status codes
            let status = resp.status();
            if !status.is_success() { return None; }
            let ct = resp.headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();
            if !ct.starts_with("image/") { return None; }

            let bytes = resp.bytes().await.ok()?;
            // Reject tiny files (likely error pages or 1×1 placeholders)
            if bytes.len() < 1024 { return None; }

            let avg = average_color(&bytes);

            // Get real dimensions from the decoded image
            let (w, h) = {
                use image::GenericImageView;
                image::load_from_memory(&bytes)
                    .ok()
                    .map(|img| img.dimensions())
                    .unwrap_or((1280, 720))
            };

            let desc = format!("Image {}", i + 1);
            Some(ImageResult::new_with_color(url, w, h, desc, avg))
        }
    }).collect();

    let results = futures::future::join_all(futs).await;
    results.into_iter().flatten().collect()
}

// ─── Tool Definitions for Orchestrator ───────────────────────────────────────

pub fn orchestrator_tools_openai() -> Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "web_search",
                "description": "Search the web for information to enhance the presentation",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "image_search",
                "description": "Search for relevant images via Google/DuckDuckGo. Returns real photos with name, URL, dimensions, average color, and recommended text color.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Image search query" }
                    },
                    "required": ["query"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "create_deck",
                "description": "Start creating a new presentation from scratch with all agents",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "topic": { "type": "string" },
                        "intent": { "type": "string", "description": "What the user wants to achieve" },
                        "audience": { "type": "string" },
                        "slide_count": { "type": "integer" },
                        "language": { "type": "string" },
                        "style_hint": { "type": "string", "description": "Visual style preferences" },
                        "web_research": { "type": "string", "description": "Relevant info from web search" },
                        "image_refs": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Image URLs to use in slides"
                        }
                    },
                    "required": ["topic", "intent", "language", "slide_count"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "edit_deck",
                "description": "Edit specific aspects of the existing presentation",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "instructions": { "type": "string", "description": "What to change and how" },
                        "scope": {
                            "type": "string",
                            "enum": ["full", "style", "content", "slide"],
                            "description": "full=rebuild all, style=only visuals, content=only text, slide=specific slide"
                        },
                        "slide_index": { "type": "integer", "description": "0-based index if scope=slide" }
                    },
                    "required": ["instructions", "scope"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "send_message",
                "description": "Respond to the user directly without generating slides",
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

pub fn orchestrator_tools_gemini() -> Value {
    json!([
        {
            "name": "web_search",
            "description": "Search the web for information",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "image_search",
            "description": "Search for relevant images. Returns real photos with name, URL, avg color, text color.",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "query": { "type": "STRING" }
                },
                "required": ["query"]
            }
        },
        {
            "name": "create_deck",
            "description": "Start creating a new presentation",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "topic": { "type": "STRING" },
                    "intent": { "type": "STRING" },
                    "audience": { "type": "STRING" },
                    "slide_count": { "type": "INTEGER" },
                    "language": { "type": "STRING" },
                    "style_hint": { "type": "STRING" },
                    "web_research": { "type": "STRING" },
                    "image_refs": { "type": "ARRAY", "items": { "type": "STRING" } }
                },
                "required": ["topic", "intent", "language", "slide_count"]
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
            "description": "Respond to the user directly",
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
