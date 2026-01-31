//! Инженерная память: user prefs + project prefs, загрузка/сохранение, MEMORY BLOCK для промпта, whitelist для memory_patch.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub const SCHEMA_VERSION: u32 = 1;

/// User preferences (оператор).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserPrefs {
    #[serde(default)]
    pub preferred_style: String, // "brief" | "normal" | "verbose"
    #[serde(default)]
    pub ask_budget: u8, // 0..2
    #[serde(default)]
    pub risk_tolerance: String, // "low" | "medium" | "high"
    #[serde(default)]
    pub default_language: String, // "python" | "node" | "go" etc.
    #[serde(default)]
    pub output_format: String, // "patch_first" | "plan_first"
}

/// Project preferences (для конкретного репо).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectPrefs {
    #[serde(default)]
    pub default_test_command: String,
    #[serde(default)]
    pub default_lint_command: String,
    #[serde(default)]
    pub default_format_command: String,
    #[serde(default)]
    pub package_manager: String,
    #[serde(default)]
    pub build_command: String,
    #[serde(default)]
    pub src_roots: Vec<String>,
    #[serde(default)]
    pub test_roots: Vec<String>,
    #[serde(default)]
    pub ci_notes: String,
}

/// Корневой файл пользовательских настроек (~/.papa-yu или app_data/papa-yu).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferencesFile {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub user: UserPrefs,
}

/// Файл настроек проекта (.papa-yu/project.json).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPrefsFile {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub project: ProjectPrefs,
}

/// Объединённый вид памяти для промпта (только непустые поля).
#[derive(Debug, Clone, Default)]
pub struct EngineeringMemory {
    pub user: UserPrefs,
    pub project: ProjectPrefs,
}

impl UserPrefs {
    pub(crate) fn is_default(&self) -> bool {
        self.preferred_style.is_empty()
            && self.ask_budget == 0
            && self.risk_tolerance.is_empty()
            && self.default_language.is_empty()
            && self.output_format.is_empty()
    }
}

impl ProjectPrefs {
    pub(crate) fn is_default(&self) -> bool {
        self.default_test_command.is_empty()
            && self.default_lint_command.is_empty()
            && self.default_format_command.is_empty()
            && self.package_manager.is_empty()
            && self.build_command.is_empty()
            && self.src_roots.is_empty()
            && self.test_roots.is_empty()
            && self.ci_notes.is_empty()
    }
}

/// Разрешённые ключи для memory_patch (dot-notation: user.*, project.*).
const MEMORY_PATCH_WHITELIST: &[&str] = &[
    "user.preferred_style",
    "user.ask_budget",
    "user.risk_tolerance",
    "user.default_language",
    "user.output_format",
    "project.default_test_command",
    "project.default_lint_command",
    "project.default_format_command",
    "project.package_manager",
    "project.build_command",
    "project.src_roots",
    "project.test_roots",
    "project.ci_notes",
];

fn is_whitelisted(key: &str) -> bool {
    MEMORY_PATCH_WHITELIST.contains(&key)
}

/// Загружает user prefs из файла (создаёт дефолт, если файла нет).
pub fn load_user_prefs(path: &Path) -> UserPrefs {
    let s = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return UserPrefs::default(),
    };
    let file: PreferencesFile = match serde_json::from_str(&s) {
        Ok(f) => f,
        Err(_) => return UserPrefs::default(),
    };
    file.user
}

/// Загружает project prefs из .papa-yu/project.json (дефолт, если нет файла).
pub fn load_project_prefs(path: &Path) -> ProjectPrefs {
    let s = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return ProjectPrefs::default(),
    };
    let file: ProjectPrefsFile = match serde_json::from_str(&s) {
        Ok(f) => f,
        Err(_) => return ProjectPrefs::default(),
    };
    file.project
}

/// Собирает объединённую память: user из user_prefs_path, project из project_prefs_path.
pub fn load_memory(user_prefs_path: &Path, project_prefs_path: &Path) -> EngineeringMemory {
    let user = load_user_prefs(user_prefs_path);
    let project = load_project_prefs(project_prefs_path);
    EngineeringMemory { user, project }
}

