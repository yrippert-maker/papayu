use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Window};

use crate::types::{Action, ActionKind, DiffItem, PreviewResult};

const PROGRESS_EVENT: &str = "analyze_progress";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewPayload {
    pub path: String,
    pub actions: Vec<Action>,
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

fn read_text_if_exists(p: &Path) -> Option<String> {
    if !p.exists() || p.is_dir() {
        return None;
    }
    let bytes = fs::read(p).ok()?;
    if bytes.len() > 200_000 {
        return Some("[слишком большой файл для предпросмотра]".into());
    }
    String::from_utf8(bytes).ok()
}

fn summarize(kind: &str, path: &str) -> String {
    match kind {
        "create" => format!("Создать файл {}", path),
        "update" => format!("Обновить файл {}", path),
        "delete" => format!("Удалить файл {}", path),
        "mkdir" => format!("Создать папку {}", path),
        "rmdir" => format!("Удалить папку {}", path),
        _ => format!("Изменение {}", path),
    }
}

#[tauri::command]
pub async fn preview_actions(
    window: Window,
    _app: AppHandle,
    payload: PreviewPayload,
) -> PreviewResult {
    let project_root = PathBuf::from(&payload.path);
    if !project_root.exists() || !project_root.is_dir() {
        return PreviewResult {
            ok: false,
            diffs: vec![],
            error: Some("path_invalid".into()),
            error_code: Some("PATH_INVALID".into()),
        };
    }

    let _ = window.emit(PROGRESS_EVENT, "Готовлю предпросмотр изменений…");

    let mut diffs: Vec<DiffItem> = vec![];

    for a in payload.actions {
        let abs = match safe_join(&project_root, &a.path) {
            Ok(p) => p,
            Err(e) => {
                return PreviewResult {
                    ok: false,
                    diffs: vec![],
                    error: Some(e),
                    error_code: Some("PATH_DENIED".into()),
                };
            }
        };

        match a.kind {
            ActionKind::CreateDir => {
                diffs.push(DiffItem {
                    path: a.path.clone(),
                    kind: "mkdir".into(),
                    before: None,
                    after: None,
                    summary: summarize("mkdir", &a.path),
                });
            }
            ActionKind::DeleteDir => {
                diffs.push(DiffItem {
                    path: a.path.clone(),
                    kind: "rmdir".into(),
                    before: None,
                    after: None,
                    summary: summarize("rmdir", &a.path),
                });
            }
            ActionKind::CreateFile => {
                diffs.push(DiffItem {
                    path: a.path.clone(),
                    kind: "create".into(),
                    before: None,
                    after: a.content.clone(),
                    summary: summarize("create", &a.path),
                });
            }
            ActionKind::UpdateFile => {
                diffs.push(DiffItem {
                    path: a.path.clone(),
                    kind: "update".into(),
                    before: read_text_if_exists(&abs),
                    after: a.content.clone(),
                    summary: summarize("update", &a.path),
                });
            }
            ActionKind::DeleteFile => {
                diffs.push(DiffItem {
                    path: a.path.clone(),
                    kind: "delete".into(),
                    before: read_text_if_exists(&abs),
                    after: None,
                    summary: summarize("delete", &a.path),
                });
            }
        }
    }

    PreviewResult {
        ok: true,
        diffs,
        error: None,
        error_code: None,
    }
}
