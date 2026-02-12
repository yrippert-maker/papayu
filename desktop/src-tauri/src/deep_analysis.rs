use crate::types::{Finding, ProjectSignal};
use std::path::Path;
use std::fs;

const MAX_SCAN_SIZE: u64 = 512 * 1024;

const CODE_EXTENSIONS: &[&str] = &[
    "js", "jsx", "ts", "tsx", "mjs", "cjs",
    "py", "rs", "go", "rb", "php", "java", "kt",
    "sh", "bash", "zsh",
    "yml", "yaml", "toml", "json",
    "sql", "env", "cfg", "ini", "conf",
];

const SECRET_PATTERNS: &[(&str, &str)] = &[
    (r"(?i)(password|passwd|pwd)\s*[:=]\s*['\x22][^'\x22]{4,}['\x22]", "Захардкоженный пароль"),
    (r"(?i)(api[_-]?key|apikey)\s*[:=]\s*['\x22][^'\x22]{8,}['\x22]", "Захардкоженный API-ключ"),
    (r"(?i)(secret|token)\s*[:=]\s*['\x22][^'\x22]{8,}['\x22]", "Захардкоженный секрет/токен"),
    (r"AKIA[0-9A-Z]{16}", "AWS Access Key ID"),
    (r"-----BEGIN (RSA |EC |DSA )?PRIVATE KEY-----", "PEM приватный ключ"),
    (r"ghp_[0-9a-zA-Z]{36}", "GitHub Personal Access Token"),
    (r"sk-[a-zA-Z0-9]{20,}", "Возможный API-ключ (sk-...)"),
];

const VULN_PATTERNS: &[(&str, &str, &str)] = &[
    (r"eval\s*\(", "Использование eval() — риск code injection", "js,jsx,ts,tsx,py"),
    (r"innerHTML\s*=", "Прямая запись innerHTML — риск XSS", "js,jsx,ts,tsx"),
    (r"document\.write\s*\(", "document.write() — устаревший метод", "js,jsx,ts,tsx"),
    (r"(?i)dangerouslySetInnerHTML", "dangerouslySetInnerHTML — риск XSS", "jsx,tsx"),
    (r"subprocess\.call\s*\(.*shell\s*=\s*True", "subprocess с shell=True", "py"),
    (r"os\.system\s*\(", "os.system() — лучше subprocess", "py"),
    (r"(?i)cors.*origin.*\*", "CORS с wildcard origin", "js,ts,py,rb"),
    (r"(?i)chmod\s+777", "chmod 777 — слишком широкие права", "sh,bash,zsh,yml,yaml"),
];

const QUALITY_PATTERNS: &[(&str, &str, &str)] = &[
    (r"TODO|FIXME|HACK|XXX", "TODO/FIXME комментарии", "js,jsx,ts,tsx,py,rs,go,rb"),
    (r"console\.(log|debug|info)\s*\(", "console.log в коде", "js,jsx,ts,tsx"),
    (r"dbg!\s*\(", "dbg!() макрос (отладочный)", "rs"),
    (r"\.unwrap\(\)", "Небезопасный .unwrap()", "rs"),
];

pub struct DeepAnalysisResult {
    pub findings: Vec<Finding>,
    pub signals: Vec<ProjectSignal>,
    pub todo_count: u32,
    pub security_issues: u32,
    pub quality_issues: u32,
    pub files_scanned: u32,
}

