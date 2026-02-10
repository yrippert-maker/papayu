//! Поиск трендовых дизайнов сайтов и приложений, иконок из безопасных источников.
//!
//! Использует Tavily Search с include_domains — только разрешённые домены.
//! Результаты возвращаются в формате рекомендаций (TrendsRecommendation) для показа в UI
//! и передачи в контекст ИИ для передовых дизайнерских решений.

use crate::online_research::{tavily_search_with_domains, SearchResult};
use crate::types::{TrendsRecommendation, TrendsResult};

/// Домены, разрешённые для поиска дизайна и иконок (безопасные, известные источники).
const ALLOWED_DESIGN_DOMAINS: &[&str] = &[
    "dribbble.com",
    "behance.net",
    "figma.com",
    "material.io",
    "heroicons.com",
    "lucide.dev",
    "fontawesome.com",
    "icons8.com",
    "flaticon.com",
    "thenounproject.com",
    "undraw.co",
    "storyset.com",
    "smashingmagazine.com",
    "uxdesign.cc",
    "nngroup.com",
    "design.google",
    "apple.com",
    "developer.apple.com",
    "m3.material.io",
    "tailwindui.com",
    "shadcn.com",
    "radix-ui.com",
    "github.com",
    "css-tricks.com",
    "web.dev",
];

fn host_from_url(url: &str) -> Option<String> {
    let url = url.trim().to_lowercase();
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?;
    let host = host.trim_matches(|c| c == '[' || c == ']');
    if host.is_empty() {
        return None;
    }
    Some(host.to_string())
}

/// Проверяет, что хост входит в allowlist (или поддомен разрешённого).
fn is_host_allowed(host: &str) -> bool {
    let host_lower = host.to_lowercase();
    ALLOWED_DESIGN_DOMAINS.iter().any(|d| {
        host_lower == *d || host_lower.ends_with(&format!(".{}", d))
    })
}

/// Двойная проверка: оставляем только результаты с разрешённых доменов.
fn filter_results_by_domains(results: Vec<SearchResult>) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|r| host_from_url(&r.url).map_or(false, |h| is_host_allowed(&h)))
        .collect()
}

/// Запрос к Tavily с ограничением по безопасным дизайн-доменам.
async fn search_design_safe(
    query: &str,
    max_results: usize,
) -> Result<Vec<SearchResult>, String> {
    let results = tavily_search_with_domains(
        query,
        max_results.min(15),
        Some(ALLOWED_DESIGN_DOMAINS),
    )
    .await?;
    Ok(filter_results_by_domains(results))
}

/// Преобразует результаты поиска в рекомендации для UI и контекста ИИ.
fn search_results_to_recommendations(
    results: Vec<SearchResult>,
    source_label: &str,
) -> Vec<TrendsRecommendation> {
    results
        .into_iter()
        .map(|r| {
            let source = host_from_url(&r.url).unwrap_or_else(|| source_label.to_string());
            TrendsRecommendation {
                title: r.title,
                summary: r.snippet,
                url: Some(r.url),
                source: Some(source),
            }
        })
        .collect()
}

/// Поиск трендов дизайна и иконок из безопасных источников.
/// Возвращает TrendsResult для отображения в модалке трендов и передачи в ИИ.
#[tauri::command]
pub async fn research_design_trends(
    query: Option<String>,
    max_results: Option<usize>,
) -> Result<TrendsResult, String> {
    let q = query
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or("trending UI UX design 2024, modern app icons, design systems");
    let max = max_results.unwrap_or(10).clamp(1, 15);

    let results = search_design_safe(q, max).await?;
    let recommendations = search_results_to_recommendations(results, "Design");

    let now = chrono::Utc::now().to_rfc3339();
    Ok(TrendsResult {
        last_updated: now,
        recommendations: if recommendations.is_empty() {
            default_design_recommendations()
        } else {
            recommendations
        },
        should_update: false,
    })
}

/// Рекомендации по умолчанию (без поиска), если Tavily недоступен или запрос пустой.
fn default_design_recommendations() -> Vec<TrendsRecommendation> {
    vec![
        TrendsRecommendation {
            title: "Material Design 3 (Material You)".to_string(),
            summary: Some(
                "Адаптивные компоненты, динамические цвета, передовые гайдлайны для приложений."
                    .to_string(),
            ),
            url: Some("https://m3.material.io/".to_string()),
            source: Some("material.io".to_string()),
        },
        TrendsRecommendation {
            title: "Lucide Icons".to_string(),
            summary: Some(
                "Современные открытые иконки, единый стиль, Tree-shakeable для React/Vue."
                    .to_string(),
            ),
            url: Some("https://lucide.dev/".to_string()),
            source: Some("lucide.dev".to_string()),
        },
        TrendsRecommendation {
            title: "shadcn/ui".to_string(),
            summary: Some(
                "Компоненты на Radix, копируешь в проект — полный контроль, тренд 2024 для React."
                    .to_string(),
            ),
            url: Some("https://ui.shadcn.com/".to_string()),
            source: Some("shadcn.com".to_string()),
        },
        TrendsRecommendation {
            title: "Heroicons".to_string(),
            summary: Some("Иконки от создателей Tailwind: outline и solid, SVG.".to_string()),
            url: Some("https://heroicons.com/".to_string()),
            source: Some("heroicons.com".to_string()),
        },
        TrendsRecommendation {
            title: "Nielsen Norman Group".to_string(),
            summary: Some(
                "Исследования UX и гайдлайны по юзабилити для веба и приложений."
                    .to_string(),
            ),
            url: Some("https://www.nngroup.com/".to_string()),
            source: Some("nngroup.com".to_string()),
        },
    ]
}
