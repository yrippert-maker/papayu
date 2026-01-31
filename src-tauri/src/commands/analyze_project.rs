use crate::types::{Action, ActionGroup, ActionKind, AnalyzeReport, Finding, FixPack, ProjectSignal};
use std::path::Path;

pub fn analyze_project(paths: Vec<String>, attached_files: Option<Vec<String>>) -> Result<AnalyzeReport, String> {
    let path = paths.first().cloned().unwrap_or_else(|| ".".to_string());
    let root = Path::new(&path);
    if !root.is_dir() {
        return Ok(AnalyzeReport {
            path: path.clone(),
            narrative: format!("Папка не найдена: {}", path),
            findings: vec![],
            recommendations: vec![],
            actions: vec![],
            action_groups: vec![],
            fix_packs: vec![],
            recommended_pack_ids: vec![],
            attached_files,
        });
    }

    let has_readme = root.join("README.md").is_file();
    let has_gitignore = root.join(".gitignore").is_file();
    let has_env = root.join(".env").is_file();
    let has_src = root.join("src").is_dir();
    let has_tests = root.join("tests").is_dir();
    let has_package = root.join("package.json").is_file();
    let has_cargo = root.join("Cargo.toml").is_file();

    let mut findings = Vec::new();
    let recommendations = Vec::new();
    let action_groups = build_action_groups(root, has_readme, has_gitignore, has_src, has_tests, has_package, has_cargo);
    let mut actions: Vec<Action> = action_groups.iter().flat_map(|g| g.actions.clone()).collect();

    if !has_readme {
        findings.push(Finding {
            title: "Нет README.md".to_string(),
            details: "Рекомендуется добавить описание проекта.".to_string(),
            path: Some(path.clone()),
        });
    }
    if !has_gitignore {
        findings.push(Finding {
            title: "Нет .gitignore".to_string(),
            details: "Рекомендуется добавить .gitignore для типа проекта.".to_string(),
            path: Some(path.clone()),
        });
    }
    if has_env {
        findings.push(Finding {
            title: "Найден .env".to_string(),
            details: "Рекомендуется создать .env.example и не коммитить .env.".to_string(),
            path: Some(path.clone()),
        });
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: ".env.example".to_string(),
            content: Some("# Copy to .env and fill\n".to_string()),
        });
    }
    if has_src && !has_tests {
        findings.push(Finding {
            title: "Нет папки tests/".to_string(),
            details: "Рекомендуется добавить tests/ и README в ней.".to_string(),
            path: Some(path.clone()),
        });
    }

    let signals = build_signals_from_findings(&findings);
    let (fix_packs, recommended_pack_ids) = build_fix_packs(&action_groups, &signals);

    let narrative = format!(
        "Проанализировано: {}. Найдено проблем: {}, рекомендаций: {}, действий: {}.",
        path,
        findings.len(),
        recommendations.len(),
        actions.len()
    );

    Ok(AnalyzeReport {
        path,
        narrative,
        findings,
        recommendations,
        actions,
        action_groups,
        fix_packs,
        recommended_pack_ids,
        attached_files,
    })
}

fn build_action_groups(
    _path: &Path,
    has_readme: bool,
    has_gitignore: bool,
    has_src: bool,
    has_tests: bool,
    has_package: bool,
    has_cargo: bool,
) -> Vec<ActionGroup> {
    let mut groups: Vec<ActionGroup> = vec![];

    if !has_readme {
        groups.push(ActionGroup {
            id: "readme".into(),
            title: "Добавить README".into(),
            description: "Создаст README.md с базовой структурой.".into(),
            actions: vec![Action {
                kind: ActionKind::CreateFile,
                path: "README.md".into(),
                content: Some("# Project\n\n## Overview\n\n## How to run\n\n## Tests\n\n".into()),
            }],
        });
    }

    if !has_gitignore {
        let content = if has_package {
            "node_modules/\n.env\n.DS_Store\ndist/\n*.log\n"
        } else if has_cargo {
            "target/\n.env\n.DS_Store\nCargo.lock\n"
        } else {
            ".env\n.DS_Store\n__pycache__/\n*.pyc\n.venv/\n"
        };
        groups.push(ActionGroup {
            id: "gitignore".into(),
            title: "Добавить .gitignore".into(),
            description: "Создаст .gitignore со стандартными исключениями.".into(),
            actions: vec![Action {
                kind: ActionKind::CreateFile,
                path: ".gitignore".into(),
                content: Some(content.to_string()),
            }],
        });
    }

    if !has_tests && has_src {
        groups.push(ActionGroup {
            id: "tests".into(),
            title: "Добавить tests/".into(),
            description: "Создаст папку tests/ и README для тестов.".into(),
            actions: vec![
                Action {
                    kind: ActionKind::CreateDir,
                    path: "tests".into(),
                    content: None,
                },
                Action {
                    kind: ActionKind::CreateFile,
                    path: "tests/README.md".into(),
                    content: Some("# Tests\n\nAdd tests here.\n".into()),
                },
            ],
        });
    }

    groups
}

fn build_signals_from_findings(findings: &[Finding]) -> Vec<ProjectSignal> {
    let mut signals: Vec<ProjectSignal> = vec![];
    for f in findings {
        if f.title.contains("gitignore") {
            signals.push(ProjectSignal {
                category: "security".into(),
                level: "high".into(),
            });
        }
        if f.title.contains("README") {
            signals.push(ProjectSignal {
                category: "quality".into(),
                level: "warn".into(),
            });
        }
        if f.title.contains("tests") || f.details.to_lowercase().contains("тест") {
            signals.push(ProjectSignal {
                category: "quality".into(),
                level: "warn".into(),
            });
        }
    }
    signals
}

fn build_fix_packs(action_groups: &[ActionGroup], signals: &[ProjectSignal]) -> (Vec<FixPack>, Vec<String>) {
    let mut security: Vec<String> = vec![];
    let mut quality: Vec<String> = vec![];
    let structure: Vec<String> = vec![];

    for g in action_groups {
        match g.id.as_str() {
            "gitignore" => security.push(g.id.clone()),
            "readme" => quality.push(g.id.clone()),
            "tests" => quality.push(g.id.clone()),
            _ => {}
        }
    }

    let mut recommended: Vec<String> = vec![];
    let has_high_security = signals
        .iter()
        .any(|s| s.category == "security" && (s.level == "high" || s.level == "critical"));
    let has_quality_issues = signals
        .iter()
        .any(|s| s.category == "quality" && (s.level == "warn" || s.level == "high"));
    let has_structure_issues = signals
        .iter()
        .any(|s| s.category == "structure" && (s.level == "warn" || s.level == "high"));

    if has_high_security && !security.is_empty() {
        recommended.push("security".into());
    }
    if has_quality_issues && !quality.is_empty() {
        recommended.push("quality".into());
    }
    if has_structure_issues && !structure.is_empty() {
        recommended.push("structure".into());
    }

    if recommended.is_empty() && !quality.is_empty() {
        recommended.push("quality".into());
    }

    let packs = vec![
        FixPack {
            id: "security".into(),
            title: "Безопасность".into(),
            description: "Снижает риск утечки секретов и мусора в репозитории.".into(),
            group_ids: security,
        },
        FixPack {
            id: "quality".into(),
            title: "Качество".into(),
            description: "Базовые улучшения читаемости и проверяемости проекта.".into(),
            group_ids: quality,
        },
        FixPack {
            id: "structure".into(),
            title: "Структура".into(),
            description: "Наводит порядок в структуре проекта и соглашениях.".into(),
            group_ids: structure,
        },
    ];

    (packs, recommended)
}

