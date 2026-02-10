//! Distill OnlineAnswer into a short domain note via LLM (topic, tags, content_md).

use jsonschema::JSONSchema;
use serde::Deserialize;

use super::storage::{
    load_domain_notes, notes_max_chars_per_note, notes_ttl_days, save_domain_notes, DomainNote,
    NoteSource,
};
use std::path::Path;

const DISTILL_SYSTEM_PROMPT: &str = r#"Сожми текст до 5–10 буллетов, только факты из источников, без воды.
Максимум 800 символов в content_md. topic — короткая тема (до 10 слов). tags — до 8 ключевых слов (python, testing, api и т.д.).
confidence — от 0 до 1 по надёжности источников. Не выдумывай."#;

#[derive(Debug, Deserialize)]
struct DistillOutput {
    topic: String,
    tags: Vec<String>,
    content_md: String,
    confidence: f64,
}

/// Distills answer_md + sources into a short note via LLM, then appends to project notes and saves.
pub async fn distill_and_save_note(
    project_path: &Path,
    query: &str,
    answer_md: &str,
    sources: &[(String, String)],
    _confidence: f64,
) -> Result<DomainNote, String> {
    let max_chars = notes_max_chars_per_note();
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../config/llm_domain_note_schema.json"))
            .map_err(|e| format!("schema: {}", e))?;

    let sources_block = sources
        .iter()
        .take(10)
        .map(|(url, title)| format!("- {}: {}", title, url))
        .collect::<Vec<_>>()
        .join("\n");

    let user_content = format!(
        "Запрос: {}\n\nОтвет (сжать):\n{}\n\nИсточники:\n{}\n\nВерни topic, tags (до 8), content_md (макс. {} символов), confidence (0-1).",
        query,
        if answer_md.len() > 4000 {
            format!("{}...", &answer_md[..4000])
        } else {
            answer_md.to_string()
        },
        sources_block,
        max_chars
    );

    let response_format = serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": "domain_note",
            "schema": schema,
            "strict": true
        }
    });

    let api_url = std::env::var("PAPAYU_LLM_API_URL").map_err(|_| "PAPAYU_LLM_API_URL not set")?;
    let api_url = api_url.trim();
    if api_url.is_empty() {
        return Err("PAPAYU_LLM_API_URL is empty".into());
    }
    let model = std::env::var("PAPAYU_ONLINE_MODEL")
        .or_else(|_| std::env::var("PAPAYU_LLM_MODEL"))
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let api_key = std::env::var("PAPAYU_LLM_API_KEY").ok();

    let body = serde_json::json!({
        "model": model.trim(),
        "messages": [
            { "role": "system", "content": DISTILL_SYSTEM_PROMPT },
            { "role": "user", "content": user_content }
        ],
        "temperature": 0.2,
        "max_tokens": 1024,
        "response_format": response_format
    });

    let timeout_sec = std::env::var("PAPAYU_LLM_TIMEOUT_SEC")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(30);
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

    let out: DistillOutput = serde_json::from_value(report).map_err(|e| format!("Parse: {}", e))?;

    let content_md = if out.content_md.chars().count() > max_chars {
        out.content_md.chars().take(max_chars).collect::<String>() + "..."
    } else {
        out.content_md
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let id = format!("note_{}_{:03}", now, (now % 1000).unsigned_abs());

    let note_sources: Vec<NoteSource> = sources
        .iter()
        .take(10)
        .map(|(url, title)| NoteSource {
            url: url.clone(),
            title: title.clone(),
        })
        .collect();

    let note = DomainNote {
        id: id.clone(),
        created_at: now,
        topic: out.topic,
        tags: out.tags.into_iter().take(8).collect(),
        content_md,
        sources: note_sources,
        confidence: out.confidence,
        ttl_days: notes_ttl_days(),
        usage_count: 0,
        last_used_at: None,
        pinned: false,
    };

    let mut data = load_domain_notes(project_path);
    data.notes.push(note.clone());
    save_domain_notes(project_path, data)?;

    Ok(note)
}
