use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use tauri::Emitter;

use crate::types::{
    Action, ActionKind, AnalyzeReport, Finding, LlmContext, ProjectContext, ProjectSignal,
    ProjectStructure, Recommendation, ReportStats,
};

const MAX_FILES: u64 = 50_000;
const MAX_DURATION_SECS: u64 = 60;
const TOP_EXTENSIONS_N: usize = 15;
const MAX_DEPTH_WARN: u32 = 6;
const ROOT_FILES_WARN: u64 = 20;

const EXCLUDED_DIRS: &[&str] = &[
    ".git", "node_modules", "dist", "build", ".next", "target", ".cache", "coverage",
];

const MARKER_README: &[&str] = &["README", "readme", "Readme"];
const MARKER_VITE: &[&str] = &["vite.config.js", "vite.config.ts", "vite.config.mjs"];

#[derive(Default)]
struct ScanState {
    file_count: u64,
    dir_count: u64,
    total_size_bytes: u64,
    extensions: HashMap<String, u64>,
    has_readme: bool,
    has_package_json: bool,
    has_cargo_toml: bool,
    has_env: bool,
    has_docker: bool,
    has_tsconfig: bool,
    has_vite: bool,
    has_next: bool,
    has_gitignore: bool,
    has_license: bool,
    has_eslint: bool,
    has_prettier: bool,
    has_tests_dir: bool,
    has_src: bool,
    has_components: bool,
    has_pages: bool,
    has_requirements_txt: bool,
    has_pyproject: bool,
    has_setup_py: bool,
    package_json_count: u32,
    cargo_toml_count: u32,
    root_file_count: u64,
    root_dirs: HashSet<String>,
    max_depth: u32,
}

const PROGRESS_EVENT: &str = "analyze_progress";

