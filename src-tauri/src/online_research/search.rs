//! Search provider: Tavily API + L1 cache (24h TTL).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
}

const CACHE_TTL_SECS: u64 = 24 * 3600;
const PROVIDER_ID: &str = "tavily";
const MAX_CACHE_ENTRIES: usize = 500;

/// Project-scoped: project_path/.papa-yu/cache/online_search_cache.json; else temp_dir/papa-yu/...
fn cache_path(project_path: Option<&Path>) -> std::path::PathBuf {
    match project_path {
        Some(p) => p
            .join(".papa-yu")
            .join("cache")
            .join("online_search_cache.json"),
        None => std::env::temp_dir()
            .join("papa-yu")
            .join("online_search_cache.json"),
    }
}

fn cache_key(normalized_query: &str, day_bucket: &str, max_results: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalized_query.as_bytes());
    hasher.update(day_bucket.as_bytes());
    hasher.update(max_results.to_string().as_bytes());
    hasher.update(PROVIDER_ID.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    created_at: u64,
    results: Vec<SearchResult>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct CacheFile {
    entries: HashMap<String, CacheEntry>,
}

fn load_cache(path: &Path) -> CacheFile {
    if let Ok(s) = fs::read_to_string(path) {
        if let Ok(f) = serde_json::from_str::<CacheFile>(&s) {
            return f;
        }
    }
    CacheFile::default()
}

fn evict_old_entries(cache: &mut CacheFile) {
    if cache.entries.len() <= MAX_CACHE_ENTRIES {
        return;
    }
    let mut by_age: Vec<(String, u64)> = cache
        .entries
        .iter()
        .map(|(k, v)| (k.clone(), v.created_at))
        .collect();
    by_age.sort_by_key(|(_, t)| *t);
    let to_remove = by_age.len().saturating_sub(MAX_CACHE_ENTRIES);
    for (k, _) in by_age.into_iter().take(to_remove) {
        cache.entries.remove(&k);
    }
}

fn save_cache(path: &Path, cache: &mut CacheFile) {
    evict_old_entries(cache);
    let _ = fs::create_dir_all(path.parent().unwrap());
    let _ = fs::write(
        path,
        serde_json::to_string_pretty(cache).unwrap_or_default(),
    );
}

/// Returns (results, cache_hit). Cache path: project_path/.papa-yu/cache/... if project_path given, else temp_dir.
pub async fn tavily_search_cached(
    query: &str,
    max_results: usize,
    project_path: Option<&Path>,
) -> Result<(Vec<SearchResult>, bool), String> {
    let normalized = query.trim().to_lowercase();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let day_secs = now.as_secs() / 86400;
    let day_bucket = day_secs.to_string();
    let key = cache_key(&normalized, &day_bucket, max_results);

    let path = cache_path(project_path);
    let mut cache = load_cache(&path);
    if let Some(_project) = project_path {
        if cache.entries.is_empty() {
            let temp_path = cache_path(None);
            if temp_path.exists() {
                let temp_cache = load_cache(&temp_path);
                if !temp_cache.entries.is_empty() {
                    cache = temp_cache;
                    let _ = fs::create_dir_all(path.parent().unwrap());
                    let _ = fs::write(
                        &path,
                        serde_json::to_string_pretty(&cache).unwrap_or_default(),
                    );
                }
            }
        }
    }
    if let Some(entry) = cache.entries.get(&key) {
        if now.as_secs().saturating_sub(entry.created_at) < CACHE_TTL_SECS {
            let results = entry.results.clone();
            let n = results.len().min(max_results);
            return Ok((results.into_iter().take(n).collect(), true));
        }
    }

    let results = tavily_search(query, max_results).await?;
    cache.entries.insert(
        key,
        CacheEntry {
            created_at: now.as_secs(),
            results: results.clone(),
        },
    );
    save_cache(&path, &mut cache);
    Ok((results, false))
}

/// Tavily Search API: POST https://api.tavily.com/search
pub async fn tavily_search(query: &str, max_results: usize) -> Result<Vec<SearchResult>, String> {
    tavily_search_with_domains(query, max_results, None).await
}

/// Tavily Search с ограничением по доменам (include_domains). Для безопасного поиска дизайна и иконок.
pub async fn tavily_search_with_domains(
    query: &str,
    max_results: usize,
    include_domains: Option<&[&str]>,
) -> Result<Vec<SearchResult>, String> {
    let api_key =
        std::env::var("PAPAYU_TAVILY_API_KEY").map_err(|_| "PAPAYU_TAVILY_API_KEY not set")?;
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("PAPAYU_TAVILY_API_KEY is empty".into());
    }

    let mut body = serde_json::json!({
        "query": query,
        "max_results": max_results,
        "include_answer": false,
        "include_raw_content": false,
    });
    if let Some(domains) = include_domains {
        if !domains.is_empty() {
            let list: Vec<serde_json::Value> =
                domains.iter().map(|d| serde_json::json!(d)).collect();
            body["include_domains"] = serde_json::Value::Array(list);
        }
    }

    let timeout_secs = std::time::Duration::from_secs(15);
    let client = reqwest::Client::builder()
        .timeout(timeout_secs)
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let resp = client
        .post("https://api.tavily.com/search")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Tavily request: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Response: {}", e))?;

    if !status.is_success() {
        return Err(format!("Tavily API {}: {}", status, text));
    }

    let val: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Tavily JSON: {}", e))?;
    let results = val
        .get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "Tavily: no results array".to_string())?;

    let out: Vec<SearchResult> = results
        .iter()
        .filter_map(|r| {
            let url = r.get("url")?.as_str()?.to_string();
            let title = r.get("title")?.as_str().unwrap_or("").to_string();
            let snippet = r
                .get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Some(SearchResult {
                title,
                url,
                snippet,
            })
        })
        .collect();

    Ok(out)
}
