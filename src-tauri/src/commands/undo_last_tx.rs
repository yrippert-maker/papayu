//! v3.1: откат последней транзакции из history/tx + history/snapshots

use std::fs;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for e in fs::read_dir(src).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let sp = e.path();
        let dp = dst.join(e.file_name());
        let ft = e.file_type().map_err(|e| e.to_string())?;
        if ft.is_dir() {
            copy_dir(&sp, &dp)?;
        } else if ft.is_file() {
            fs::copy(&sp, &dp).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn undo_last_tx(app: AppHandle, path: String) -> Result<bool, String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let tx_dir = data_dir.join("history").join("tx");
    let snap_base = data_dir.join("history").join("snapshots");

    if !tx_dir.exists() {
        return Ok(false);
    }

    let mut items: Vec<(std::time::SystemTime, PathBuf)> = vec![];
    for e in fs::read_dir(&tx_dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let meta = e.metadata().map_err(|e| e.to_string())?;
        let m = meta.modified().map_err(|e| e.to_string())?;
        items.push((m, e.path()));
    }
    items.sort_by(|a, b| b.0.cmp(&a.0));
    let last = match items.first() {
        Some((_, p)) => p.clone(),
        None => return Ok(false),
    };

    let raw = fs::read_to_string(&last).map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let tx_id = v
        .get("txId")
        .and_then(|x| x.as_str())
        .ok_or("txId missing")?;
    let tx_path = v.get("path").and_then(|x| x.as_str()).unwrap_or("");
    if tx_path != path {
        return Ok(false);
    }

    let snap_dir = snap_base.join(tx_id);
    if !snap_dir.exists() {
        return Ok(false);
    }

    let root = PathBuf::from(&path);
    if !root.exists() {
        return Ok(false);
    }

    let exclude = [
        ".git",
        "node_modules",
        "dist",
        "build",
        ".next",
        "target",
        ".cache",
        "coverage",
    ];
    for entry in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        let name = entry.file_name();
        if exclude
            .iter()
            .any(|x| name.to_string_lossy().as_ref() == *x)
        {
            continue;
        }
        if p.is_dir() {
            fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(&p).map_err(|e| e.to_string())?;
        }
    }

    copy_dir(&snap_dir, &root)?;
    Ok(true)
}