#[tauri::command]
pub fn analyze_project(window: tauri::Window, path: String) -> Result<AnalyzeReport, String> {
    let root = PathBuf::from(&path);
    if !root.exists() {
        return Err("Путь не существует".to_string());
    }
    if !root.is_dir() {
        return Err("Путь не является папкой".to_string());
    }

    let _ = window.emit(PROGRESS_EVENT, "Сканирую структуру…");

    let deadline = Instant::now() + std::time::Duration::from_secs(MAX_DURATION_SECS);
    let mut state = ScanState::default();

    scan_dir(&root, &root, 0, &mut state, &deadline)?;

    let _ = window.emit(PROGRESS_EVENT, "Анализирую архитектуру…");

    let top_extensions: Vec<(String, u64)> = {
        let mut v: Vec<_> = state.extensions.iter().map(|(k, v)| (k.clone(), *v)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        v.into_iter().take(TOP_EXTENSIONS_N).collect()
    };

    let stats = ReportStats {
        file_count: state.file_count,
        dir_count: state.dir_count,
        total_size_bytes: state.total_size_bytes,
        top_extensions,
        max_depth: state.max_depth as u64,
    };

    let structure = build_structure(&state);
    let mut findings: Vec<Finding> = Vec::new();
    let mut recommendations: Vec<Recommendation> = Vec::new();
    let mut signals: Vec<ProjectSignal> = Vec::new();

    if state.has_env {
        findings.push(Finding {
            severity: "high".to_string(),
            title: "Риск секретов".to_string(),
            details: "Обнаружены файлы .env или .env.* — не коммитьте секреты в репозиторий.".to_string(),
        });
        signals.push(ProjectSignal {
            category: "security".to_string(),
            level: "high".to_string(),
            message: "Есть .env файл — риск утечки секретов.".to_string(),
        });
    }

    if !state.has_readme {
        recommendations.push(Recommendation {
            title: "Добавить README".to_string(),
            details: "Опишите проект и инструкцию по запуску.".to_string(),
            priority: "medium".to_string(),
            effort: "low".to_string(),
            impact: "high".to_string(),
        });
        signals.push(ProjectSignal {
            category: "quality".to_string(),
            level: "warn".to_string(),
            message: "Нет README.".to_string(),
        });
    }

    if !state.has_gitignore {
        recommendations.push(Recommendation {
            title: "Добавить .gitignore".to_string(),
            details: "Исключите артефакты сборки и зависимости из репозитория.".to_string(),
            priority: "medium".to_string(),
            effort: "low".to_string(),
            impact: "medium".to_string(),
        });
        signals.push(ProjectSignal {
            category: "quality".to_string(),
            level: "warn".to_string(),
            message: "Нет .gitignore.".to_string(),
        });
    }

    if !state.has_license {
        recommendations.push(Recommendation {
            title: "Указать лицензию".to_string(),
            details: "Добавьте LICENSE или LICENSE.md.".to_string(),
            priority: "low".to_string(),
            effort: "low".to_string(),
            impact: "medium".to_string(),
        });
        signals.push(ProjectSignal {
            category: "quality".to_string(),
            level: "info".to_string(),
            message: "Нет файла лицензии.".to_string(),
        });
    }

    if state.has_src && !state.has_tests_dir {
        recommendations.push(Recommendation {
            title: "Добавить тесты".to_string(),
            details: "Есть src/, но нет папки tests/ — добавьте базовые тесты.".to_string(),
            priority: "high".to_string(),
            effort: "medium".to_string(),
            impact: "high".to_string(),
        });
        signals.push(ProjectSignal {
            category: "structure".to_string(),
            level: "warn".to_string(),
            message: "Есть src/, нет tests/.".to_string(),
        });
    }

    if state.has_components && !state.has_pages && state.has_package_json {
        recommendations.push(Recommendation {
            title: "Проверить структуру фронтенда".to_string(),
            details: "Есть components/, но нет pages/ — возможно, маршруты или страницы в другом месте.".to_string(),
            priority: "low".to_string(),
            effort: "low".to_string(),
            impact: "low".to_string(),
        });
    }

    if state.root_file_count >= ROOT_FILES_WARN {
        findings.push(Finding {
            severity: "warn".to_string(),
            title: "Много файлов в корне".to_string(),
            details: format!("В корне {} файлов — рассмотрите группировку по папкам.", state.root_file_count),
        });
        signals.push(ProjectSignal {
            category: "structure".to_string(),
            level: "warn".to_string(),
            message: "Слишком много файлов в корне проекта.".to_string(),
        });
    }

    if state.max_depth >= MAX_DEPTH_WARN {
        findings.push(Finding {
            severity: "warn".to_string(),
            title: "Глубокая вложенность".to_string(),
            details: format!("Вложенность до {} уровней — усложняет навигацию.", state.max_depth),
        });
        signals.push(ProjectSignal {
            category: "structure".to_string(),
            level: "warn".to_string(),
            message: "Глубокая вложенность папок.".to_string(),
        });
    }

    if state.has_package_json && !state.has_eslint && !state.has_cargo_toml {
        recommendations.push(Recommendation {
            title: "Добавить линтер".to_string(),
            details: "Рекомендуется ESLint (и при необходимости Prettier) для JavaScript/TypeScript.".to_string(),
            priority: "medium".to_string(),
            effort: "low".to_string(),
            impact: "medium".to_string(),
        });
    }

    if state.has_cargo_toml && !state.has_eslint {
        recommendations.push(Recommendation {
            title: "Использовать Clippy".to_string(),
            details: "Добавьте в CI или pre-commit: cargo clippy.".to_string(),
            priority: "medium".to_string(),
            effort: "low".to_string(),
            impact: "medium".to_string(),
        });
    }

    if !state.has_package_json && !state.has_cargo_toml && !state.has_pyproject && !state.has_requirements_txt {
        findings.push(Finding {
            severity: "warn".to_string(),
            title: "Неопределён тип проекта".to_string(),
            details: "Не найдены привычные манифесты (package.json, Cargo.toml, pyproject.toml).".to_string(),
        });
    }

    if state.file_count > 30_000 || state.dir_count > 5_000 {
        recommendations.push(Recommendation {
            title: "Сузить область анализа".to_string(),
            details: "Очень много файлов или папок — добавьте исключения или выберите подпапку.".to_string(),
            priority: "medium".to_string(),
            effort: "low".to_string(),
            impact: "low".to_string(),
        });
    }

    let _ = window.emit(PROGRESS_EVENT, "Глубокий анализ кода…");
    let deep = crate::deep_analysis::run_deep_analysis(std::path::Path::new(&path));
    findings.extend(deep.findings);
    signals.extend(deep.signals);

    let _ = window.emit(PROGRESS_EVENT, "Формирую вывод…");

    let recommendations = enrich_recommendations(recommendations);
    let project_context = build_project_context(&state, &findings, &signals);
    let actions = build_actions(state.has_readme, state.has_tests_dir, state.has_gitignore);

    let narrative = build_narrative(&state, &structure, &findings, &recommendations);

    let report = AnalyzeReport {
        path: path.clone(),
        narrative: narrative.clone(),
        stats: stats.clone(),
        structure: structure.clone(),
        project_context: project_context.clone(),
        findings: findings.clone(),
        recommendations: recommendations.clone(),
        actions: actions.clone(),
        signals: signals.clone(),
        report_md: String::new(),
        llm_context: LlmContext {
            concise_summary: String::new(),
            key_risks: Vec::new(),
            top_recommendations: Vec::new(),
            signals: Vec::new(),
        },
    };
    let report_md = build_markdown_report(&report);
    let llm_context = build_llm_context(&report);

    Ok(AnalyzeReport {
        path: path.clone(),
        narrative: report.narrative,
        stats: report.stats,
        structure: report.structure,
        project_context: report.project_context,
        findings: report.findings,
        recommendations: report.recommendations,
        actions: report.actions,
        signals: report.signals,
        report_md,
        llm_context,
    })
}

fn build_actions(has_readme: bool, has_tests: bool, has_gitignore: bool) -> Vec<Action> {
    let mut actions = vec![];

    if !has_readme {
        actions.push(Action {
            id: "add-readme".into(),
            title: "Добавить README.md".into(),
            description: "В проекте отсутствует README.md".into(),
            kind: ActionKind::CreateFile,
            path: "README.md".into(),
            content: Some("# Project\n\nDescribe your project.\n".into()),
        });
    }

    if !has_tests {
        actions.push(Action {
            id: "add-tests-dir".into(),
            title: "Создать папку tests/".into(),
            description: "В проекте нет tests/ (минимальная структура для тестов)".into(),
            kind: ActionKind::CreateDir,
            path: "tests".into(),
            content: None,
        });
    }

    if !has_gitignore {
        actions.push(Action {
            id: "add-gitignore".into(),
            title: "Добавить .gitignore".into(),
            description: "Рекомендуется добавить базовый .gitignore".into(),
            kind: ActionKind::CreateFile,
            path: ".gitignore".into(),
            content: Some("node_modules/\ndist/\nbuild/\n.target/\n.DS_Store\n".into()),
        });
    }

    actions
}

fn enrich_recommendations(mut recs: Vec<Recommendation>) -> Vec<Recommendation> {
    for r in &mut recs {
        let (p, e, i) = if r.title.contains("README") {
            ("high", "low", "high")
        } else if r.title.contains("тест") || r.title.contains("тесты") || r.title.contains("Add tests") || r.title.contains("tests") {
            ("high", "medium", "high")
        } else if r.title.contains(".gitignore") {
            ("medium", "low", "medium")
        } else if r.title.contains(".env") || r.title.contains("секрет") {
            ("high", "low", "high")
        } else if r.title.contains("лицензи") {
            ("low", "low", "medium")
        } else if r.title.contains("линтер") || r.title.contains("Clippy") {
            ("medium", "low", "medium")
        } else {
            (r.priority.as_str(), r.effort.as_str(), r.impact.as_str())
        };
        r.priority = p.to_string();
        r.effort = e.to_string();
        r.impact = i.to_string();
    }
    recs
}

fn build_project_context(
    state: &ScanState,
    findings: &[Finding],
    signals: &[ProjectSignal],
) -> ProjectContext {
    let risk_level = if state.has_env
        || signals.iter().any(|s| s.category == "security" && s.level == "high")
    {
        "High"
    } else if findings.len() > 5
        || signals.iter().any(|s| s.level == "warn")
    {
        "Medium"
    } else {
        "Low"
    };

    let complexity = if state.file_count > 5000 || state.dir_count > 500 || state.max_depth > 8 {
        "High"
    } else if state.file_count > 800 || state.dir_count > 120 {
        "Medium"
    } else {
        "Low"
    };

    let maturity = if state.has_readme && (state.has_tests_dir || state.has_eslint) {
        "Production-like"
    } else if state.has_readme {
        "MVP"
    } else {
        "Prototype"
    };

    let mut stack = Vec::new();
    if state.has_package_json {
        stack.push("Node.js".to_string());
    }
    if state.has_cargo_toml {
        stack.push("Rust".to_string());
    }
    if state.has_vite {
        stack.push("Vite".to_string());
    }
    if state.has_next {
        stack.push("Next.js".to_string());
    }
    if state.has_pyproject || state.has_requirements_txt {
        stack.push("Python".to_string());
    }
    if stack.is_empty() {
        stack.push("Unknown".to_string());
    }

    let domain = if state.has_next || state.has_vite {
        "frontend"
    } else if state.has_cargo_toml {
        "systems"
    } else if state.has_package_json {
        "fullstack"
    } else {
        "general"
    }
    .to_string();

    ProjectContext {
        stack,
        domain,
        maturity: maturity.to_string(),
        complexity: complexity.to_string(),
        risk_level: risk_level.to_string(),
    }
}

fn build_markdown_report(report: &AnalyzeReport) -> String {
    let mut md = String::new();
    md.push_str("# PAPA YU — отчёт анализа проекта\n\n");
    md.push_str(&report.narrative);
    md.push_str("\n\n---\n\n");
    md.push_str("## Статистика\n\n");
    md.push_str(&format!(
        "- Файлов: {}\n- Папок: {}\n- Max depth: {}\n- Размер: {} Б\n\n",
        report.stats.file_count,
        report.stats.dir_count,
        report.stats.max_depth,
        report.stats.total_size_bytes
    ));
    md.push_str("## Контекст проекта\n\n");
    md.push_str(&format!(
        "- Стек: {}\n- Зрелость: {}\n- Сложность: {}\n- Риск: {}\n\n",
        report.project_context.stack.join(", "),
        report.project_context.maturity,
        report.project_context.complexity,
        report.project_context.risk_level
    ));
    if !report.findings.is_empty() {
        md.push_str("## Находки\n\n");
        for f in &report.findings {
            md.push_str(&format!("- **{}**: {}\n", f.title, f.details));
        }
        md.push_str("\n");
    }
    if !report.recommendations.is_empty() {
        md.push_str("## Рекомендации\n\n");
        for r in &report.recommendations {
            md.push_str(&format!(
                "- **{}** [{} / effort:{} / impact:{}]\n  {}\n",
                r.title, r.priority, r.effort, r.impact, r.details
            ));
        }
    }
    md
}

fn build_llm_context(report: &AnalyzeReport) -> LlmContext {
    let concise_summary = format!(
        "{}; {}; {} файлов, {} папок. Риск: {}, зрелость: {}.",
        report.structure.project_type,
        report.structure.architecture,
        report.stats.file_count,
        report.stats.dir_count,
        report.project_context.risk_level,
        report.project_context.maturity
    );
    let key_risks: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.severity == "high")
        .map(|f| format!("{}: {}", f.title, f.details))
        .collect();
    let top_recommendations: Vec<String> = report
        .recommendations
        .iter()
        .take(5)
        .map(|r| format!("[{}] {}", r.priority, r.title))
        .collect();
    LlmContext {
        concise_summary,
        key_risks,
        top_recommendations,
        signals: report.signals.clone(),
    }
}