pub fn run_deep_analysis(root: &Path) -> DeepAnalysisResult {
    let mut result = DeepAnalysisResult {
        findings: Vec::new(), signals: Vec::new(),
        todo_count: 0, security_issues: 0, quality_issues: 0, files_scanned: 0,
    };
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    collect_files(root, root, 0, &mut files);

    for file_path in &files {
        let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        let content = match fs::read_to_string(file_path) { Ok(c) => c, Err(_) => continue };
        result.files_scanned += 1;
        let rel = file_path.strip_prefix(root).unwrap_or(file_path).to_string_lossy().to_string();

        if !rel.contains(".example") && !rel.contains(".sample") {
            for (pat, desc) in SECRET_PATTERNS {
                if let Ok(re) = regex::Regex::new(pat) {
                    if re.is_match(&content) {
                        result.security_issues += 1;
                        result.findings.push(Finding { severity: "high".into(), title: format!("🔐 {}", desc), details: format!("Файл: {}", rel) });
                        result.signals.push(ProjectSignal { category: "security".into(), level: "high".into(), message: format!("{} в {}", desc, rel) });
                    }
                }
            }
        }

        for (pat, title, exts) in VULN_PATTERNS {
            let applicable: Vec<&str> = exts.split(',').collect();
            if !applicable.contains(&ext.as_str()) { continue; }
            if let Ok(re) = regex::Regex::new(pat) {
                let matches: Vec<_> = re.find_iter(&content).collect();
                if !matches.is_empty() {
                    result.security_issues += 1;
                    let line = content[..matches[0].start()].chars().filter(|c| *c == '\n').count() + 1;
                    result.findings.push(Finding { severity: "high".into(), title: format!("⚠️ {}", title), details: format!("{}:{} ({} шт.)", rel, line, matches.len()) });
                }
            }
        }

        for (pat, title, exts) in QUALITY_PATTERNS {
            let applicable: Vec<&str> = exts.split(',').collect();
            if !applicable.contains(&ext.as_str()) { continue; }
            if let Ok(re) = regex::Regex::new(pat) {
                let matches: Vec<_> = re.find_iter(&content).collect();
                if !matches.is_empty() {
                    let count = matches.len();
                    if pat.contains("TODO") { result.todo_count += count as u32; }
                    result.quality_issues += count as u32;
                    if count >= 3 || pat.contains("unwrap") {
                        result.findings.push(Finding { severity: "warn".into(), title: format!("📝 {}", title), details: format!("{}: {} шт.", rel, count) });
                    }
                }
            }
        }

        let lines = content.lines().count();
        if lines > 500 {
            result.findings.push(Finding { severity: "warn".into(), title: "📏 Большой файл".into(), details: format!("{}: {} строк", rel, lines) });
        }

        if rel == "package.json" { check_package_json(&content, &mut result); }
        if rel == "requirements.txt" { check_requirements_txt(&content, &mut result); }
    }

    if result.security_issues > 0 {
        result.signals.push(ProjectSignal { category: "security".into(), level: "high".into(), message: format!("Deep analysis: {} проблем безопасности", result.security_issues) });
    }
    if result.todo_count > 5 {
        result.signals.push(ProjectSignal { category: "quality".into(), level: "warn".into(), message: format!("{} TODO/FIXME комментариев", result.todo_count) });
    }
    result
}

fn collect_files(root: &Path, dir: &Path, depth: u32, out: &mut Vec<std::path::PathBuf>) {
    if depth > 10 || out.len() > 500 { return; }
    let entries = match fs::read_dir(dir) { Ok(e) => e, Err(_) => return };
    let excluded = ["node_modules", ".git", "target", "dist", "build", ".next", "__pycache__", ".venv", "venv", "vendor", ".cargo"];
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if excluded.contains(&name) { continue; }
            collect_files(root, &path, depth + 1, out);
            continue;
        }
        if let Ok(meta) = path.metadata() { if meta.len() > MAX_SCAN_SIZE { continue; } }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if CODE_EXTENSIONS.contains(&ext) { out.push(path); }
    }
}

fn check_package_json(content: &str, result: &mut DeepAnalysisResult) {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
        if let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) {
            if !scripts.contains_key("test") || scripts.get("test").and_then(|t| t.as_str()).unwrap_or("").contains("no test specified") {
                result.findings.push(Finding { severity: "warn".into(), title: "🧪 Нет скрипта test".into(), details: "npm test не настроен".into() });
            }
        }
    }
}

fn check_requirements_txt(content: &str, result: &mut DeepAnalysisResult) {
    let unpinned: u32 = content.lines().filter(|l| { let l = l.trim(); !l.is_empty() && !l.starts_with('#') && !l.contains("==") }).count() as u32;
    if unpinned > 3 {
        result.findings.push(Finding { severity: "warn".into(), title: "📦 Незафиксированные версии".into(), details: format!("{} пакетов без ==", unpinned) });
    }
}
