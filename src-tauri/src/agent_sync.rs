//! Запись agent-sync.json для синхронизации с Cursor / Claude Code.
//! Включается через PAPAYU_AGENT_SYNC=1.
//! Опционально: Snyk Code (PAPAYU_SNYK_SYNC=1), Documatic — архитектура из .papa-yu/architecture.md.

use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::Serialize;

use crate::types::{AnalyzeReport, Finding};

#[derive(Serialize)]
struct AgentSyncPayload {
    path: String,
    updated_at: String,
    narrative: String,
    findings_count: usize,
    actions_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    snyk_findings: Option<Vec<Finding>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    architecture_summary: Option<String>,
}

/// Читает описание архитектуры для агента (Documatic и др.): .papa-yu/architecture.md или путь из PAPAYU_DOCUMATIC_ARCH_PATH.
fn read_architecture_summary(project_root: &Path) -> Option<String> {
    let path = std::env::var("PAPAYU_DOCUMATIC_ARCH_PATH")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|s| project_root.join(s))
        .unwrap_or_else(|| project_root.join(".papa-yu").join("architecture.md"));
    if path.exists() {
        fs::read_to_string(&path)
            .ok()
            .map(|s| s.chars().take(16_000).collect())
    } else {
        None
    }
}

/// Записывает .papa-yu/agent-sync.json в корень проекта при PAPAYU_AGENT_SYNC=1.
/// snyk_findings — при PAPAYU_SNYK_SYNC=1 (подгружается снаружи асинхронно).
pub fn write_agent_sync_if_enabled(report: &AnalyzeReport, snyk_findings: Option<Vec<Finding>>) {
    let enabled = std::env::var("PAPAYU_AGENT_SYNC")
        .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    if !enabled {
        return;
    }
    let root = Path::new(&report.path);
    if !root.is_dir() {
        return;
    }
    let dir = root.join(".papa-yu");
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("agent_sync: create_dir_all .papa-yu: {}", e);
        return;
    }
    let file = dir.join("agent-sync.json");
    let architecture_summary = read_architecture_summary(root);
    let payload = AgentSyncPayload {
        path: report.path.clone(),
        updated_at: Utc::now().to_rfc3339(),
        narrative: report.narrative.clone(),
        findings_count: report.findings.len(),
        actions_count: report.actions.len(),
        snyk_findings,
        architecture_summary,
    };
    let json = match serde_json::to_string_pretty(&payload) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("agent_sync: serialize: {}", e);
            return;
        }
    };
    if let Err(e) = fs::write(&file, json) {
        eprintln!("agent_sync: write {}: {}", file.display(), e);
    }
}
