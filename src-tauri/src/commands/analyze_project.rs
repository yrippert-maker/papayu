use crate::commands::get_project_profile::detect_project_type;
use crate::types::{
    Action, ActionGroup, ActionKind, AnalyzeReport, Finding, FixPack, ProjectSignal,
};
use crate::types::ProjectType;
use std::path::Path;
use walkdir::WalkDir;

pub fn analyze_project(
    paths: Vec<String>,
    attached_files: Option<Vec<String>>,
) -> Result<AnalyzeReport, String> {
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
    let has_lockfile = root.join("package-lock.json").is_file()
        || root.join("yarn.lock").is_file()
        || root.join("Cargo.lock").is_file();
    let has_editorconfig = root.join(".editorconfig").is_file();

    let mut findings = Vec::new();
    let recommendations = Vec::new();
    let action_groups = build_action_groups(
        root,
        has_readme,
        has_gitignore,
        has_src,
        has_tests,
        has_package,
        has_cargo,
    );
    let mut actions: Vec<Action> = action_groups
        .iter()
        .flat_map(|g| g.actions.clone())
        .collect();

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
            patch: None,
            base_sha256: None,
            edits: None,
        });
    }
    if has_src && !has_tests {
        findings.push(Finding {
            title: "Нет папки tests/".to_string(),
            details: "Рекомендуется добавить tests/ и README в ней.".to_string(),
            path: Some(path.clone()),
        });
    }
    if has_env && !has_gitignore {
        findings.push(Finding {
            title: ".env без .gitignore (критично)".to_string(),
            details: "Файл .env может попасть в репозиторий. Добавьте .gitignore с .env.".to_string(),
            path: Some(path.clone()),
        });
    }
    if (has_package || has_cargo) && !has_lockfile {
        findings.push(Finding {
            title: "Нет lock-файла".to_string(),
            details: "Рекомендуется добавить package-lock.json, yarn.lock или Cargo.lock для воспроизводимых сборок.".to_string(),
            path: Some(path.clone()),
        });
    }
    if !has_editorconfig {
        findings.push(Finding {
            title: "Нет .editorconfig".to_string(),
            details: "Рекомендуется добавить .editorconfig для единообразного форматирования.".to_string(),
            path: Some(path.clone()),
        });
    }
    if has_package {
        if let Some(scripts_missing) = check_package_scripts(root) {
            findings.push(Finding {
                title: "package.json без scripts (build/test/lint)".to_string(),
                details: scripts_missing,
                path: Some(path.clone()),
            });
        }
    }
    for f in check_empty_dirs(root) {
        findings.push(f);
    }
    for f in check_large_files(root, 500) {
        findings.push(f);
    }
    for f in check_utils_dump(root, 20) {
        findings.push(f);
    }
    for f in check_large_dir(root, 50) {
        findings.push(f);
    }
    for f in check_monolith_structure(root) {
        findings.push(f);
    }
    for f in check_prettier_config(root) {
        findings.push(f);
    }
    for f in check_ci_workflows(root) {
        findings.push(f);
    }

    let signals = build_signals_from_findings(&findings);
    let (fix_packs, recommended_pack_ids) = build_fix_packs(&action_groups, &signals);

    let narrative = build_human_narrative(root, &path, &findings, &actions, has_src, has_tests);

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
                patch: None,
                base_sha256: None,
                edits: None,
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
                patch: None,
                base_sha256: None,
                edits: None,
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
                    patch: None,
                    base_sha256: None,
                    edits: None,
                },
                Action {
                    kind: ActionKind::CreateFile,
                    path: "tests/README.md".into(),
                    content: Some("# Tests\n\nAdd tests here.\n".into()),
                    patch: None,
                    base_sha256: None,
                    edits: None,
                },
            ],
        });
    }

    groups
}

