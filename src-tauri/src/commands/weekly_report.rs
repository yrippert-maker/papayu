//! Weekly Report Analyzer: агрегация трасс и генерация отчёта через LLM.

use super::trace_fields::{
    trace_error_code, trace_has_action_kind, trace_protocol_fallback_reason,
    trace_protocol_version_used,
};
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyStatsBundle {
    pub period_from: String,
    pub period_to: String,
    pub apply_count: u64,
    pub fallback_count: u64,
    pub fallback_rate: f64,
    pub fallback_by_reason: BTreeMap<String, u64>,
    pub fallback_by_group: BTreeMap<String, u64>,
    pub fallback_excluding_non_utf8_rate: f64,
    pub repair_attempt_rate: f64,
    pub repair_success_rate: f64,
    pub repair_to_fallback_rate: f64,
    pub sha_injection_rate: f64,
    pub top_sha_injected_paths: Vec<(String, u64)>,
    pub top_error_codes: Vec<(String, u64)>,
    pub error_codes_by_group: BTreeMap<String, u64>,
    pub new_error_codes: Vec<(String, u64)>,
    pub context: ContextAgg,
    pub cache: CacheAgg,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_search_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_search_cache_hit_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub online_early_stop_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_online_pages_ok: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<PreviousPeriodStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deltas: Option<DeltaStats>,
    // v3 EDIT_FILE metrics
    pub v3_apply_count: u64,
    pub v3_edit_apply_count: u64,
    pub v3_patch_apply_count: u64,
    pub v3_edit_error_count: u64,
    pub v3_err_edit_anchor_not_found_count: u64,
    pub v3_err_edit_before_not_found_count: u64,
    pub v3_err_edit_ambiguous_count: u64,
    pub v3_err_edit_base_mismatch_count: u64,
    pub v3_err_edit_apply_failed_count: u64,
    pub v3_edit_fail_rate: f64,
    pub v3_edit_anchor_not_found_rate: f64,
    pub v3_edit_before_not_found_rate: f64,
    pub v3_edit_ambiguous_rate: f64,
    pub v3_edit_base_mismatch_rate: f64,
    pub v3_edit_apply_failed_rate: f64,
    pub v3_edit_to_patch_ratio: f64,
    pub v3_patch_share_in_v3: f64,
    pub v3_fallback_to_v2_count: u64,
    pub v3_fallback_to_v2_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviousPeriodStats {
    pub period_from: String,
    pub period_to: String,
    pub apply_count: u64,
    pub fallback_count: u64,
    pub fallback_rate: f64,
    pub fallback_excluding_non_utf8_rate: f64,
    pub repair_attempt_rate: f64,
    pub repair_success_rate: f64,
    pub repair_to_fallback_rate: f64,
    pub sha_injection_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaStats {
    pub delta_apply_count: i64,
    pub delta_fallback_count: i64,
    pub delta_fallback_rate: f64,
    pub delta_fallback_excluding_non_utf8_rate: f64,
    pub delta_repair_attempt_rate: f64,
    pub delta_repair_success_rate: f64,
    pub delta_repair_to_fallback_rate: f64,
    pub delta_sha_injection_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextAgg {
    pub avg_total_chars: f64,
    pub p95_total_chars: u64,
    pub avg_files_count: f64,
    pub avg_dropped_files: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheAgg {
    pub avg_hit_rate: f64,
    pub env_hit_rate: f64,
    pub read_hit_rate: f64,
    pub search_hit_rate: f64,
    pub logs_hit_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyReportResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats_bundle: Option<WeeklyStatsBundle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm_report: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_md: Option<String>,
}

/// Нормализует error_code в группу для breakdown.
fn group_error_code(code: &str) -> &'static str {
    let code = code.to_uppercase();
    if code.contains("ERR_EDIT_") {
        "EDIT"
    } else if code.contains("SCHEMA")
        || code.contains("JSON_PARSE")
        || code.contains("JSON_EXTRACT")
        || code.contains("VALIDATION")
    {
        "LLM_FORMAT"
    } else if code.contains("PATCH")
        || code.contains("BASE_MISMATCH")
        || code.contains("BASE_SHA256")
    {
        "PATCH"
    } else if code.contains("PATH")
        || code.contains("CONFLICT")
        || code.contains("PROTECTED")
        || code.contains("UPDATE_WITHOUT_BASE")
    {
        "SAFETY"
    } else if code.contains("NON_UTF8") || code.contains("UTF8") || code.contains("ENCODING") {
        "ENCODING"
    } else if code.contains("UPDATE_EXISTING") || code.contains("UPDATE_FILE") {
        "V2_UPDATE"
    } else {
        "OTHER"
    }
}

/// Извлекает базовый ERR_ код (до двоеточия).
fn extract_base_error_code(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with("ERR_") {
        let base = s.split(':').next().unwrap_or(s).trim().to_string();
        if !base.is_empty() {
            return Some(base);
        }
    }
    None
}

/// Собирает error codes из golden traces (result.error_code). Ищет в project_path/docs/golden_traces и в родительских каталогах (для papa-yu repo).
fn golden_trace_error_codes(project_path: &Path) -> std::collections::HashSet<String> {
    use std::collections::HashSet;
    let mut codes = HashSet::new();
    let mut search_dirs = vec![project_path.to_path_buf()];
    if let Some(parent) = project_path.parent() {
        search_dirs.push(parent.to_path_buf());
    }
    for base in search_dirs {
        for subdir in ["v1", "v2", "v3"] {
            let dir = base.join("docs").join("golden_traces").join(subdir);
            if !dir.exists() {
                continue;
            }
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(content) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) else {
                    continue;
                };
                if let Some(ec) = val
                    .get("result")
                    .and_then(|r| r.get("error_code"))
                    .and_then(|v| v.as_str())
                {
                    if let Some(b) = extract_base_error_code(ec) {
                        codes.insert(b);
                    }
                }
            }
        }
    }
    codes
}

fn trace_to_sample(trace: &serde_json::Value) -> serde_json::Value {
    let error_code = trace
        .get("error_code")
        .and_then(|v| v.as_str())
        .or_else(|| trace.get("error").and_then(|v| v.as_str()));
    serde_json::json!({
        "event": trace.get("event"),
        "error_code": error_code,
        "protocol_attempts": trace.get("protocol_attempts"),
        "protocol_fallback_reason": trace.get("protocol_fallback_reason"),
        "protocol_repair_attempt": trace.get("protocol_repair_attempt"),
        "repair_injected_paths": trace.get("repair_injected_paths"),
        "actions_count": trace.get("actions_count"),
        "context_stats": trace.get("context_stats"),
        "cache_stats": trace.get("cache_stats"),
    })
}

/// Собирает трассы из .papa-yu/traces за период (по mtime файла).
pub fn collect_traces(
    project_path: &Path,
    from_secs: u64,
    to_secs: u64,
) -> Result<Vec<(u64, serde_json::Value)>, String> {
    let traces_dir = project_path.join(".papa-yu").join("traces");
    if !traces_dir.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&traces_dir).map_err(|e| format!("read_dir: {}", e))? {
        let entry = entry.map_err(|e| format!("read_dir entry: {}", e))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let meta = entry.metadata().map_err(|e| format!("metadata: {}", e))?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if mtime < from_secs || mtime > to_secs {
            continue;
        }
        let content =
            fs::read_to_string(&path).map_err(|e| format!("read {}: {}", path.display(), e))?;
        let trace: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| format!("parse {}: {}", path.display(), e))?;
        out.push((mtime, trace));
    }
    Ok(out)
}