fn build_structure(state: &ScanState) -> ProjectStructure {
    let mut project_type = String::new();
    let mut architecture = String::new();
    let mut structure_notes: Vec<String> = Vec::new();

    let is_monorepo = state.package_json_count > 1 || state.cargo_toml_count > 1;

    if state.has_cargo_toml {
        project_type = "Rust / Cargo".to_string();
        architecture = "Rust-проект".to_string();
        if is_monorepo {
            project_type = "Rust monorepo".to_string();
        }
    }
    if state.has_package_json {
        if !project_type.is_empty() {
            project_type = format!("{} + Node", project_type);
        } else if state.has_next {
            project_type = "Next.js".to_string();
            architecture = "React (Next.js) fullstack".to_string();
        } else if state.has_vite {
            project_type = "React + Vite".to_string();
            architecture = "Frontend SPA (Vite)".to_string();
        } else {
            project_type = "Node.js".to_string();
            architecture = "Node / frontend или backend".to_string();
        }
        if is_monorepo && !project_type.contains("monorepo") {
            project_type = format!("{} (monorepo)", project_type);
        }
    }
    if state.has_pyproject || state.has_requirements_txt || state.has_setup_py {
        if !project_type.is_empty() {
            project_type = format!("{} + Python", project_type);
        } else {
            project_type = "Python".to_string();
            architecture = "Python-проект (Django/FastAPI или скрипты)".to_string();
        }
    }
    if project_type.is_empty() {
        project_type = "Неопределён".to_string();
        architecture = "Тип по манифестам не определён".to_string();
    }

    if state.has_src && state.has_tests_dir {
        structure_notes.push("Есть src/ и tests/ — хорошее разделение.".to_string());
    } else if state.has_src && !state.has_tests_dir {
        structure_notes.push("Есть src/, нет tests/ — стоит добавить тесты.".to_string());
    }
    if state.root_file_count >= ROOT_FILES_WARN {
        structure_notes.push("Много файлов в корне — структура упрощённая.".to_string());
    }
    if state.max_depth >= MAX_DEPTH_WARN {
        structure_notes.push("Глубокая вложенность папок.".to_string());
    }
    if structure_notes.is_empty() {
        structure_notes.push("Структура без явного разделения на домены.".to_string());
    }

    ProjectStructure {
        project_type,
        architecture,
        structure_notes,
    }
}

