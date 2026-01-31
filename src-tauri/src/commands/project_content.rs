//! Сбор полного содержимого проекта для ИИ: все релевантные файлы/папки в пределах лимитов.
//! Анализ ИИ-агентом делается по всему содержимому, а не по трём файлам.

use std::fs;
use std::path::Path;

/// Расширения текстовых файлов для включения в контекст ИИ
const TEXT_EXT: &[&str] = &[
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "rs", "py", "json", "toml", "md", "yml", "yaml",
    "css", "scss", "html", "xml", "vue", "svelte", "go", "rb", "java", "kt", "swift", "c", "h",
    "cpp", "hpp", "sh", "bash", "zsh", "sql", "graphql",
];

/// Папки, которые не сканируем
const EXCLUDE_DIRS: &[&str] = &[
    "node_modules", "target", "dist", "build", ".git", ".next", ".nuxt", ".cache",
    "coverage", "__pycache__", ".venv", "venv", ".idea", ".vscode", "vendor",
];

/// Макс. символов на файл (чтобы не перегружать контекст)
const MAX_BYTES_PER_FILE: usize = 80_000;
/// Макс. суммарных символов для контекста LLM (~200k токенов)
const MAX_TOTAL_CHARS: usize = 600_000;
/// Макс. число файлов
const MAX_FILES: usize = 500;

/// Собирает содержимое релевантных файлов проекта в одну строку для передачи в LLM.
/// Сканирует всю папку/папки (без искусственного ограничения «тремя файлами»).
pub fn get_project_content_for_llm(root: &Path, max_total_chars: Option<usize>) -> String {
    let limit = max_total_chars.unwrap_or(MAX_TOTAL_CHARS);
    let mut out = String::with_capacity(limit.min(MAX_TOTAL_CHARS + 1024));
    let mut total = 0usize;
    let mut files_added = 0usize;

    if !root.exists() || !root.is_dir() {
        return "Папка не найдена или пуста. Можно создать проект с нуля.".to_string();
    }

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            if total >= limit || files_added >= MAX_FILES {
                break;
            }
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if EXCLUDE_DIRS.contains(&name) {
                    continue;
                }
                collect_dir(&path, root, &mut out, &mut total, &mut files_added, limit);
            } else if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext = ext.to_str().unwrap_or("").to_lowercase();
                    if TEXT_EXT.iter().any(|e| *e == ext) {
                        if let Ok(content) = fs::read_to_string(&path) {
                            let rel = path.strip_prefix(root).unwrap_or(&path);
                            let rel_str = rel.display().to_string();
                            let truncated = if content.len() > MAX_BYTES_PER_FILE {
                                format!("{}…\n(обрезано, всего {} байт)", &content[..MAX_BYTES_PER_FILE], content.len())
                            } else {
                                content
                            };
                            let block = format!("\n=== {} ===\n{}\n", rel_str, truncated);
                            if total + block.len() <= limit {
                                out.push_str(&block);
                                total += block.len();
                                files_added += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    if out.is_empty() {
        out = "В папке нет релевантных исходных файлов. Можно создать проект с нуля.".to_string();
    } else {
        out.insert_str(0, "Содержимое файлов проекта (полный контекст для анализа):\n");
    }
    out
}

fn collect_dir(
    dir: &Path,
    root: &Path,
    out: &mut String,
    total: &mut usize,
    files_added: &mut usize,
    limit: usize,
) {
    if *total >= limit || *files_added >= MAX_FILES {
        return;
    }
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };
    let mut entries: Vec<_> = read.flatten().collect();
    entries.sort_by(|a, b| {
        let a = a.path();
        let b = b.path();
        let a_dir = a.is_dir();
        let b_dir = b.is_dir();
        match (a_dir, b_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
    for entry in entries {
        if *total >= limit || *files_added >= MAX_FILES {
            break;
        }
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if EXCLUDE_DIRS.contains(&name) {
                continue;
            }
            collect_dir(&path, root, out, total, files_added, limit);
        } else if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext = ext.to_str().unwrap_or("").to_lowercase();
                if TEXT_EXT.iter().any(|e| *e == ext) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        let rel = path.strip_prefix(root).unwrap_or(&path);
                        let rel_str = rel.display().to_string();
                        let truncated = if content.len() > MAX_BYTES_PER_FILE {
                            format!("{}…\n(обрезано, всего {} байт)", &content[..MAX_BYTES_PER_FILE], content.len())
                        } else {
                            content
                        };
                        let block = format!("\n=== {} ===\n{}\n", rel_str, truncated);
                        if *total + block.len() <= limit {
                            out.push_str(&block);
                            *total += block.len();
                            *files_added += 1;
                        }
                    }
                }
            }
        }
    }
}
