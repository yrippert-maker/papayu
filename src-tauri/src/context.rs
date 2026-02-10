//! Автосбор контекста для LLM: env, project prefs, context_requests (read_file, search, logs).
//! Кеш read/search/logs/env в пределах сессии (plan-цикла).
//! Protocol v2/v3: FILE[path] (sha256=...) для base_sha256 в PATCH_FILE/EDIT_FILE.

use crate::memory::EngineeringMemory;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn protocol_version() -> u32 {
    crate::protocol::protocol_version(None)
}

const MAX_CONTEXT_LINE_LEN: usize = 80_000;
const SEARCH_MAX_HITS: usize = 50;

fn context_max_files() -> usize {
    std::env::var("PAPAYU_CONTEXT_MAX_FILES")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(8)
}

fn context_max_file_chars() -> usize {
    std::env::var("PAPAYU_CONTEXT_MAX_FILE_CHARS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(20_000)
}

pub fn context_max_total_chars() -> usize {
    std::env::var("PAPAYU_CONTEXT_MAX_TOTAL_CHARS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(120_000)
}

#[allow(dead_code)]
fn context_max_log_chars() -> usize {
    std::env::var("PAPAYU_CONTEXT_MAX_LOG_CHARS")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(12_000)
}

/// Ключ кеша контекста.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub enum ContextCacheKey {
    Env,
    Logs { source: String, last_n: u32 },
    ReadFile { path: String, start: u32, end: u32 },
    Search { query: String, glob: Option<String> },
}

/// Статистика кеша (hits/misses по типам).
#[derive(Default, Clone, Debug)]
pub struct CacheStats {
    pub env_hits: u32,
    pub env_misses: u32,
    pub logs_hits: u32,
    pub logs_misses: u32,
    pub read_hits: u32,
    pub read_misses: u32,
    pub search_hits: u32,
    pub search_misses: u32,
}

impl CacheStats {
    pub fn total_hits(&self) -> u32 {
        self.env_hits + self.logs_hits + self.read_hits + self.search_hits
    }
    pub fn total_misses(&self) -> u32 {
        self.env_misses + self.logs_misses + self.read_misses + self.search_misses
    }
    pub fn hit_rate(&self) -> f64 {
        let t = self.total_hits() + self.total_misses();
        if t == 0 {
            0.0
        } else {
            self.total_hits() as f64 / t as f64
        }
    }
}

/// Статистика контекста (диета).
#[derive(Default, Clone, Debug)]
pub struct ContextStats {
    pub context_files_count: u32,
    pub context_files_dropped_count: u32,
    pub context_total_chars: usize,
    pub context_logs_chars: usize,
    pub context_truncated_files_count: u32,
}

/// Кеш контекста для сессии (plan-цикла).
#[derive(Default)]
pub struct ContextCache {
    map: HashMap<ContextCacheKey, String>,
    pub cache_stats: CacheStats,
}

impl ContextCache {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
            cache_stats: CacheStats::default(),
        }
    }

    pub fn get(&self, key: &ContextCacheKey) -> Option<&String> {
        self.map.get(key)
    }

    pub fn put(&mut self, key: ContextCacheKey, value: String) {
        self.map.insert(key, value);
    }

    pub fn stats(&self) -> &CacheStats {
        &self.cache_stats
    }
}

