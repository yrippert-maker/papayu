//! v3.0: агент предложения исправлений (эвристика или LLM по конфигу).
//! Для LLM передаётся полное содержимое проекта (все файлы), не только отчёт.
//! Инженерная память: user prefs (app_data/papa-yu/preferences.json), project prefs (.papa-yu/project.json).

use std::path::Path;

use crate::online_research;
use crate::types::{Action, ActionKind, AgentPlan};
use tauri::Manager;

use super::llm_planner;
use super::project_content;

fn has_readme(root: &str) -> bool {
    ["README.md", "README.MD", "README.txt", "README"]
        .iter()
        .any(|f| Path::new(root).join(f).exists())
}

fn has_gitignore(root: &str) -> bool {
    Path::new(root).join(".gitignore").exists()
}

fn has_license(root: &str) -> bool {
    ["LICENSE", "LICENSE.md", "LICENSE.txt"]
        .iter()
        .any(|f| Path::new(root).join(f).exists())
}

/// Триггеры перехода Plan→Apply (пользователь подтвердил план).
/// Извлекает префикс ошибки (ERR_XXX или LLM_REQUEST_TIMEOUT) из сообщения.
fn extract_error_code(msg: &str) -> &str {
    if let Some(colon) = msg.find(':') {
        let prefix = msg[..colon].trim();
        if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return prefix;
        }
    }
    ""
}

const APPLY_TRIGGERS: &[&str] = &[
    "ok", "ок", "apply", "применяй", "применить", "делай", "да", "yes", "go", "вперёд",
];

