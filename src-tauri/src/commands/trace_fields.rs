//! Универсальный слой извлечения полей из trace JSON.
//! Корректно работает при разных форматах (root vs result vs request) и эволюции полей.

use serde_json::Value;

fn get_str<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut cur = v;
    for p in path {
        cur = cur.get(*p)?;
    }
    cur.as_str()
}

fn get_u64(v: &Value, path: &[&str]) -> Option<u64> {
    let mut cur = v;
    for p in path {
        cur = cur.get(*p)?;
    }
    cur.as_u64()
}

#[allow(dead_code)]
fn get_arr<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Vec<Value>> {
    let mut cur = v;
    for p in path {
        cur = cur.get(*p)?;
    }
    cur.as_array()
}

/// mode может жить в разных местах. Возвращаем "plan"/"apply" если нашли.
#[allow(dead_code)]
pub fn trace_mode(trace: &Value) -> Option<&str> {
    get_str(trace, &["request", "mode"])
        .or_else(|| get_str(trace, &["result", "request", "mode"]))
        .or_else(|| get_str(trace, &["request_mode"]))
        .or_else(|| get_str(trace, &["mode"]))
}

/// protocol_version_used / schema_version: где реально применили протокол.
/// В papa-yu schema_version (1/2/3) соответствует протоколу.
pub fn trace_protocol_version_used(trace: &Value) -> Option<u8> {
    let v = get_u64(trace, &["protocol_version_used"])
        .or_else(|| get_u64(trace, &["result", "protocol_version_used"]))
        .or_else(|| get_u64(trace, &["plan", "protocol_version_used"]))
        .or_else(|| get_u64(trace, &["config_snapshot", "protocol_version_used"]))
        .or_else(|| get_u64(trace, &["schema_version"]))
        .or_else(|| get_u64(trace, &["config_snapshot", "schema_version"]))?;
    u8::try_from(v).ok()
}

/// protocol_attempts: попытки (например [3,2] или ["v3","v2]).
#[allow(dead_code)]
pub fn trace_protocol_attempts(trace: &Value) -> Vec<u8> {
    let arr = get_arr(trace, &["protocol_attempts"])
        .or_else(|| get_arr(trace, &["result", "protocol_attempts"]))
        .or_else(|| get_arr(trace, &["plan", "protocol_attempts"]));
    match arr {
        Some(a) => a
            .iter()
            .filter_map(|x| {
                x.as_u64().and_then(|n| u8::try_from(n).ok()).or_else(|| {
                    x.as_str()
                        .and_then(|s| s.strip_prefix('v').and_then(|n| n.parse::<u8>().ok()))
                })
            })
            .collect(),
        None => vec![],
    }
}

/// error_code: итоговый код ошибки.
pub fn trace_error_code(trace: &Value) -> Option<String> {
    get_str(trace, &["error_code"])
        .or_else(|| get_str(trace, &["result", "error_code"]))
        .or_else(|| get_str(trace, &["error", "code"]))
        .or_else(|| get_str(trace, &["result", "error", "code"]))
        .or_else(|| get_str(trace, &["validation_failed", "code"]))
        .map(|s| s.to_string())
        .or_else(|| {
            get_str(trace, &["error"]).map(|s| s.split(':').next().unwrap_or(s).trim().to_string())
        })
}

/// protocol_fallback_reason: причина fallback.
pub fn trace_protocol_fallback_reason(trace: &Value) -> Option<String> {
    get_str(trace, &["protocol_fallback_reason"])
        .or_else(|| get_str(trace, &["result", "protocol_fallback_reason"]))
        .map(|s| s.to_string())
}

/// validated_json как объект. Если строка — парсит.
fn trace_validated_json_owned(trace: &Value) -> Option<Value> {
    let v = trace
        .get("validated_json")
        .or_else(|| trace.get("result").and_then(|r| r.get("validated_json")))
        .or_else(|| trace.get("trace_val").and_then(|r| r.get("validated_json")))?;
    if let Some(s) = v.as_str() {
        serde_json::from_str(s).ok()
    } else {
        Some(v.clone())
    }
}

/// actions из validated_json (root.actions или proposed_changes.actions).
pub fn trace_actions(trace: &Value) -> Vec<Value> {
    let vj = match trace_validated_json_owned(trace) {
        Some(v) => v,
        None => return vec![],
    };
    if let Some(a) = vj.get("actions").and_then(|x| x.as_array()) {
        return a.clone();
    }
    if let Some(a) = vj
        .get("proposed_changes")
        .and_then(|pc| pc.get("actions"))
        .and_then(|x| x.as_array())
    {
        return a.clone();
    }
    vec![]
}

/// Есть ли action с kind в actions.
pub fn trace_has_action_kind(trace: &Value, kind: &str) -> bool {
    trace_actions(trace)
        .iter()
        .any(|a| a.get("kind").and_then(|k| k.as_str()) == Some(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_mode() {
        let t = serde_json::json!({ "request": { "mode": "apply" } });
        assert_eq!(trace_mode(&t), Some("apply"));
        let t2 = serde_json::json!({ "mode": "plan" });
        assert_eq!(trace_mode(&t2), Some("plan"));
    }

    #[test]
    fn test_trace_protocol_version_used() {
        let t = serde_json::json!({ "schema_version": 3 });
        assert_eq!(trace_protocol_version_used(&t), Some(3));
        let t2 = serde_json::json!({ "config_snapshot": { "schema_version": 2 } });
        assert_eq!(trace_protocol_version_used(&t2), Some(2));
    }

    #[test]
    fn test_trace_has_action_kind() {
        let t = serde_json::json!({
            "validated_json": {
                "actions": [
                    { "kind": "EDIT_FILE", "path": "src/main.rs" },
                    { "kind": "CREATE_FILE", "path": "x" }
                ]
            }
        });
        assert!(trace_has_action_kind(&t, "EDIT_FILE"));
        assert!(trace_has_action_kind(&t, "CREATE_FILE"));
        assert!(!trace_has_action_kind(&t, "PATCH_FILE"));
    }

    #[test]
    fn test_trace_error_code() {
        let t = serde_json::json!({ "error_code": "ERR_EDIT_AMBIGUOUS" });
        assert_eq!(trace_error_code(&t).as_deref(), Some("ERR_EDIT_AMBIGUOUS"));
        let t2 = serde_json::json!({ "result": { "error_code": "ERR_PATCH_APPLY_FAILED" } });
        assert_eq!(
            trace_error_code(&t2).as_deref(),
            Some("ERR_PATCH_APPLY_FAILED")
        );
    }

    #[test]
    fn test_trace_adapters_golden() {
        let apply_v3 = serde_json::json!({
            "request": { "mode": "apply" },
            "schema_version": 3,
            "validated_json": {
                "actions": [{ "kind": "EDIT_FILE", "path": "src/main.rs" }],
                "summary": "Fix"
            }
        });
        assert_eq!(trace_mode(&apply_v3), Some("apply"));
        assert_eq!(trace_protocol_version_used(&apply_v3), Some(3));
        assert!(trace_has_action_kind(&apply_v3, "EDIT_FILE"));
        assert!(!trace_has_action_kind(&apply_v3, "PATCH_FILE"));

        let err_trace = serde_json::json!({
            "event": "VALIDATION_FAILED",
            "schema_version": 3,
            "error_code": "ERR_EDIT_AMBIGUOUS"
        });
        assert_eq!(trace_protocol_version_used(&err_trace), Some(3));
        assert_eq!(
            trace_error_code(&err_trace).as_deref(),
            Some("ERR_EDIT_AMBIGUOUS")
        );
    }
}
