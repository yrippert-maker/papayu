//! v2.4: Build ActionPlan from analyze report (recommendations → actions).

use std::path::Path;

use crate::types::{ActionItem, ActionKind, ActionPlan, AnalyzeReport, GenerateActionsPayload};

fn rel(p: &str) -> String {
    p.replace('\\', "/")
}

fn mk_id(prefix: &str, n: usize) -> String {
    format!("{}-{}", prefix, n)
}

fn report_mentions_readme(report: &AnalyzeReport) -> bool {
    report
        .findings
        .iter()
        .any(|f| f.title.contains("README") || f.details.to_lowercase().contains("readme"))
        || report
            .recommendations
            .iter()
            .any(|r| r.title.to_lowercase().contains("readme") || r.details.to_lowercase().contains("readme"))
}

fn report_mentions_gitignore(report: &AnalyzeReport) -> bool {
    report
        .findings
        .iter()
        .any(|f| f.title.contains("gitignore") || f.details.to_lowercase().contains("gitignore"))
        || report
            .recommendations
            .iter()
            .any(|r| r.title.to_lowercase().contains("gitignore") || r.details.to_lowercase().contains("gitignore"))
}

fn report_mentions_tests(report: &AnalyzeReport) -> bool {
    report
        .findings
        .iter()
        .any(|f| f.title.contains("tests") || f.details.to_lowercase().contains("тест"))
        || report
            .recommendations
            .iter()
            .any(|r| r.title.to_lowercase().contains("test") || r.details.to_lowercase().contains("тест"))
}

pub fn build_actions_from_report(report: &AnalyzeReport, mode: &str) -> Vec<ActionItem> {
    let mut out: Vec<ActionItem> = vec![];

    if report_mentions_readme(report) {
        out.push(ActionItem {
            id: mk_id("action", out.len() + 1),
            kind: ActionKind::CreateFile,
            path: rel("README.md"),
            content: Some(
                "# PAPA YU Project\n\n## Описание\n\nКратко опишите проект.\n\n## Запуск\n\n- dev: ...\n- build: ...\n\n## Структура\n\n- src/\n- tests/\n".into(),
            ),
            summary: "Добавить README.md".into(),
            rationale: "Улучшает понимание проекта и снижает риск ошибок при работе с кодом.".into(),
            tags: vec!["docs".into(), "quality".into()],
            risk: "low".into(),
        });
    }

    if report_mentions_gitignore(report) {
        out.push(ActionItem {
            id: mk_id("action", out.len() + 1),
            kind: ActionKind::CreateFile,
            path: rel(".gitignore"),
            content: Some(
                "node_modules/\ndist/\nbuild/\n.next/\ncoverage/\n.env\n.env.*\n.DS_Store\n".into(),
            ),
            summary: "Добавить .gitignore".into(),
            rationale: "Исключает мусор и потенциально секретные файлы из репозитория.".into(),
            tags: vec!["quality".into(), "security".into()],
            risk: "low".into(),
        });
    }

    if report_mentions_tests(report) {
        out.push(ActionItem {
            id: mk_id("action", out.len() + 1),
            kind: ActionKind::CreateDir,
            path: rel("tests"),
            content: None,
            summary: "Создать папку tests/".into(),
            rationale: "Готовит структуру под тесты.".into(),
            tags: vec!["quality".into(), "tests".into()],
            risk: "low".into(),
        });
        out.push(ActionItem {
            id: mk_id("action", out.len() + 1),
            kind: ActionKind::CreateFile,
            path: rel("tests/smoke.test.txt"),
            content: Some("TODO: add smoke tests\n".into()),
            summary: "Добавить tests/smoke.test.txt".into(),
            rationale: "Минимальный маркер тестов. Замените на реальные тесты позже.".into(),
            tags: vec!["tests".into()],
            risk: "low".into(),
        });
    }

    if mode == "balanced" {
        let root = Path::new(&report.path);
        let has_node = root.join("package.json").exists();
        let has_react = root.join("package.json").exists() && (root.join("src").join("App.jsx").exists() || root.join("src").join("App.tsx").exists());
        if has_node || has_react {
            out.push(ActionItem {
                id: mk_id("action", out.len() + 1),
                kind: ActionKind::CreateFile,
                path: rel(".prettierrc"),
                content: Some("{\n  \"singleQuote\": true,\n  \"semi\": true\n}\n".into()),
                summary: "Добавить .prettierrc".into(),
                rationale: "Стабилизирует форматирование кода.".into(),
                tags: vec!["quality".into()],
                risk: "low".into(),
            });
        }
    }

    out
}

#[tauri::command]
pub async fn generate_actions(payload: GenerateActionsPayload) -> Result<ActionPlan, String> {
    let path = payload.path.clone();
    let mode = if payload.mode.is_empty() { "safe" } else { payload.mode.as_str() };

    let report = crate::commands::analyze_project(vec![path.clone()], None)?;
    let mut actions = build_actions_from_report(&report, mode);

    if !payload.selected.is_empty() {
        let sel: Vec<String> = payload.selected.iter().map(|s| s.to_lowercase()).collect();
        actions = actions
            .into_iter()
            .filter(|a| {
                let txt = format!("{} {} {} {:?}", a.summary, a.rationale, a.risk, a.tags).to_lowercase();
                sel.iter().any(|k| txt.contains(k))
            })
            .collect();
    }

    let warnings = vec!["Все изменения применяются только через предпросмотр и могут быть отменены.".into()];

    Ok(ActionPlan {
        plan_id: format!("plan-{}", chrono::Utc::now().timestamp_millis()),
        root_path: path,
        title: "План исправлений (MVP)".into(),
        actions,
        warnings,
    })
}
