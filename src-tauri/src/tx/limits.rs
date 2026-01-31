//! v2.3.4 Safe Guards: preflight checks and limits.

use std::path::Path;

use crate::types::{Action, ActionKind};
use crate::tx::safe_join;

pub const MAX_ACTIONS: usize = 50;
pub const MAX_FILES_TOUCHED: usize = 50;
pub const MAX_BYTES_WRITTEN: u64 = 2 * 1024 * 1024; // 2MB
pub const MAX_DIRS_CREATED: usize = 20;
pub const MAX_FILE_SIZE_UPDATE: u64 = 1024 * 1024; // 1MB for UpdateFile content

static FORBIDDEN_PREFIXES: &[&str] = &[
    ".git/",
    "node_modules/",
    "target/",
    "dist/",
    "build/",
    ".next/",
    ".cache/",
    "coverage/",
];

pub const PRECHECK_DENIED: &str = "PRECHECK_DENIED";
pub const LIMIT_EXCEEDED: &str = "LIMIT_EXCEEDED";
pub const PATH_FORBIDDEN: &str = "PATH_FORBIDDEN";

/// Preflight: validate paths and limits. Returns Err((message, error_code)) on failure.
pub fn preflight_actions(root: &Path, actions: &[Action]) -> Result<(), (String, String)> {
    if actions.len() > MAX_ACTIONS {
        return Err((
            format!("Превышен лимит действий: {} (макс. {})", actions.len(), MAX_ACTIONS),
            LIMIT_EXCEEDED.into(),
        ));
    }

    let mut files_touched = 0usize;
    let mut dirs_created = 0usize;
    let mut total_bytes: u64 = 0;

    for a in actions {
        let rel = a.path.replace('\\', "/");
        if rel.contains("..") {
            return Err(("Путь не должен содержать ..".into(), PATH_FORBIDDEN.into()));
        }
        if Path::new(&rel).is_absolute() {
            return Err(("Абсолютные пути запрещены".into(), PATH_FORBIDDEN.into()));
        }

        for prefix in FORBIDDEN_PREFIXES {
            if rel.starts_with(prefix) || rel == prefix.trim_end_matches('/') {
                return Err((
                    format!("Запрещённая зона: {}", rel),
                    PATH_FORBIDDEN.into(),
                ));
            }
        }

        let abs = safe_join(root, &rel).map_err(|e| (e, PATH_FORBIDDEN.into()))?;
        if abs.exists() && abs.is_symlink() {
            return Err(("Симлинки не поддерживаются".into(), PRECHECK_DENIED.into()));
        }

        match a.kind {
            ActionKind::CreateFile | ActionKind::UpdateFile => {
                files_touched += 1;
                let len = a.content.as_deref().map(|s| s.len() as u64).unwrap_or(0);
                if a.kind == ActionKind::UpdateFile && len > MAX_FILE_SIZE_UPDATE {
                    return Err((
                        format!("Файл для обновления слишком большой: {} байт", len),
                        LIMIT_EXCEEDED.into(),
                    ));
                }
                total_bytes += len;
            }
            ActionKind::PatchFile => {
                files_touched += 1;
                total_bytes += a.patch.as_deref().map(|s| s.len() as u64).unwrap_or(0);
            }
            ActionKind::CreateDir => {
                dirs_created += 1;
            }
            ActionKind::DeleteFile => {
                files_touched += 1;
            }
            ActionKind::DeleteDir => {}
        }
    }

    if files_touched > MAX_FILES_TOUCHED {
        return Err((
            format!("Превышен лимит файлов: {} (макс. {})", files_touched, MAX_FILES_TOUCHED),
            LIMIT_EXCEEDED.into(),
        ));
    }
    if dirs_created > MAX_DIRS_CREATED {
        return Err((
            format!("Превышен лимит создаваемых папок: {} (макс. {})", dirs_created, MAX_DIRS_CREATED),
            LIMIT_EXCEEDED.into(),
        ));
    }
    if total_bytes > MAX_BYTES_WRITTEN {
        return Err((
            format!("Превышен лимит объёма записи: {} байт (макс. {})", total_bytes, MAX_BYTES_WRITTEN),
            LIMIT_EXCEEDED.into(),
        ));
    }

    Ok(())
}
