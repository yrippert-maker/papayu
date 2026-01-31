use std::path::Path;
use tauri::AppHandle;

use crate::commands::auto_check::auto_check;
use crate::tx::{
    apply_one_action, clear_redo, collect_rel_paths, ensure_history, new_tx_id,
    preflight_actions, push_undo, rollback_tx, snapshot_before, sort_actions_for_apply, write_manifest,
};
use crate::types::{ApplyPayload, ApplyResult, TxManifest};

pub const AUTO_CHECK_FAILED_REVERTED: &str = "AUTO_CHECK_FAILED_REVERTED";
#[allow(dead_code)]
pub const APPLY_FAILED_REVERTED: &str = "APPLY_FAILED_REVERTED";
/// v2.3.3: apply failed at step N, rolled back applied steps
pub const AUTO_ROLLBACK_DONE: &str = "AUTO_ROLLBACK_DONE";

pub fn apply_actions(app: AppHandle, payload: ApplyPayload) -> ApplyResult {
    let root = match Path::new(&payload.root_path).canonicalize() {
        Ok(p) => p,
        Err(_) => Path::new(&payload.root_path).to_path_buf(),
    };
    if !root.exists() || !root.is_dir() {
        return ApplyResult {
            ok: false,
            tx_id: None,
            applied_count: None,
            failed_at: None,
            error: Some("path invalid".into()),
            error_code: Some("PATH_INVALID".into()),
        };
    }

    if !payload.user_confirmed {
        return ApplyResult {
            ok: false,
            tx_id: None,
            applied_count: None,
            failed_at: None,
            error: Some("confirmation required".into()),
            error_code: Some("CONFIRM_REQUIRED".into()),
        };
    }

    if ensure_history(&app).is_err() {
        return ApplyResult {
            ok: false,
            tx_id: None,
            applied_count: None,
            failed_at: None,
            error: Some("history init failed".into()),
            error_code: Some("HISTORY_INIT_FAILED".into()),
        };
    }

    if payload.actions.is_empty() {
        return ApplyResult {
            ok: true,
            tx_id: None,
            applied_count: Some(0),
            failed_at: None,
            error: None,
            error_code: None,
        };
    }

    if let Err((msg, code)) = preflight_actions(&root, &payload.actions) {
        return ApplyResult {
            ok: false,
            tx_id: None,
            applied_count: None,
            failed_at: None,
            error: Some(msg),
            error_code: Some(code),
        };
    }

    let tx_id = new_tx_id();
    let rel_paths = collect_rel_paths(&payload.actions);
    let touched = match snapshot_before(&app, &tx_id, &root, &rel_paths) {
        Ok(t) => t,
        Err(e) => {
            return ApplyResult {
                ok: false,
                tx_id: Some(tx_id.clone()),
                applied_count: None,
                failed_at: None,
                error: Some(e),
                error_code: Some("SNAPSHOT_FAILED".into()),
            };
        }
    };

    let mut manifest = TxManifest {
        tx_id: tx_id.clone(),
        root_path: payload.root_path.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        label: payload.label.clone(),
        status: "pending".into(),
        applied_actions: payload.actions.clone(),
        touched: touched.clone(),
        auto_check: payload.auto_check.unwrap_or(false),
        snapshot_items: None,
    };

    if let Err(e) = write_manifest(&app, &manifest) {
        return ApplyResult {
            ok: false,
            tx_id: Some(tx_id),
            applied_count: None,
            failed_at: None,
            error: Some(e.to_string()),
            error_code: Some("MANIFEST_WRITE_FAILED".into()),
        };
    }

    // v2.4.2: guard — запрет lock/бинарников/не-текстовых
    for action in &payload.actions {
        let rel = action.path.as_str();
        if is_protected_file(rel) || !is_text_allowed(rel) {
            return ApplyResult {
                ok: false,
                tx_id: Some(tx_id.clone()),
                applied_count: None,
                failed_at: None,
                error: Some(format!("protected or non-text file: {}", rel)),
                error_code: Some("PROTECTED_PATH".into()),
            };
        }
    }

    // v2.3.3: apply one-by-one; on first failure rollback and return AUTO_ROLLBACK_DONE
    // Порядок применения: CREATE_DIR → CREATE/UPDATE → DELETE_FILE → DELETE_DIR
    let mut sorted_actions = payload.actions.clone();
    sort_actions_for_apply(&mut sorted_actions);
    for (i, action) in sorted_actions.iter().enumerate() {
        if let Err(e) = apply_one_action(&root, action) {
            let _ = rollback_tx(&app, &tx_id);
            manifest.status = "rolled_back".into();
            let _ = write_manifest(&app, &manifest);
            return ApplyResult {
                ok: false,
                tx_id: Some(tx_id.clone()),
                applied_count: Some(i),
                failed_at: Some(i),
                error: Some(format!("apply failed, rolled back: {}", e)),
                error_code: Some(AUTO_ROLLBACK_DONE.into()),
            };
        }
    }

    if payload.auto_check.unwrap_or(false) {
        if let Err(_) = auto_check(&root) {
            let _ = rollback_tx(&app, &tx_id);
            return ApplyResult {
                ok: false,
                tx_id: Some(tx_id),
                applied_count: Some(payload.actions.len()),
                failed_at: None,
                error: Some("Ошибки после изменений. Откат выполнен.".into()),
                error_code: Some(AUTO_CHECK_FAILED_REVERTED.into()),
            };
        }
    }

    manifest.status = "committed".into();
    let _ = write_manifest(&app, &manifest);
    let _ = push_undo(&app, tx_id.clone());
    let _ = clear_redo(&app);

    ApplyResult {
        ok: true,
        tx_id: Some(tx_id),
        applied_count: Some(payload.actions.len()),
        failed_at: None,
        error: None,
        error_code: None,
    }
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