/// Собирает базовый контекст перед первым запросом к модели: env, команды из project prefs.
pub fn gather_base_context(_project_root: &Path, mem: &EngineeringMemory) -> String {
    let mut parts = Vec::new();

    let env_block = gather_env();
    if !env_block.is_empty() {
        parts.push(format!("ENV:\n{}", env_block));
    }

    if !mem.project.is_default() {
        let mut prefs = Vec::new();
        if !mem.project.default_test_command.is_empty() {
            prefs.push(format!(
                "default_test_command: {}",
                mem.project.default_test_command
            ));
        }
        if !mem.project.default_lint_command.is_empty() {
            prefs.push(format!(
                "default_lint_command: {}",
                mem.project.default_lint_command
            ));
        }
        if !mem.project.default_format_command.is_empty() {
            prefs.push(format!(
                "default_format_command: {}",
                mem.project.default_format_command
            ));
        }
        if !mem.project.src_roots.is_empty() {
            prefs.push(format!("src_roots: {:?}", mem.project.src_roots));
        }
        if !mem.project.test_roots.is_empty() {
            prefs.push(format!("test_roots: {:?}", mem.project.test_roots));
        }
        if !prefs.is_empty() {
            parts.push(format!("PROJECT_PREFS:\n{}", prefs.join("\n")));
        }
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("\n\nAUTO_CONTEXT:\n{}\n", parts.join("\n\n"))
    }
}

fn gather_env() -> String {
    let mut lines = Vec::new();
    if let Ok(os) = std::env::var("OS") {
        lines.push(format!("OS: {}", os));
    }
    #[cfg(target_os = "macos")]
    lines.push("OS: macOS".to_string());
    #[cfg(target_os = "linux")]
    lines.push("OS: Linux".to_string());
    #[cfg(target_os = "windows")]
    lines.push("OS: Windows".to_string());
    if let Ok(lang) = std::env::var("LANG") {
        lines.push(format!("LANG: {}", lang));
    }
    if let Ok(py) = std::env::var("VIRTUAL_ENV") {
        lines.push(format!("VIRTUAL_ENV: {}", py));
    }
    if let Ok(node) = std::env::var("NODE_VERSION") {
        lines.push(format!("NODE_VERSION: {}", node));
    }
    lines.join("\n")
}

/// Результат fulfill_context_requests: текст + статистика контекста.
pub struct FulfillResult {
    pub content: String,
    pub context_stats: ContextStats,
}

