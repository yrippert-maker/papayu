//! Мониторинг трендов в программировании: рекомендации в автоматическом режиме не реже раз в месяц.
//! Данные хранятся в app_data_dir/trends.json; при первом запуске или если прошло >= 30 дней — should_update = true.

use std::fs;

use chrono::{DateTime, Utc};
use tauri::{AppHandle, Manager};

use crate::types::{TrendsRecommendation, TrendsResult};

const TRENDS_FILENAME: &str = "trends.json";
const RECOMMEND_UPDATE_DAYS: i64 = 30;

fn default_recommendations() -> Vec<TrendsRecommendation> {
    vec![
        TrendsRecommendation {
            title: "TypeScript и строгая типизация".to_string(),
            summary: Some(
                "Использование TypeScript в веб- и Node-проектах снижает количество ошибок."
                    .to_string(),
            ),
            url: Some("https://www.typescriptlang.org/".to_string()),
            source: Some("PAPA YU".to_string()),
        },
        TrendsRecommendation {
            title: "React Server Components и Next.js".to_string(),
            summary: Some(
                "Тренд на серверный рендеринг и стриминг в React-экосистеме.".to_string(),
            ),
            url: Some("https://nextjs.org/".to_string()),
            source: Some("PAPA YU".to_string()),
        },
        TrendsRecommendation {
            title: "Rust для инструментов и WASM".to_string(),
            summary: Some("Rust растёт в CLI, инструментах и веб-сборке (WASM).".to_string()),
            url: Some("https://www.rust-lang.org/".to_string()),
            source: Some("PAPA YU".to_string()),
        },
        TrendsRecommendation {
            title: "Обновляйте зависимости и линтеры".to_string(),
            summary: Some(
                "Регулярно обновляйте npm/cargo зависимости и настройте линтеры (ESLint, Clippy)."
                    .to_string(),
            ),
            url: None,
            source: Some("PAPA YU".to_string()),
        },
    ]
}

fn app_trends_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(TRENDS_FILENAME))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StoredTrends {
    last_updated: String,
    recommendations: Vec<TrendsRecommendation>,
}

/// Возвращает сохранённые тренды и флаг should_update (true, если прошло >= 30 дней или данных нет).
#[tauri::command]
pub fn get_trends_recommendations(app: AppHandle) -> TrendsResult {
    let path = match app_trends_path(&app) {
        Ok(p) => p,
        Err(_) => {
            return TrendsResult {
                last_updated: String::new(),
                recommendations: default_recommendations(),
                should_update: true,
            };
        }
    };
    if !path.exists() {
        return TrendsResult {
            last_updated: String::new(),
            recommendations: default_recommendations(),
            should_update: true,
        };
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            return TrendsResult {
                last_updated: String::new(),
                recommendations: default_recommendations(),
                should_update: true,
            };
        }
    };
    let stored: StoredTrends = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => {
            return TrendsResult {
                last_updated: String::new(),
                recommendations: default_recommendations(),
                should_update: true,
            };
        }
    };
    let should_update =
        parse_and_check_older_than_days(&stored.last_updated, RECOMMEND_UPDATE_DAYS);
    TrendsResult {
        last_updated: stored.last_updated,
        recommendations: stored.recommendations,
        should_update,
    }
}

fn parse_and_check_older_than_days(iso: &str, days: i64) -> bool {
    if iso.is_empty() {
        return true;
    }
    let dt: DateTime<Utc> = match DateTime::parse_from_rfc3339(iso) {
        Ok(d) => d.with_timezone(&Utc),
        Err(_) => return true,
    };
    let now = Utc::now();
    (now - dt).num_days() >= days
}

/// Разрешённые URL для запроса трендов (только эти домены).
const ALLOWED_TRENDS_HOSTS: &[&str] = &[
    "raw.githubusercontent.com",
    "api.github.com",
    "jsonplaceholder.typicode.com",
];

fn url_allowed(url: &str) -> bool {
    let url = url.trim().to_lowercase();
    if !url.starts_with("https://") {
        return false;
    }
    let rest = url.strip_prefix("https://").unwrap_or("");
    let host = rest.split('/').next().unwrap_or("");
    ALLOWED_TRENDS_HOSTS
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{}", h)))
}

/// Обновляет тренды: запрашивает данные по allowlist URL (PAPAYU_TRENDS_URL или встроенный список) и сохраняет.
#[tauri::command]
pub async fn fetch_trends_recommendations(app: AppHandle) -> TrendsResult {
    let now = Utc::now();
    let iso = now.to_rfc3339();

    let urls: Vec<String> = std::env::var("PAPAYU_TRENDS_URLS")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_else(Vec::new);

    let mut recommendations = Vec::new();
    const MAX_TRENDS_RESPONSE_BYTES: usize = 1_000_000;
    const TRENDS_FETCH_TIMEOUT_SEC: u64 = 15;
    if !urls.is_empty() {
        for url in urls {
            if !url_allowed(&url) {
                continue;
            }
            match crate::net::fetch_url_safe(
                &url,
                MAX_TRENDS_RESPONSE_BYTES,
                TRENDS_FETCH_TIMEOUT_SEC,
            )
            .await
            {
                Ok(body) => {
                    if let Ok(parsed) = serde_json::from_str::<Vec<TrendsRecommendation>>(&body) {
                        recommendations.extend(parsed);
                    } else if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&body) {
                        if let Some(arr) = obj.get("recommendations").and_then(|a| a.as_array()) {
                            for v in arr {
                                if let Ok(r) =
                                    serde_json::from_value::<TrendsRecommendation>(v.clone())
                                {
                                    recommendations.push(r);
                                }
                            }
                        }
                    }
                }
                Err(_) => {}
            }
        }
    }
    if recommendations.is_empty() {
        recommendations = default_recommendations();
    }

    let stored = StoredTrends {
        last_updated: iso.clone(),
        recommendations: recommendations.clone(),
    };
    if let Ok(path) = app_trends_path(&app) {
        let _ = fs::write(
            path,
            serde_json::to_string_pretty(&stored).unwrap_or_default(),
        );
    }

    TrendsResult {
        last_updated: iso,
        recommendations,
        should_update: false,
    }
}
