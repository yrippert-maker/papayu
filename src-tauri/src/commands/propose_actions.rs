//! v3.0: агент предложения исправлений (эвристика или LLM по конфигу).
//! Для LLM передаётся полное содержимое проекта (все файлы), не только отчёт.
//! Инженерная память: user prefs (app_data/papa-yu/preferences.json), project prefs (.papa-yu/project.json).

use std::path::Path;

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
        )
        .await
        {
            Ok(plan) => plan,
            Err(e) => AgentPlan {
                ok: false,
                summary: String::new(),
                actions: vec![],
                error: Some(e),
                error_code: Some("LLM_ERROR".into()),
                plan_json: None,
                plan_context: None,
            },
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
            });
            summary.push("Добавлю main.py (скелет)".into());
        }
    }

    if !has_license(&path) {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: "LICENSE".into(),
            content: Some("UNLICENSED\n".into()),
        });
        summary.push("Добавлю LICENSE (пометка UNLICENSED)".into());
    }

    if report_json.contains(".env") {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: ".env.example".into(),
            content: Some("VITE_API_URL=\n# пример, без секретов\n".into()),
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
    }
}
