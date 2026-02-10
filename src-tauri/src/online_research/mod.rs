//! Online Research Fallback: Search API + Fetch + LLM.
//!
//! Env: PAPAYU_ONLINE_RESEARCH, PAPAYU_SEARCH_PROVIDER, PAPAYU_TAVILY_API_KEY,
//! PAPAYU_ONLINE_MODEL, PAPAYU_ONLINE_MAX_SOURCES, PAPAYU_ONLINE_MAX_PAGES,
//! PAPAYU_ONLINE_PAGE_MAX_BYTES, PAPAYU_ONLINE_TIMEOUT_SEC.

mod extract;
mod fallback;
mod fetch;
mod llm;
mod online_context;
mod search;

use url::Url;

/// S3: For trace privacy, store origin + pathname (no query/fragment). UI may show full URL.
pub fn url_for_trace(url_str: &str) -> String {
    Url::parse(url_str)
        .map(|u| format!("{}{}", u.origin().ascii_serialization(), u.path()))
        .unwrap_or_else(|_| url_str.to_string())
}

#[cfg(test)]
mod online_context_auto_test;

pub use self::online_context::{
    build_online_context_block, effective_online_max_chars, online_context_max_chars,
    online_context_max_sources, OnlineBlockResult,
};
#[allow(unused_imports)]
pub use fallback::{extract_error_code_prefix, maybe_online_fallback};

use serde::{Deserialize, Serialize};

pub use fetch::fetch_url_safe;
pub use search::{tavily_search_with_domains, SearchResult};

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

/// Writes a minimal trace for weekly aggregation (event ONLINE_RESEARCH).
fn write_online_trace(
    project_path: &std::path::Path,
    online_search_cache_hit: bool,
    online_early_stop: bool,
    online_pages_ok: usize,
    online_pages_fail: usize,
    online_search_results_count: usize,
) {
    let trace_dir = project_path.join(".papa-yu").join("traces");
    let _ = std::fs::create_dir_all(&trace_dir);
    let name = format!("online_{}.json", uuid::Uuid::new_v4());
    let path = trace_dir.join(name);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let trace = serde_json::json!({
        "event": "ONLINE_RESEARCH",
        "online_search_cache_hit": online_search_cache_hit,
        "online_early_stop": online_early_stop,
        "online_pages_ok": online_pages_ok,
        "online_pages_fail": online_pages_fail,
        "online_search_results_count": online_search_results_count,
        "timestamp": now.as_secs(),
    });
    let _ = std::fs::write(
        path,
        serde_json::to_string_pretty(&trace).unwrap_or_default(),
    );
}

/// Orchestrates: search → fetch → extract → LLM summarize.
/// If project_path is Some, cache is stored in project_path/.papa-yu/cache/; else in temp_dir.
pub async fn research_answer(
    query: &str,
    project_path: Option<&std::path::Path>,
) -> Result<OnlineAnswer, String> {
    if !is_online_research_enabled() {
        return Err("Online research disabled (PAPAYU_ONLINE_RESEARCH=1 to enable)".into());
    }
    let max_sources = max_sources();
    let max_pages = max_pages();
    let page_max_bytes = page_max_bytes();
    let timeout_sec = timeout_sec();

    let (search_results, online_search_cache_hit) =
        search::tavily_search_cached(query, max_sources, project_path).await?;
    let mut pages: Vec<(String, String, String)> = vec![];
    let mut fetch_failures = 0usize;
    const EARLY_STOP_CHARS: usize = 80_000;
    const EARLY_STOP_CHARS_SUFFICIENT: usize = 40_000;
    const MIN_PAGES_FOR_EARLY: usize = 2;
    const FETCH_CONCURRENCY: usize = 3;
    let mut total_chars = 0usize;
    let mut early_stop = false;
    let urls_to_fetch: Vec<_> = search_results.iter().take(max_pages).collect();
    for chunk in urls_to_fetch.chunks(FETCH_CONCURRENCY) {
        let futures: Vec<_> = chunk
            .iter()
            .map(|r| {
                let url = r.url.clone();
                let title = r.title.clone();
                async move {
                    fetch::fetch_url_safe(&url, page_max_bytes, timeout_sec)
                        .await
                        .map(|body| (url, title, extract::extract_text(&body)))
                }
            })
            .collect();
        let outcomes = futures::future::join_all(futures).await;
        for outcome in outcomes {
            match outcome {
                Ok((url, title, text)) => {
                    if !text.trim().is_empty() {
                        total_chars += text.len();
                        pages.push((url, title, text));
                    }
                }
                Err(e) => {
                    fetch_failures += 1;
                    eprintln!("[online_research] fetch failed: {}", e);
                }
            }
        }
        if total_chars >= EARLY_STOP_CHARS {
            early_stop = true;
            break;
        }
        if pages.len() >= MIN_PAGES_FOR_EARLY && total_chars >= EARLY_STOP_CHARS_SUFFICIENT {
            early_stop = true;
            break;
        }
    }

    let online_model = std::env::var("PAPAYU_ONLINE_MODEL")
        .or_else(|_| std::env::var("PAPAYU_LLM_MODEL"))
        .unwrap_or_else(|_| "gpt-4o-mini".to_string());
    eprintln!(
        "[trace] ONLINE_RESEARCH query_len={} online_search_results_count={} online_pages_ok={} online_pages_fail={} model={} online_search_cache_hit={} online_fetch_parallelism={} online_early_stop={}",
        query.len(),
        search_results.len(),
        pages.len(),
        fetch_failures,
        online_model.trim(),
        online_search_cache_hit,
        FETCH_CONCURRENCY,
        early_stop
    );
    if let Some(project) = project_path {
        write_online_trace(
            project,
            online_search_cache_hit,
            early_stop,
            pages.len(),
            fetch_failures,
            search_results.len(),
        );
    }

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
#[allow(dead_code)]
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
