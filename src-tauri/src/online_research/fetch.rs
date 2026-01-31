//! SSRF-safe HTTP fetch: запрет localhost, RFC1918, link-local.

use std::net::IpAddr;
use url::Url;

/// Проверяет, разрешён ли URL для fetch (запрет SSRF).
fn is_url_allowed(u: &Url) -> bool {
    let scheme = u.scheme().to_lowercase();
    if scheme != "http" && scheme != "https" {
        return false;
    }
    let host = match u.host_str() {
        Some(h) => h,
        None => return false,
    };
    let host_lower = host.to_lowercase();
    if host_lower == "localhost"
        || host_lower == "127.0.0.1"
        || host_lower == "::1"
        || host_lower.ends_with(".localhost")
    {
        return false;
    }
    let host_clean = host.trim_matches(|c| c == '[' || c == ']');
    if let Ok(ip) = host_clean.parse::<IpAddr>() {
        if ip.is_loopback() {
            return false;
        }
        if let IpAddr::V4(v4) = ip {
            if v4.is_private() {
                return false;
            }
            if v4.is_link_local() {
                return false;
            }
            let octets = v4.octets();
            if octets[0] == 169 && octets[1] == 254 {
                return false;
            }
        }
        if let IpAddr::V6(v6) = ip {
            if v6.is_loopback() {
                return false;
            }
            let s = v6.to_string();
            if s.starts_with("fe80") || s.starts_with("fe8") || s.starts_with("fe9") {
                return false;
            }
        }
    }
    true
}

/// Скачивает URL с ограничениями по размеру и таймауту. SSRF-safe.
pub async fn fetch_url_safe(
    url_str: &str,
    max_bytes: usize,
    timeout_sec: u64,
) -> Result<String, String> {
    let url = Url::parse(url_str).map_err(|e| format!("Invalid URL: {}", e))?;
    if !is_url_allowed(&url) {
        return Err("URL not allowed (SSRF protection)".into());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_sec))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let resp = client
        .get(url.as_str())
        .send()
        .await
        .map_err(|e| format!("Request: {}", e))?;

    let final_url = resp.url().clone();
    if !is_url_allowed(&final_url) {
        return Err("Redirect to disallowed URL (SSRF protection)".into());
    }

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("HTTP {}", status));
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();
    if !content_type.is_empty()
        && !content_type.contains("text/html")
        && !content_type.contains("text/plain")
        && !content_type.contains("application/json")
        && !content_type.contains("application/xhtml")
    {
        return Err(format!("Unsupported content-type: {}", content_type));
    }

    let bytes = resp.bytes().await.map_err(|e| format!("Body: {}", e))?;
    if bytes.len() > max_bytes {
        return Err(format!("Response too large: {} > {}", bytes.len(), max_bytes));
    }

    let text = String::from_utf8_lossy(&bytes);
    Ok(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssrf_block_localhost() {
        assert!(!is_url_allowed(&Url::parse("http://localhost/").unwrap()));
        assert!(!is_url_allowed(&Url::parse("http://127.0.0.1/").unwrap()));
        assert!(!is_url_allowed(&Url::parse("http://[::1]/").unwrap()));
    }

    #[test]
    fn test_ssrf_block_rfc1918() {
        assert!(!is_url_allowed(&Url::parse("http://192.168.1.1/").unwrap()));
        assert!(!is_url_allowed(&Url::parse("http://10.0.0.1/").unwrap()));
        assert!(!is_url_allowed(&Url::parse("http://172.16.0.1/").unwrap()));
    }

    #[test]
    fn test_ssrf_block_link_local() {
        assert!(!is_url_allowed(&Url::parse("http://169.254.1.1/").unwrap()));
    }

    #[test]
    fn test_ssrf_allow_public() {
        assert!(is_url_allowed(&Url::parse("https://example.com/").unwrap()));
        assert!(is_url_allowed(&Url::parse("https://8.8.8.8/").unwrap()));
    }

    #[test]
    fn test_ssrf_block_file() {
        assert!(!is_url_allowed(&Url::parse("file:///etc/passwd").unwrap()));
    }
}
