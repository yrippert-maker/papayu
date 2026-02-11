//! Сканирование проекта на типичные утечки: ключи в коде, .env в репо, хардкод паролей.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const MAX_FILE_SIZE: usize = 256 * 1024; // 256 KB
const SKIP_DIRS: &[&str] = &["node_modules", "target", "dist", ".git", "build", ".next"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretSuspicion {
    pub path: String,
    pub line: Option<u32>,
    pub kind: String,
    pub snippet: String,
}

fn is_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name)
}

fn check_content(path: &str, content: &str) -> Vec<SecretSuspicion> {
    let mut out = Vec::new();
    let _lower = content.to_lowercase();
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("api_key") && line_lower.contains("=") && !line_lower.contains("example") {
            out.push(SecretSuspicion {
                path: path.to_string(),
                line: Some((i + 1) as u32),
                kind: "api_key".to_string(),
                snippet: line.trim().chars().take(80).collect::<String>(),
            });
        }
        if line_lower.contains("password") && line_lower.contains("=") && !line_lower.contains("example") && !line_lower.contains("env.example") {
            out.push(SecretSuspicion {
                path: path.to_string(),
                line: Some((i + 1) as u32),
                kind: "password".to_string(),
                snippet: line.trim().chars().take(80).collect::<String>(),
            });
        }
        if line_lower.contains("secret") && line_lower.contains("=") && !line_lower.contains("example") {
            out.push(SecretSuspicion {
                path: path.to_string(),
                line: Some((i + 1) as u32),
                kind: "secret".to_string(),
                snippet: line.trim().chars().take(80).collect::<String>(),
            });
        }
        if (line_lower.contains("sk-") || line_lower.contains("ghp_") || line_lower.contains("xoxb-")) && line.len() > 10 {
            out.push(SecretSuspicion {
                path: path.to_string(),
                line: Some((i + 1) as u32),
                kind: "token_like".to_string(),
                snippet: "[REDACTED]".to_string(),
            });
        }
    }
    if path.ends_with(".env") && !path.ends_with(".env.example") {
        if !out.is_empty() {
            return out;
        }
        out.push(SecretSuspicion {
            path: path.to_string(),
            line: None,
            kind: "env_file".to_string(),
            snippet: ".env не должен быть в репозитории; используйте .env.example".to_string(),
        });
    }
    out
}

/// Сканирует директорию и возвращает список подозрений на утечку секретов.
pub fn scan_secrets(project_path: &Path) -> Vec<SecretSuspicion> {
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(project_path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            e.path().file_name().map_or(true, |n| {
                !is_skip_dir(n.to_string_lossy().as_ref())
            })
        });
    for entry in walker.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let allowed = matches!(
            (ext, name),
            ("ts", _) | ("tsx", _) | ("js", _) | ("jsx", _) | ("rs", _) | ("py", _) | ("json", _)
                | ("env", _) | ("yaml", _) | ("yml", _) | ("toml", _) | ("md", _)
        ) || name.starts_with(".env");
        if !allowed {
            continue;
        }
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if content.len() > MAX_FILE_SIZE {
            continue;
        }
        let rel = path.strip_prefix(project_path).unwrap_or(path);
        let rel_str = rel.to_string_lossy().to_string();
        for s in check_content(&rel_str, &content) {
            out.push(s);
        }
    }
    out
}
