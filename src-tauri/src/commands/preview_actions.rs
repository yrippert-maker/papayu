use crate::patch::{apply_unified_diff_to_text, looks_like_unified_diff, sha256_hex};
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
                bytes_before: None,
                bytes_after: None,
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
                bytes_before: None,
                bytes_after: None,
            },
            ActionKind::CreateDir => DiffItem {
                kind: "mkdir".to_string(),
                path: a.path.clone(),
                old_content: None,
                new_content: None,
                summary: None,
                bytes_before: None,
                bytes_after: None,
            },
            ActionKind::UpdateFile => {
                let old = read_text_if_exists(root, &a.path);
                DiffItem {
                    kind: "update".to_string(),
                    path: a.path.clone(),
                    old_content: old.clone(),
                    new_content: a.content.clone(),
                    summary: None,
                    bytes_before: old.as_ref().map(|s| s.len()),
                    bytes_after: a.content.as_ref().map(|s| s.len()),
                }
            }
            ActionKind::PatchFile => {
                let (diff, summary, bytes_before, bytes_after) = preview_patch_file(root, &a.path, a.patch.as_deref().unwrap_or(""), a.base_sha256.as_deref().unwrap_or(""));
                DiffItem {
                    kind: "patch".to_string(),
                    path: a.path.clone(),
                    old_content: None,
                    new_content: Some(diff),
                    summary,
                    bytes_before,
                    bytes_after,
                }
            }
            ActionKind::DeleteFile => {
                let old = read_text_if_exists(root, &a.path);
                DiffItem {
                    kind: "delete".to_string(),
                    path: a.path.clone(),
                    old_content: old.clone(),
                    new_content: None,
                    summary: None,
                    bytes_before: old.as_ref().map(|s| s.len()),
                    bytes_after: None,
                }
            }
            ActionKind::DeleteDir => DiffItem {
                kind: "rmdir".to_string(),
                path: a.path.clone(),
                old_content: None,
                new_content: None,
                summary: None,
                bytes_before: None,
                bytes_after: None,
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

/// Returns (diff, summary, bytes_before, bytes_after).
fn preview_patch_file(
    root: &std::path::Path,
    rel: &str,
    patch_text: &str,
    base_sha256: &str,
) -> (String, Option<String>, Option<usize>, Option<usize>) {
    if !looks_like_unified_diff(patch_text) {
        return (patch_text.to_string(), Some("ERR_PATCH_NOT_UNIFIED: patch is not unified diff".into()), None, None);
    }
    let p = match safe_join(root, rel) {
        Ok(p) => p,
        Err(_) => return (patch_text.to_string(), Some("ERR_INVALID_PATH".into()), None, None),
    };
    if !p.is_file() {
        return (patch_text.to_string(), Some("ERR_BASE_MISMATCH: file not found".into()), None, None);
    }
    let old_bytes = match fs::read(&p) {
        Ok(b) => b,
        Err(_) => return (patch_text.to_string(), Some("ERR_IO: cannot read file".into()), None, None),
    };
    let old_sha = sha256_hex(&old_bytes);
    if old_sha != base_sha256 {
        return (patch_text.to_string(), Some(format!("ERR_BASE_MISMATCH: have {}, want {}", old_sha, base_sha256)), None, None);
    }
    let old_text = match String::from_utf8(old_bytes) {
        Ok(s) => s,
        Err(_) => return (patch_text.to_string(), Some("ERR_NON_UTF8_FILE: PATCH_FILE требует UTF-8. Файл не UTF-8.".into()), None, None),
    };
    let bytes_before = old_text.len();
    match apply_unified_diff_to_text(&old_text, patch_text) {
        Ok(new_text) => (patch_text.to_string(), None, Some(bytes_before), Some(new_text.len())),
        Err(_) => (patch_text.to_string(), Some("ERR_PATCH_APPLY_FAILED: could not apply patch".into()), None, None),
    }
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
    let patch = diffs.iter().filter(|d| d.kind == "patch").count();
    let delete = diffs.iter().filter(|d| d.kind == "delete").count();
    let mkdir = diffs.iter().filter(|d| d.kind == "mkdir").count();
    let rmdir = diffs.iter().filter(|d| d.kind == "rmdir").count();
    let blocked = diffs.iter().filter(|d| d.kind == "blocked").count();
    let mut s = format!(
        "Создать: {}, изменить: {}, patch: {}, удалить: {}, mkdir: {}, rmdir: {}",
        create, update, patch, delete, mkdir, rmdir
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