/// Формирует текст MEMORY BLOCK для вставки в system prompt (~1–2 KB).
pub fn build_memory_block(mem: &EngineeringMemory) -> String {
    if mem.user.is_default() && mem.project.is_default() {
        return String::new();
    }
    let mut obj = serde_json::Map::new();
    if !mem.user.is_default() {
        let mut user = serde_json::Map::new();
        if !mem.user.preferred_style.is_empty() {
            user.insert("preferred_style".into(), serde_json::Value::String(mem.user.preferred_style.clone()));
        }
        if mem.user.ask_budget > 0 {
            user.insert("ask_budget".into(), serde_json::Value::Number(serde_json::Number::from(mem.user.ask_budget)));
        }
        if !mem.user.risk_tolerance.is_empty() {
            user.insert("risk_tolerance".into(), serde_json::Value::String(mem.user.risk_tolerance.clone()));
        }
        if !mem.user.default_language.is_empty() {
            user.insert("default_language".into(), serde_json::Value::String(mem.user.default_language.clone()));
        }
        if !mem.user.output_format.is_empty() {
            user.insert("output_format".into(), serde_json::Value::String(mem.user.output_format.clone()));
        }
        obj.insert("user".into(), serde_json::Value::Object(user));
    }
    if !mem.project.is_default() {
        let mut project = serde_json::Map::new();
        if !mem.project.default_test_command.is_empty() {
            project.insert("default_test_command".into(), serde_json::Value::String(mem.project.default_test_command.clone()));
        }
        if !mem.project.default_lint_command.is_empty() {
            project.insert("default_lint_command".into(), serde_json::Value::String(mem.project.default_lint_command.clone()));
        }
        if !mem.project.default_format_command.is_empty() {
            project.insert("default_format_command".into(), serde_json::Value::String(mem.project.default_format_command.clone()));
        }
        if !mem.project.package_manager.is_empty() {
            project.insert("package_manager".into(), serde_json::Value::String(mem.project.package_manager.clone()));
        }
        if !mem.project.build_command.is_empty() {
            project.insert("build_command".into(), serde_json::Value::String(mem.project.build_command.clone()));
        }
        if !mem.project.src_roots.is_empty() {
            project.insert("src_roots".into(), serde_json::to_value(&mem.project.src_roots).unwrap_or(serde_json::Value::Array(vec![])));
        }
        if !mem.project.test_roots.is_empty() {
            project.insert("test_roots".into(), serde_json::to_value(&mem.project.test_roots).unwrap_or(serde_json::Value::Array(vec![])));
        }
        if !mem.project.ci_notes.is_empty() {
            project.insert("ci_notes".into(), serde_json::Value::String(mem.project.ci_notes.clone()));
        }
        obj.insert("project".into(), serde_json::Value::Object(project));
    }
    if obj.is_empty() {
        return String::new();
    }
    let json_str = serde_json::to_string(&serde_json::Value::Object(obj)).unwrap_or_default();
    format!(
        "\n\nENGINEERING_MEMORY (trusted by user; update only when user requests):\n{}\n\nUse ENGINEERING_MEMORY as defaults. If user explicitly asks to change — suggest updating memory and show new JSON.",
        json_str
    )
}

/// Применяет memory_patch (ключи через точку, только whitelist). Возвращает обновлённые user + project.
pub fn apply_memory_patch(
    patch: &HashMap<String, serde_json::Value>,
    current_user: &UserPrefs,
    current_project: &ProjectPrefs,
) -> (UserPrefs, ProjectPrefs) {
    let mut user = current_user.clone();
    let mut project = current_project.clone();
    for (key, value) in patch {
        if !is_whitelisted(key) {
            continue;
        }
        if key.starts_with("user.") {
            let field = &key[5..];
            match field {
                "preferred_style" => if let Some(s) = value.as_str() { user.preferred_style = s.to_string(); },
                "ask_budget" => if let Some(n) = value.as_u64() { user.ask_budget = n as u8; },
                "risk_tolerance" => if let Some(s) = value.as_str() { user.risk_tolerance = s.to_string(); },
                "default_language" => if let Some(s) = value.as_str() { user.default_language = s.to_string(); },
                "output_format" => if let Some(s) = value.as_str() { user.output_format = s.to_string(); },
                _ => {}
            }
        } else if key.starts_with("project.") {
            let field = &key[8..];
            match field {
                "default_test_command" => if let Some(s) = value.as_str() { project.default_test_command = s.to_string(); },
                "default_lint_command" => if let Some(s) = value.as_str() { project.default_lint_command = s.to_string(); },
                "default_format_command" => if let Some(s) = value.as_str() { project.default_format_command = s.to_string(); },
                "package_manager" => if let Some(s) = value.as_str() { project.package_manager = s.to_string(); },
                "build_command" => if let Some(s) = value.as_str() { project.build_command = s.to_string(); },
                "src_roots" => if let Some(arr) = value.as_array() {
                    project.src_roots = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                },
                "test_roots" => if let Some(arr) = value.as_array() {
                    project.test_roots = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                },
                "ci_notes" => if let Some(s) = value.as_str() { project.ci_notes = s.to_string(); },
                _ => {}
            }
        }
    }
    (user, project)
}

/// Сохраняет user prefs в файл. Создаёт родительскую папку при необходимости.
pub fn save_user_prefs(path: &Path, user: &UserPrefs) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = PreferencesFile {
        schema_version: SCHEMA_VERSION,
        user: user.clone(),
    };
    let s = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    fs::write(path, s).map_err(|e| e.to_string())
}

/// Сохраняет project prefs в .papa-yu/project.json.
pub fn save_project_prefs(path: &Path, project: &ProjectPrefs) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let file = ProjectPrefsFile {
        schema_version: SCHEMA_VERSION,
        project: project.clone(),
    };
    let s = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    fs::write(path, s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_accepts_user_and_project() {
        assert!(is_whitelisted("user.preferred_style"));
        assert!(is_whitelisted("project.default_test_command"));
        assert!(!is_whitelisted("session.foo"));
    }

    #[test]
    fn apply_patch_updates_user_and_project() {
        let mut patch = HashMap::new();
        patch.insert("user.preferred_style".into(), serde_json::Value::String("brief".into()));
        patch.insert("project.default_test_command".into(), serde_json::Value::String("pytest -q".into()));
        let (user, project) = apply_memory_patch(&patch, &UserPrefs::default(), &ProjectPrefs::default());
        assert_eq!(user.preferred_style, "brief");
        assert_eq!(project.default_test_command, "pytest -q");
    }
}
