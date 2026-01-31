mod limits;
mod store;

pub use limits::preflight_actions;
pub use store::{clear_redo, get_undo_redo_state, pop_redo, pop_undo, push_redo, push_undo};

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde_json::json;
use tauri::{AppHandle, Manager};

use crate::types::{Action, ActionKind, TxManifest, TxTouchedItem};

pub fn user_data_dir(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("app_data_dir")
}

pub fn history_dir(app: &AppHandle) -> PathBuf {
    user_data_dir(app).join("history")
}

pub fn tx_dir(app: &AppHandle, tx_id: &str) -> PathBuf {
    history_dir(app).join(tx_id)
}

pub fn tx_manifest_path(app: &AppHandle, tx_id: &str) -> PathBuf {
    tx_dir(app, tx_id).join("manifest.json")
}

pub fn tx_before_dir(app: &AppHandle, tx_id: &str) -> PathBuf {
    tx_dir(app, tx_id).join("before")
}

pub fn ensure_history(app: &AppHandle) -> io::Result<()> {
    fs::create_dir_all(history_dir(app))?;
    Ok(())
}

pub fn new_tx_id() -> String {
    format!("tx-{}", Utc::now().format("%Y%m%d-%H%M%S-%3f"))
}

pub fn write_manifest(app: &AppHandle, manifest: &TxManifest) -> io::Result<()> {
    let tx_id = &manifest.tx_id;
    fs::create_dir_all(tx_dir(app, tx_id))?;
    let p = tx_manifest_path(app, tx_id);
    let bytes = serde_json::to_vec_pretty(manifest).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(p, bytes)?;
    Ok(())
}

