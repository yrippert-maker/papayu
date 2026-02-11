use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmRequest {
    pub provider: String,       // "openai" | "anthropic" | "ollama"
    pub model: String,          // "gpt-4o" | "claude-sonnet-4-20250514" | "llama3"
    pub api_key: Option<String>,
    pub base_url: Option<String>, // for Ollama: http://localhost:11434
    pub context: String,        // llm_context JSON
    pub prompt: String,         // user question or system prompt
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmResponse {
    pub ok: bool,
    pub content: String,
    pub model: String,
    pub usage: Option<LlmUsage>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LlmUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ---- OpenAI-compatible request/response ----

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Option<Vec<OpenAiChoice>>,
    usage: Option<OpenAiUsage>,
    error: Option<OpenAiError>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Deserialize)]
struct OpenAiError {
    message: String,
}

// ---- Anthropic request/response ----

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Option<Vec<AnthropicContent>>,
    usage: Option<AnthropicUsage>,
    error: Option<AnthropicError>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Deserialize)]
struct AnthropicError {
    message: String,
}

#[tauri::command]
pub async fn ask_llm(request: LlmRequest) -> Result<LlmResponse, String> {
    let api_key = request.api_key.clone().unwrap_or_default();
    if api_key.is_empty() && request.provider != "ollama" {
        return Ok(LlmResponse {
            ok: false,
            content: String::new(),
            model: request.model.clone(),
            usage: None,
            error: Some("API-ключ не указан. Откройте Настройки → LLM.".into()),
        });
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP client error: {e}"))?;

    match request.provider.as_str() {
        "openai" => call_openai(&client, &request, &api_key).await,
        "anthropic" => call_anthropic(&client, &request, &api_key).await,
        "ollama" => call_ollama(&client, &request).await,
        other => Ok(LlmResponse {
            ok: false,
            content: String::new(),
            model: request.model.clone(),
            usage: None,
            error: Some(format!("Неизвестный провайдер: {other}")),
        }),
    }
}

async fn call_openai(
    client: &reqwest::Client,
    req: &LlmRequest,
    api_key: &str,
) -> Result<LlmResponse, String> {
    let url = req
        .base_url
        .clone()
        .unwrap_or_else(|| "https://api.openai.com/v1/chat/completions".into());

    let body = OpenAiRequest {
        model: req.model.clone(),
        messages: vec![
            OpenAiMessage {
                role: "system".into(),
                content: build_system_prompt(&req.context),
            },
            OpenAiMessage {
                role: "user".into(),
                content: req.prompt.clone(),
            },
        ],
        max_tokens: req.max_tokens.unwrap_or(2048),
        temperature: 0.3,
    };

    let resp = client
        .post(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("OpenAI request failed: {e}"))?;

    let data: OpenAiResponse = resp
        .json()
        .await
        .map_err(|e| format!("OpenAI parse error: {e}"))?;

    if let Some(err) = data.error {
        return Ok(LlmResponse {
            ok: false,
            content: String::new(),
            model: req.model.clone(),
            usage: None,
            error: Some(err.message),
        });
    }

    let content = data
        .choices
        .and_then(|c| c.into_iter().next())
        .map(|c| c.message.content)
        .unwrap_or_default();

    let usage = data.usage.map(|u| LlmUsage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    });

    Ok(LlmResponse {
        ok: true,
        content,
        model: req.model.clone(),
        usage,
        error: None,
    })
}

async fn call_anthropic(
    client: &reqwest::Client,
    req: &LlmRequest,
    api_key: &str,
) -> Result<LlmResponse, String> {
    let url = "https://api.anthropic.com/v1/messages";

    let body = AnthropicRequest {
        model: req.model.clone(),
        max_tokens: req.max_tokens.unwrap_or(2048),
        system: build_system_prompt(&req.context),
        messages: vec![AnthropicMessage {
            role: "user".into(),
            content: req.prompt.clone(),
        }],
    };

    let resp = client
        .post(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Anthropic request failed: {e}"))?;

    let data: AnthropicResponse = resp
        .json()
        .await
        .map_err(|e| format!("Anthropic parse error: {e}"))?;

    if let Some(err) = data.error {
        return Ok(LlmResponse {
            ok: false,
            content: String::new(),
            model: req.model.clone(),
            usage: None,
            error: Some(err.message),
        });
    }

    let content = data
        .content
        .and_then(|c| c.into_iter().next())
        .map(|c| c.text)
        .unwrap_or_default();

    let usage = data.usage.map(|u| LlmUsage {
        prompt_tokens: u.input_tokens,
        completion_tokens: u.output_tokens,
        total_tokens: u.input_tokens + u.output_tokens,
    });

    Ok(LlmResponse {
        ok: true,
        content,
        model: req.model.clone(),
        usage,
        error: None,
    })
}

async fn call_ollama(
    client: &reqwest::Client,
    req: &LlmRequest,
) -> Result<LlmResponse, String> {
    let base = req
        .base_url
        .clone()
        .unwrap_or_else(|| "http://localhost:11434".into());
    let url = format!("{base}/api/chat");

    let mut body = HashMap::new();
    body.insert("model", serde_json::json!(req.model));
    body.insert("stream", serde_json::json!(false));
    body.insert(
        "messages",
        serde_json::json!([
            {
                "role": "system",
                "content": build_system_prompt(&req.context)
            },
            {
                "role": "user",
                "content": req.prompt
            }
        ]),
    );

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {e}. Убедитесь что Ollama запущен."))?;

    let data: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Ollama parse error: {e}"))?;

    if let Some(err) = data.get("error").and_then(|e| e.as_str()) {
        return Ok(LlmResponse {
            ok: false,
            content: String::new(),
            model: req.model.clone(),
            usage: None,
            error: Some(err.to_string()),
        });
    }

    let content = data
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();

    Ok(LlmResponse {
        ok: true,
        content,
        model: req.model.clone(),
        usage: None,
        error: None,
    })
}

fn build_system_prompt(context_json: &str) -> String {
    format!(
        r#"Ты — PAPA YU, AI-аудитор программных проектов. Тебе предоставлен контекст анализа проекта в формате JSON.

На основе этого контекста ты должен:
1. Дать краткое, понятное резюме состояния проекта
2. Выделить критичные проблемы безопасности (если есть)
3. Предложить конкретные шаги по улучшению (приоритезированные)
4. Оценить общее качество и зрелость проекта

Отвечай на русском. Будь конкретен — называй файлы, пути, технологии. Избегай общих фраз.

Контекст проекта:
{context_json}"#
    )
}
