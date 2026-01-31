//! Search provider: Tavily API.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
}

/// Tavily Search API: POST https://api.tavily.com/search
pub async fn tavily_search(query: &str, max_results: usize) -> Result<Vec<SearchResult>, String> {
    let api_key = std::env::var("PAPAYU_TAVILY_API_KEY")
        .map_err(|_| "PAPAYU_TAVILY_API_KEY not set")?;
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err("PAPAYU_TAVILY_API_KEY is empty".into());
    }

    let body = serde_json::json!({
        "query": query,
        "max_results": max_results,
        "include_answer": false,
        "include_raw_content": false,
    });

    let timeout = std::time::Duration::from_secs(15);
    let client = reqwest::Client::builder()
        .timeout(timeout)
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
            let snippet = r.get("content").and_then(|v| v.as_str()).map(|s| s.to_string());
            Some(SearchResult { title, url, snippet })
        })
        .collect();

    Ok(out)
}
