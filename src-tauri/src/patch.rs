//! PATCH_FILE engine: sha256, unified diff validation, apply.
//! v3 EDIT_FILE engine: anchor/before/after replace.

use crate::types::EditOp;
use sha2::{Digest, Sha256};

pub const ERR_NON_UTF8_FILE: &str = "ERR_NON_UTF8_FILE";
pub const ERR_EDIT_BASE_MISMATCH: &str = "ERR_EDIT_BASE_MISMATCH";
pub const ERR_EDIT_ANCHOR_NOT_FOUND: &str = "ERR_EDIT_ANCHOR_NOT_FOUND";
pub const ERR_EDIT_BEFORE_NOT_FOUND: &str = "ERR_EDIT_BEFORE_NOT_FOUND";
pub const ERR_EDIT_AMBIGUOUS: &str = "ERR_EDIT_AMBIGUOUS";
pub const ERR_EDIT_APPLY_FAILED: &str = "ERR_EDIT_APPLY_FAILED";
pub const ERR_EDIT_BASE_SHA256_INVALID: &str = "ERR_EDIT_BASE_SHA256_INVALID";
pub const ERR_EDIT_NO_EDITS: &str = "ERR_EDIT_NO_EDITS";

const EDIT_WINDOW_CHARS: usize = 4000;

/// SHA256 hex (lowercase) от bytes.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Проверка: строка — валидный sha256 hex (64 символа, 0-9a-f).
pub fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Минимальная проверка unified diff: хотя бы один hunk, желательно ---/+++.
pub fn looks_like_unified_diff(patch: &str) -> bool {
    let mut has_hunk = false;
    let mut has_minus_file = false;
    let mut has_plus_file = false;

    for line in patch.lines() {
        if line.starts_with("@@") {
            has_hunk = true;
        }
        if line.starts_with("--- ") {
            has_minus_file = true;
        }
        if line.starts_with("+++ ") {
            has_plus_file = true;
        }
    }

    has_hunk && ((has_minus_file && has_plus_file) || patch.len() > 40)
}

/// Применяет unified diff к тексту. Возвращает Err("parse_failed") или Err("apply_failed").
pub fn apply_unified_diff_to_text(
    old_text: &str,
    patch_text: &str,
) -> Result<String, &'static str> {
    use diffy::{apply, Patch};
    let patch = Patch::from_str(patch_text).map_err(|_| "parse_failed")?;
    apply(old_text, &patch).map_err(|_| "apply_failed")
}

/// PAPAYU_NORMALIZE_EOL=lf — \r\n→\n, trailing newline.
pub fn normalize_lf_with_trailing_newline(s: &str) -> String {
    let mut out = s.replace("\r\n", "\n").replace('\r', "\n");
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// v3 EDIT_FILE: применяет список replace-правок к тексту. Окно ±EDIT_WINDOW_CHARS вокруг anchor.
/// Ошибки: ERR_EDIT_ANCHOR_NOT_FOUND, ERR_EDIT_BEFORE_NOT_FOUND, ERR_EDIT_AMBIGUOUS, ERR_EDIT_APPLY_FAILED.
pub fn apply_edit_file_to_text(file_text: &str, edits: &[EditOp]) -> Result<String, String> {
    let mut text = file_text.to_string();
    for (i, edit) in edits.iter().enumerate() {
        if edit.op != "replace" {
            return Err(format!(
                "{}: unsupported op '{}' at edit {}",
                ERR_EDIT_APPLY_FAILED, edit.op, i
            ));
        }
        let anchor = edit.anchor.as_str();
        let before = edit.before.as_str();
        let after = edit.after.as_str();
        let occurrence = edit.occurrence.max(1);

        let anchor_positions: Vec<usize> = text.match_indices(anchor).map(|(pos, _)| pos).collect();
        if anchor_positions.is_empty() {
            return Err(ERR_EDIT_ANCHOR_NOT_FOUND.to_string());
        }
        let anchor_idx = match occurrence as usize {
            n if n <= anchor_positions.len() => anchor_positions[n - 1],
            _ => return Err(ERR_EDIT_ANCHOR_NOT_FOUND.to_string()),
        };

        let start = anchor_idx.saturating_sub(EDIT_WINDOW_CHARS);
        let end = (anchor_idx + anchor.len() + EDIT_WINDOW_CHARS).min(text.len());
        let window = &text[start..end];

        let before_positions: Vec<usize> = window
            .match_indices(before)
            .map(|(pos, _)| start + pos)
            .collect();
        if before_positions.is_empty() {
            return Err(ERR_EDIT_BEFORE_NOT_FOUND.to_string());
        }
        let occ = occurrence as usize;
        if before_positions.len() > 1 && (occ == 0 || occ > before_positions.len()) {
            return Err(ERR_EDIT_AMBIGUOUS.to_string());
        }
        let replace_at = before_positions[occ.saturating_sub(1).min(before_positions.len() - 1)];

        text.replace_range(replace_at..replace_at + before.len(), after);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use diffy::create_patch;

    #[test]
    fn test_sha256_hex() {
        let s = "hello";
        let h = sha256_hex(s.as_bytes());
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_is_valid_sha256_hex() {
        assert!(is_valid_sha256_hex("a".repeat(64).as_str()));
        assert!(is_valid_sha256_hex(&"0".repeat(64)));
        assert!(!is_valid_sha256_hex("abc"));
        assert!(!is_valid_sha256_hex(&"g".repeat(64)));
    }

    #[test]
    fn test_looks_like_unified_diff() {
        let patch = r#"--- a/foo
+++ b/foo
@@ -1,3 +1,4 @@
 line1
+line2
 line3"#;
        assert!(looks_like_unified_diff(patch));
        assert!(!looks_like_unified_diff("not a diff"));
    }

    #[test]
    fn test_apply_unified_diff() {
        // Используем create_patch для гарантированного формата diffy
        let old = "line1\nline3\n";
        let new_expected = "line1\nline2\nline3\n";
        let patch = create_patch(old, new_expected);
        let patch_str = format!("{}", patch);
        let applied = apply_unified_diff_to_text(old, &patch_str).unwrap();
        assert_eq!(applied, new_expected);
    }
}