/// Агрегирует трассы в WeeklyStatsBundle. Без previous/deltas/new_error_codes — их добавляет analyze_weekly_reports.
pub fn aggregate_weekly(
    traces: &[(u64, serde_json::Value)],
    period_from: &str,
    period_to: &str,
) -> WeeklyStatsBundle {
    let mut apply_count: u64 = 0;
    let mut fallback_count: u64 = 0;
    let mut repair_attempt_count: u64 = 0;
    let mut repair_to_fallback_count: u64 = 0;
    let mut fallback_by_reason: BTreeMap<String, u64> = BTreeMap::new();
    let mut fallback_non_utf8: u64 = 0;
    let mut sha_injection_count: u64 = 0;
    let mut path_counts: HashMap<String, u64> = HashMap::new();
    let mut error_code_counts: HashMap<String, u64> = HashMap::new();
    let mut context_total_chars: Vec<u64> = Vec::new();
    let mut context_files_count: Vec<u64> = Vec::new();
    let mut context_dropped: Vec<u64> = Vec::new();
    let mut cache_hit_rates: Vec<f64> = Vec::new();
    let mut cache_env_hits: u64 = 0;
    let mut cache_env_misses: u64 = 0;
    let mut cache_read_hits: u64 = 0;
    let mut cache_read_misses: u64 = 0;
    let mut cache_search_hits: u64 = 0;
    let mut cache_search_misses: u64 = 0;
    let mut cache_logs_hits: u64 = 0;
    let mut cache_logs_misses: u64 = 0;
    let mut online_search_count: u64 = 0;
    let mut online_search_cache_hits: u64 = 0;
    let mut online_early_stops: u64 = 0;
    let mut online_pages_ok_sum: u64 = 0;
    // v3 EDIT_FILE metrics
    let mut v3_apply_count: u64 = 0;
    let mut v3_edit_apply_count: u64 = 0;
    let mut v3_patch_apply_count: u64 = 0;
    let mut v3_edit_error_count: u64 = 0;
    let mut v3_err_edit_anchor_not_found: u64 = 0;
    let mut v3_err_edit_before_not_found: u64 = 0;
    let mut v3_err_edit_ambiguous: u64 = 0;
    let mut v3_err_edit_base_mismatch: u64 = 0;
    let mut v3_err_edit_apply_failed: u64 = 0;
    let mut v3_fallback_to_v2_count: u64 = 0;

    for (_, trace) in traces {
        let event = trace.get("event").and_then(|v| v.as_str());
        if event == Some("ONLINE_RESEARCH") {
            online_search_count += 1;
            if trace
                .get("online_search_cache_hit")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                online_search_cache_hits += 1;
            }
            if trace
                .get("online_early_stop")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                online_early_stops += 1;
            }
            online_pages_ok_sum += trace
                .get("online_pages_ok")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            continue;
        }
        if event != Some("LLM_PLAN_OK") {
            if event.is_some() {
                let code = trace_error_code(trace);
                if let Some(ref c) = code {
                    *error_code_counts.entry(c.clone()).or_insert(0) += 1;
                    if trace_protocol_version_used(trace) == Some(3) && c.starts_with("ERR_EDIT_") {
                        v3_edit_error_count += 1;
                        let base = extract_base_error_code(c).unwrap_or_else(|| c.clone());
                        match base.as_str() {
                            "ERR_EDIT_ANCHOR_NOT_FOUND" => v3_err_edit_anchor_not_found += 1,
                            "ERR_EDIT_BEFORE_NOT_FOUND" => v3_err_edit_before_not_found += 1,
                            "ERR_EDIT_AMBIGUOUS" => v3_err_edit_ambiguous += 1,
                            "ERR_EDIT_BASE_MISMATCH" | "ERR_EDIT_BASE_SHA256_INVALID" => {
                                v3_err_edit_base_mismatch += 1
                            }
                            "ERR_EDIT_APPLY_FAILED" => v3_err_edit_apply_failed += 1,
                            _ => {}
                        }
                    }
                }
            }
            continue;
        }
        apply_count += 1;

        // v3 metrics via trace field adapters
        let protocol_ver = trace_protocol_version_used(trace);
        let is_v3 = protocol_ver == Some(3);
        let fallback_reason = trace_protocol_fallback_reason(trace).unwrap_or_default();
        let is_v3_fallback_edit = fallback_reason.starts_with("ERR_EDIT_");

        if is_v3 || is_v3_fallback_edit {
            v3_apply_count += 1;
            let has_edit = trace_has_action_kind(trace, "EDIT_FILE");
            let has_patch = trace_has_action_kind(trace, "PATCH_FILE");
            if has_edit {
                v3_edit_apply_count += 1;
            }
            if has_patch {
                v3_patch_apply_count += 1;
            }
            if trace
                .get("protocol_fallback_attempted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                && is_v3_fallback_edit
            {
                v3_fallback_to_v2_count += 1;
                v3_edit_error_count += 1;
                let base = extract_base_error_code(&fallback_reason)
                    .unwrap_or_else(|| fallback_reason.clone());
                match base.as_str() {
                    "ERR_EDIT_ANCHOR_NOT_FOUND" => v3_err_edit_anchor_not_found += 1,
                    "ERR_EDIT_BEFORE_NOT_FOUND" => v3_err_edit_before_not_found += 1,
                    "ERR_EDIT_AMBIGUOUS" => v3_err_edit_ambiguous += 1,
                    "ERR_EDIT_BASE_MISMATCH" | "ERR_EDIT_BASE_SHA256_INVALID" => {
                        v3_err_edit_base_mismatch += 1
                    }
                    "ERR_EDIT_APPLY_FAILED" => v3_err_edit_apply_failed += 1,
                    _ => {}
                }
            }
            if is_v3_fallback_edit && !is_v3 {
                // Fallback trace: schema_version is v2, but the failed attempt had EDIT
                v3_edit_apply_count += 1;
            }
        }

        if trace
            .get("protocol_repair_attempt")
            .and_then(|v| v.as_u64())
            == Some(0)
        {
            repair_attempt_count += 1;
        }
        if trace
            .get("protocol_repair_attempt")
            .and_then(|v| v.as_u64())
            == Some(1)
        {
            let fallback_attempted = trace
                .get("protocol_fallback_attempted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let reason = trace
                .get("protocol_fallback_reason")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !fallback_attempted || reason.is_empty() {
                eprintln!(
                    "[trace] WEEKLY_REPORT_INVARIANT_VIOLATION protocol_repair_attempt=1 expected protocol_fallback_attempted=true and protocol_fallback_reason non-empty, got fallback_attempted={} reason_len={}",
                    fallback_attempted,
                    reason.len()
                );
            }
            repair_to_fallback_count += 1;
        }

        if trace
            .get("protocol_fallback_attempted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            fallback_count += 1;
            let reason = trace
                .get("protocol_fallback_reason")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            *fallback_by_reason.entry(reason.clone()).or_insert(0) += 1;
            if reason == "ERR_NON_UTF8_FILE" {
                fallback_non_utf8 += 1;
            }
        }

        if trace
            .get("repair_injected_sha256")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            sha_injection_count += 1;
            if let Some(paths) = trace
                .get("repair_injected_paths")
                .and_then(|v| v.as_array())
            {
                for p in paths {
                    if let Some(s) = p.as_str() {
                        *path_counts.entry(s.to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        if let Some(ctx) = trace.get("context_stats") {
            if let Some(n) = ctx.get("context_total_chars").and_then(|v| v.as_u64()) {
                context_total_chars.push(n);
            }
            if let Some(n) = ctx.get("context_files_count").and_then(|v| v.as_u64()) {
                context_files_count.push(n);
            }
            if let Some(n) = ctx
                .get("context_files_dropped_count")
                .and_then(|v| v.as_u64())
            {
                context_dropped.push(n);
            }
        }

        if let Some(cache) = trace.get("cache_stats") {
            if let Some(r) = cache.get("hit_rate").and_then(|v| v.as_f64()) {
                cache_hit_rates.push(r);
            }
            cache_env_hits += cache.get("env_hits").and_then(|v| v.as_u64()).unwrap_or(0);
            cache_env_misses += cache
                .get("env_misses")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            cache_read_hits += cache.get("read_hits").and_then(|v| v.as_u64()).unwrap_or(0);
            cache_read_misses += cache
                .get("read_misses")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            cache_search_hits += cache
                .get("search_hits")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            cache_search_misses += cache
                .get("search_misses")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            cache_logs_hits += cache.get("logs_hits").and_then(|v| v.as_u64()).unwrap_or(0);
            cache_logs_misses += cache
                .get("logs_misses")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
        }
    }

    let fallback_excluding_non_utf8 = fallback_count.saturating_sub(fallback_non_utf8);
    let fallback_excluding_non_utf8_rate = if apply_count > 0 {
        fallback_excluding_non_utf8 as f64 / apply_count as f64
    } else {
        0.0
    };

    let sha_injection_rate = if apply_count > 0 {
        sha_injection_count as f64 / apply_count as f64
    } else {
        0.0
    };

    let mut top_paths: Vec<(String, u64)> = path_counts.into_iter().collect();
    top_paths.sort_by(|a, b| b.1.cmp(&a.1));
    top_paths.truncate(10);

    let mut top_errors: Vec<(String, u64)> = error_code_counts
        .iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    top_errors.sort_by(|a, b| b.1.cmp(&a.1));
    top_errors.truncate(10);

    let mut error_codes_by_group: BTreeMap<String, u64> = BTreeMap::new();
    for (code, count) in &error_code_counts {
        let group = group_error_code(code).to_string();
        *error_codes_by_group.entry(group).or_insert(0) += count;
    }
    for (reason, count) in &fallback_by_reason {
        let group = group_error_code(reason).to_string();
        *error_codes_by_group
            .entry(format!("fallback:{}", group))
            .or_insert(0) += count;
    }

    let mut fallback_by_group: BTreeMap<String, u64> = BTreeMap::new();
    for (reason, count) in &fallback_by_reason {
        let group = group_error_code(reason).to_string();
        *fallback_by_group.entry(group).or_insert(0) += count;
    }

    let denom_edit = v3_edit_apply_count.max(1) as f64;
    let denom_v3 = v3_apply_count.max(1) as f64;
    let denom_patch = v3_patch_apply_count.max(1) as f64;
    let v3_edit_fail_rate = v3_edit_error_count as f64 / denom_edit;
    let v3_edit_anchor_not_found_rate = v3_err_edit_anchor_not_found as f64 / denom_edit;
    let v3_edit_before_not_found_rate = v3_err_edit_before_not_found as f64 / denom_edit;
    let v3_edit_ambiguous_rate = v3_err_edit_ambiguous as f64 / denom_edit;
    let v3_edit_base_mismatch_rate = v3_err_edit_base_mismatch as f64 / denom_edit;
    let v3_edit_apply_failed_rate = v3_err_edit_apply_failed as f64 / denom_edit;
    let v3_patch_share_in_v3 = v3_patch_apply_count as f64 / denom_v3;
    let v3_edit_to_patch_ratio = v3_edit_apply_count as f64 / denom_patch;
    let v3_fallback_to_v2_rate = v3_fallback_to_v2_count as f64 / denom_v3;

    let fallback_rate = if apply_count > 0 {
        fallback_count as f64 / apply_count as f64
    } else {
        0.0
    };

    let repair_attempt_rate = if apply_count > 0 {
        repair_attempt_count as f64 / apply_count as f64
    } else {
        0.0
    };

    let (repair_success_rate, repair_to_fallback_rate) = if repair_attempt_count > 0 {
        let success_count = repair_attempt_count.saturating_sub(repair_to_fallback_count);
        (
            success_count as f64 / repair_attempt_count as f64,
            repair_to_fallback_count as f64 / repair_attempt_count as f64,
        )
    } else {
        (0.0, 0.0)
    };

    let avg_total_chars = if context_total_chars.is_empty() {
        0.0
    } else {
        context_total_chars.iter().sum::<u64>() as f64 / context_total_chars.len() as f64
    };
    let mut sorted_chars = context_total_chars.clone();
    sorted_chars.sort();
    let p95_idx = (sorted_chars.len() as f64 * 0.95) as usize;
    let p95_idx2 = p95_idx.min(sorted_chars.len().saturating_sub(1));
    let p95_total_chars = *sorted_chars.get(p95_idx2).unwrap_or(&0);

    let avg_files_count = if context_files_count.is_empty() {
        0.0
    } else {
        context_files_count.iter().sum::<u64>() as f64 / context_files_count.len() as f64
    };
    let avg_dropped_files = if context_dropped.is_empty() {
        0.0
    } else {
        context_dropped.iter().sum::<u64>() as f64 / context_dropped.len() as f64
    };

    let avg_hit_rate = if cache_hit_rates.is_empty() {
        0.0
    } else {
        cache_hit_rates.iter().sum::<f64>() / cache_hit_rates.len() as f64
    };
    let env_total = cache_env_hits + cache_env_misses;
    let env_hit_rate = if env_total > 0 {
        cache_env_hits as f64 / env_total as f64
    } else {
        0.0
    };
    let read_total = cache_read_hits + cache_read_misses;
    let read_hit_rate = if read_total > 0 {
        cache_read_hits as f64 / read_total as f64
    } else {
        0.0
    };
    let search_total = cache_search_hits + cache_search_misses;
    let search_hit_rate = if search_total > 0 {
        cache_search_hits as f64 / search_total as f64
    } else {
        0.0
    };
    let logs_total = cache_logs_hits + cache_logs_misses;
    let logs_hit_rate = if logs_total > 0 {
        cache_logs_hits as f64 / logs_total as f64
    } else {
        0.0
    };

    WeeklyStatsBundle {
        period_from: period_from.to_string(),
        period_to: period_to.to_string(),
        apply_count,
        fallback_count,
        fallback_rate,
        fallback_by_reason,
        fallback_by_group,
        fallback_excluding_non_utf8_rate,
        repair_attempt_rate,
        repair_success_rate,
        repair_to_fallback_rate,
        sha_injection_rate,
        top_sha_injected_paths: top_paths,
        top_error_codes: top_errors,
        error_codes_by_group,
        new_error_codes: vec![],
        context: ContextAgg {
            avg_total_chars,
            p95_total_chars,
            avg_files_count,
            avg_dropped_files,
        },
        cache: CacheAgg {
            avg_hit_rate,
            env_hit_rate,
            read_hit_rate,
            search_hit_rate,
            logs_hit_rate,
        },
        online_search_count: if online_search_count > 0 {
            Some(online_search_count)
        } else {
            None
        },
        online_search_cache_hit_rate: if online_search_count > 0 {
            Some(online_search_cache_hits as f64 / online_search_count as f64)
        } else {
            None
        },
        online_early_stop_rate: if online_search_count > 0 {
            Some(online_early_stops as f64 / online_search_count as f64)
        } else {
            None
        },
        avg_online_pages_ok: if online_search_count > 0 {
            Some(online_pages_ok_sum as f64 / online_search_count as f64)
        } else {
            None
        },
        previous: None,
        deltas: None,
        v3_apply_count,
        v3_edit_apply_count,
        v3_patch_apply_count,
        v3_edit_error_count,
        v3_err_edit_anchor_not_found_count: v3_err_edit_anchor_not_found,
        v3_err_edit_before_not_found_count: v3_err_edit_before_not_found,
        v3_err_edit_ambiguous_count: v3_err_edit_ambiguous,
        v3_err_edit_base_mismatch_count: v3_err_edit_base_mismatch,
        v3_err_edit_apply_failed_count: v3_err_edit_apply_failed,
        v3_edit_fail_rate,
        v3_edit_anchor_not_found_rate,
        v3_edit_before_not_found_rate,
        v3_edit_ambiguous_rate,
        v3_edit_base_mismatch_rate,
        v3_edit_apply_failed_rate,
        v3_edit_to_patch_ratio,
        v3_patch_share_in_v3,
        v3_fallback_to_v2_count,
        v3_fallback_to_v2_rate,
    }
}

const WEEKLY_REPORT_SYSTEM_PROMPT: &str = r#"Ты анализируешь телеметрию работы AI-агента (протоколы v1/v2/v3).
Твоя задача: составить еженедельный отчёт для оператора с выводами и конкретными предложениями улучшений.
Никаких патчей к проекту. Никаких actions. Только отчёт по схеме.
Пиши кратко, по делу. Предлагай меры, которые оператор реально может сделать.

ВАЖНО: Используй только предоставленные числа. Не выдумывай цифры. В evidence ссылайся на конкретные поля, например: fallback_rate_excluding_non_utf8_rate=0.012, fallback_by_reason.ERR_PATCH_APPLY_FAILED=3.

Предлагай **только** то, что можно обосновать полями bundle + deltas. В proposals заполняй kind, title, why, risk, steps, expected_impact (и evidence при наличии).

Типовые proposals:
- prompt_change: если PATCH группа растёт или ERR_PATCH_APPLY_FAILED растёт — усиление patch-инструкций / увеличение контекста / чтение больше строк. Если v3_edit_ambiguous_rate или v3_edit_before_not_found_rate растёт — усилить prompt: «before должен включать 1–2 строки контекста», «before в пределах 50 строк от anchor».
- setting_change (auto-use): если online_fallback_suggested часто и auto-use выключен — предложить включить; если auto-use включён и помогает — оставить.
- golden_trace_add: если new_error_codes содержит код и count>=2 — предложить добавить golden trace.
- limit_tuning: если context часто dropped — предложить повысить PAPAYU_ONLINE_CONTEXT_MAX_CHARS и т.п.
- safety_rule: расширить protected paths при необходимости.

Рекомендуемые направления:
- Снизить ERR_PATCH_APPLY_FAILED: увеличить контекст hunks/прочитать больше строк вокруг
- Снизить UPDATE_FILE violations: усилить prompt или добавить ещё один repair шаблон
- Подкрутить контекст-диету/лимиты если p95 chars часто близко к лимиту
- Расширить protected paths если видны попытки трогать секреты
- Добавить golden trace сценарий если появляется новый тип фейла"#;

/// Вызывает LLM для генерации отчёта по агрегированным данным.
pub async fn call_llm_report(
    stats: &WeeklyStatsBundle,
    traces: &[(u64, serde_json::Value)],
) -> Result<serde_json::Value, String> {
    let api_url = std::env::var("PAPAYU_LLM_API_URL").map_err(|_| "PAPAYU_LLM_API_URL not set")?;
    let api_url = api_url.trim();
    if api_url.is_empty() {
        return Err("PAPAYU_LLM_API_URL is empty".into());
    }
    let model = std::env::var("PAPAYU_LLM_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
    let api_key = std::env::var("PAPAYU_LLM_API_KEY").ok();

    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../config/llm_weekly_report_schema.json"))
            .map_err(|e| format!("schema parse: {}", e))?;

    let stats_json =
        serde_json::to_string_pretty(stats).map_err(|e| format!("serialize stats: {}", e))?;
    let samples: Vec<serde_json::Value> = traces
        .iter()
        .take(5)
        .map(|(_, t)| trace_to_sample(t))
        .collect();
    let samples_json =
        serde_json::to_string_pretty(&samples).map_err(|e| format!("serialize samples: {}", e))?;

    let user_content = format!(
        "Агрегированная телеметрия за период {} — {}:\n\n```json\n{}\n```\n\nПримеры трасс (без raw_content):\n\n```json\n{}\n```",
        stats.period_from,
        stats.period_to,
        stats_json,
        samples_json
    );

    let response_format = serde_json::json!({
        "type": "json_schema",
        "json_schema": {
            "name": "weekly_report",
            "schema": schema,
            "strict": true
        }
    });

    let body = serde_json::json!({
        "model": model.trim(),
        "messages": [
            { "role": "system", "content": WEEKLY_REPORT_SYSTEM_PROMPT },
            { "role": "user", "content": user_content }
        ],
        "temperature": 0.2,
        "max_tokens": 8192,
        "response_format": response_format
    });

    let timeout_sec = std::env::var("PAPAYU_LLM_TIMEOUT_SEC")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(90);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_sec))
        .build()
        .map_err(|e| format!("HTTP client: {}", e))?;

    let mut req = client.post(api_url).json(&body);
    if let Some(ref key) = api_key {
        if !key.trim().is_empty() {
            req = req.header("Authorization", format!("Bearer {}", key.trim()));
        }
    }

    let resp = req.send().await.map_err(|e| format!("Request: {}", e))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| format!("Response: {}", e))?;

    if !status.is_success() {
        return Err(format!("API error {}: {}", status, text));
    }

    let chat: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Response JSON: {}", e))?;
    let content = chat
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| "No content in API response".to_string())?;

    let report: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("Report JSON: {}", e))?;

    let compiled = JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft7)
        .compile(&schema)
        .map_err(|e| format!("Schema compile: {}", e))?;

    if let Err(e) = compiled.validate(&report) {
        let msg: Vec<String> = e.map(|ve| format!("{}", ve)).collect();
        return Err(format!("Schema validation: {}", msg.join("; ")));
    }

    Ok(report)
}

