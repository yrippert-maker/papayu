//! v3.2: generate Action[] from AnalyzeReport (safe create-only, no LLM).

use std::path::Path;

use crate::tx::safe_join;
use crate::types::{Action, ActionKind, AnalyzeReport, GenerateActionsResult};

const MAX_ACTIONS: usize = 20;

/// Forbidden path segments (no write under these).
const FORBIDDEN: &[&str] = &[".git", "node_modules", "target", "dist", "build", ".next"];

fn rel(p: &str) -> String {
    p.replace('\\', "/")
}

fn is_path_forbidden(rel: &str) -> bool {
    let r = rel.trim_start_matches('/');
    if r.contains("..") || rel.starts_with('/') || rel.starts_with('\\') {
        return true;
    }
    let parts: Vec<&str> = r.split('/').collect();
    for part in &parts {
        if FORBIDDEN.contains(part) {
            return true;
        }
    }
    false
}

fn has_readme(root: &Path) -> bool {
    ["README.md", "README.MD", "README.txt", "README"]
        .iter()
        .any(|f| root.join(f).exists())
}

fn has_gitignore(root: &Path) -> bool {
    root.join(".gitignore").exists()
}

fn has_license(root: &Path) -> bool {
    ["LICENSE", "LICENSE.md", "LICENSE.txt"]
        .iter()
        .any(|f| root.join(f).exists())
}

fn has_src(root: &Path) -> bool {
    root.join("src").is_dir()
}

fn has_tests(root: &Path) -> bool {
    root.join("tests").is_dir()
}

#[tauri::command]
pub async fn generate_actions_from_report(
    path: String,
    report: AnalyzeReport,
    mode: String,
) -> GenerateActionsResult {
    let _ = report; // reserved for future use (e.g. narrative/signals)
    let root = Path::new(&path);
    if !root.exists() || !root.is_dir() {
        return GenerateActionsResult {
            ok: false,
            actions: vec![],
            skipped: vec![],
            error: Some("path not found".into()),
            error_code: Some("PATH_NOT_FOUND".into()),
        };
    }

    let create_only = mode == "safe_create_only" || mode == "safe" || mode.is_empty();
    let mut actions: Vec<Action> = vec![];
    let mut skipped: Vec<String> = vec![];

    // 1. README
    if !has_readme(root) {
        let rel_path = rel("README.md");
        if is_path_forbidden(&rel_path) {
            skipped.push("README.md (forbidden path)".into());
        } else if safe_join(root, &rel_path).is_ok() {
            actions.push(Action {
                kind: ActionKind::CreateFile,
                path: rel_path.clone(),
                content: Some(
                    "# Project\n\n## Описание\n\nКратко опишите проект.\n\n## Запуск\n\n- dev: ...\n- build: ...\n\n## Структура\n\n- src/\n- tests/\n".into(),
                ),
            });
        }
    }

    // 2. .gitignore
    if !has_gitignore(root) {
        let rel_path = rel(".gitignore");
        if is_path_forbidden(&rel_path) {
            skipped.push(".gitignore (forbidden path)".into());
        } else if safe_join(root, &rel_path).is_ok() {
            actions.push(Action {
                kind: ActionKind::CreateFile,
                path: rel_path,
                content: Some(
                    "node_modules/\ndist/\nbuild/\n.next/\ncoverage/\n.env\n.env.*\n.DS_Store\n.target/\n".into(),
                ),
            });
        }
    }

    // 3. LICENSE
    if !has_license(root) {
        let rel_path = rel("LICENSE");
        if is_path_forbidden(&rel_path) {
            skipped.push("LICENSE (forbidden path)".into());
        } else if safe_join(root, &rel_path).is_ok() {
            actions.push(Action {
                kind: ActionKind::CreateFile,
                path: rel_path,
                content: Some("MIT License\n\nCopyright (c) <year> <copyright holders>\n".into()),
            });
        }
    }

    // 4. tests/ + tests/.gitkeep (when src exists and tests missing)
    if has_src(root) && !has_tests(root) {
        let dir_path = rel("tests");
        if !is_path_forbidden(&dir_path) && safe_join(root, &dir_path).is_ok() {
            actions.push(Action {
                kind: ActionKind::CreateDir,
                path: dir_path,
                content: None,
            });
        }
        let keep_path = rel("tests/.gitkeep");
        if !is_path_forbidden(&keep_path) && safe_join(root, &keep_path).is_ok() {
            actions.push(Action {
                kind: ActionKind::CreateFile,
                path: keep_path,
                content: Some("".into()),
            });
        }
    }

    if create_only {
        // v3.3: only CreateFile and CreateDir; any other kind would be skipped (we already only create)
    }

    if actions.len() > MAX_ACTIONS {
        return GenerateActionsResult {
            ok: false,
            actions: vec![],
            skipped: vec![format!("more than {} actions", MAX_ACTIONS)],
            error: Some(format!("max {} actions per run", MAX_ACTIONS)),
            error_code: Some("TOO_MANY_ACTIONS".into()),
        };
    }

    GenerateActionsResult {
        ok: true,
        actions,
        skipped,
        error: None,
        error_code: None,
    }
}
