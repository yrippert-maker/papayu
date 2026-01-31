use tauri::AppHandle;

use crate::tx::{get_undo_redo_state, pop_undo, push_redo, rollback_tx};
use crate::types::{UndoAvailableResult, UndoRedoState, UndoResult};

#[tauri::command]
pub async fn get_undo_redo_state_cmd(app: AppHandle) -> UndoRedoState {
    let (undo_available, redo_available) = get_undo_redo_state(&app);
    UndoRedoState {
        undo_available,
        redo_available,
    }
}

#[tauri::command]
pub async fn undo_available(app: AppHandle) -> UndoAvailableResult {
    let (undo_avail, _) = get_undo_redo_state(&app);
    UndoAvailableResult {
        ok: true,
        available: undo_avail,
        tx_id: None,
    }
}

#[tauri::command]
pub async fn undo_last(app: AppHandle) -> UndoResult {
    let Some(tx_id) = pop_undo(&app) else {
        return UndoResult {
            ok: false,
            tx_id: None,
            error: Some("nothing to undo".into()),
            error_code: Some("UNDO_NOTHING".into()),
        };
    };

    if let Err(e) = rollback_tx(&app, &tx_id) {
        return UndoResult {
            ok: false,
            tx_id: Some(tx_id),
            error: Some(e),
            error_code: Some("ROLLBACK_FAILED".into()),
        };
    }

    let _ = push_redo(&app, tx_id.clone());

    UndoResult {
        ok: true,
        tx_id: Some(tx_id),
        error: None,
        error_code: None,
    }
}