/// Выполняет context_requests от модели и возвращает текст для добавления в user message.
/// Использует кеш, если передан; логирует CONTEXT_CACHE_HIT/MISS при trace_id.
/// При protocol_version>=2 (v2 PATCH_FILE, v3 EDIT_FILE) добавляет sha256 в FILE-блоки: FILE[path] (sha256=...).
pub fn fulfill_context_requests(
    project_root: &Path,
    requests: &[serde_json::Value],
    max_log_lines: usize,
    mut cache: Option<&mut ContextCache>,
    trace_id: Option<&str>,
) -> FulfillResult {
    let include_sha256 = protocol_version() >= 2;
    let mut parts = Vec::new();
    let mut logs_chars: usize = 0;
    for r in requests {
        let obj = match r.as_object() {
            Some(o) => o,
            None => continue,
        };
        let rtype = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match rtype {
            "read_file" => {
                if let Some(path) = obj.get("path").and_then(|v| v.as_str()) {
                    let start = obj.get("start_line").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                    let end = obj
                        .get("end_line")
                        .and_then(|v| v.as_u64())
                        .unwrap_or((start + 200) as u64) as u32;
                    let key = ContextCacheKey::ReadFile {
                        path: path.to_string(),
                        start,
                        end,
                    };
                    let content = if let Some(ref mut c) = cache {
                        let hit = c.get(&key).map(|v| v.clone());
                        if let Some(v) = hit {
                            c.cache_stats.read_hits += 1;
                            if let Some(tid) = trace_id {
                                eprintln!(
                                    "[{}] CONTEXT_CACHE_HIT key=read_file path={}",
                                    tid, path
                                );
                            }
                            v
                        } else {
                            c.cache_stats.read_misses += 1;
                            let (snippet, sha) = read_file_snippet_with_sha256(
                                project_root,
                                path,
                                start as usize,
                                end as usize,
                            );
                            let out = if include_sha256 && !sha.is_empty() {
                                format!("FILE[{}] (sha256={}):\n{}", path, sha, snippet)
                            } else {
                                format!("FILE[{}]:\n{}", path, snippet)
                            };
                            if let Some(tid) = trace_id {
                                eprintln!(
                                    "[{}] CONTEXT_CACHE_MISS key=read_file path={} size={}",
                                    tid,
                                    path,
                                    out.len()
                                );
                            }
                            c.put(key, out.clone());
                            out
                        }
                    } else {
                        let (snippet, sha) = read_file_snippet_with_sha256(
                            project_root,
                            path,
                            start as usize,
                            end as usize,
                        );
                        if include_sha256 && !sha.is_empty() {
                            format!("FILE[{}] (sha256={}):\n{}", path, sha, snippet)
                        } else {
                            format!("FILE[{}]:\n{}", path, snippet)
                        }
                    };
                    parts.push(content);
                }
            }
            "search" => {
                if let Some(query) = obj.get("query").and_then(|v| v.as_str()) {
                    let glob = obj.get("glob").and_then(|v| v.as_str()).map(String::from);
                    let key = ContextCacheKey::Search {
                        query: query.to_string(),
                        glob: glob.clone(),
                    };
                    let content = if let Some(ref mut c) = cache {
                        let hit = c.get(&key).map(|v| v.clone());
                        if let Some(v) = hit {
                            c.cache_stats.search_hits += 1;
                            if let Some(tid) = trace_id {
                                eprintln!("[{}] CONTEXT_CACHE_HIT key=search query={}", tid, query);
                            }
                            v
                        } else {
                            c.cache_stats.search_misses += 1;
                            let hits = search_in_project(project_root, query, glob.as_deref());
                            let out = format!("SEARCH[{}]:\n{}", query, hits.join("\n"));
                            if let Some(tid) = trace_id {
                                eprintln!(
                                    "[{}] CONTEXT_CACHE_MISS key=search query={} hits={}",
                                    tid,
                                    query,
                                    hits.len()
                                );
                            }
                            c.put(key, out.clone());
                            out
                        }
                    } else {
                        let hits = search_in_project(project_root, query, glob.as_deref());
                        format!("SEARCH[{}]:\n{}", query, hits.join("\n"))
                    };
                    parts.push(content);
                }
            }
            "logs" => {
                let source = obj
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("runtime");
                let last_n = obj
                    .get("last_n")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(max_log_lines as u64) as u32;
                let key = ContextCacheKey::Logs {
                    source: source.to_string(),
                    last_n,
                };
                let content = if let Some(ref mut c) = cache {
                    let hit = c.get(&key).map(|v| v.clone());
                    if let Some(v) = hit {
                        c.cache_stats.logs_hits += 1;
                        if let Some(tid) = trace_id {
                            eprintln!("[{}] CONTEXT_CACHE_HIT key=logs source={}", tid, source);
                        }
                        v
                    } else {
                        c.cache_stats.logs_misses += 1;
                        let v = format!(
                            "LOGS[{}]: (last_n={}; приложение не имеет доступа к логам runtime — передай вывод в запросе)\n",
                            source, last_n
                        );
                        if let Some(tid) = trace_id {
                            eprintln!("[{}] CONTEXT_CACHE_MISS key=logs source={}", tid, source);
                        }
                        c.put(key, v.clone());
                        v
                    }
                } else {
                    format!(
                        "LOGS[{}]: (last_n={}; приложение не имеет доступа к логам runtime — передай вывод в запросе)\n",
                        source, last_n
                    )
                };
                logs_chars += content.len();
                parts.push(content);
            }
            "env" => {
                let key = ContextCacheKey::Env;
                let content = if let Some(ref mut c) = cache {
                    let hit = c.get(&key).map(|v| v.clone());
                    if let Some(v) = hit {
                        c.cache_stats.env_hits += 1;
                        if let Some(tid) = trace_id {
                            eprintln!("[{}] CONTEXT_CACHE_HIT key=env", tid);
                        }
                        v
                    } else {
                        c.cache_stats.env_misses += 1;
                        let v = format!("ENV (повторно):\n{}", gather_env());
                        if let Some(tid) = trace_id {
                            eprintln!("[{}] CONTEXT_CACHE_MISS key=env size={}", tid, v.len());
                        }
                        c.put(key, v.clone());
                        v
                    }
                } else {
                    format!("ENV (повторно):\n{}", gather_env())
                };
                parts.push(content);
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        FulfillResult {
            content: String::new(),
            context_stats: ContextStats::default(),
        }
    } else {
        let max_files = context_max_files();
        let max_total = context_max_total_chars();
        const MIN_CHARS_FOR_PRIORITY0: usize = 4096;
        let header = "\n\nFULFILLED_CONTEXT:\n";
        let mut total_chars = header.len();
        let mut result_parts = Vec::with_capacity(parts.len().min(max_files));
        let mut dropped = 0;
        let mut truncated = 0;
        for (_i, p) in parts.iter().enumerate() {
            if result_parts.len() >= max_files {
                dropped += 1;
                continue;
            }
            let part_len = p.len() + if result_parts.is_empty() { 0 } else { 2 };
            let budget_left = max_total.saturating_sub(total_chars);
            if total_chars + part_len > max_total && !result_parts.is_empty() {
                let is_file = p.starts_with("FILE[");
                if is_file && budget_left >= MIN_CHARS_FOR_PRIORITY0 {
                    let to_add = if p.len() > budget_left {
                        truncated += 1;
                        let head = (budget_left as f32 * 0.6) as usize;
                        format!("{}...[TRUNCATED]...", &p[..head.min(p.len())])
                    } else {
                        p.clone()
                    };
                    total_chars += to_add.len() + if result_parts.is_empty() { 0 } else { 2 };
                    result_parts.push(to_add);
                } else {
                    dropped += 1;
                }
                continue;
            }
            let to_add = if total_chars + part_len > max_total {
                let allowed = max_total - total_chars - 30;
                if allowed > 100 {
                    truncated += 1;
                    format!("{}...[TRUNCATED]...", &p[..allowed.min(p.len())])
                } else {
                    p.clone()
                }
            } else {
                p.clone()
            };
            total_chars += to_add.len() + if result_parts.is_empty() { 0 } else { 2 };
            result_parts.push(to_add);
        }
        let content = format!("{}{}", header, result_parts.join("\n\n"));
        let files_in_result = result_parts
            .iter()
            .filter(|s| s.starts_with("FILE["))
            .count() as u32;
        let context_stats = ContextStats {
            context_files_count: files_in_result,
            context_files_dropped_count: dropped as u32,
            context_total_chars: total_chars,
            context_logs_chars: logs_chars,
            context_truncated_files_count: truncated,
        };
        if let Some(tid) = trace_id {
            if dropped > 0 || truncated > 0 {
                eprintln!(
                    "[{}] CONTEXT_DIET_APPLIED files={} dropped={} truncated={} total_chars={}",
                    tid,
                    result_parts.len(),
                    dropped,
                    truncated,
                    total_chars
                );
            }
        }
        FulfillResult {
            content,
            context_stats,
        }
    }
}

/// Читает файл и возвращает (snippet, sha256_hex). sha256 — от полного содержимого файла.
fn read_file_snippet_with_sha256(
    root: &Path,
    rel_path: &str,
    start_line: usize,
    end_line: usize,
) -> (String, String) {
    let path = root.join(rel_path);
    if !path.is_file() {
        return (format!("(файл не найден: {})", rel_path), String::new());
    }
    let full_content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return ("(не удалось прочитать)".to_string(), String::new()),
    };
    let sha256_hex = {
        let mut hasher = Sha256::new();
        hasher.update(full_content.as_bytes());
        format!("{:x}", hasher.finalize())
    };
    let lines: Vec<&str> = full_content.lines().collect();
    let start = start_line.saturating_sub(1).min(lines.len());
    let end = end_line.min(lines.len()).max(start);
    let slice: Vec<&str> = lines
        .get(start..end)
        .unwrap_or(&[])
        .into_iter()
        .copied()
        .collect();
    let mut out = String::new();
    for (i, line) in slice.iter().enumerate() {
        let line_no = start + i + 1;
        out.push_str(&format!("{}|{}\n", line_no, line));
    }
    let max_chars = context_max_file_chars().min(MAX_CONTEXT_LINE_LEN);
    let snippet = if out.len() > max_chars {
        let head = (max_chars as f32 * 0.6) as usize;
        let tail = max_chars - head - 30;
        format!(
            "{}...[TRUNCATED {} chars]...\n{}",
            &out[..head.min(out.len())],
            out.len(),
            &out[out.len().saturating_sub(tail)..]
        )
    } else {
        out
    };
    (snippet, sha256_hex)
}

fn read_file_snippet(root: &Path, rel_path: &str, start_line: usize, end_line: usize) -> String {
    let path = root.join(rel_path);
    if !path.is_file() {
        return format!("(файл не найден: {})", rel_path);
    }
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return "(не удалось прочитать)".to_string(),
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = start_line.saturating_sub(1).min(lines.len());
    let end = end_line.min(lines.len()).max(start);
    let slice: Vec<&str> = lines
        .get(start..end)
        .unwrap_or(&[])
        .into_iter()
        .copied()
        .collect();
    let mut out = String::new();
    for (i, line) in slice.iter().enumerate() {
        let line_no = start + i + 1;
        out.push_str(&format!("{}|{}\n", line_no, line));
    }
    let max_chars = context_max_file_chars().min(MAX_CONTEXT_LINE_LEN);
    if out.len() > max_chars {
        let head = (max_chars as f32 * 0.6) as usize;
        let tail = max_chars - head - 30;
        format!(
            "{}...[TRUNCATED {} chars]...\n{}",
            &out[..head.min(out.len())],
            out.len(),
            &out[out.len().saturating_sub(tail)..]
        )
    } else {
        out
    }
}

fn search_in_project(root: &Path, query: &str, _glob: Option<&str>) -> Vec<String> {
    let mut hits = Vec::new();
    let walk = walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_str().unwrap_or("");
            !n.starts_with('.')
                && n != "node_modules"
                && n != "target"
                && n != "dist"
                && n != "__pycache__"
        });
    for entry in walk.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_text = [
            "py", "rs", "ts", "tsx", "js", "jsx", "md", "json", "toml", "yml", "yaml",
        ]
        .contains(&ext);
        if !is_text {
            continue;
        }
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        for (i, line) in content.lines().enumerate() {
            if line.contains(query) {
                let rel = path
                    .strip_prefix(root)
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| path.display().to_string());
                hits.push(format!("{}:{}: {}", rel, i + 1, line.trim()));
                if hits.len() >= SEARCH_MAX_HITS {
                    return hits;
                }
            }
        }
    }
    hits
}

