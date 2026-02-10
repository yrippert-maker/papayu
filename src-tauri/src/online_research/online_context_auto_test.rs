//! Tests for auto-use online context flow.

#[cfg(test)]
mod tests {
    use crate::online_research;

    #[test]
    fn test_is_online_auto_use_disabled_by_default() {
        std::env::remove_var("PAPAYU_ONLINE_AUTO_USE_AS_CONTEXT");
        assert!(!online_research::is_online_auto_use_as_context());
    }

    #[test]
    fn test_is_online_auto_use_enabled_when_set() {
        std::env::set_var("PAPAYU_ONLINE_AUTO_USE_AS_CONTEXT", "1");
        assert!(online_research::is_online_auto_use_as_context());
        std::env::remove_var("PAPAYU_ONLINE_AUTO_USE_AS_CONTEXT");
    }

    #[test]
    fn test_extract_error_code_prefix_timeout() {
        let msg = "LLM_REQUEST_TIMEOUT: request timed out";
        assert_eq!(
            online_research::extract_error_code_prefix(msg),
            "LLM_REQUEST_TIMEOUT"
        );
    }

    #[test]
    fn test_extract_error_code_prefix_schema() {
        let msg = "ERR_SCHEMA_VALIDATION: missing required property";
        assert_eq!(
            online_research::extract_error_code_prefix(msg),
            "ERR_SCHEMA_VALIDATION"
        );
    }

    #[test]
    fn test_extract_error_code_prefix_empty_when_no_prefix() {
        let msg = "Some generic error message";
        assert_eq!(online_research::extract_error_code_prefix(msg), "");
    }
}