fn build_narrative(
    state: &ScanState,
    structure: &ProjectStructure,
    findings: &[Finding],
    recommendations: &[Recommendation],
) -> String {
    let mut parts = Vec::new();
    parts.push("Я проанализировал ваш проект.".to_string());
    parts.push(format!(
        "Это {} ({}).",
        structure.project_type.to_lowercase(),
        structure.architecture
    ));
    if !structure.structure_notes.is_empty() {
        parts.push(structure.structure_notes.join(" "));
    }

    let size_label = if state.file_count < 50 {
        "небольшой"
    } else if state.file_count < 500 {
        "среднего размера"
    } else {
        "крупный"
    };
    parts.push(format!(
        "По размеру — {} проект: {} файлов, {} папок.",
        size_label, state.file_count, state.dir_count
    ));

    if !findings.is_empty() {
        parts.push("".to_string());
        parts.push("Основные проблемы:".to_string());
        for f in findings.iter().take(7) {
            parts.push(format!("– {}", f.title));
        }
    }

    if !recommendations.is_empty() {
        parts.push("".to_string());
        parts.push("Я бы рекомендовал начать с:".to_string());
        for (i, r) in recommendations.iter().take(5).enumerate() {
            parts.push(format!("{}. {}", i + 1, r.title));
        }
    }

    parts.join("\n\n")
}