/// Собирает самодостаточный markdown: KPI-таблица и Top reasons в начале, затем текст LLM.
pub fn build_self_contained_md(stats: &WeeklyStatsBundle, llm_md: &str) -> String {
    let mut md = format!(
        "# Weekly Report\n\nПериод: {} — {}\n\n",
        stats.period_from, stats.period_to
    );

    md.push_str("## KPI (фактические)\n\n");
    md.push_str("| Метрика | Значение |\n|--------|----------|\n");
    md.push_str(&format!("| apply_count | {} |\n", stats.apply_count));
    md.push_str(&format!("| fallback_count | {} |\n", stats.fallback_count));
    md.push_str(&format!("| fallback_rate | {:.4} |\n", stats.fallback_rate));
    md.push_str(&format!(
        "| fallback_excluding_non_utf8_rate | {:.4} |\n",
        stats.fallback_excluding_non_utf8_rate
    ));
    md.push_str(&format!(
        "| repair_attempt_rate | {:.4} |\n",
        stats.repair_attempt_rate
    ));
    md.push_str(&format!(
        "| repair_success_rate | {:.4} |\n",
        stats.repair_success_rate
    ));
    md.push_str(&format!(
        "| repair_to_fallback_rate | {:.4} |\n",
        stats.repair_to_fallback_rate
    ));
    md.push_str(&format!(
        "| sha_injection_rate | {:.4} |\n",
        stats.sha_injection_rate
    ));
    md.push_str("\n");

    if stats.v3_apply_count > 0 {
        md.push_str("### v3 EDIT_FILE\n\n");
        md.push_str(&format!(
            "- v3_apply_count={}, v3_edit_apply_count={}, v3_patch_apply_count={}\n",
            stats.v3_apply_count, stats.v3_edit_apply_count, stats.v3_patch_apply_count
        ));
        md.push_str(&format!(
            "- v3_edit_fail_rate={:.3}, ambiguous={:.3}, before_not_found={:.3}, anchor_not_found={:.3}\n",
            stats.v3_edit_fail_rate,
            stats.v3_edit_ambiguous_rate,
            stats.v3_edit_before_not_found_rate,
            stats.v3_edit_anchor_not_found_rate
        ));
        md.push_str(&format!(
            "- v3_fallback_to_v2_rate={:.3}, patch_share_in_v3={:.3}, edit_to_patch_ratio={:.2}\n",
            stats.v3_fallback_to_v2_rate, stats.v3_patch_share_in_v3, stats.v3_edit_to_patch_ratio
        ));
        md.push_str("\n");
    }

    if !stats.fallback_by_reason.is_empty() {
        md.push_str("## Top fallback reasons\n\n");
        md.push_str("| Причина | Кол-во |\n|---------|--------|\n");
        for (reason, count) in stats.fallback_by_reason.iter().take(10) {
            md.push_str(&format!("| {} | {} |\n", reason, count));
        }
        md.push_str("\n");
    }

    if !stats.fallback_by_group.is_empty() {
        md.push_str("## Fallback по группам\n\n");
        md.push_str("| Группа | Кол-во |\n|--------|--------|\n");
        for (group, count) in &stats.fallback_by_group {
            md.push_str(&format!("| {} | {} |\n", group, count));
        }
        md.push_str("\n");
    }

    if !stats.new_error_codes.is_empty() {
        md.push_str("## Новые error codes (кандидаты на golden trace)\n\n");
        for (code, count) in &stats.new_error_codes {
            md.push_str(&format!("- {} ({} раз)\n", code, count));
        }
        md.push_str("\n");
    }

    if let Some(ref deltas) = stats.deltas {
        md.push_str("## Дельты vs предыдущая неделя\n\n");
        md.push_str(&format!(
            "| delta_apply_count | {} |\n",
            deltas.delta_apply_count
        ));
        md.push_str(&format!(
            "| delta_fallback_rate | {:+.4} |\n",
            deltas.delta_fallback_rate
        ));
        md.push_str(&format!(
            "| delta_repair_attempt_rate | {:+.4} |\n",
            deltas.delta_repair_attempt_rate
        ));
        md.push_str(&format!(
            "| delta_repair_success_rate | {:+.4} |\n",
            deltas.delta_repair_success_rate
        ));
        md.push_str("\n");
    }

    md.push_str("---\n\n");
    md.push_str(llm_md);
    md
}

