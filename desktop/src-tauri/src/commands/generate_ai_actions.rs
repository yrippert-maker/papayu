use crate::types::{Action, ActionKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateActionsRequest {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub context: String,        // llm_context JSON
    pub findings_json: String,  // findings array JSON
    pub project_path: String,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateActionsResponse {
    pub ok: bool,
    pub actions: Vec<Action>,
    pub explanation: String,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LlmActionsOutput {
    actions: Vec<LlmAction>,
    explanation: String,
}

#[derive(Debug, Deserialize)]
struct LlmAction {
    id: String,
    title: String,
    description: String,
    kind: String,       // "create_file" | "update_file" | "create_dir"
    path: String,
    content: Option<String>,
}

#[tauri::command]
pub async fn generate_ai_actions(
    request: GenerateActionsRequest,
) -> Result<GenerateActionsResponse, String> {
    let api_key = request.api_key.clone().unwrap_or_default();
    if api_key.is_empty() && request.provider != "ollama" {
        return Ok(GenerateActionsResponse {
            ok: false,
            actions: vec![],
            explanation: String::new(),
            error: Some("API-ключ не указан.".into()),
        });
    }

    let user_prompt = format!(
        "Ты — PAPA YU, AI-аудитор проектов. На основе контекста и списка найденных проблем сгенерируй конкретные действия для исправления.\n\nВАЖНО: Отвечай ТОЛЬКО валидным JSON без markdown-обёртки. Формат:\n{{\n  \"actions\": [\n    {{\n      \"id\": \"уникальный-id\",\n      \"title\": \"Краткое название\",\n      \"description\": \"Что делает\",\n      \"kind\": \"create_file\",\n      \"path\": \"путь/к/файлу\",\n      \"content\": \"содержимое\"\n    }}\n  ],\n  \"explanation\": \"Краткое объяснение\"\n}}\n\nДопустимые kind: \"create_file\", \"update_file\", \"create_dir\"\nПуть — относительный от корня проекта. Не более 10 действий.\nПуть проекта: {}\n\nПроблемы:\n{}",
        request.project_path,
        request.findings_json
    );

    let llm_request = super::ask_llm::LlmRequest {
        provider: request.provider,
        model: request.model,
        api_key: request.api_key,
        base_url: request.base_url,
        context: request.context,
        prompt: user_prompt,
        max_tokens: request.max_tokens.or(Some(4096)),
    };

    let llm_response = super::ask_llm::ask_llm(llm_request).await?;

    if !llm_response.ok {
        return Ok(GenerateActionsResponse {
            ok: false,
            actions: vec![],
            explanation: String::new(),
            error: llm_response.error,
        });
    }

    // Parse JSON from LLM response
    let content = llm_response.content.trim().to_string();
    // Strip markdown code fences if present
    let json_str = content
        .strip_prefix("```json")
        .or_else(|| content.strip_prefix("```"))
        .unwrap_or(&content)
        .strip_suffix("```")
        .unwrap_or(&content)
        .trim();

    match serde_json::from_str::<LlmActionsOutput>(json_str) {
        Ok(output) => {
            let actions: Vec<Action> = output
                .actions
                .into_iter()
                .filter_map(|a| {
                    let kind = match a.kind.as_str() {
                        "create_file" => ActionKind::CreateFile,
                        "update_file" => ActionKind::UpdateFile,
                        "create_dir" => ActionKind::CreateDir,
                        "delete_file" => ActionKind::DeleteFile,
                        _ => return None,
                    };
                    Some(Action {
                        id: format!("ai-{}", a.id),
                        title: a.title,
                        description: a.description,
                        kind,
                        path: a.path,
                        content: a.content,
                    })
                })
                .collect();

            Ok(GenerateActionsResponse {
                ok: true,
                actions,
                explanation: output.explanation,
                error: None,
            })
        }
        Err(e) => Ok(GenerateActionsResponse {
            ok: false,
            actions: vec![],
            explanation: String::new(),
            error: Some(format!(
                "Ошибка парсинга ответа LLM: {}. Ответ: {}",
                e,
                &json_str[..json_str.len().min(200)]
            )),
        }),
    }
}