#[tauri::command]
pub async fn propose_actions(
    app: tauri::AppHandle,
    path: String,
    report_json: String,
    user_goal: String,
    design_style: Option<String>,
    trends_context: Option<String>,
    last_plan_json: Option<String>,
    last_context: Option<String>,
    apply_error_code: Option<String>,
    apply_error_validated_json: Option<String>,
    apply_repair_attempt: Option<u32>,
    apply_error_stage: Option<String>,
    online_fallback_attempted: Option<bool>,
    online_context_md: Option<String>,
    online_context_sources: Option<Vec<String>>,
    online_fallback_executed: Option<bool>,
    online_fallback_reason: Option<String>,
) -> AgentPlan {
    let goal_trim = user_goal.trim();
    let goal_lower = goal_trim.to_lowercase();
    let root = Path::new(&path);
    if !root.exists() || !root.is_dir() {
        return AgentPlan {
            ok: false,
            summary: String::new(),
            actions: vec![],
            error: Some("path not found".into()),
            error_code: Some("PATH_NOT_FOUND".into()),
            plan_json: None,
            plan_context: None,
            protocol_version_used: None,
            online_fallback_suggested: None,
            online_context_used: None,
        };
    }

    if llm_planner::is_llm_configured() {
        let app_data = match app.path().app_data_dir() {
            Ok(d) => d,
            Err(e) => {
                return AgentPlan {
                    ok: false,
                    summary: String::new(),
                    actions: vec![],
                    error: Some(format!("app data dir: {}", e)),
                error_code: Some("APP_DATA_DIR".into()),
                plan_json: None,
                plan_context: None,
                protocol_version_used: None,
                online_fallback_suggested: None,
                online_context_used: None,
            };
            }
        };
        let user_prefs_path = app_data.join("papa-yu").join("preferences.json");
        let project_prefs_path = root.join(".papa-yu").join("project.json");

        let full_content = project_content::get_project_content_for_llm(root, None);
        let content_for_plan = if full_content.is_empty() {
            None
        } else {
            Some(full_content.as_str())
        };
        let design_ref = design_style.as_deref();
        let trends_ref = trends_context.as_deref();

        // Определение режима: префиксы plan:/apply:, триггер "ok/применяй" + last_plan, или по умолчанию
        let output_format_override: Option<&str> = if goal_lower.starts_with("plan:") {
            Some("plan")
        } else if goal_lower.starts_with("apply:") {
            Some("apply")
        } else if APPLY_TRIGGERS.contains(&goal_lower.as_str()) && last_plan_json.is_some() {
            Some("apply")
        } else if goal_lower.contains("исправь") || goal_lower.contains("почини") || goal_lower.contains("fix ") || goal_lower.contains("исправить") {
            Some("plan")
        } else if goal_lower.contains("создай") || goal_lower.contains("сгенерируй") || goal_lower.contains("create") || goal_lower.contains("с нуля") {
            Some("apply")
        } else {
            None
        };

        let last_plan_ref = last_plan_json.as_deref();
        let last_ctx_ref = last_context.as_deref();
        let apply_error = apply_error_code.as_deref().and_then(|code| {
            apply_error_validated_json.as_deref().map(|json| (code, json))
        });
        let force_protocol = {
            let code = apply_error_code.as_deref().unwrap_or("");
            let repair_attempt = apply_repair_attempt.unwrap_or(0);
            if llm_planner::is_protocol_fallback_applicable(code, repair_attempt) {
                let stage = apply_error_stage.as_deref().unwrap_or("apply");
                eprintln!("[trace] PROTOCOL_FALLBACK from=v2 to=v1 reason={} stage={}", code, stage);
                Some(1u32)
            } else {
                None
            }
        };
        let apply_error_stage_ref = apply_error_stage.as_deref();
        let online_md_ref = online_context_md.as_deref();
        let online_sources_ref: Option<&[String]> = online_context_sources.as_deref();
        let online_reason_ref = online_fallback_reason.as_deref();
        return match llm_planner::plan(
            &user_prefs_path,
            &project_prefs_path,
            &path,
            &report_json,
            goal_trim,
            content_for_plan,
            design_ref,
            trends_ref,
            output_format_override,
            last_plan_ref,
            last_ctx_ref,
            apply_error,
            force_protocol,
            apply_error_stage_ref,
            apply_repair_attempt,
            online_md_ref,
            online_sources_ref,
            online_fallback_executed,
            online_reason_ref,
        )
        .await
        {
            Ok(plan) => plan,
            Err(e) => {
                let error_code_str = extract_error_code(&e).to_string();
                let online_suggested = online_research::maybe_online_fallback(
                    Some(&e),
                    online_research::is_online_research_enabled(),
                    online_fallback_attempted.unwrap_or(false),
                )
                .then_some(goal_trim.to_string());
                if online_suggested.is_some() {
                    eprintln!("[trace] ONLINE_FALLBACK_SUGGESTED error_code={} query_len={}", error_code_str, goal_trim.len());
                }
                AgentPlan {
                    ok: false,
                    summary: String::new(),
                    actions: vec![],
                    error: Some(e),
                    error_code: Some(if error_code_str.is_empty() { "LLM_ERROR".into() } else { error_code_str }),
                    plan_json: None,
                    plan_context: None,
                    protocol_version_used: None,
                    online_fallback_suggested: online_suggested,
                    online_context_used: None,
                }
            }
        };
    }

    // Запросы не про код/проект — не предлагать план с LICENSE, а ответить коротко.
    let goal_trim = user_goal.trim();
    let goal_lower = goal_trim.to_lowercase();
    let off_topic = goal_lower.is_empty()
        || goal_lower.contains("погода")
        || goal_lower.contains("weather")
        || goal_lower.contains("как дела")
        || goal_lower.contains("what's the")
        || goal_lower == "привет"
        || goal_lower == "hello"
        || goal_lower == "hi";
    if off_topic {
        return AgentPlan {
            ok: true,
            summary: "Я помогаю с кодом и проектами. Напиши, например: «сделай README», «добавь тесты», «создай проект с нуля».".into(),
            actions: vec![],
            error: None,
            error_code: None,
            plan_json: None,
            plan_context: None,
            protocol_version_used: None,
            online_fallback_suggested: None,
            online_context_used: None,
        };
    }

    // При запросе «создать программу» сначала скелет (README, .gitignore, точка входа), LICENSE — в конце.
    let want_skeleton = goal_lower.contains("создаю программу")
        || goal_lower.contains("создать программу")
        || goal_lower.contains("create a program")
        || goal_lower.contains("create program")
        || goal_lower.contains("новая программа")
        || goal_lower.contains("с нуля")
        || goal_lower.contains("from scratch");

    let mut actions: Vec<Action> = vec![];
    let mut summary: Vec<String> = vec![];

    if !has_readme(&path) {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: "README.md".into(),
            content: Some(format!(
                "# PAPA YU Project\n\n## Цель\n{}\n\n## Как запустить\n- (добавить)\n\n## Структура\n- (добавить)\n",
                user_goal
            )),
            patch: None,
            base_sha256: None,
        });
        summary.push("Добавлю README.md".into());
    }

    if !has_gitignore(&path) {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: ".gitignore".into(),
            content: Some(
                "node_modules/\ndist/\nbuild/\n.DS_Store\n.env\n.env.*\ncoverage/\n.target/\n".into(),
            ),
            patch: None,
            base_sha256: None,
        });
        summary.push("Добавлю .gitignore".into());
    }

    // При «создать программу»: добавить минимальную точку входа, если нет ни src/, ни main.
    let has_src = root.join("src").is_dir();
    let has_main = root.join("main.py").exists()
        || root.join("main.js").exists()
        || root.join("src").join("main.py").exists()
        || root.join("src").join("main.js").exists();
    if want_skeleton && !has_src && !has_main {
        let main_path = "main.py";
        if !root.join(main_path).exists() {
            actions.push(Action {
                kind: ActionKind::CreateFile,
                path: main_path.into(),
                content: Some(
                    "\"\"\"Точка входа. Запуск: python main.py\"\"\"\n\ndef main() -> None:\n    print(\"Hello\")\n\n\nif __name__ == \"__main__\":\n    main()\n".into(),
                ),
                patch: None,
                base_sha256: None,
            });
            summary.push("Добавлю main.py (скелет)".into());
        }
    }

    if !has_license(&path) {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: "LICENSE".into(),
            content: Some("UNLICENSED\n".into()),
            patch: None,
            base_sha256: None,
        });
        summary.push("Добавлю LICENSE (пометка UNLICENSED)".into());
    }

    if report_json.contains(".env") {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: ".env.example".into(),
            content: Some("VITE_API_URL=\n# пример, без секретов\n".into()),
            patch: None,
            base_sha256: None,
        });
        summary.push("Добавлю .env.example (без секретов)".into());
    }

    if actions.is_empty() {
        return AgentPlan {
            ok: true,
            summary: "Нет безопасных минимальных правок, которые можно применить автоматически.".into(),
            actions,
            error: None,
            error_code: None,
            plan_json: None,
            plan_context: None,
            protocol_version_used: None,
            online_fallback_suggested: None,
            online_context_used: None,
        };
    }

    AgentPlan {
        ok: true,
        summary: format!("План действий: {}", summary.join(", ")),
        actions,
        error: None,
        error_code: None,
        plan_json: None,
        plan_context: None,
        protocol_version_used: None,
        online_fallback_suggested: None,
        online_context_used: None,
    }
}
