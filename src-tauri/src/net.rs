//! Единая точка сетевого доступа.
//!
//! Политика:
//! - Все fetch внешних URL (от пользователя, API, конфига) — через `fetch_url_safe`.
//! - LLM/API вызовы на доверенные URL из env — через reqwest с таймаутами.
//! - Запрет: прямой `reqwest::get()` для URL извне без проверки.

pub use crate::online_research::fetch_url_safe;
