use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, Window};

use crate::types::UndoResult;

const PROGRESS_EVENT: &str = "analyze_progress";

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|_| "app_data_dir_unavailable".to_string())
}

fn revert_snapshot(
    session_dir: &PathBuf,
    project_root: &PathBuf,
) -> Result<Vec<String>, String> {
    let snap_dir = session_dir.join("snapshot");
    if !snap_dir.exists() {
        return Err("snapshot_missing".into());
    }

    let mut restored = vec![];

    for entry in walkdir::WalkDir::new(&snap_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_dir() {
            continue;
        }
        let snap_path = entry.path().to_path_buf();
        let rel = snap_path
            .strip_prefix(&snap_dir)
            .map_err(|e| e.to_string())?;

        let rel_str = rel.to_string_lossy();
        if rel_str.starts_with(".missing/") || rel_str.starts_with(".missing\\") {
            let orig: &str = rel_str
                .strip_prefix(".missing/")
                .or_else(|| rel_str.strip_prefix(".missing\\"))
                .unwrap_or(&rel_str);
            let abs = project_root.join(orig);
            if abs.exists() {
                fs::remove_file(&abs).map_err(|e| e.to_string())?;
                restored.push(orig.to_string());
            }
            continue;
        }

        let abs = project_root.join(rel);
        if let Some(parent) = abs.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::copy(&snap_path, &abs).map_err(|e| e.to_string())?;
        restored.push(rel.to_string_lossy().to_string());
    }

    Ok(restored)
}

#[tauri::command]
pub async fn undo_last(window: Window, app: AppHandle, path: String) -> UndoResult {
    let project_root = PathBuf::from(&path);
    if !project_root.exists() || !project_root.is_dir() {
        return UndoResult {
            ok: false,
            session_id: String::new(),
            restored: vec![],
            error: Some("path_invalid".into()),
            error_code: Some("PATH_INVALID".into()),
        };
    }

    let data_dir = match app_data_dir(&app) {
        Ok(d) => d,
        Err(e) => {
            return UndoResult {
                ok: false,
                session_id: String::new(),
                restored: vec![],
                error: Some(e),
                error_code: Some("APP_DATA_DIR".into()),
            };
        }
    };

    let last_path = data_dir.join("history").join("last_session.txt");
    let session_id = match fs::read_to_string(&last_path) {
        Ok(s) => s.trim().to_string(),
        Err(_) => {
            return UndoResult {
                ok: false,
                session_id: String::new(),
                restored: vec![],
                error: Some("no_undo_available".into()),
                error_code: Some("UNDO_NOT_AVAILABLE".into()),
            };
        }
    };

    let session_dir = data_dir.join("history").join(&session_id);

    let _ = window.emit(PROGRESS_EVENT, "Откатываю изменения…");

    match revert_snapshot(&session_dir, &project_root) {
        Ok(restored) => UndoResult {
            ok: true,
            session_id,
            restored,
            error: None,
            error_code: None,
        },
        Err(e) => UndoResult {
            ok: false,
            session_id,
            restored: vec![],
            error: Some(e),
            error_code: Some("UNDO_FAILED".into()),
        },
    }
}
