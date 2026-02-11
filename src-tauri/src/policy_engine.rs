//! Движок политик: проверка проекта по правилам (README, .gitignore, .env не в репо и т.д.).

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub check: String, // "file_exists" | "file_missing" | "no_env_in_repo"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyCheckResult {
    pub rule_id: String,
    pub passed: bool,
    pub message: String,
}

fn default_rules() -> Vec<PolicyRule> {
    vec![
        PolicyRule {
            id: "readme".to_string(),
            name: "README".to_string(),
            description: "В корне должен быть README.md или аналог".to_string(),
            check: "file_exists".to_string(),
        },
        PolicyRule {
            id: "gitignore".to_string(),
            name: ".gitignore".to_string(),
            description: "Должен быть .gitignore".to_string(),
            check: "file_exists".to_string(),
        },
        PolicyRule {
            id: "no_env".to_string(),
            name: ".env не в репо".to_string(),
            description: ".env не должен коммититься (должен быть в .gitignore)".to_string(),
            check: "no_env_in_repo".to_string(),
        },
        PolicyRule {
            id: "tests".to_string(),
            name: "Папка tests/".to_string(),
            description: "Рекомендуется иметь tests/ или __tests__".to_string(),
            check: "dir_exists".to_string(),
        },
    ]
}

/// Возвращает список правил по умолчанию.
pub fn get_policies() -> Vec<PolicyRule> {
    default_rules()
}

/// Запускает проверку проекта по правилам.
pub fn run_policy_check(project_path: &Path) -> Vec<PolicyCheckResult> {
    let rules = get_policies();
    let mut results = Vec::with_capacity(rules.len());
    for rule in rules {
        let (passed, message) = match rule.check.as_str() {
            "file_exists" => {
                let files = match rule.id.as_str() {
                    "readme" => ["README.md", "README.MD", "README.txt", "README"],
                    "gitignore" => [".gitignore", "", "", ""],
                    _ => continue,
                };
                let exists = files.iter().any(|f| !f.is_empty() && project_path.join(f).exists());
                (
                    exists,
                    if exists {
                        "OK".to_string()
                    } else {
                        format!("Отсутствует: {}", rule.name)
                    },
                )
            }
            "dir_exists" => {
                let exists = project_path.join("tests").is_dir() || project_path.join("__tests__").is_dir();
                (
                    exists,
                    if exists { "OK".to_string() } else { "Нет tests/ или __tests__".to_string() },
                )
            }
            "no_env_in_repo" => {
                let env_path = project_path.join(".env");
                let gitignore = project_path.join(".gitignore");
                let env_exists = env_path.is_file();
                let ignored = if gitignore.is_file() {
                    std::fs::read_to_string(&gitignore).map_or(false, |c| c.lines().any(|l| l.trim() == ".env"))
                } else {
                    false
                };
                let passed = !env_exists || ignored;
                (
                    passed,
                    if passed {
                        "OK".to_string()
                    } else {
                        ".env присутствует; добавьте .env в .gitignore".to_string()
                    },
                )
            }
            _ => (false, "Неизвестное правило".to_string()),
        };
        results.push(PolicyCheckResult {
            rule_id: rule.id.clone(),
            passed,
            message,
        });
    }
    results
}
