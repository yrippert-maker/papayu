//! Сбор ответов от нескольких ИИ (Claude, OpenAI и др.), анализ и выдача оптимального плана.
//!
//! Включение: задайте PAPAYU_LLM_PROVIDERS (JSON-массив провайдеров).
//! Опционально: PAPAYU_LLM_AGGREGATOR_URL — ИИ-агрегатор для слияния планов.

use crate::commands::llm_planner;
use crate::types::AgentPlan;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct ProviderConfig {
    pub url: String,
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
}

/// Парсит PAPAYU_LLM_PROVIDERS: JSON-массив объектов { "url", "model", "api_key" (опционально) }.
pub fn parse_providers_from_env() -> Result<Vec<ProviderConfig>, String> {
    let s = std::env::var("PAPAYU_LLM_PROVIDERS").map_err(|_| "PAPAYU_LLM_PROVIDERS not set")?;
    let s = s.trim();
    if s.is_empty() {
        return Err("PAPAYU_LLM_PROVIDERS is empty".into());
    }
    let list: Vec<ProviderConfig> =
        serde_json::from_str(s).map_err(|e| format!("PAPAYU_LLM_PROVIDERS JSON: {}", e))?;
    if list.is_empty() {
        return Err("PAPAYU_LLM_PROVIDERS: empty array".into());
    }
    Ok(list)
}

/// Запрашивает план у одного провайдера. Имя провайдера — для логов и агрегации.
pub async fn fetch_plan_from_provider(
    name: String,
    config: &ProviderConfig,
    system_content: &str,
    user_message: &str,
    path: &str,
) -> Result<AgentPlan, String> {
    let fallback_key = std::env::var("PAPAYU_LLM_API_KEY").ok();
    let api_key = config
        .api_key
        .as_deref()
        .filter(|k| !k.is_empty())
        .or_else(|| fallback_key.as_deref());
    llm_planner::request_one_plan(
        &config.url,
        api_key,
        &config.model,
        system_content,
        user_message,
        path,
    )
    .await
    .map_err(|e| format!("{}: {}", name, e))
}

/// Собирает планы от всех провайдеров параллельно.
pub async fn fetch_all_plans(
    providers: &[ProviderConfig],
    system_content: &str,
    user_message: &str,
    path: &str,
) -> Vec<(String, AgentPlan)> {
    let mut handles = Vec::with_capacity(providers.len());
    for (i, config) in providers.iter().enumerate() {
        let name = format!(
            "provider_{}_{}",
            i,
            config
                .url
                .split('/')
                .nth(2)
                .unwrap_or("unknown")
        );
        let config = config.clone();
        let system_content = system_content.to_string();
        let user_message = user_message.to_string();
        let path = path.to_string();
        handles.push(async move {
            let result = fetch_plan_from_provider(
                name.clone(),
                &config,
                &system_content,
                &user_message,
                &path,
            )
            .await;
            result.map(|plan| (name, plan))
        });
    }
    let results = futures::future::join_all(handles).await;
    results.into_iter().filter_map(Result::ok).collect()
}

/// Объединяет планы: по пути действия дедуплицируются (оставляем первое вхождение).
fn merge_plans_rust(plans: Vec<(String, AgentPlan)>) -> AgentPlan {
    let mut all_actions = Vec::new();
    let mut seen_paths: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    let mut summary_parts = Vec::new();
    let mut plan_json_merged: Option<String> = None;
    let protocol_version_used = plans.first().and_then(|(_, p)| p.protocol_version_used);

    for (name, plan) in &plans {
        summary_parts.push(format!("{} ({} действий)", name, plan.actions.len()));
        for action in &plan.actions {
            let key = (action.path.clone(), format!("{:?}", action.kind));
            if seen_paths.insert(key) {
                all_actions.push(action.clone());
            }
        }
        if plan_json_merged.is_none() {
            plan_json_merged = plan.plan_json.clone();
        }
    }

    let summary = format!(
        "Объединённый план из {} ИИ: {}. Всего действий: {}.",
        plans.len(),
        summary_parts.join("; "),
        all_actions.len()
    );

    AgentPlan {
        ok: true,
        summary,
        actions: all_actions,
        error: None,
        error_code: None,
        plan_json: plan_json_merged,
        plan_context: None,
        protocol_version_used,
        online_fallback_suggested: None,
        online_context_used: Some(false),
    }
}

/// Вызывает агрегатор-ИИ: один запрос с текстом всех планов, ожидаем один оптимальный план в том же JSON-формате.
async fn aggregate_via_llm(
    plans: Vec<(String, AgentPlan)>,
    _system_content: &str,
    user_message: &str,
    path: &str,
) -> Result<AgentPlan, String> {
    let aggregator_url =
        std::env::var("PAPAYU_LLM_AGGREGATOR_URL").map_err(|_| "PAPAYU_LLM_AGGREGATOR_URL not set")?;
    let aggregator_url = aggregator_url.trim();
    if aggregator_url.is_empty() {
        return Err("PAPAYU_LLM_AGGREGATOR_URL is empty".into());
    }
    let aggregator_key = std::env::var("PAPAYU_LLM_AGGREGATOR_KEY").ok();
    let aggregator_model = std::env::var("PAPAYU_LLM_AGGREGATOR_MODEL")
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let plans_text: Vec<String> = plans
        .iter()
        .map(|(name, plan)| {
            let actions_json = serde_json::to_string(&plan.actions).unwrap_or_else(|_| "[]".into());
            format!("--- {} ---\nsummary: {}\nactions: {}\n", name, plan.summary, actions_json)
        })
        .collect();
    let aggregator_prompt = format!(
        "Ниже приведены планы от разных ИИ (Claude, OpenAI и др.) по одной и той же задаче.\n\
         Твоя задача: проанализировать все планы и выдать ОДИН оптимальный план (объединённый или лучший).\n\
         Ответь в том же JSON-формате, что и входные планы: объект с полем \"actions\" (массив действий) и опционально \"summary\".\n\n\
         Планы:\n{}\n\n\
         Исходный запрос пользователя (контекст):\n{}",
        plans_text.join("\n"),
        user_message.chars().take(4000).collect::<String>()
    );
    let system_aggregator = "Ты — агрегатор планов. На вход даны несколько планов от разных ИИ. Выдай один итоговый план в формате JSON: { \"summary\": \"...\", \"actions\": [ ... ] }. Без markdown-обёртки.";
    llm_planner::request_one_plan(
        aggregator_url,
        aggregator_key.as_deref(),
        &aggregator_model,
        system_aggregator,
        &aggregator_prompt,
        path,
    )
    .await
}

/// Собирает планы от всех провайдеров и возвращает один оптимальный (агрегатор-ИИ или слияние в Rust).
pub async fn fetch_and_aggregate(
    system_content: &str,
    user_message: &str,
    path: &str,
) -> Result<AgentPlan, String> {
    let providers = parse_providers_from_env()?;
    let plans = fetch_all_plans(&providers, system_content, user_message, path).await;
    if plans.is_empty() {
        return Err("Ни один из ИИ-провайдеров не вернул валидный план".into());
    }
    if plans.len() == 1 {
        return Ok(plans.into_iter().next().unwrap().1);
    }
    let use_aggregator = std::env::var("PAPAYU_LLM_AGGREGATOR_URL")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if use_aggregator {
        aggregate_via_llm(plans, system_content, user_message, path).await
    } else {
        Ok(merge_plans_rust(plans))
    }
}
