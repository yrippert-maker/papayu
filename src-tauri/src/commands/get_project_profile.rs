//! v2.4.3: detect project profile by path (type, limits, goal_template).

use std::path::Path;
use std::time::Instant;

use tauri::{Emitter, Window};

use crate::types::{ProjectLimits, ProjectProfile, ProjectType};

fn has_file(root: &Path, rel: &str) -> bool {
    root.join(rel).is_file()
}
fn has_dir(root: &Path, rel: &str) -> bool {
    root.join(rel).is_dir()
}

pub fn detect_project_type(root: &Path) -> ProjectType {
    if has_file(root, "next.config.js")
        || has_file(root, "next.config.mjs")
        || has_file(root, "next.config.ts")
        || (has_dir(root, "app") || has_dir(root, "pages"))
    {
        return ProjectType::NextJs;
    }

    if has_file(root, "vite.config.ts")
        || has_file(root, "vite.config.js")
        || has_file(root, "vite.config.mjs")
    {
        return ProjectType::ReactVite;
    }

    if has_file(root, "package.json") {
        return ProjectType::Node;
    }

    if has_file(root, "Cargo.toml") {
        return ProjectType::Rust;
    }

    if has_file(root, "pyproject.toml")
        || has_file(root, "requirements.txt")
        || has_file(root, "setup.py")
    {
        return ProjectType::Python;
    }

    ProjectType::Unknown
}

fn build_goal_template(pt: &ProjectType) -> String {
    let tone = "Отвечай коротко и по-человечески, как коллега в чате.";
    match pt {
        ProjectType::ReactVite => format!("Цель: {{goal}}\nКонтекст: это React+Vite проект. Действуй безопасно: только preview → apply_tx → auto_check → undo при ошибке.\nОграничения: никаких shell; только safe FS; не трогать секреты.\n{}", tone),
        ProjectType::NextJs => format!("Цель: {{goal}}\nКонтекст: это Next.js проект. Действуй безопасно: только preview → apply_tx → auto_check → undo при ошибке.\nОграничения: никаких shell; только safe FS; не трогать секреты.\n{}", tone),
        ProjectType::Rust => format!("Цель: {{goal}}\nКонтекст: это Rust/Cargo проект. Действуй безопасно: только preview → apply_tx → auto_check → undo при ошибке.\nОграничения: никаких shell; только safe FS; не трогать секреты.\n{}", tone),
        ProjectType::Python => format!("Цель: {{goal}}\nКонтекст: это Python проект. Действуй безопасно: только preview → apply_tx → auto_check → undo при ошибке.\nОграничения: никаких shell; только safe FS; не трогать секреты.\n{}", tone),
        ProjectType::Node => format!("Цель: {{goal}}\nКонтекст: это Node проект. Действуй безопасно: только preview → apply_tx → auto_check → undo при ошибке.\nОграничения: никаких shell; только safe FS; не трогать секреты.\n{}", tone),
        ProjectType::Unknown => format!("Цель: {{goal}}\nКонтекст: тип проекта не определён. Действуй максимально безопасно: только preview → apply_tx → auto_check → undo при ошибке.\nОграничения: никаких shell; только safe FS; не трогать секреты.\n{}", tone),
    }
}

/// v2.4.4: get limits for a path (used by apply_actions_tx and run_batch for max_actions_per_tx and timeout).
pub fn get_project_limits(root: &Path) -> ProjectLimits {
    default_limits(&detect_project_type(root))
}

fn default_limits(pt: &ProjectType) -> ProjectLimits {
    match pt {
        ProjectType::ReactVite | ProjectType::NextJs | ProjectType::Node => ProjectLimits {
            max_files: 50_000,
            timeout_sec: 60,
            max_actions_per_tx: 25,
        },
        ProjectType::Rust => ProjectLimits {
            max_files: 50_000,
            timeout_sec: 60,
            max_actions_per_tx: 20,
        },
        ProjectType::Python => ProjectLimits {
            max_files: 50_000,
            timeout_sec: 60,
            max_actions_per_tx: 20,
        },
        ProjectType::Unknown => ProjectLimits {
            max_files: 30_000,
            timeout_sec: 45,
            max_actions_per_tx: 15,
        },
    }
}

#[tauri::command]
pub async fn get_project_profile(window: Window, path: String) -> Result<ProjectProfile, String> {
    let root = Path::new(&path);
    if !root.exists() {
        return Err("PATH_NOT_FOUND".to_string());
    }
    if !root.is_dir() {
        return Err("PATH_NOT_DIRECTORY".to_string());
    }

    let _ = window.emit("analyze_progress", "Определяю профиль проекта…");

    let start = Instant::now();
    let project_type = detect_project_type(root);
    let limits = default_limits(&project_type);

    let safe_mode = true;

    let max_attempts = match project_type {
        ProjectType::Unknown => 2,
        _ => 3,
    };

    let goal_template = build_goal_template(&project_type);

    let _elapsed = start.elapsed();

    Ok(ProjectProfile {
        path,
        project_type,
        safe_mode,
        max_attempts,
        goal_template,
        limits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_project_type_unknown_empty() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        assert_eq!(detect_project_type(root), ProjectType::Unknown);
    }

    #[test]
    fn test_detect_project_type_node() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("package.json"), "{}").unwrap();
        assert_eq!(detect_project_type(root), ProjectType::Node);
    }

    #[test]
    fn test_detect_project_type_rust() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        assert_eq!(detect_project_type(root), ProjectType::Rust);
    }

    #[test]
    fn test_detect_project_type_react_vite() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("vite.config.ts"), "export default {}").unwrap();
        assert_eq!(detect_project_type(root), ProjectType::ReactVite);
    }

    #[test]
    fn test_detect_project_type_python() {
        let dir = tempfile::TempDir::new().unwrap();
        let root = dir.path();
        fs::write(root.join("pyproject.toml"), "[project]\nname = \"x\"").unwrap();
        assert_eq!(detect_project_type(root), ProjectType::Python);
    }

    #[test]
    fn test_get_project_limits_unknown() {
        let dir = tempfile::TempDir::new().unwrap();
        let limits = get_project_limits(dir.path());
        assert_eq!(limits.max_actions_per_tx, 15);
        assert_eq!(limits.timeout_sec, 45);
    }

    #[test]
    fn test_get_project_limits_rust() {
        let dir = tempfile::TempDir::new().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"").unwrap();
        let limits = get_project_limits(dir.path());
        assert_eq!(limits.max_actions_per_tx, 20);
        assert_eq!(limits.timeout_sec, 60);
    }
}
