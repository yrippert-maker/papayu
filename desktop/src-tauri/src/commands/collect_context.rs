use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const MAX_CONTEXT_BYTES: usize = 100_000;
const MAX_FILE_BYTES: u64 = 30_000;

const CODE_EXTENSIONS: &[&str] = &[
    "js","jsx","ts","tsx","mjs","cjs","py","rs","go","rb","php","java","kt",
    "sh","bash","yml","yaml","toml","json","md","txt","sql","graphql",
    "css","scss","html","vue","svelte",
];

const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",".git","target","dist","build",".next",
    "__pycache__",".venv","venv","vendor",".cargo",
];

const PRIORITY_FILES: &[&str] = &[
    "package.json","Cargo.toml","pyproject.toml","requirements.txt",
    "README.md","readme.md","tsconfig.json",
    "next.config.js","next.config.ts","vite.config.ts","vite.config.js",
    "Dockerfile","docker-compose.yml",".env.example",".gitignore",
];

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectContextRequest { pub path: String }

#[derive(Debug, Serialize, Deserialize)]
pub struct FileContext { pub path: String, pub content: String, pub lines: u32 }

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectContextResponse {
    pub ok: bool, pub files: Vec<FileContext>,
    pub total_files: u32, pub total_bytes: u32,
    pub truncated: bool, pub error: Option<String>,
}

#[tauri::command]
pub async fn collect_project_context(request: ProjectContextRequest) -> Result<ProjectContextResponse, String> {
    let root = Path::new(&request.path);
    if !root.exists() || !root.is_dir() {
        return Ok(ProjectContextResponse { ok: false, files: vec![], total_files: 0, total_bytes: 0, truncated: false, error: Some(format!("Путь не существует: {}", request.path)) });
    }
    let mut files: Vec<FileContext> = Vec::new();
    let mut total_bytes: usize = 0;
    let mut truncated = false;

    for pf in PRIORITY_FILES {
        let fp = root.join(pf);
        if fp.exists() && fp.is_file() {
            if let Some(fc) = read_file_ctx(root, &fp) { total_bytes += fc.content.len(); files.push(fc); }
        }
    }

    let mut all: Vec<std::path::PathBuf> = Vec::new();
    collect_code_files(root, root, 0, &mut all);
    all.sort_by(|a, b| {
        let a_src = a.to_string_lossy().contains("src/");
        let b_src = b.to_string_lossy().contains("src/");
        match (a_src, b_src) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.metadata().map(|m| m.len()).unwrap_or(u64::MAX).cmp(&b.metadata().map(|m| m.len()).unwrap_or(u64::MAX)),
        }
    });

    for fp in &all {
        if total_bytes >= MAX_CONTEXT_BYTES { truncated = true; break; }
        let rel = fp.strip_prefix(root).unwrap_or(fp).to_string_lossy().to_string();
        if files.iter().any(|f| f.path == rel) { continue; }
        if let Some(fc) = read_file_ctx(root, fp) {
            if total_bytes + fc.content.len() > MAX_CONTEXT_BYTES { truncated = true; break; }
            total_bytes += fc.content.len();
            files.push(fc);
        }
    }

    Ok(ProjectContextResponse { ok: true, total_files: files.len() as u32, total_bytes: total_bytes as u32, truncated, files, error: None })
}

fn read_file_ctx(root: &Path, fp: &Path) -> Option<FileContext> {
    let meta = fp.metadata().ok()?;
    if meta.len() > MAX_FILE_BYTES { return None; }
    let content = fs::read_to_string(fp).ok()?;
    let rel = fp.strip_prefix(root).unwrap_or(fp).to_string_lossy().to_string();
    Some(FileContext { path: rel, lines: content.lines().count() as u32, content })
}

fn collect_code_files(root: &Path, dir: &Path, depth: u32, out: &mut Vec<std::path::PathBuf>) {
    if depth > 8 || out.len() > 300 { return; }
    let entries = match fs::read_dir(dir) { Ok(e) => e, Err(_) => return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if EXCLUDED_DIRS.contains(&name) || name.starts_with('.') { continue; }
            collect_code_files(root, &path, depth + 1, out);
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if CODE_EXTENSIONS.contains(&ext) { out.push(path); }
    }
}