pub fn read_manifest(app: &AppHandle, tx_id: &str) -> io::Result<TxManifest> {
    let p = tx_manifest_path(app, tx_id);
    let bytes = fs::read(p)?;
    serde_json::from_slice(&bytes).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

#[allow(dead_code)]
pub fn set_latest_tx(app: &AppHandle, tx_id: &str) -> io::Result<()> {
    let p = history_dir(app).join("latest.json");
    let bytes = serde_json::to_vec_pretty(&json!({ "txId": tx_id })).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    fs::write(p, bytes)?;
    Ok(())
}

#[allow(dead_code)]
pub fn clear_latest_tx(app: &AppHandle) -> io::Result<()> {
    let p = history_dir(app).join("latest.json");
    let _ = fs::remove_file(p);
    Ok(())
}

#[allow(dead_code)]
pub fn get_latest_tx(app: &AppHandle) -> Option<String> {
    let p = history_dir(app).join("latest.json");
    let bytes = fs::read(p).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v.get("txId")?.as_str().map(|s| s.to_string())
}

/// Safe join: root + relative (forbids absolute and "..")
pub fn safe_join(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rp = Path::new(rel);
    if rp.is_absolute() {
        return Err("absolute paths forbidden".into());
    }
    if rel.contains("..") {
        return Err("path traversal forbidden".into());
    }
    Ok(root.join(rp))
}

/// Snapshot: only copy existed files to before/; build touched (rel_path, kind, existed, bytes).
pub fn snapshot_before(
    app: &AppHandle,
    tx_id: &str,
    root: &Path,
    rel_paths: &[String],
) -> Result<Vec<TxTouchedItem>, String> {
    let before = tx_before_dir(app, tx_id);
    fs::create_dir_all(&before).map_err(|e| e.to_string())?;

    let mut touched = vec![];

    for rel in rel_paths {
        let abs = safe_join(root, rel)?;
        if abs.exists() && !abs.is_symlink() {
            if abs.is_file() {
                let bytes = fs::metadata(&abs).map(|m| m.len()).unwrap_or(0);
                let dst = safe_join(&before, rel)?;
                if let Some(parent) = dst.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::copy(&abs, &dst).map_err(|e| e.to_string())?;
                touched.push(TxTouchedItem {
                    rel_path: rel.clone(),
                    kind: "file".into(),
                    existed: true,
                    bytes,
                });
            } else if abs.is_dir() {
                touched.push(TxTouchedItem {
                    rel_path: rel.clone(),
                    kind: "dir".into(),
                    existed: true,
                    bytes: 0,
                });
            }
        } else {
            touched.push(TxTouchedItem {
                rel_path: rel.clone(),
                kind: if rel.ends_with('/') || rel.is_empty() { "dir".into() } else { "file".into() },
                existed: false,
                bytes: 0,
            });
        }
    }

    Ok(touched)
}

/// Rollback tx: existed file -> restore from before; created file/dir -> remove; existed dir -> skip.
pub fn rollback_tx(app: &AppHandle, tx_id: &str) -> Result<(), String> {
    let mut manifest = read_manifest(app, tx_id).map_err(|e| e.to_string())?;
    let root = PathBuf::from(manifest.root_path.clone());
    let before = tx_before_dir(app, tx_id);

    let items: Vec<(String, String, bool)> = if !manifest.touched.is_empty() {
        manifest.touched.iter().map(|t| (t.rel_path.clone(), t.kind.clone(), t.existed)).collect()
    } else if let Some(ref snap) = manifest.snapshot_items {
        snap.iter().map(|s| (s.rel_path.clone(), s.kind.clone(), s.existed)).collect()
    } else {
        return Err("manifest has no touched or snapshot_items".into());
    };

    for (rel, kind, existed) in items {
        let abs = safe_join(&root, &rel)?;
        let src = safe_join(&before, &rel).ok();

        if existed {
            if kind == "file" {
                if let Some(ref s) = src {
                    if s.is_file() {
                        if let Some(parent) = abs.parent() {
                            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        fs::copy(s, &abs).map_err(|e| e.to_string())?;
                    }
                }
            }
            // existed dir: skip (nothing to restore)
        } else {
            if abs.is_file() {
                let _ = fs::remove_file(&abs);
            }
            if abs.is_dir() {
                let _ = fs::remove_dir_all(&abs);
            }
        }
    }

    manifest.status = "rolled_back".into();
    let _ = write_manifest(app, &manifest);
    Ok(())
}

/// Collect unique rel_paths from actions (for snapshot).
pub fn collect_rel_paths(actions: &[Action]) -> Vec<String> {
    let mut paths: Vec<String> = actions.iter().map(|a| a.path.clone()).collect();
    paths.sort();
    paths.dedup();
    paths
}

/// PAPAYU_NORMALIZE_EOL=lf — нормализовать \r\n→\n, добавить trailing newline.
pub fn normalize_content_for_write(content: &str, _path: &Path) -> String {
    let mode = std::env::var("PAPAYU_NORMALIZE_EOL")
        .map(|s| s.trim().to_lowercase())
        .unwrap_or_else(|_| "keep".to_string());
    if mode != "lf" {
        return content.to_string();
    }
    let mut s = content.replace("\r\n", "\n").replace('\r', "\n");
    if !s.is_empty() && !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// Apply a single action to disk (v2.3.3: for atomic apply + rollback on first failure).
pub fn apply_one_action(root: &Path, action: &Action) -> Result<(), String> {
    let full = safe_join(root, &action.path)?;
    match action.kind {
        ActionKind::CreateFile | ActionKind::UpdateFile => {
            if let Some(p) = full.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let content = action.content.as_deref().unwrap_or("");
            let normalized = normalize_content_for_write(content, &full);
            fs::write(&full, normalized).map_err(|e| e.to_string())?;
        }
        ActionKind::CreateDir => {
            fs::create_dir_all(&full).map_err(|e| e.to_string())?;
        }
        ActionKind::DeleteFile => {
            if full.exists() {
                fs::remove_file(&full).map_err(|e| e.to_string())?;
            }
        }
        ActionKind::DeleteDir => {
            if full.is_dir() {
                fs::remove_dir_all(&full).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Порядок применения: CREATE_DIR → CREATE_FILE/UPDATE_FILE → DELETE_FILE → DELETE_DIR.
pub fn sort_actions_for_apply(actions: &mut [Action]) {
    fn order(k: &ActionKind) -> u8 {
        match k {
            ActionKind::CreateDir => 0,
            ActionKind::CreateFile | ActionKind::UpdateFile => 1,
            ActionKind::DeleteFile => 2,
            ActionKind::DeleteDir => 3,
        }
    }
    actions.sort_by_key(|a| (order(&a.kind), a.path.clone()));
}

/// Apply actions to disk (create/update/delete files and dirs).
/// Actions are sorted: CREATE_DIR → CREATE/UPDATE → DELETE_FILE → DELETE_DIR.
pub fn apply_actions_to_disk(root: &Path, actions: &[Action]) -> Result<(), String> {
    let mut sorted: Vec<Action> = actions.to_vec();
    sort_actions_for_apply(&mut sorted);
    for a in &sorted {
        apply_one_action(root, a)?;
    }
    Ok(())
}
