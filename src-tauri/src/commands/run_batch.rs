use std::path::Path;

use crate::commands::get_project_profile::get_project_limits;
use crate::commands::{analyze_project, apply_actions, preview_actions};
use crate::tx::get_undo_redo_state;
use crate::types::{BatchEvent, BatchPayload};
use tauri::AppHandle;

pub async fn run_batch(app: AppHandle, payload: BatchPayload) -> Result<Vec<BatchEvent>, String> {
    let mut events = Vec::new();

    let paths = if payload.paths.is_empty() {
        vec![".".to_string()]
    } else {
        payload.paths.clone()
    };

    let report = analyze_project(paths.clone(), payload.attached_files.clone()).map_err(|e| e.to_string())?;
    events.push(BatchEvent {
        kind: "report".to_string(),
        report: Some(report.clone()),
        preview: None,
        apply_result: None,
        message: Some(report.narrative.clone()),
        undo_available: None,
    });

    let actions = payload
        .selected_actions
        .unwrap_or(report.actions.clone());
    if actions.is_empty() {
        return Ok(events);
    }

    let root_path = report.path.clone();
    let preview = preview_actions(crate::types::ApplyPayload {
        root_path: root_path.clone(),
        actions: actions.clone(),
        auto_check: None,
        label: None,
        user_confirmed: false,
    })
    .map_err(|e| e.to_string())?;
    events.push(BatchEvent {
        kind: "preview".to_string(),
        report: None,
        preview: Some(preview),
        apply_result: None,
        message: None,
        undo_available: None,
    });

    if !payload.confirm_apply {
        return Ok(events);
    }

    let limits = get_project_limits(Path::new(&root_path));
    if actions.len() > limits.max_actions_per_tx as usize {
        return Err(format!(
            "too many actions: {} > {} (max_actions_per_tx)",
            actions.len(),
            limits.max_actions_per_tx
        ));
    }

    let result = apply_actions(
        app.clone(),
        crate::types::ApplyPayload {
            root_path: root_path.clone(),
            actions,
            auto_check: Some(payload.auto_check),
            label: None,
            user_confirmed: payload.user_confirmed,
        },
    );
    let (undo_avail, _) = get_undo_redo_state(&app);
    events.push(BatchEvent {
        kind: "apply".to_string(),
        report: None,
        preview: None,
        apply_result: Some(result.clone()),
        message: result.error.clone(),
        undo_available: Some(result.ok && undo_avail),
    });

    Ok(events)
}
