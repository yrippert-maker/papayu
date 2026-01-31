use std::path::Path;
use tauri::AppHandle;

use crate::tx::{apply_actions_to_disk, pop_redo, push_undo, read_manifest};
use crate::types::RedoResult;

#[tauri::command]
pub async fn redo_last(app: AppHandle) -> RedoResult {
    let Some(tx_id) = pop_redo(&app) else {
        return RedoResult {
            ok: false,
            tx_id: None,
            error: Some("nothing to redo".into()),
            error_code: Some("REDO_NOTHING".into()),
        };
    };

    let manifest = match read_manifest(&app, &tx_id) {
        Ok(m) => m,
        Err(e) => {
            return RedoResult {
                ok: false,
                tx_id: Some(tx_id),
                error: Some(e.to_string()),
                error_code: Some("REDO_READ_MANIFEST_FAILED".into()),
            };
        }
    };

    if manifest.applied_actions.is_empty() {
        return RedoResult {
            ok: false,
            tx_id: Some(tx_id),
            error: Some("Legacy transaction cannot be redone (no applied_actions)".into()),
            error_code: Some("REDO_LEGACY".into()),
        };
    }

    let root = Path::new(&manifest.root_path);
    if let Err(e) = apply_actions_to_disk(root, &manifest.applied_actions) {
        return RedoResult {
            ok: false,
            tx_id: Some(tx_id),
            error: Some(e),
            error_code: Some("REDO_APPLY_FAILED".into()),
        };
    }

    let _ = push_undo(&app, tx_id.clone());

    RedoResult {
        ok: true,
        tx_id: Some(tx_id),
        error: None,
        error_code: None,
    }
}
