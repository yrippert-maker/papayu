//! Синхронизация с Snyk Code: получение результатов анализа кода через REST API
//! и дополнение отчёта/agent-sync для ИИ-агента.
//!
//! Env: PAPAYU_SNYK_SYNC=1, PAPAYU_SNYK_TOKEN (или SNYK_TOKEN), PAPAYU_SNYK_ORG_ID,
//! опционально PAPAYU_SNYK_PROJECT_ID.

use crate::types::Finding;
use serde::Deserialize;
use url::Url;

const SNYK_API_BASE: &str = "https://api.snyk.io/rest";
const SNYK_API_VERSION: &str = "2024-04-02~experimental";

fn snyk_token() -> Option<String> {
    std::env::var("PAPAYU_SNYK_TOKEN")
        .or_else(|_| std::env::var("SNYK_TOKEN"))
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn org_id() -> Option<String> {
    std::env::var("PAPAYU_SNYK_ORG_ID")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub fn is_snyk_sync_enabled() -> bool {
    std::env::var("PAPAYU_SNYK_SYNC")
        .ok()
        .map(|s| matches!(s.trim().to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false)
}

#[derive(Deserialize)]
struct SnykIssuesResponse {
    data: Option<Vec<SnykIssueResource>>,
}

#[derive(Deserialize)]
struct SnykIssueResource {
    #[allow(dead_code)]
    id: Option<String>,
    #[serde(rename = "attributes")]
    attrs: Option<SnykIssueAttrs>,
}

#[derive(Deserialize)]
struct SnykIssueAttrs {
    title: Option<String>,
    description: Option<String>,
    effective_severity_level: Option<String>,
    #[serde(rename = "problems")]
    problems: Option<Vec<SnykProblem>>,
}

#[derive(Deserialize)]
struct SnykProblem {
    #[serde(rename = "path")]
    path: Option<Vec<String>>,
    #[allow(dead_code)]
    message: Option<String>,
}

/// Загружает issues типа "code" по организации (и опционально по проекту).
pub async fn fetch_snyk_code_issues() -> Result<Vec<Finding>, String> {
    let token = snyk_token().ok_or_else(|| "PAPAYU_SNYK_TOKEN or SNYK_TOKEN not set".to_string())?;
    let org = org_id().ok_or_else(|| "PAPAYU_SNYK_ORG_ID not set".to_string())?;

    let mut params: Vec<(String, String)> = vec![
        ("version".into(), SNYK_API_VERSION.to_string()),
        ("type".into(), "code".to_string()),
        ("limit".into(), "100".to_string()),
    ];
    if let Ok(project_id) = std::env::var("PAPAYU_SNYK_PROJECT_ID") {
        let pid = project_id.trim().to_string();
        if !pid.is_empty() {
            params.push(("scan_item.id".into(), pid));
            params.push(("scan_item.type".into(), "project".to_string()));
        }
    }
    let url = Url::parse_with_params(
        &format!("{}/orgs/{}/issues", SNYK_API_BASE, org),
        params.iter().map(|(a, b)| (a.as_str(), b.as_str())),
    )
    .map_err(|e| format!("Snyk URL: {}", e))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let resp = client
        .get(url.as_str())
        .header("Authorization", format!("Token {}", token))
        .header("Accept", "application/vnd.api+json")
        .send()
        .await
        .map_err(|e| format!("Snyk request: {}", e))?;

    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Snyk response: {}", e))?;

    if !status.is_success() {
        return Err(format!("Snyk API {}: {}", status, text.chars().take(500).collect::<String>()));
    }

    let parsed: SnykIssuesResponse = serde_json::from_str(&text)
        .map_err(|e| format!("Snyk JSON: {}", e))?;

    let mut findings = Vec::new();
    for item in parsed.data.unwrap_or_default() {
        let attrs = match item.attrs {
            Some(a) => a,
            None => continue,
        };
        let title = attrs
            .title
            .unwrap_or_else(|| "Snyk Code issue".to_string());
        let desc = attrs.description.unwrap_or_default();
        let severity = attrs.effective_severity_level.unwrap_or_default();
        let path = attrs
            .problems
            .as_ref()
            .and_then(|p| p.first())
            .and_then(|p| p.path.as_ref())
            .and_then(|path_parts| path_parts.first().cloned());
        let details = if severity.is_empty() {
            desc
        } else {
            format!("[{}] {}", severity, desc)
        };
        findings.push(Finding {
            title,
            details: details.chars().take(2000).collect(),
            path,
        });
    }
    Ok(findings)
}
