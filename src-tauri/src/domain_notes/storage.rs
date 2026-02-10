//! Load/save domain_notes.json and eviction.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainNotes {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub updated_at: i64,
    pub notes: Vec<DomainNote>,
}

fn default_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainNote {
    pub id: String,
    #[serde(default)]
    pub created_at: i64,
    pub topic: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub content_md: String,
    #[serde(default)]
    pub sources: Vec<NoteSource>,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default = "default_ttl_days")]
    pub ttl_days: u32,
    #[serde(default)]
    pub usage_count: u32,
    #[serde(default)]
    pub last_used_at: Option<i64>,
    #[serde(default)]
    pub pinned: bool,
}

fn default_ttl_days() -> u32 {
    notes_ttl_days()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteSource {
    pub url: String,
    #[serde(default)]
    pub title: String,
}

/// PAPAYU_NOTES_MAX_ITEMS (default 50)
pub fn notes_max_items() -> usize {
    std::env::var("PAPAYU_NOTES_MAX_ITEMS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(50)
        .clamp(5, 200)
}

/// PAPAYU_NOTES_MAX_CHARS_PER_NOTE (default 800)
pub fn notes_max_chars_per_note() -> usize {
    std::env::var("PAPAYU_NOTES_MAX_CHARS_PER_NOTE")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(800)
        .clamp(128, 2000)
}

/// PAPAYU_NOTES_MAX_TOTAL_CHARS (default 4000)
pub fn notes_max_total_chars() -> usize {
    std::env::var("PAPAYU_NOTES_MAX_TOTAL_CHARS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(4000)
        .clamp(512, 16000)
}

/// PAPAYU_NOTES_TTL_DAYS (default 30)
pub fn notes_ttl_days() -> u32 {
    std::env::var("PAPAYU_NOTES_TTL_DAYS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(30)
        .clamp(1, 365)
}

fn notes_file_path(project_path: &Path) -> std::path::PathBuf {
    project_path
        .join(".papa-yu")
        .join("notes")
        .join("domain_notes.json")
}

/// Load domain notes from project. Returns empty notes if file missing or invalid.
pub fn load_domain_notes(project_path: &Path) -> DomainNotes {
    let path = notes_file_path(project_path);
    let Ok(data) = fs::read_to_string(&path) else {
        return DomainNotes {
            schema_version: 1,
            updated_at: 0,
            notes: vec![],
        };
    };
    match serde_json::from_str::<DomainNotes>(&data) {
        Ok(mut d) => {
            d.notes.retain(|n| !is_note_expired(n));
            d
        }
        Err(_) => DomainNotes {
            schema_version: 1,
            updated_at: 0,
            notes: vec![],
        },
    }
}

/// Returns true if note is past TTL.
pub fn is_note_expired(note: &DomainNote) -> bool {
    let ttl_sec = (note.ttl_days as i64) * 24 * 3600;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    now - note.created_at > ttl_sec
}

/// Evict: drop expired, then by LRU (least recently used first: last_used_at, usage_count, created_at) until <= max_items.
/// Pinned notes are never evicted.
fn evict_notes(notes: &mut Vec<DomainNote>, max_items: usize) {
    notes.retain(|n| !is_note_expired(n) || n.pinned);
    if notes.len() <= max_items {
        return;
    }
    let (pinned, mut non_pinned): (Vec<DomainNote>, Vec<DomainNote>) =
        notes.drain(..).partition(|n| n.pinned);
    non_pinned.sort_by(|a, b| {
        let a_used = a.last_used_at.unwrap_or(0);
        let b_used = b.last_used_at.unwrap_or(0);
        a_used
            .cmp(&b_used)
            .then_with(|| a.usage_count.cmp(&b.usage_count))
            .then_with(|| a.created_at.cmp(&b.created_at))
    });
    let keep_count = max_items.saturating_sub(pinned.len());
    let to_take = keep_count.min(non_pinned.len());
    let start = non_pinned.len().saturating_sub(to_take);
    let kept: Vec<DomainNote> = non_pinned.drain(start..).collect();
    notes.extend(pinned);
    notes.extend(kept);
}

/// Save domain notes to project. Creates .papa-yu/notes if needed. Applies eviction before save.
pub fn save_domain_notes(project_path: &Path, mut data: DomainNotes) -> Result<(), String> {
    let max_items = notes_max_items();
    evict_notes(&mut data.notes, max_items);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|e| e.to_string())?;
    data.updated_at = now;

    let path = notes_file_path(project_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create notes dir: {}", e))?;
    }
    let json = serde_json::to_string_pretty(&data).map_err(|e| format!("serialize: {}", e))?;
    fs::write(&path, json).map_err(|e| format!("write: {}", e))?;
    Ok(())
}

/// Mark a note as used (usage_count += 1, last_used_at = now). Call after injecting into prompt.
pub fn mark_note_used(note: &mut DomainNote) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    note.usage_count = note.usage_count.saturating_add(1);
    note.last_used_at = Some(now);
}

/// Delete note by id. Returns true if removed.
pub fn delete_note(project_path: &Path, note_id: &str) -> Result<bool, String> {
    let mut data = load_domain_notes(project_path);
    let len_before = data.notes.len();
    data.notes.retain(|n| n.id != note_id);
    let removed = data.notes.len() < len_before;
    if removed {
        save_domain_notes(project_path, data)?;
    }
    Ok(removed)
}

/// Remove expired notes (non-pinned). Returns count removed.
pub fn clear_expired_notes(project_path: &Path) -> Result<usize, String> {
    let mut data = load_domain_notes(project_path);
    let before = data.notes.len();
    data.notes.retain(|n| !is_note_expired(n) || n.pinned);
    let removed = before - data.notes.len();
    if removed > 0 {
        save_domain_notes(project_path, data)?;
    }
    Ok(removed)
}

/// Set pinned flag for a note.
pub fn pin_note(project_path: &Path, note_id: &str, pinned: bool) -> Result<bool, String> {
    let mut data = load_domain_notes(project_path);
    let mut found = false;
    for n in &mut data.notes {
        if n.id == note_id {
            n.pinned = pinned;
            found = true;
            break;
        }
    }
    if found {
        save_domain_notes(project_path, data)?;
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_note_expired_fresh() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let note = DomainNote {
            id: "x".into(),
            created_at: now - 1000,
            topic: "t".into(),
            tags: vec![],
            content_md: "c".into(),
            sources: vec![],
            confidence: 0.8,
            ttl_days: 30,
            usage_count: 0,
            last_used_at: None,
            pinned: false,
        };
        assert!(!is_note_expired(&note));
    }

    #[test]
    fn test_notes_limits_defaults() {
        std::env::remove_var("PAPAYU_NOTES_MAX_ITEMS");
        assert!(notes_max_items() >= 5 && notes_max_items() <= 200);
        assert!(notes_max_chars_per_note() >= 128);
        assert!(notes_max_total_chars() >= 512);
    }
}
