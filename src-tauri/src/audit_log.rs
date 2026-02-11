//! Журнал аудита: запись событий (анализ, apply, undo) в файл.
//! Файл: app_data_dir/papa-yu/audit.log или project_path/.papa-yu/audit.log при указании пути проекта.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub ts: String,
    pub event_type: String,
    pub project_path: Option<String>,
    pub result: Option<String>,
    pub details: Option<String>,
}

fn audit_file_path(base_dir: &Path) -> std::path::PathBuf {
    base_dir.join("papa-yu").join("audit.log")
}

/// Записывает событие в audit.log в app_data_dir (глобальный лог приложения).
pub fn log_event(
    app_audit_dir: &Path,
    event_type: &str,
    project_path: Option<&str>,
    result: Option<&str>,
    details: Option<&str>,
) -> Result<(), String> {
    let file_path = audit_file_path(app_audit_dir);
    if let Some(parent) = file_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .map_err(|e| format!("audit log open: {}", e))?;
    let ts = Utc::now().to_rfc3339();
    let line = serde_json::json!({
        "ts": ts,
        "event_type": event_type,
        "project_path": project_path,
        "result": result,
        "details": details
    });
    writeln!(file, "{}", line).map_err(|e| format!("audit log write: {}", e))?;
    file.flush().map_err(|e| format!("audit log flush: {}", e))?;
    Ok(())
}

/// Читает последние N строк из audit.log. Возвращает события от новых к старым.
pub fn read_events(app_audit_dir: &Path, limit: usize) -> Vec<AuditEvent> {
    let file_path = audit_file_path(app_audit_dir);
    let content = match std::fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let lines: Vec<&str> = content.lines().rev().take(limit).collect();
    let mut out = Vec::with_capacity(lines.len());
    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Ok(ev) = serde_json::from_str::<AuditEvent>(trimmed) {
            out.push(ev);
        }
    }
    out.reverse();
    out
}
