//! v2.9.3: доступен ли откат (есть ли последняя транзакция в papayu/transactions)

use std::fs;
use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::types::UndoStatus;

#[tauri::command]
pub async fn undo_status(app: AppHandle) -> UndoStatus {
    let base: PathBuf = match app.path().app_data_dir() {
        Ok(v) => v,
        Err(_) => {
            return UndoStatus {
                available: false,
                tx_id: None,
            }
        }
    };

    let dir = base.join("history").join("tx");
    let Ok(rd) = fs::read_dir(&dir) else {
        return UndoStatus {
            available: false,
            tx_id: None,
        };
    };

    let last = rd
        .filter_map(|e| e.ok())
        .max_by_key(|e| e.metadata().ok().and_then(|m| m.modified().ok()));

    match last {
        Some(f) => {
            let name = f.file_name().to_string_lossy().to_string();
            UndoStatus {
                available: true,
                tx_id: Some(name),
            }
        }
        None => UndoStatus {
            available: false,
            tx_id: None,
        },
    }
}
