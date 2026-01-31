//! LLM summarize with sources (OpenAI Chat Completions + json_schema).

use jsonschema::JSONSchema;
use super::{OnlineAnswer, OnlineSource, SearchResult};

const SYSTEM_PROMPT: &str = r#"Ты отвечаешь на вопрос, используя ТОЛЬКО предоставленные источники (вырезки веб-страниц).
Если в источниках нет ответа — скажи, что данных недостаточно, и предложи уточняющий запрос.
В ответе:
- answer_md: кратко и по делу (markdown)
- sources: перечисли 2–5 наиболее релевантных URL, которые реально использовал
- confidence: 0..1 (0.3 если источники слабые/противоречат)
Не выдумывай факты. Не используй знания вне источников."#;

/// Суммаризирует страницы через LLM с response_format json_schema.
pub async fn summarize_with_sources(
    query: &str,
    pages: &[(String, String, String)],
    search_results: &[SearchResult],
) -> Result<OnlineAnswer, String> {
    let api_url = std::env::var("PAPAYU_LLM_API_URL").map_err(|_| "PAPAYU_LLM_API_URL not set")?;
    let api_url = api_url.trim();
    if api_url.is_empty() {
        return Err("PAPAYU_LLM_API_URL is empty".into());
    }
    let model = std::env::var("PAPAYU_ONLINE_MODEL")
        .or_else(|_| std::env::var("PAPAYU_LLM_MODEL"))
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let api_key = std::env::var("PAPAYU_LLM_API_KEY").ok();

    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../config/llm_online_answer_schema.json"))
            .map_err(|e| format!("schema: {}", e))?;

    let mut sources_block = String::new();
    for (i, (url, title, text)) in pages.iter().enumerate() {
        let truncated = if text.len() > 15_000 {
            format!("{}...", &text[..15_000])
        } else {
            text.clone()
        };
        sources_block.push_str(&format!(
            "\n\n--- Источник {}: {} ---\nURL: {}\n\n{}\n",
            i + 1,
            title,
            url,
            truncated
        ));
    }

    let user_content = format!(
        "Вопрос: {}\n\nИспользуй только эти источники для ответа:\n{}",
        query, sources_block
    );

    let response_format = serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": "online_answer",
            "schema": schema,
            "strict": true
        }
    });

    let body = serde_json::json!({
        "model": model.trim(),
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": user_content }
        ],
        "temperature": 0.2,
        "max_tokens": 4096,
        "response_format": response_format
    });

    let timeout_sec = std::env::var("PAPAYU_ONLINE_TIMEOUT_SEC")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(20);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_sec))
        .build()
        .map_err(|e| format!("HTTP: {}", e))?;

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
        return Err(format!("API {}: {}", status, text));
    }

    let chat: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("JSON: {}", e))?;
    let content = chat
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or("No content in response")?;

    let report: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("Report JSON: {}", e))?;

    let compiled = JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .compile(&schema)
        .map_err(|e| format!("Schema: {}", e))?;
    if let Err(e) = compiled.validate(&report) {
        let msg: Vec<String> = e.map(|ve| format!("{}", ve)).collect();
        return Err(format!("Validation: {}", msg.join("; ")));
    }

    let answer_md = report
        .get("answer_md")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let confidence = report.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let notes = report.get("notes").and_then(|v| v.as_str()).map(|s| s.to_string());

    let sources: Vec<OnlineSource> = report
        .get("sources")
        .and_then(|v| v.as_array())
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|s| {
            let url = s.get("url")?.as_str()?.to_string();
            let title = s.get("title")?.as_str().unwrap_or("").to_string();
            let published_at = s.get("published_at").and_then(|v| v.as_str()).map(|s| s.to_string());
            let snippet = s.get("snippet").and_then(|v| v.as_str()).map(|s| s.to_string());
            Some(OnlineSource {
                url,
                title,
                published_at,
                snippet,
            })
        })
        .collect();

    let mut final_sources = sources;
    if final_sources.is_empty() {
        for r in search_results.iter().take(5) {
            final_sources.push(OnlineSource {
                url: r.url.clone(),
                title: r.title.clone(),
                published_at: None,
                snippet: r.snippet.clone(),
            });
        }
    }

    Ok(OnlineAnswer {
        answer_md,
        sources: final_sources,
        confidence,
        notes,
    })
}
