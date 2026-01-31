//! PATCH_FILE engine: sha256, unified diff validation, apply.

use sha2::{Digest, Sha256};

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
pub fn apply_unified_diff_to_text(old_text: &str, patch_text: &str) -> Result<String, &'static str> {
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