fn check_package_scripts(root: &Path) -> Option<String> {
    let pkg_path = root.join("package.json");
    let content = std::fs::read_to_string(&pkg_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let scripts = json.get("scripts")?.as_object()?;
    let mut missing = Vec::new();
    if scripts.get("build").is_none() {
        missing.push("build");
    }
    if scripts.get("test").is_none() {
        missing.push("test");
    }
    if scripts.get("lint").is_none() {
        missing.push("lint");
    }
    if missing.is_empty() {
        None
    } else {
        Some(format!(
            "Отсутствуют scripts: {}. Рекомендуется добавить для CI и локальной разработки.",
            missing.join(", ")
        ))
    }
}

fn check_empty_dirs(root: &Path) -> Vec<Finding> {
    let mut out = Vec::new();
    for e in WalkDir::new(root)
        .max_depth(4)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()))
        .flatten()
    {
        if e.file_type().is_dir() {
            let p = e.path();
            if p.read_dir().is_ok_and(|mut it| it.next().is_none()) {
                if let Ok(rel) = p.strip_prefix(root) {
                    let rel_str = rel.to_string_lossy();
                    if !rel_str.is_empty() && !rel_str.starts_with('.') {
                        out.push(Finding {
                            title: "Пустая папка".to_string(),
                            details: format!("Папка {} пуста. Можно удалить или добавить .gitkeep.", rel_str),
                            path: Some(p.to_string_lossy().to_string()),
                        });
                    }
                }
            }
        }
    }
    out.truncate(3); // не более 3, чтобы не засорять отчёт
    out
}

fn is_ignored(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .map(|n| {
            n == "node_modules"
                || n == "target"
                || n == "dist"
                || n == ".git"
                || n.starts_with('.')
        })
        .unwrap_or(false)
}

