//! Decision layer for online fallback.

/// Триггеры online fallback.
const ONLINE_FALLBACK_ERROR_CODES: &[&str] = &[
    "LLM_REQUEST_TIMEOUT",
    "ERR_JSON_PARSE",
    "ERR_JSON_EXTRACT",
    "ERR_SCHEMA_VALIDATION",
];

/// Решает, нужно ли предлагать online fallback по ошибке PRIMARY.
///
/// Triggers: timeout, ERR_JSON_PARSE/ERR_JSON_EXTRACT/ERR_SCHEMA_VALIDATION after repair,
/// или явный NEEDS_ONLINE_RESEARCH в summary/context_requests.
///
/// Ограничение: один раз на запрос (online_fallback_already_attempted).
pub fn maybe_online_fallback(
    error_message: Option<&str>,
    online_enabled: bool,
    online_fallback_already_attempted: bool,
) -> bool {
    if !online_enabled || online_fallback_already_attempted {
        return false;
    }
    let msg = match error_message {
        Some(m) => m,
        None => return false,
    };
    let code = extract_error_code_prefix(msg);
    ONLINE_FALLBACK_ERROR_CODES.contains(&code)
}

/// Извлекает префикс вида "ERR_XXX:" или "LLM_REQUEST_TIMEOUT:" из сообщения.
pub fn extract_error_code_prefix(msg: &str) -> &str {
    if let Some(colon) = msg.find(':') {
        let prefix = msg[..colon].trim();
        if !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return prefix;
        }
    }
    ""
}

/// Проверяет наличие NEEDS_ONLINE_RESEARCH или ONLINE: в summary/context_requests.
#[allow(dead_code)]
pub fn extract_needs_online_from_plan(summary: Option<&str>, context_requests_json: Option<&str>) -> Option<String> {
    if let Some(s) = summary {
        if let Some(q) = extract_online_query_from_text(s) {
            return Some(q);
        }
    }
    if let Some(json) = context_requests_json {
        if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(json) {
            for req in arr {
                if let Some(obj) = req.as_object() {
                    let ty = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    let query = obj.get("query").and_then(|v| v.as_str()).unwrap_or("");
                    if ty == "search" && query.starts_with("ONLINE:") {
                        let q = query.strip_prefix("ONLINE:").map(|s| s.trim()).unwrap_or(query).to_string();
                        if !q.is_empty() {
                            return Some(q);
                        }
                    }
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
fn extract_online_query_from_text(s: &str) -> Option<String> {
    if let Some(idx) = s.find("NEEDS_ONLINE_RESEARCH:") {
        let rest = &s[idx + "NEEDS_ONLINE_RESEARCH:".len()..];
        let q = rest.lines().next().map(|l| l.trim()).unwrap_or(rest.trim());
        if !q.is_empty() {
            return Some(q.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maybe_online_timeout() {
        assert!(maybe_online_fallback(
            Some("LLM_REQUEST_TIMEOUT: Request: timed out"),
            true,
            false
        ));
    }

    #[test]
    fn test_maybe_online_schema() {
        assert!(maybe_online_fallback(
            Some("ERR_SCHEMA_VALIDATION: missing required property"),
            true,
            false
        ));
    }

    #[test]
    fn test_maybe_online_disabled() {
        assert!(!maybe_online_fallback(
            Some("ERR_SCHEMA_VALIDATION: x"),
            false,
            false
        ));
    }

    #[test]
    fn test_maybe_online_already_attempted() {
        assert!(!maybe_online_fallback(
            Some("ERR_SCHEMA_VALIDATION: x"),
            true,
            true
        ));
    }

    #[test]
    fn test_extract_needs_online() {
        assert_eq!(
            extract_needs_online_from_plan(Some("NEEDS_ONLINE_RESEARCH: latest React version"), None),
            Some("latest React version".to_string())
        );
    }
}
