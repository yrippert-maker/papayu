//! Online Research Fallback: Search API + Fetch + LLM.
//!
//! Env: PAPAYU_ONLINE_RESEARCH, PAPAYU_SEARCH_PROVIDER, PAPAYU_TAVILY_API_KEY,
//! PAPAYU_ONLINE_MODEL, PAPAYU_ONLINE_MAX_SOURCES, PAPAYU_ONLINE_MAX_PAGES,
//! PAPAYU_ONLINE_PAGE_MAX_BYTES, PAPAYU_ONLINE_TIMEOUT_SEC.

mod online_context;
mod extract;
mod fallback;
mod fetch;
mod llm;
mod search;

#[cfg(test)]
mod online_context_auto_test;

pub use fallback::{maybe_online_fallback, extract_error_code_prefix};
pub use self::online_context::{
    build_online_context_block, effective_online_max_chars, online_context_max_chars,
    online_context_max_sources, OnlineBlockResult,
};

use serde::{Deserialize, Serialize};

pub use search::SearchResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineAnswer {
    pub answer_md: String,
    pub sources: Vec<OnlineSource>,
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnlineSource {
    pub url: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

/// Orchestrates: search → fetch → extract → LLM summarize.
pub async fn research_answer(query: &str) -> Result<OnlineAnswer, String> {
    if !is_online_research_enabled() {
        return Err("Online research disabled (PAPAYU_ONLINE_RESEARCH=1 to enable)".into());
    }
    let max_sources = max_sources();
    let max_pages = max_pages();
    let page_max_bytes = page_max_bytes();
    let timeout_sec = timeout_sec();

    let search_results = search::tavily_search(query, max_sources).await?;
    let mut pages: Vec<(String, String, String)> = vec![];
    let mut fetch_failures = 0usize;
    for r in search_results.iter().take(max_pages) {
        match fetch::fetch_url_safe(&r.url, page_max_bytes, timeout_sec).await {
            Ok(body) => {
                let text = extract::extract_text(&body);
                if !text.trim().is_empty() {
                    pages.push((r.url.clone(), r.title.clone(), text));
                }
            }
            Err(e) => {
                fetch_failures += 1;
                eprintln!("[online_research] fetch {} failed: {}", r.url, e);
            }
        }
    }

    let online_model = std::env::var("PAPAYU_ONLINE_MODEL")
        .or_else(|_| std::env::var("PAPAYU_LLM_MODEL"))
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());
    eprintln!(
        "[trace] ONLINE_RESEARCH query_len={} sources_count={} pages_fetched={} fetch_failures={} model={}",
        query.len(),
        search_results.len(),
        pages.len(),
        fetch_failures,
        online_model.trim()
    );

    if pages.is_empty() {
        return Ok(OnlineAnswer {
            answer_md: format!(
                "Не удалось загрузить источники для запроса «{}». Попробуйте уточнить запрос или проверить доступность поиска.",
                query
            ),
            sources: search_results
                .iter()
                .take(5)
                .map(|r| OnlineSource {
                    url: r.url.clone(),
                    title: r.title.clone(),
                    published_at: None,
                    snippet: r.snippet.clone(),
                })
                .collect(),
            confidence: 0.0,
            notes: Some("No pages fetched".into()),
        });
    }

    llm::summarize_with_sources(query, &pages, &search_results).await
}

pub fn is_online_research_enabled() -> bool {
    std::env::var("PAPAYU_ONLINE_RESEARCH")
        .ok()
        .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

/// Проверяет, включен ли auto-use as context для online research.
pub fn is_online_auto_use_as_context() -> bool {
    std::env::var("PAPAYU_ONLINE_AUTO_USE_AS_CONTEXT")
        .ok()
        .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

fn max_sources() -> usize {
    std::env::var("PAPAYU_ONLINE_MAX_SOURCES")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(5)
        .clamp(1, 20)
}

fn max_pages() -> usize {
    std::env::var("PAPAYU_ONLINE_MAX_PAGES")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(4)
        .clamp(1, 10)
}

fn page_max_bytes() -> usize {
    std::env::var("PAPAYU_ONLINE_PAGE_MAX_BYTES")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(200_000)
        .clamp(10_000, 500_000)
}

fn timeout_sec() -> u64 {
    std::env::var("PAPAYU_ONLINE_TIMEOUT_SEC")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(20)
        .clamp(5, 60)
}
