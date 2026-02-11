use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Window};

use crate::types::{Action, ActionKind, ApplyResult};

const PROGRESS_EVENT: &str = "analyze_progress";

fn app_data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|_| "app_data_dir_unavailable".to_string())
}

fn safe_join(base: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel_path = PathBuf::from(rel);
    if rel_path.is_absolute() {
        return Err("absolute_path_denied".into());
    }
    if rel.contains("..") {
        return Err("path_traversal_denied".into());
    }
    Ok(base.join(rel_path))
}

fn snapshot_paths(
    session_dir: &Path,
    project_root: &Path,
    targets: &[PathBuf],
) -> Result<(), String> {
    let snap_dir = session_dir.join("snapshot");
    fs::create_dir_all(&snap_dir).map_err(|e| e.to_string())?;

    for t in targets {
        let abs = project_root.join(t);
        let snap = snap_dir.join(t);

        if abs.is_dir() {
            fs::create_dir_all(&snap).map_err(|e| e.to_string())?;
            continue;
        }

        if abs.exists() {
            if let Some(parent) = snap.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&abs, &snap).map_err(|e| e.to_string())?;
        } else {
            let missing_marker = snap_dir.join(".missing").join(t);
            if let Some(parent) = missing_marker.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(&missing_marker, b"").map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

fn revert_snapshot(session_dir: &Path, project_root: &Path) -> Result<Vec<String>, String> {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPayload {
    pub path: String,
    pub actions: Vec<Action>,
}

#[tauri::command]
pub async fn apply_actions(window: Window, app: AppHandle, payload: ApplyPayload) -> ApplyResult {
    let project_root = PathBuf::from(&payload.path);
    if !project_root.exists() || !project_root.is_dir() {
        return ApplyResult {
            ok: false,
            session_id: String::new(),
            applied: vec![],
            skipped: payload.actions.iter().map(|a| a.id.clone()).collect(),
            error: Some("path_invalid".into()),
            error_code: Some("PATH_INVALID".into()),
            undo_available: false,
        };
    }

    let data_dir = match app_data_dir(&app) {
        Ok(d) => d,
        Err(e) => {
            return ApplyResult {
                ok: false,
                session_id: String::new(),
                applied: vec![],
                skipped: vec![],
                error: Some(e),
                error_code: Some("APP_DATA_DIR".into()),
                undo_available: false,
            };
        }
    };

    let session_id = format!("{}", chrono::Utc::now().timestamp_millis());
    let session_dir = data_dir.join("history").join(&session_id);

    if fs::create_dir_all(&session_dir).is_err() {
        return ApplyResult {
            ok: false,
            session_id: session_id.clone(),
            applied: vec![],
            skipped: vec![],
            error: Some("HISTORY_CREATE_FAILED".into()),
            error_code: Some("HISTORY_CREATE_FAILED".into()),
            undo_available: false,
        };
    }

    let _ = window.emit(PROGRESS_EVENT, "Готовлю откат (snapshot)…");

    let targets: Vec<PathBuf> = payload.actions.iter().map(|a| PathBuf::from(&a.path)).collect();

    if let Err(e) = snapshot_paths(&session_dir, &project_root, &targets) {
        return ApplyResult {
            ok: false,
            session_id: session_id.clone(),
            applied: vec![],
            skipped: payload.actions.iter().map(|a| a.id.clone()).collect(),
            error: Some(e),
            error_code: Some("SNAPSHOT_FAILED".into()),
            undo_available: false,
        };
    }

    let _ = window.emit(PROGRESS_EVENT, "Применяю изменения…");

    let mut applied = vec![];

    let result_apply = (|| -> Result<(), String> {
        for a in &payload.actions {
            let abs = safe_join(&project_root, &a.path)?;

            match a.kind {
                ActionKind::CreateDir => {
                    fs::create_dir_all(&abs).map_err(|e| e.to_string())?;
                }
                ActionKind::DeleteDir => {
                    if abs.exists() {
                        fs::remove_dir_all(&abs).map_err(|e| e.to_string())?;
                    }
                }
                ActionKind::CreateFile | ActionKind::UpdateFile => {
                    let content = a
                        .content
                        .clone()
                        .ok_or_else(|| "content_missing".to_string())?;
                    if let Some(parent) = abs.parent() {
                        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    fs::write(&abs, content.as_bytes()).map_err(|e| e.to_string())?;
                }
                ActionKind::DeleteFile => {
                    if abs.exists() {
                        fs::remove_file(&abs).map_err(|e| e.to_string())?;
                    }
                }
            }

            applied.push(a.id.clone());
        }
        Ok(())
    })();

    if let Err(err) = result_apply {
        let _ = window.emit(PROGRESS_EVENT, "Обнаружена ошибка. Откатываю изменения…");
        let _ = revert_snapshot(&session_dir, &project_root);
        let skipped: Vec<String> = payload
            .actions
            .iter()
            .map(|a| a.id.clone())
            .filter(|id| !applied.contains(id))
            .collect();
        return ApplyResult {
            ok: false,
            session_id,
            applied,
            skipped,
            error: Some(err),
            error_code: Some("APPLY_FAILED_ROLLED_BACK".into()),
            undo_available: false,
        };
    }

    let _ = fs::write(
        data_dir.join("history").join("last_session.txt"),
        session_id.as_bytes(),
    );

    let _ = window.emit(PROGRESS_EVENT, "Готово. Изменения применены.");

    ApplyResult {
        ok: true,
        session_id,
        applied,
        skipped: vec![],
        error: None,
        error_code: None,
        undo_available: true,
    }
}