/// Эвристики автосбора контекста до первого вызова LLM.
/// Возвращает дополнительный контекст на основе user_goal/report (Traceback, ImportError и т.д.).
pub fn gather_auto_context_from_message(project_root: &Path, user_message: &str) -> String {
    let mut parts = Vec::new();

    // Traceback / Exception → извлечь пути и прочитать файлы ±80 строк
    let traceback_files = extract_traceback_files(user_message);
    let root_str = project_root.display().to_string();
    for (path_from_tb, line_no) in traceback_files {
        // Преобразовать абсолютный путь в относительный (если project_root — префикс)
        let rel_path = if path_from_tb.starts_with('/')
            || (path_from_tb.len() >= 2 && path_from_tb.chars().nth(1) == Some(':'))
        {
            // Абсолютный путь: убрать префикс project_root
            let normalized = path_from_tb.replace('\\', "/");
            let root_norm = root_str.replace('\\', "/");
            if normalized.starts_with(&root_norm) {
                normalized
                    .strip_prefix(&root_norm)
                    .map(|s| s.trim_start_matches('/').to_string())
                    .unwrap_or(path_from_tb)
            } else {
                path_from_tb
            }
        } else {
            path_from_tb
        };
        let start = line_no.saturating_sub(80).max(1);
        let end = line_no + 80;
        let content = read_file_snippet(project_root, &rel_path, start, end);
        if !content.contains("не найден") && !content.contains("не удалось") {
            parts.push(format!("AUTO_TRACEBACK[{}]:\n{}", rel_path, content));
        }
    }

    // ImportError / ModuleNotFoundError → env + lock/deps файлы
    let lower = user_message.to_lowercase();
    if lower.contains("importerror")
        || lower.contains("modulenotfounderror")
        || lower.contains("cannot find module")
        || lower.contains("module not found")
    {
        parts.push(format!("ENV (для ImportError):\n{}", gather_env()));
        // Попытаться добавить содержимое pyproject.toml, requirements.txt, package.json
        for rel in [
            "pyproject.toml",
            "requirements.txt",
            "package.json",
            "poetry.lock",
        ] {
            let p = project_root.join(rel);
            if p.is_file() {
                if let Ok(s) = fs::read_to_string(&p) {
                    let trimmed = if s.len() > 8000 {
                        format!("{}…\n(обрезано)", &s[..8000])
                    } else {
                        s
                    };
                    parts.push(format!("DEPS[{}]:\n{}", rel, trimmed));
                }
            }
        }
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!("\n\nAUTO_CONTEXT_FROM_MESSAGE:\n{}\n", parts.join("\n\n"))
    }
}

