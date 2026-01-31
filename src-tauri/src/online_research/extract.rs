//! Извлечение текста из HTML.

use scraper::{Html, Selector};

pub(crate) const MAX_CHARS: usize = 40_000;

/// Извлекает текст из HTML: убирает script/style, берёт body, нормализует пробелы.
pub fn extract_text(html: &str) -> String {
    let doc = Html::parse_document(html);
    let body_html = match Selector::parse("body") {
        Ok(s) => doc.select(&s).next().map(|el| el.html()),
        Err(_) => None,
    };
    let fragment = body_html.unwrap_or_else(|| doc.root_element().html());

    let without_script = remove_tag_content(&fragment, "script");
    let without_style = remove_tag_content(&without_script, "style");
    let without_noscript = remove_tag_content(&without_style, "noscript");
    let cleaned = strip_tags_simple(&without_noscript);
    let normalized = normalize_whitespace(&cleaned);
    truncate_to(&normalized, MAX_CHARS)
}

fn remove_tag_content(html: &str, tag: &str) -> String {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    let bytes = html.as_bytes();
    while i < bytes.len() {
        if let Some(start) = find_ignore_case(bytes, i, &open) {
            let after_open = start + open.len();
            if let Some(end) = find_ignore_case(bytes, after_open, &close) {
                out.push_str(&html[i..start]);
                i = end + close.len();
                continue;
            }
        }
        if i < bytes.len() {
            out.push(html.chars().nth(i).unwrap_or(' '));
            i += 1;
        }
    }
    if out.is_empty() {
        html.to_string()
    } else {
        out
    }
}

fn find_ignore_case(haystack: &[u8], start: usize, needle: &str) -> Option<usize> {
    let needle_bytes = needle.as_bytes();
    haystack[start..]
        .windows(needle_bytes.len())
        .position(|w| w.eq_ignore_ascii_case(needle_bytes))
        .map(|p| start + p)
}

fn strip_tags_simple(html: &str) -> String {
    let doc = Html::parse_fragment(html);
    let root = doc.root_element();
    let mut text = root.text().collect::<Vec<_>>().join(" ");
    text = text.replace("\u{a0}", " ");
    text
}

fn normalize_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

pub(crate) fn truncate_to(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "..."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_text_basic() {
        let html = r#"<html><body><h1>Title</h1><p>Paragraph text.</p></body></html>"#;
        let t = extract_text(html);
        assert!(t.contains("Title"));
        assert!(t.contains("Paragraph"));
    }

    #[test]
    fn test_extract_removes_script() {
        let html = r#"<body><p>Hello</p><script>alert(1)</script><p>World</p></body>"#;
        let t = extract_text(html);
        assert!(!t.contains("alert"));
        assert!(t.contains("Hello"));
        assert!(t.contains("World"));
    }

    #[test]
    fn test_truncate_to() {
        let s = "a".repeat(50_000);
        let t = super::truncate_to(&s, super::MAX_CHARS);
        assert!(t.ends_with("..."));
        assert!(t.chars().count() <= super::MAX_CHARS + 3);
    }
}
