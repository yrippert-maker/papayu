//! Online context: truncation, sanitization, block building.

/// Максимум символов для online summary (PAPAYU_ONLINE_CONTEXT_MAX_CHARS).
pub fn online_context_max_chars() -> usize {
    std::env::var("PAPAYU_ONLINE_CONTEXT_MAX_CHARS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(8000)
        .clamp(256, 32_000)
}

/// Максимум источников (PAPAYU_ONLINE_CONTEXT_MAX_SOURCES).
pub fn online_context_max_sources() -> usize {
    std::env::var("PAPAYU_ONLINE_CONTEXT_MAX_SOURCES")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(10)
        .clamp(1, 20)
}

/// Урезает и санитизирует online markdown: по char boundary, без NUL/control, \r\n -> \n.
pub fn truncate_online_context(md: &str, max_chars: usize) -> String {
    let sanitized: String = md
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect();
    let normalized = sanitized.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.chars().count() <= max_chars {
        normalized
    } else {
        normalized.chars().take(max_chars).collect::<String>() + "..."
    }
}

/// Результат сборки online-блока: (block, was_truncated, dropped).
#[derive(Clone, Debug)]
pub struct OnlineBlockResult {
    pub block: String,
    pub was_truncated: bool,
    pub dropped: bool,
    pub chars_used: usize,
    pub sources_count: usize,
}

/// Собирает блок ONLINE_RESEARCH_SUMMARY + ONLINE_SOURCES для вставки в prompt.
/// sources — список URL (обрезается по max_sources).
pub fn build_online_context_block(
    md: &str,
    sources: &[String],
    max_chars: usize,
    max_sources: usize,
) -> OnlineBlockResult {
    let truncated = truncate_online_context(md, max_chars);
    let was_truncated = md.chars().count() > max_chars;

    if truncated.trim().len() < 64 {
        return OnlineBlockResult {
            block: String::new(),
            was_truncated: false,
            dropped: true,
            chars_used: 0,
            sources_count: 0,
        };
    }

    let sources_trimmed: Vec<&str> = sources
        .iter()
        .map(|s| s.as_str())
        .take(max_sources)
        .collect();
    let mut block = String::new();
    block.push_str("\n\nONLINE_RESEARCH_SUMMARY:\n");
    block.push_str(&truncated);
    block.push_str("\n\nONLINE_SOURCES:\n");
    for url in &sources_trimmed {
        block.push_str("- ");
        block.push_str(url);
        block.push('\n');
    }

    let chars_used = block.chars().count();
    OnlineBlockResult {
        block,
        was_truncated,
        dropped: false,
        chars_used,
        sources_count: sources_trimmed.len(),
    }
}

/// Вычисляет допустимый max_chars для online с учётом общего бюджета.
/// rest_context_chars — размер base + prompt_body + auto без online.
/// priority0_reserved — минимальный резерв для FILE (4096).
/// Если после вычета online осталось бы < 512 chars — вернёт 0 (drop).
pub fn effective_online_max_chars(
    rest_context_chars: usize,
    max_total: usize,
    priority0_reserved: usize,
) -> usize {
    let available = max_total
        .saturating_sub(rest_context_chars)
        .saturating_sub(priority0_reserved);
    if available < 512 {
        0
    } else {
        available
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_online_context_limits() {
        let md = "a".repeat(10_000);
        let t = truncate_online_context(&md, 1000);
        assert!(t.len() <= 1004); // 1000 + "..."
        assert!(t.ends_with("..."));
    }

    #[test]
    fn test_truncate_removes_control() {
        let md = "hello\x00world\nok";
        let t = truncate_online_context(md, 100);
        assert!(!t.contains('\x00'));
        assert!(t.contains("hello"));
    }

    #[test]
    fn test_truncate_normalizes_crlf() {
        let md = "a\r\nb\r\nc";
        let t = truncate_online_context(md, 100);
        assert!(!t.contains("\r"));
    }

    #[test]
    fn test_build_block_dropped_when_short() {
        let r = build_online_context_block("x", &[], 8000, 10);
        assert!(r.block.is_empty());
        assert!(r.dropped);
    }

    #[test]
    fn test_build_block_contains_summary() {
        let md = "This is a longer summary with enough content to pass the 64 char minimum.";
        let r = build_online_context_block(md, &["https://example.com".into()], 8000, 10);
        assert!(!r.dropped);
        assert!(r.block.contains("ONLINE_RESEARCH_SUMMARY:"));
        assert!(r.block.contains("ONLINE_SOURCES:"));
        assert!(r.block.contains("https://example.com"));
    }

    #[test]
    fn test_effective_online_max_chars_drops_when_budget_small() {
        let rest = 119_000;
        let max_total = 120_000;
        let reserved = 4096;
        let effective = effective_online_max_chars(rest, max_total, reserved);
        assert_eq!(effective, 0);
    }

    #[test]
    fn test_effective_online_max_chars_returns_available() {
        let rest = 50_000;
        let max_total = 120_000;
        let reserved = 4096;
        let effective = effective_online_max_chars(rest, max_total, reserved);
        assert!(effective >= 65_000);
    }
}