/// Извлекает path → sha256 из контекста (FILE[path] (sha256=...):). Для диагностики и repair.
pub fn extract_file_sha256_from_context(
    context: &str,
) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let mut m = HashMap::new();
    for line in context.lines() {
        if !line.starts_with("FILE[") {
            continue;
        }
        let close = match line.find(']') {
            Some(i) => i,
            None => continue,
        };
        let path = &line[5..close];
        let sha_tag = "(sha256=";
        let sha_pos = match line.find(sha_tag) {
            Some(i) => i,
            None => continue,
        };
        let sha_start = sha_pos + sha_tag.len();
        let sha_end = match line[sha_start..].find(')') {
            Some(j) => sha_start + j,
            None => continue,
        };
        let sha = &line[sha_start..sha_end];
        if sha.len() == 64 && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
            m.insert(path.to_string(), sha.to_string());
        }
    }
    m
}

/// Извлекает пути и строки из traceback в тексте (Python). Используется при автосборе контекста по ошибке.
pub fn extract_traceback_files(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("File \"") {
            if let Some(rest) = line.strip_prefix("File \"") {
                if let Some(end) = rest.find('\"') {
                    let path = rest[..end].to_string();
                    let after = &rest[end + 1..];
                    let line_no = after
                        .trim_start_matches(", line ")
                        .split(',')
                        .next()
                        .and_then(|s| s.trim().parse::<usize>().ok())
                        .unwrap_or(0);
                    if !path.is_empty() && line_no > 0 {
                        out.push((path, line_no));
                    }
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_read_file_hit() {
        let mut cache = ContextCache::new();
        let key = ContextCacheKey::ReadFile {
            path: "foo.rs".to_string(),
            start: 1,
            end: 10,
        };
        cache.put(key.clone(), "FILE[foo.rs]:\n1|line1".to_string());
        assert!(cache.get(&key).is_some());
        assert!(cache.get(&key).unwrap().contains("foo.rs"));
    }

    #[test]
    fn test_cache_search_hit() {
        let mut cache = ContextCache::new();
        let key = ContextCacheKey::Search {
            query: "test".to_string(),
            glob: None,
        };
        cache.put(key.clone(), "SEARCH[test]:\nfoo:1: test".to_string());
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn test_cache_env_hit() {
        let mut cache = ContextCache::new();
        let key = ContextCacheKey::Env;
        cache.put(key.clone(), "ENV:\nOS: test".to_string());
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn test_cache_logs_hit() {
        let mut cache = ContextCache::new();
        let key = ContextCacheKey::Logs {
            source: "runtime".to_string(),
            last_n: 100,
        };
        cache.put(key.clone(), "LOGS[runtime]: ...".to_string());
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn test_cache_logs_key_includes_last_n() {
        let mut cache = ContextCache::new();
        cache.put(
            ContextCacheKey::Logs {
                source: "runtime".to_string(),
                last_n: 200,
            },
            "LOGS last_n=200".to_string(),
        );
        cache.put(
            ContextCacheKey::Logs {
                source: "runtime".to_string(),
                last_n: 500,
            },
            "LOGS last_n=500".to_string(),
        );
        assert!(cache
            .get(&ContextCacheKey::Logs {
                source: "runtime".to_string(),
                last_n: 200
            })
            .unwrap()
            .contains("200"));
        assert!(cache
            .get(&ContextCacheKey::Logs {
                source: "runtime".to_string(),
                last_n: 500
            })
            .unwrap()
            .contains("500"));
    }

    #[test]
    fn test_context_diet_max_files() {
        let max = context_max_files();
        assert!(max >= 1 && max <= 100);
    }

    #[test]
    fn test_context_diet_limits() {
        assert!(context_max_file_chars() > 1000);
        assert!(context_max_total_chars() > 10000);
    }

    #[test]
    fn extract_traceback_parses_file_line() {
        let t = r#"  File "/home/x/src/main.py", line 42, in foo
    bar()
"#;
        let files = extract_traceback_files(t);
        assert_eq!(files.len(), 1);
        assert!(files[0].0.contains("main.py"));
        assert_eq!(files[0].1, 42);
    }

    #[test]
    fn test_extract_file_sha256_from_context() {
        let ctx = r#"FILE[src/parser.py] (sha256=7f3f2a0c9f8b1a0c9b4c0f9e3d8a4b2d8c9e7f1a0b3c4d5e6f7a8b9c0d1e2f3a):
1|def parse

FILE[src/main.rs]:
fn main() {}"#;
        let m = extract_file_sha256_from_context(ctx);
        assert_eq!(m.len(), 1);
        assert_eq!(
            m.get("src/parser.py").map(|s| s.as_str()),
            Some("7f3f2a0c9f8b1a0c9b4c0f9e3d8a4b2d8c9e7f1a0b3c4d5e6f7a8b9c0d1e2f3a")
        );
        // src/main.rs без sha256 — не попадёт
        assert!(m.get("src/main.rs").is_none());

        let sha_a = "a".repeat(64);
        let sha_b = "b".repeat(64);
        let ctx2a = format!("FILE[a.py] (sha256={}):\ncontent\n", sha_a);
        let ctx2b = format!("FILE[b.rs] (sha256={}):\ncontent\n", sha_b);
        let m2a = extract_file_sha256_from_context(&ctx2a);
        let m2b = extract_file_sha256_from_context(&ctx2b);
        assert_eq!(m2a.len(), 1);
        assert_eq!(m2b.len(), 1);
        assert_eq!(m2a.get("a.py").map(|s| s.len()), Some(64));
        assert_eq!(m2b.get("b.rs").map(|s| s.len()), Some(64));
    }

    #[test]
    fn test_render_file_block_v2_includes_sha256() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {}\n").unwrap();
        std::env::set_var("PAPAYU_PROTOCOL_VERSION", "2");
        let reqs = vec![
            serde_json::json!({"type": "read_file", "path": "src/main.rs", "start_line": 1, "end_line": 10}),
        ];
        let result = fulfill_context_requests(root, &reqs, 200, None, None);
        std::env::remove_var("PAPAYU_PROTOCOL_VERSION");
        assert!(result.content.contains("FILE[src/main.rs] (sha256="));
        assert!(result.content.contains("):"));
        let m = extract_file_sha256_from_context(&result.content);
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("src/main.rs").map(|s| s.len()), Some(64));
    }
}