fn scan_dir(
    root: &Path,
    dir: &Path,
    depth: u32,
    state: &mut ScanState,
    deadline: &Instant,
) -> Result<(), String> {
    if Instant::now() > *deadline {
        return Err("Превышено время анализа (таймаут)".to_string());
    }
    if state.file_count >= MAX_FILES {
        return Err("Превышен лимит количества файлов".to_string());
    }

    if depth > state.max_depth {
        state.max_depth = depth;
    }

    let is_root = dir == root;

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.is_symlink() {
            continue;
        }

        if meta.is_dir() {
            state.dir_count += 1;
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if is_root {
                state.root_dirs.insert(name.to_lowercase());
            }
            let name_lower = name.to_lowercase();
            if EXCLUDED_DIRS.contains(&name) {
                continue;
            }
            if name_lower == "src" {
                state.has_src = true;
            }
            if name_lower == "tests" || name_lower == "test" || name_lower == "__tests__" {
                state.has_tests_dir = true;
            }
            if name_lower == "components" {
                state.has_components = true;
            }
            if name_lower == "pages" || name_lower == "app" {
                state.has_pages = true;
            }
            scan_dir(root, &path, depth + 1, state, deadline)?;
            continue;
        }

        state.file_count += 1;
        if state.file_count >= MAX_FILES {
            return Err("Превышен лимит количества файлов".to_string());
        }

        if is_root {
            state.root_file_count += 1;
        }

        state.total_size_bytes = state.total_size_bytes.saturating_add(meta.len());

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            let name_lower = name.to_lowercase();
            if let Some(ext) = path.extension() {
                let ext = ext.to_string_lossy().to_string();
                *state.extensions.entry(ext).or_insert(0) += 1;
            }

            if name_lower == "package.json" {
                state.has_package_json = true;
                state.package_json_count += 1;
            }
            if name_lower == "cargo.toml" {
                state.has_cargo_toml = true;
                state.cargo_toml_count += 1;
            }
            if name_lower == "tsconfig.json" {
                state.has_tsconfig = true;
            }
            if name_lower == "dockerfile" || name_lower == "docker-compose.yml" {
                state.has_docker = true;
            }
            if name_lower.starts_with(".env") {
                state.has_env = true;
            }
            if name_lower == ".gitignore" {
                state.has_gitignore = true;
            }
            if name_lower == "license" || name_lower == "license.md" || name_lower.starts_with("license.") {
                state.has_license = true;
            }
            if name_lower == "eslint.config.js" || name_lower == ".eslintrc" || name_lower.starts_with(".eslintrc") {
                state.has_eslint = true;
            }
            if name_lower == ".prettierrc" || name_lower == "prettier.config" || name_lower.starts_with("prettier.config") {
                state.has_prettier = true;
            }
            if name_lower == "next.config.js" || name_lower == "next.config.mjs" || name_lower == "next.config.ts" {
                state.has_next = true;
            }
            if name_lower == "requirements.txt" {
                state.has_requirements_txt = true;
            }
            if name_lower == "pyproject.toml" {
                state.has_pyproject = true;
            }
            if name_lower == "setup.py" {
                state.has_setup_py = true;
            }
            for m in MARKER_README {
                if name_lower.starts_with(&m.to_lowercase()) {
                    state.has_readme = true;
                    break;
                }
            }
            for m in MARKER_VITE {
                if name_lower == *m {
                    state.has_vite = true;
                    break;
                }
            }
        }
    }

    Ok(())
}