fn check_large_files(root: &Path, max_lines: u32) -> Vec<Finding> {
    let mut candidates: Vec<(String, u32)> = Vec::new();
    for e in WalkDir::new(root)
        .max_depth(6)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()))
        .flatten()
    {
        if e.file_type().is_file() {
            let p = e.path();
            if let Some(ext) = p.extension() {
                let ext = ext.to_string_lossy();
                if ["rs", "ts", "tsx", "js", "jsx", "py", "java"].contains(&ext.as_ref()) {
                    if let Ok(content) = std::fs::read_to_string(p) {
                        let lines = content.lines().count() as u32;
                        if lines > max_lines {
                            if let Ok(rel) = p.strip_prefix(root) {
                                candidates.push((rel.to_string_lossy().to_string(), lines));
                            }
                        }
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates
        .into_iter()
        .take(3)
        .map(|(rel, lines)| Finding {
            title: "Файл > 500 строк".to_string(),
            details: format!("{}: {} строк. Рекомендуется разбить на модули.", rel, lines),
            path: Some(rel),
        })
        .collect()
}

fn check_utils_dump(root: &Path, threshold: usize) -> Vec<Finding> {
    let utils = root.join("utils");
    if !utils.is_dir() {
        return vec![];
    }
    let count = WalkDir::new(&utils)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .count();
    if count > threshold {
        vec![Finding {
            title: "utils/ как свалка".to_string(),
            details: format!(
                "В utils/ {} файлов (порог {}). Рекомендуется структурировать по доменам.",
                count, threshold
            ),
            path: Some(utils.to_string_lossy().to_string()),
        }]
    } else {
        vec![]
    }
}

fn check_monolith_structure(root: &Path) -> Vec<Finding> {
    let src = root.join("src");
    if !src.is_dir() {
        return vec![];
    }
    let (files_in_src, has_subdirs) = {
        let mut files = 0usize;
        let mut dirs = false;
        for e in WalkDir::new(&src).max_depth(1).into_iter().filter_map(|e| e.ok()) {
            if e.file_type().is_file() {
                files += 1;
            } else if e.file_type().is_dir() && e.path() != src {
                dirs = true;
            }
        }
        (files, dirs)
    };
    if files_in_src > 15 && !has_subdirs {
        vec![Finding {
            title: "Монолитная структура src/".to_string(),
            details: "Много файлов в корне src/ без подпапок. Рекомендуется разделение по feature/domain.".to_string(),
            path: Some(src.to_string_lossy().to_string()),
        }]
    } else {
        vec![]
    }
}

fn check_prettier_config(root: &Path) -> Vec<Finding> {
    let has_prettier = root.join(".prettierrc").is_file()
        || root.join(".prettierrc.json").is_file()
        || root.join("prettier.config.js").is_file();
    if has_package(root) && !has_prettier {
        vec![Finding {
            title: "Нет конфигурации Prettier".to_string(),
            details: "Рекомендуется добавить .prettierrc для JS/TS проектов.".to_string(),
            path: Some(root.to_string_lossy().to_string()),
        }]
    } else {
        vec![]
    }
}

fn has_package(root: &Path) -> bool {
    root.join("package.json").is_file()
}

fn check_ci_workflows(root: &Path) -> Vec<Finding> {
    let has_pkg = root.join("package.json").is_file();
    let has_cargo = root.join("Cargo.toml").is_file();
    if !has_pkg && !has_cargo {
        return vec![];
    }
    let gh = root.join(".github").join("workflows");
    if !gh.is_dir() {
        vec![Finding {
            title: "Нет GitHub Actions CI".to_string(),
            details: "Рекомендуется добавить .github/workflows/ для lint, test, build.".to_string(),
            path: Some(root.to_string_lossy().to_string()),
        }]
    } else {
        vec![]
    }
}

fn check_large_dir(root: &Path, threshold: usize) -> Vec<Finding> {
    let mut out = Vec::new();
    for e in WalkDir::new(root)
        .max_depth(3)
        .min_depth(1)
        .into_iter()
        .filter_entry(|e| !is_ignored(e.path()))
        .flatten()
    {
        if e.file_type().is_dir() {
            let p = e.path();
            let count = WalkDir::new(p)
                .max_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .count();
            if count > threshold {
                if let Ok(rel) = p.strip_prefix(root) {
                    out.push(Finding {
                        title: "Слишком много файлов в одной папке".to_string(),
                        details: format!(
                            "{}: {} файлов. Рекомендуется разбить на подпапки.",
                            rel.to_string_lossy(),
                            count
                        ),
                        path: Some(p.to_string_lossy().to_string()),
                    });
                }
            }
        }
    }
    out.truncate(2);
    out
}

fn build_human_narrative(
    root: &Path,
    path: &str,
    findings: &[Finding],
    actions: &[Action],
    has_src: bool,
    has_tests: bool,
) -> String {
    let pt = detect_project_type(root);
    let stack = match pt {
        ProjectType::ReactVite => "React + Vite (Frontend SPA)",
        ProjectType::NextJs => "Next.js",
        ProjectType::Node => "Node.js",
        ProjectType::Rust => "Rust/Cargo",
        ProjectType::Python => "Python",
        ProjectType::Unknown => "тип не определён",
    };
    let mut lines = vec![
        format!("Я проанализировал проект {}.", path),
        format!("Это {}.", stack),
    ];
    if has_src {
        lines.push("Есть src/.".to_string());
    }
    if has_src && !has_tests {
        lines.push("Нет tests/ — стоит добавить тесты.".to_string());
    }
    let n = findings.len();
    if n > 0 {
        lines.push(format!(
            "Найдено проблем: {}. Рекомендую начать с: {}.",
            n,
            findings
                .iter()
                .take(3)
                .map(|f| f.title.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        ));
    }
    if !actions.is_empty() {
        lines.push(format!(
            "Можно применить {} безопасных исправлений.",
            actions.len()
        ));
    }
    lines.join(" ")
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

fn build_fix_packs(
    action_groups: &[ActionGroup],
    signals: &[ProjectSignal],
) -> (Vec<FixPack>, Vec<String>) {
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
