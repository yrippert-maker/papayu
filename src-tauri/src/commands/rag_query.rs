//! Упрощённый RAG: контекст из файлов проекта + вопрос → LLM. Без эмбеддингов, keyword-контекст.

use std::path::Path;

use super::project_content;

const RAG_CONTEXT_CHARS: usize = 80_000;

/// Собирает контекст по проекту и отправляет вопрос в LLM. Возвращает ответ или ошибку.
pub async fn chat_on_project(project_path: &Path, question: &str) -> Result<String, String> {
    if !project_path.exists() || !project_path.is_dir() {
        return Err("Папка проекта не найдена".to_string());
    }
    let context = project_content::get_project_content_for_llm(
        project_path,
        Some(RAG_CONTEXT_CHARS),
    );
    let api_url = std::env::var("PAPAYU_LLM_API_URL").map_err(|_| "PAPAYU_LLM_API_URL не задан".to_string())?;
    let api_url = api_url.trim();
    if api_url.is_empty() {
        return Err("PAPAYU_LLM_API_URL пустой".to_string());
    }
    let api_key = std::env::var("PAPAYU_LLM_API_KEY").ok();
    let model = std::env::var("PAPAYU_LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let timeout_sec = std::env::var("PAPAYU_LLM_TIMEOUT_SEC")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(60);
    let max_tokens = std::env::var("PAPAYU_LLM_MAX_TOKENS")
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(4096);

    let system = "Ты — ассистент по коду проекта. Отвечай кратко по контексту ниже. Если в контексте нет ответа — так и скажи. Язык ответа: русский.";
    let user = format!(
        "Контекст (файлы проекта):\n\n{}\n\nВопрос: {}",
        context.chars().take(120_000).collect::<String>(),
        question
    );

    #[derive(serde::Serialize)]
    struct ChatMessage {
        role: String,
        content: String,
    }
    #[derive(serde::Serialize)]
    struct ChatRequest {
        model: String,
        messages: Vec<ChatMessage>,
        temperature: f32,
        max_tokens: u32,
    }
    #[derive(serde::Deserialize)]
    struct ChatChoice {
        message: ChatMessageResponse,
    }
    #[derive(serde::Deserialize)]
    struct ChatMessageResponse {
        content: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct ChatResponse {
        choices: Option<Vec<ChatChoice>>,
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_sec))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;
    let body = ChatRequest {
        model: model.trim().to_string(),
        messages: vec![
            ChatMessage { role: "system".to_string(), content: system.to_string() },
            ChatMessage { role: "user".to_string(), content: user },
        ],
        temperature: 0.3,
        max_tokens,
    };
    let mut req = client.post(api_url).json(&body);
    if let Some(ref key) = api_key {
        if !key.trim().is_empty() {
            req = req.header("Authorization", format!("Bearer {}", key.trim()));
        }
    }
    let resp = req.send().await.map_err(|e| format!("Request: {}", e))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Response: {}", e))?;
    if !status.is_success() {
        return Err(format!("API {}: {}", status, text.chars().take(300).collect::<String>()));
    }
    let chat: ChatResponse = serde_json::from_str(&text).map_err(|e| format!("JSON: {}", e))?;
    let content = chat
        .choices
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.message.content.as_deref())
        .unwrap_or("")
        .trim();
    if content.is_empty() {
        return Err("Пустой ответ от API".to_string());
    }
    Ok(content.to_string())
}