/// Формирует Markdown отчёт из LLM ответа.
pub fn report_to_md(report: &serde_json::Value) -> String {
    let title = report
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Weekly Report");
    let period = report.get("period");
    let from = period
        .and_then(|p| p.get("from"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let to = period
        .and_then(|p| p.get("to"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let summary = report
        .get("summary_md")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut md = format!(
        "# {}\n\nПериод: {} — {}\n\n{}\n\n",
        title, from, to, summary
    );

    if let Some(kpis) = report.get("kpis") {
        md.push_str("## KPI\n\n");
        md.push_str("| Метрика | Значение |\n|--------|----------|\n");
        for (key, val) in kpis.as_object().unwrap_or(&serde_json::Map::new()) {
            let v = match val {
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::String(s) => s.clone(),
                _ => format!("{:?}", val),
            };
            md.push_str(&format!("| {} | {} |\n", key, v));
        }
        md.push_str("\n");
    }

    if let Some(findings) = report.get("findings").and_then(|v| v.as_array()) {
        md.push_str("## Выводы\n\n");
        for f in findings {
            let sev = f.get("severity").and_then(|v| v.as_str()).unwrap_or("info");
            let title_f = f.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let ev = f.get("evidence").and_then(|v| v.as_str()).unwrap_or("");
            md.push_str(&format!("- **{}** [{}]: {}\n", title_f, sev, ev));
        }
        md.push_str("\n");
    }

    if let Some(recs) = report.get("recommendations").and_then(|v| v.as_array()) {
        md.push_str("## Рекомендации\n\n");
        for r in recs {
            let pri = r.get("priority").and_then(|v| v.as_str()).unwrap_or("p2");
            let title_r = r.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let rat = r.get("rationale").and_then(|v| v.as_str()).unwrap_or("");
            md.push_str(&format!(
                "- [{}] **{}**: {} — {}\n",
                pri,
                title_r,
                rat,
                r.get("expected_impact")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
            ));
        }
        md.push_str("\n");
    }

    if let Some(actions) = report.get("operator_actions").and_then(|v| v.as_array()) {
        md.push_str("## Действия оператора\n\n");
        for a in actions {
            let title_a = a.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let empty: Vec<serde_json::Value> = vec![];
            let steps = a.get("steps").and_then(|v| v.as_array()).unwrap_or(&empty);
            let est = a
                .get("time_estimate_minutes")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            md.push_str(&format!("### {}\n\nОценка: {} мин\n\n", title_a, est));
            for (i, s) in steps.iter().enumerate() {
                if let Some(st) = s.as_str() {
                    md.push_str(&format!("{}. {}\n", i + 1, st));
                }
            }
            md.push_str("\n");
        }
    }

    if let Some(proposals) = report.get("proposals").and_then(|v| v.as_array()) {
        md.push_str("## Предложения (proposals)\n\n");
        for p in proposals {
            let kind = p.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let title_p = p.get("title").and_then(|v| v.as_str()).unwrap_or("");
            let why = p.get("why").and_then(|v| v.as_str()).unwrap_or("");
            let risk = p.get("risk").and_then(|v| v.as_str()).unwrap_or("");
            let impact = p
                .get("expected_impact")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            md.push_str(&format!(
                "- **{}** [{}] risk={}: {} — {}\n",
                kind, title_p, risk, why, impact
            ));
            let empty: Vec<serde_json::Value> = vec![];
            let steps = p.get("steps").and_then(|v| v.as_array()).unwrap_or(&empty);
            for (i, s) in steps.iter().enumerate() {
                if let Some(st) = s.as_str() {
                    md.push_str(&format!("  {}. {}\n", i + 1, st));
                }
            }
        }
        md.push_str("\n");
    }

    md
}

/// Анализирует трассы и генерирует еженедельный отчёт.
pub async fn analyze_weekly_reports(
    project_path: &Path,
    from: Option<String>,
    to: Option<String>,
) -> WeeklyReportResult {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));
    let now_secs = now.as_secs();
    let week_secs: u64 = 7 * 24 * 3600;
    let (to_secs, from_secs) = if let (Some(f), Some(t)) = (&from, &to) {
        let from_secs = chrono_parse_or_default(f, now_secs.saturating_sub(week_secs));
        let to_secs = chrono_parse_or_default(t, now_secs);
        (to_secs, from_secs)
    } else {
        (now_secs, now_secs.saturating_sub(week_secs))
    };

    let traces = match collect_traces(project_path, from_secs, to_secs) {
        Ok(t) => t,
        Err(e) => {
            return WeeklyReportResult {
                ok: false,
                error: Some(e),
                stats_bundle: None,
                llm_report: None,
                report_md: None,
            };
        }
    };

    let from_str = format_timestamp(from_secs);
    let to_str = format_timestamp(to_secs);
    let period_secs = to_secs.saturating_sub(from_secs);
    let prev_from_secs = from_secs.saturating_sub(period_secs);
    let prev_to_secs = from_secs;
    let prev_from_str = format_timestamp(prev_from_secs);
    let prev_to_str = format_timestamp(prev_to_secs);

    let mut stats = aggregate_weekly(&traces, &from_str, &to_str);

    let prev_traces =
        collect_traces(project_path, prev_from_secs, prev_to_secs).unwrap_or_default();
    if !prev_traces.is_empty() {
        let prev_stats = aggregate_weekly(&prev_traces, &prev_from_str, &prev_to_str);
        stats.previous = Some(PreviousPeriodStats {
            period_from: prev_stats.period_from,
            period_to: prev_stats.period_to,
            apply_count: prev_stats.apply_count,
            fallback_count: prev_stats.fallback_count,
            fallback_rate: prev_stats.fallback_rate,
            fallback_excluding_non_utf8_rate: prev_stats.fallback_excluding_non_utf8_rate,
            repair_attempt_rate: prev_stats.repair_attempt_rate,
            repair_success_rate: prev_stats.repair_success_rate,
            repair_to_fallback_rate: prev_stats.repair_to_fallback_rate,
            sha_injection_rate: prev_stats.sha_injection_rate,
        });
        stats.deltas = Some(DeltaStats {
            delta_apply_count: stats.apply_count as i64 - prev_stats.apply_count as i64,
            delta_fallback_count: stats.fallback_count as i64 - prev_stats.fallback_count as i64,
            delta_fallback_rate: stats.fallback_rate - prev_stats.fallback_rate,
            delta_fallback_excluding_non_utf8_rate: stats.fallback_excluding_non_utf8_rate
                - prev_stats.fallback_excluding_non_utf8_rate,
            delta_repair_attempt_rate: stats.repair_attempt_rate - prev_stats.repair_attempt_rate,
            delta_repair_success_rate: stats.repair_success_rate - prev_stats.repair_success_rate,
            delta_repair_to_fallback_rate: stats.repair_to_fallback_rate
                - prev_stats.repair_to_fallback_rate,
            delta_sha_injection_rate: stats.sha_injection_rate - prev_stats.sha_injection_rate,
        });
    }

    let golden = golden_trace_error_codes(project_path);
    let mut new_counts: HashMap<String, u64> = HashMap::new();
    for (code, count) in stats
        .top_error_codes
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .chain(
            stats
                .fallback_by_reason
                .iter()
                .map(|(k, v)| (k.as_str(), *v)),
        )
    {
        if let Some(base) = extract_base_error_code(code) {
            if !golden.contains(&base) {
                *new_counts.entry(base).or_insert(0) += count;
            }
        }
    }
    let mut new_errors: Vec<(String, u64)> = new_counts.into_iter().collect();
    new_errors.sort_by(|a, b| b.1.cmp(&a.1));
    stats.new_error_codes = new_errors;

    if traces.is_empty() {
        let report_md = format!(
            "# Weekly Report\n\nПериод: {} — {}\n\nТрасс за период не найдено. Включи PAPAYU_TRACE=1 и выполни несколько операций.",
            from_str, to_str
        );
        return WeeklyReportResult {
            ok: true,
            error: None,
            stats_bundle: Some(stats),
            llm_report: None,
            report_md: Some(report_md),
        };
    }

    match call_llm_report(&stats, &traces).await {
        Ok(report) => {
            let llm_md = report_to_md(&report);
            let report_md = build_self_contained_md(&stats, &llm_md);
            WeeklyReportResult {
                ok: true,
                error: None,
                stats_bundle: Some(stats),
                llm_report: Some(report),
                report_md: Some(report_md),
            }
        }
        Err(e) => WeeklyReportResult {
            ok: false,
            error: Some(e),
            stats_bundle: Some(stats),
            llm_report: None,
            report_md: None,
        },
    }
}

fn chrono_parse_or_default(s: &str, default: u64) -> u64 {
    use chrono::{NaiveDate, NaiveDateTime};
    let s = s.trim();
    if s.is_empty() {
        return default;
    }
    for fmt in ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return dt.and_utc().timestamp() as u64;
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        if let Some(dt) = d.and_hms_opt(0, 0, 0) {
            return dt.and_utc().timestamp() as u64;
        }
    }
    default
}

fn format_timestamp(secs: u64) -> String {
    use chrono::{DateTime, Utc};
    let dt = DateTime::<Utc>::from_timestamp_secs(secs as i64)
        .unwrap_or_else(|| DateTime::<Utc>::from_timestamp_secs(0).unwrap());
    dt.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregate_weekly_empty() {
        let traces: Vec<(u64, serde_json::Value)> = vec![];
        let stats = aggregate_weekly(&traces, "2024-01-01", "2024-01-07");
        assert_eq!(stats.apply_count, 0);
        assert_eq!(stats.fallback_count, 0);
        assert_eq!(stats.fallback_excluding_non_utf8_rate, 0.0);
        assert_eq!(stats.repair_success_rate, 0.0);
        assert_eq!(stats.sha_injection_rate, 0.0);
    }

    #[test]
    fn test_aggregate_weekly_llm_plan_ok() {
        let traces = vec![
            (
                1704067200, // 2024-01-01: repair attempt that succeeded (no fallback)
                serde_json::json!({
                    "event": "LLM_PLAN_OK",
                    "protocol_repair_attempt": 0,
                    "actions_count": 2,
                    "context_stats": { "context_total_chars": 1000, "context_files_count": 1, "context_files_dropped_count": 0 },
                    "cache_stats": { "hit_rate": 0.5, "env_hits": 0, "env_misses": 1, "read_hits": 1, "read_misses": 0, "search_hits": 0, "search_misses": 0, "logs_hits": 0, "logs_misses": 0 }
                }),
            ),
            (
                1704153600, // repair failed → fallback plan
                serde_json::json!({
                    "event": "LLM_PLAN_OK",
                    "protocol_repair_attempt": 1,
                    "protocol_fallback_attempted": true,
                    "protocol_fallback_reason": "ERR_PATCH_APPLY_FAILED",
                    "actions_count": 1,
                    "context_stats": { "context_total_chars": 500, "context_files_count": 1, "context_files_dropped_count": 0 },
                    "cache_stats": { "hit_rate": 0.6, "env_hits": 1, "env_misses": 0, "read_hits": 1, "read_misses": 0, "search_hits": 0, "search_misses": 0, "logs_hits": 0, "logs_misses": 0 }
                }),
            ),
        ];
        let stats = aggregate_weekly(&traces, "2024-01-01", "2024-01-07");
        assert_eq!(stats.apply_count, 2);
        assert_eq!(stats.fallback_count, 1);
        assert!((stats.fallback_excluding_non_utf8_rate - 0.5).abs() < 0.001);
        assert!((stats.repair_attempt_rate - 0.5).abs() < 0.001); // 1 repair attempt / 2 applies
        assert!((stats.repair_success_rate - 0.0).abs() < 0.001); // 0/1 repair attempts succeeded
        assert!((stats.repair_to_fallback_rate - 1.0).abs() < 0.001); // 1/1 went to fallback
        assert_eq!(
            stats.fallback_by_reason.get("ERR_PATCH_APPLY_FAILED"),
            Some(&1)
        );
    }

    #[test]
    fn test_aggregate_weekly_v3_edit_metrics() {
        let traces = vec![
            (
                1704067200,
                serde_json::json!({
                    "event": "LLM_PLAN_OK",
                    "schema_version": 3,
                    "validated_json": {
                        "actions": [
                            { "kind": "EDIT_FILE", "path": "src/main.rs", "base_sha256": "abc123", "edits": [] }
                        ],
                        "summary": "Fix"
                    },
                    "context_stats": {},
                    "cache_stats": { "hit_rate": 0.5, "env_hits": 0, "env_misses": 1, "read_hits": 1, "read_misses": 0, "search_hits": 0, "search_misses": 0, "logs_hits": 0, "logs_misses": 0 }
                }),
            ),
            (
                1704153600,
                serde_json::json!({
                    "event": "VALIDATION_FAILED",
                    "schema_version": 3,
                    "error_code": "ERR_EDIT_AMBIGUOUS",
                    "validated_json": { "actions": [] }
                }),
            ),
        ];
        let stats = aggregate_weekly(&traces, "2024-01-01", "2024-01-07");
        assert_eq!(stats.v3_apply_count, 1);
        assert_eq!(stats.v3_edit_apply_count, 1);
        assert_eq!(stats.v3_edit_error_count, 1);
        assert_eq!(stats.v3_err_edit_ambiguous_count, 1);
        assert!((stats.v3_edit_fail_rate - 1.0).abs() < 0.001); // 1 error / 1 edit apply
        assert!((stats.v3_edit_ambiguous_rate - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_group_error_code() {
        assert_eq!(group_error_code("ERR_SCHEMA_VALIDATION"), "LLM_FORMAT");
        assert_eq!(group_error_code("ERR_JSON_PARSE"), "LLM_FORMAT");
        assert_eq!(group_error_code("ERR_PATCH_APPLY_FAILED"), "PATCH");
        assert_eq!(group_error_code("ERR_BASE_MISMATCH"), "PATCH");
        assert_eq!(group_error_code("ERR_NON_UTF8_FILE"), "ENCODING");
        assert_eq!(group_error_code("ERR_INVALID_PATH"), "SAFETY");
        assert_eq!(
            group_error_code("ERR_V2_UPDATE_EXISTING_FORBIDDEN"),
            "V2_UPDATE"
        );
        assert_eq!(group_error_code("ERR_EDIT_ANCHOR_NOT_FOUND"), "EDIT");
        assert_eq!(group_error_code("ERR_EDIT_AMBIGUOUS"), "EDIT");
    }

    #[test]
    fn test_build_self_contained_md() {
        let stats = WeeklyStatsBundle {
            period_from: "2024-01-01".into(),
            period_to: "2024-01-07".into(),
            apply_count: 10,
            fallback_count: 1,
            fallback_rate: 0.1,
            fallback_by_reason: [("ERR_PATCH_APPLY_FAILED".into(), 1)].into_iter().collect(),
            fallback_by_group: [("PATCH".into(), 1)].into_iter().collect(),
            fallback_excluding_non_utf8_rate: 0.1,
            repair_attempt_rate: 0.2,
            repair_success_rate: 0.9,
            repair_to_fallback_rate: 0.1,
            sha_injection_rate: 0.05,
            top_sha_injected_paths: vec![],
            top_error_codes: vec![],
            error_codes_by_group: [("PATCH".into(), 1)].into_iter().collect(),
            new_error_codes: vec![("ERR_XYZ".into(), 2)],
            context: ContextAgg {
                avg_total_chars: 0.0,
                p95_total_chars: 0,
                avg_files_count: 0.0,
                avg_dropped_files: 0.0,
            },
            cache: CacheAgg {
                avg_hit_rate: 0.0,
                env_hit_rate: 0.0,
                read_hit_rate: 0.0,
                search_hit_rate: 0.0,
                logs_hit_rate: 0.0,
            },
            online_search_count: None,
            online_search_cache_hit_rate: None,
            online_early_stop_rate: None,
            avg_online_pages_ok: None,
            previous: None,
            deltas: None,
            v3_apply_count: 0,
            v3_edit_apply_count: 0,
            v3_patch_apply_count: 0,
            v3_edit_error_count: 0,
            v3_err_edit_anchor_not_found_count: 0,
            v3_err_edit_before_not_found_count: 0,
            v3_err_edit_ambiguous_count: 0,
            v3_err_edit_base_mismatch_count: 0,
            v3_err_edit_apply_failed_count: 0,
            v3_edit_fail_rate: 0.0,
            v3_edit_anchor_not_found_rate: 0.0,
            v3_edit_before_not_found_rate: 0.0,
            v3_edit_ambiguous_rate: 0.0,
            v3_edit_base_mismatch_rate: 0.0,
            v3_edit_apply_failed_rate: 0.0,
            v3_edit_to_patch_ratio: 0.0,
            v3_patch_share_in_v3: 0.0,
            v3_fallback_to_v2_count: 0,
            v3_fallback_to_v2_rate: 0.0,
        };
        let md = build_self_contained_md(&stats, "## LLM Summary\n\nText.");
        assert!(md.contains("apply_count"));
        assert!(md.contains("ERR_PATCH_APPLY_FAILED"));
        assert!(md.contains("ERR_XYZ"));
        assert!(md.contains("LLM Summary"));
        // v3 section not shown when v3_apply_count=0
        assert!(!md.contains("v3_apply_count"));
    }

    #[test]
    fn test_build_self_contained_md_v3_section() {
        let stats = WeeklyStatsBundle {
            period_from: "2024-01-01".into(),
            period_to: "2024-01-07".into(),
            apply_count: 5,
            fallback_count: 0,
            fallback_rate: 0.0,
            fallback_by_reason: BTreeMap::new(),
            fallback_by_group: BTreeMap::new(),
            fallback_excluding_non_utf8_rate: 0.0,
            repair_attempt_rate: 0.0,
            repair_success_rate: 0.0,
            repair_to_fallback_rate: 0.0,
            sha_injection_rate: 0.0,
            top_sha_injected_paths: vec![],
            top_error_codes: vec![],
            error_codes_by_group: BTreeMap::new(),
            new_error_codes: vec![],
            context: ContextAgg {
                avg_total_chars: 0.0,
                p95_total_chars: 0,
                avg_files_count: 0.0,
                avg_dropped_files: 0.0,
            },
            cache: CacheAgg {
                avg_hit_rate: 0.0,
                env_hit_rate: 0.0,
                read_hit_rate: 0.0,
                search_hit_rate: 0.0,
                logs_hit_rate: 0.0,
            },
            online_search_count: None,
            online_search_cache_hit_rate: None,
            online_early_stop_rate: None,
            avg_online_pages_ok: None,
            previous: None,
            deltas: None,
            v3_apply_count: 3,
            v3_edit_apply_count: 2,
            v3_patch_apply_count: 1,
            v3_edit_error_count: 1,
            v3_err_edit_anchor_not_found_count: 0,
            v3_err_edit_before_not_found_count: 0,
            v3_err_edit_ambiguous_count: 1,
            v3_err_edit_base_mismatch_count: 0,
            v3_err_edit_apply_failed_count: 0,
            v3_edit_fail_rate: 0.5,
            v3_edit_anchor_not_found_rate: 0.0,
            v3_edit_before_not_found_rate: 0.0,
            v3_edit_ambiguous_rate: 0.5,
            v3_edit_base_mismatch_rate: 0.0,
            v3_edit_apply_failed_rate: 0.0,
            v3_edit_to_patch_ratio: 2.0,
            v3_patch_share_in_v3: 0.333,
            v3_fallback_to_v2_count: 0,
            v3_fallback_to_v2_rate: 0.0,
        };
        let md = build_self_contained_md(&stats, "");
        assert!(md.contains("v3_apply_count=3"));
        assert!(md.contains("v3_edit_apply_count=2"));
        assert!(md.contains("v3_edit_fail_rate=0.500"));
        assert!(md.contains("edit_to_patch_ratio=2.00"));
    }

    #[test]
    fn test_report_to_md() {
        let report = serde_json::json!({
            "title": "Test Report",
            "period": { "from": "2024-01-01", "to": "2024-01-07" },
            "summary_md": "Summary text.",
            "kpis": { "apply_count": 10, "fallback_count": 1 },
            "findings": [{ "severity": "info", "title": "Finding 1", "evidence": "Evidence 1" }],
            "recommendations": [{ "priority": "p1", "title": "Rec 1", "rationale": "Why", "expected_impact": "Impact" }],
            "operator_actions": [{ "title": "Action 1", "steps": ["Step 1"], "time_estimate_minutes": 5 }]
        });
        let md = report_to_md(&report);
        assert!(md.contains("# Test Report"));
        assert!(md.contains("Summary text."));
        assert!(md.contains("apply_count"));
        assert!(md.contains("Finding 1"));
        assert!(md.contains("Rec 1"));
        assert!(md.contains("Action 1"));
    }
}

/// Сохраняет отчёт в docs/reports/weekly_YYYY-MM-DD.md.
pub fn save_report_to_file(
    project_path: &Path,
    report_md: &str,
    date: Option<&str>,
) -> Result<String, String> {
    let date_str = date
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
    let reports_dir = project_path.join("docs").join("reports");
    fs::create_dir_all(&reports_dir).map_err(|e| format!("create_dir: {}", e))?;
    let file_path = reports_dir.join(format!("weekly_{}.md", date_str));
    fs::write(&file_path, report_md).map_err(|e| format!("write: {}", e))?;
    Ok(file_path.to_string_lossy().to_string())
}
