use crate::tx::safe_join;
use crate::types::{ActionKind, ApplyPayload, DiffItem, PreviewResult};
use std::fs;

const MAX_PREVIEW_SIZE: usize = 200_000;

pub fn preview_actions(payload: ApplyPayload) -> Result<PreviewResult, String> {
    let root = std::path::Path::new(&payload.root_path);
    let mut diffs = Vec::new();
    for a in &payload.actions {
        let rel = a.path.as_str();
        if is_protected_file(rel) || !is_text_allowed(rel) {
            diffs.push(DiffItem {
                kind: "blocked".to_string(),
                path: a.path.clone(),
                old_content: Some("(blocked)".to_string()),
                new_content: Some("(blocked)".to_string()),
                summary: Some("BLOCKED: protected or non-text file".to_string()),
            });
            continue;
        }
        let item = match &a.kind {
            ActionKind::CreateFile => DiffItem {
                kind: "create".to_string(),
                path: a.path.clone(),
                old_content: None,
                new_content: a.content.clone(),
                summary: None,
            },
            ActionKind::CreateDir => DiffItem {
                kind: "mkdir".to_string(),
                path: a.path.clone(),
                old_content: None,
                new_content: None,
                summary: None,
            },
            ActionKind::UpdateFile => {
                let old = read_text_if_exists(root, &a.path);
                DiffItem {
                    kind: "update".to_string(),
                    path: a.path.clone(),
                    old_content: old,
                    new_content: a.content.clone(),
                    summary: None,
                }
            }
            ActionKind::DeleteFile => {
                let old = read_text_if_exists(root, &a.path);
                DiffItem {
                    kind: "delete".to_string(),
                    path: a.path.clone(),
                    old_content: old,
                    new_content: None,
                    summary: None,
                }
            }
            ActionKind::DeleteDir => DiffItem {
                kind: "rmdir".to_string(),
                path: a.path.clone(),
                old_content: None,
                new_content: None,
                summary: None,
            },
        };
        diffs.push(item);
    }
    let summary = summarize(&diffs);
    let files = diffs.len();
    let bytes = diffs
        .iter()
        .map(|d| d.old_content.as_ref().unwrap_or(&String::new()).len() + d.new_content.as_ref().unwrap_or(&String::new()).len())
        .sum::<usize>();
    eprintln!("[PREVIEW_READY] path={} files={} diffs={} bytes={}", payload.root_path, files, diffs.len(), bytes);
    Ok(PreviewResult { diffs, summary })
}

fn read_text_if_exists(root: &std::path::Path, rel: &str) -> Option<String> {
    let p = safe_join(root, rel).ok()?;
    if !p.is_file() {
        return None;
    }
    let s = fs::read_to_string(&p).ok()?;
    if s.len() > MAX_PREVIEW_SIZE {
        Some(format!("{}... (truncated)", &s[..MAX_PREVIEW_SIZE]))
    } else {
        Some(s)
    }
}

fn summarize(diffs: &[DiffItem]) -> String {
    let create = diffs.iter().filter(|d| d.kind == "create").count();
    let update = diffs.iter().filter(|d| d.kind == "update").count();
    let delete = diffs.iter().filter(|d| d.kind == "delete").count();
    let mkdir = diffs.iter().filter(|d| d.kind == "mkdir").count();
    let rmdir = diffs.iter().filter(|d| d.kind == "rmdir").count();
    let blocked = diffs.iter().filter(|d| d.kind == "blocked").count();
    let mut s = format!(
        "Создать: {}, изменить: {}, удалить: {}, mkdir: {}, rmdir: {}",
        create, update, delete, mkdir, rmdir
    );
    if blocked > 0 {
        s.push_str(&format!(", заблокировано: {}", blocked));
    }
    s
}

fn is_protected_file(p: &str) -> bool {
    let lower = p.to_lowercase().replace('\\', "/");
    if lower == ".env" || lower.ends_with("/.env") { return true; }
    if lower.ends_with(".pem") || lower.ends_with(".key") || lower.ends_with(".p12") { return true; }
    if lower.contains("id_rsa") { return true; }
    if lower.contains("/secrets/") || lower.starts_with("secrets/") { return true; }
    if lower.ends_with("cargo.lock") { return true; }
    if lower.ends_with("package-lock.json") { return true; }
    if lower.ends_with("pnpm-lock.yaml") { return true; }
    if lower.ends_with("yarn.lock") { return true; }
    if lower.ends_with("composer.lock") { return true; }
    if lower.ends_with("poetry.lock") { return true; }
    if lower.ends_with("pipfile.lock") { return true; }
    let bin_ext = [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg",
        ".pdf", ".zip", ".7z", ".rar", ".dmg", ".pkg",
        ".exe", ".dll", ".so", ".dylib", ".bin",
        ".mp3", ".mp4", ".mov", ".avi",
        ".wasm", ".class",
    ];
    for ext in bin_ext {
        if lower.ends_with(ext) { return true; }
    }
    false
}

fn is_text_allowed(p: &str) -> bool {
    let lower = p.to_lowercase();
    let ok_ext = [
        ".ts", ".tsx", ".js", ".jsx", ".json", ".md", ".txt", ".toml", ".yaml", ".yml",
        ".rs", ".py", ".go", ".java", ".kt", ".c", ".cpp", ".h", ".hpp",
        ".css", ".scss", ".html", ".env", ".gitignore", ".editorconfig",
    ];
    ok_ext.iter().any(|e| lower.ends_with(e)) || !lower.contains('.')
}
