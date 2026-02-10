//! Select relevant notes for goal and build PROJECT_DOMAIN_NOTES block.

use std::path::Path;

use super::storage::{
    load_domain_notes, mark_note_used, notes_max_total_chars, save_domain_notes, DomainNote,
};

/// Simple tokenize: split on whitespace, lowercase, non-empty.
fn tokenize(s: &str) -> std::collections::HashSet<String> {
    s.to_lowercase()
        .split_whitespace()
        .filter(|w| w.len() > 1)
        .map(|w| w.to_string())
        .collect()
}

/// Score note relevance to goal by token overlap (tags, topic, content_md).
fn score_note(goal_tokens: &std::collections::HashSet<String>, note: &DomainNote) -> usize {
    let topic_tags = tokenize(&note.topic);
    let tags: std::collections::HashSet<String> =
        note.tags.iter().map(|t| t.to_lowercase()).collect();
    let content = tokenize(&note.content_md);
    let mut all = topic_tags;
    all.extend(tags);
    all.extend(content);
    goal_tokens.intersection(&all).count()
}

/// Select notes most relevant to goal_text, up to max_total_chars. Returns (selected notes, total chars).
pub fn select_relevant_notes(
    goal_text: &str,
    notes: &[DomainNote],
    max_total_chars: usize,
) -> Vec<DomainNote> {
    let goal_tokens = tokenize(goal_text);
    if goal_tokens.is_empty() {
        return notes.iter().take(10).cloned().collect();
    }

    let mut scored: Vec<(usize, DomainNote)> = notes
        .iter()
        .map(|n| (score_note(&goal_tokens, n), n.clone()))
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));

    let mut out = Vec::new();
    let mut total = 0usize;
    for (_, note) in scored {
        let len = note.content_md.len() + note.topic.len() + 50;
        if total + len > max_total_chars && !out.is_empty() {
            break;
        }
        total += len;
        out.push(note);
    }
    out
}

/// Build PROJECT_DOMAIN_NOTES block text.
fn build_notes_block(notes: &[DomainNote]) -> String {
    let mut s = String::from("\n\nPROJECT_DOMAIN_NOTES (curated, may be stale):\n");
    for n in notes {
        s.push_str(&format!("- [{}] {}\n", n.topic, n.content_md));
        if !n.sources.is_empty() {
            let urls: Vec<&str> = n.sources.iter().take(3).map(|x| x.url.as_str()).collect();
            s.push_str(&format!("  sources: {}\n", urls.join(", ")));
        }
    }
    s
}

/// Load notes, select relevant to goal, build block, mark used, save. Returns (block, note_ids, chars_used).
pub fn get_notes_block_for_prompt(
    project_path: &Path,
    goal_text: &str,
) -> Option<(String, Vec<String>, usize)> {
    let mut data = load_domain_notes(project_path);
    if data.notes.is_empty() {
        return None;
    }

    let max_chars = notes_max_total_chars();
    let selected = select_relevant_notes(goal_text, &data.notes, max_chars);
    if selected.is_empty() {
        return None;
    }

    let ids: Vec<String> = selected.iter().map(|n| n.id.clone()).collect();
    let block = build_notes_block(&selected);
    let chars_used = block.chars().count();

    for id in &ids {
        if let Some(n) = data.notes.iter_mut().find(|x| x.id == *id) {
            mark_note_used(n);
        }
    }
    let _ = save_domain_notes(project_path, data);

    Some((block, ids, chars_used))
}
