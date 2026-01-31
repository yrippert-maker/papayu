//! Protocol versioning: v1/v2 default, fallback, env vars.

use std::cell::RefCell;

/// Коды ошибок, при которых v2 fallback на v1 (только для APPLY).
pub const V2_FALLBACK_ERROR_CODES: &[&str] = &[
    "ERR_PATCH_APPLY_FAILED",
    "ERR_NON_UTF8_FILE",
    "ERR_V2_UPDATE_EXISTING_FORBIDDEN",
];

/// Ошибки, для которых сначала repair v2, потом fallback.
pub const V2_REPAIR_FIRST_ERROR_CODES: &[&str] = &[
    "ERR_PATCH_APPLY_FAILED",
    "ERR_V2_UPDATE_EXISTING_FORBIDDEN",
];

/// Ошибка, для которой fallback сразу (repair бессмысленен).
pub const V2_IMMEDIATE_FALLBACK_ERROR_CODES: &[&str] = &["ERR_NON_UTF8_FILE"];

thread_local! {
    static EFFECTIVE_PROTOCOL: RefCell<Option<u32>> = RefCell::new(None);
}

/// Читает PAPAYU_PROTOCOL_DEFAULT, затем PAPAYU_PROTOCOL_VERSION. Default 2.
pub fn protocol_default() -> u32 {
    std::env::var("PAPAYU_PROTOCOL_DEFAULT")
        .or_else(|_| std::env::var("PAPAYU_PROTOCOL_VERSION"))
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .filter(|v| *v == 1 || *v == 2)
        .unwrap_or(2)
}

/// Читает PAPAYU_PROTOCOL_FALLBACK_TO_V1. Default 1 (включён).
pub fn protocol_fallback_enabled() -> bool {
    std::env::var("PAPAYU_PROTOCOL_FALLBACK_TO_V1")
        .ok()
        .map(|s| matches!(s.trim(), "1" | "true" | "yes"))
        .unwrap_or(true)
}

/// Эффективная версия: thread-local override → arg override → default.
pub fn protocol_version(override_version: Option<u32>) -> u32 {
    if let Some(v) = override_version.filter(|v| *v == 1 || *v == 2) {
        return v;
    }
    EFFECTIVE_PROTOCOL.with(|c| {
        if let Some(v) = *c.borrow() {
            return v;
        }
        protocol_default()
    })
}

/// Устанавливает версию протокола для текущего потока. Очищается при drop.
pub fn set_protocol_version(version: u32) -> ProtocolVersionGuard {
    EFFECTIVE_PROTOCOL.with(|c| {
        *c.borrow_mut() = Some(version);
    });
    ProtocolVersionGuard
}

pub struct ProtocolVersionGuard;

impl Drop for ProtocolVersionGuard {
    fn drop(&mut self) {
        EFFECTIVE_PROTOCOL.with(|c| {
            *c.borrow_mut() = None;
        });
    }
}

/// Проверяет, нужен ли fallback на v1 при данной ошибке.
/// repair_attempt: 0 = первый retry, 1 = repair уже пробовали.
/// Для ERR_NON_UTF8_FILE — fallback сразу. Для PATCH_APPLY_FAILED и UPDATE_EXISTING_FORBIDDEN — repair сначала.
pub fn should_fallback_to_v1(error_code: &str, repair_attempt: u32) -> bool {
    if !V2_FALLBACK_ERROR_CODES.contains(&error_code) {
        return false;
    }
    if V2_IMMEDIATE_FALLBACK_ERROR_CODES.contains(&error_code) {
        return true;
    }
    if V2_REPAIR_FIRST_ERROR_CODES.contains(&error_code) && repair_attempt >= 1 {
        return true;
    }
    false
}
