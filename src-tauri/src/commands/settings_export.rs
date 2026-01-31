//! v2.4.4: Export/import settings (projects, profiles, sessions, folder_links).

use crate::commands::folder_links::{load_folder_links, save_folder_links, FolderLinks};
use crate::store::{load_profiles, load_projects, load_sessions, save_profiles, save_projects, save_sessions};
use crate::types::{Project, ProjectSettings, Session};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::Manager;

/// Bundle of all exportable settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsBundle {
    pub version: String,
    pub exported_at: String,
    pub projects: Vec<Project>,
    pub profiles: HashMap<String, ProjectSettings>,
    pub sessions: Vec<Session>,
    pub folder_links: FolderLinks,
}

fn app_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

/// Export all settings as JSON string
#[tauri::command]
pub fn export_settings(app: tauri::AppHandle) -> Result<String, String> {
    let dir = app_data_dir(&app)?;
    
    let bundle = SettingsBundle {
        version: "2.4.4".to_string(),
        exported_at: chrono::Utc::now().to_rfc3339(),
        projects: load_projects(&dir),
        profiles: load_profiles(&dir),
        sessions: load_sessions(&dir),
        folder_links: load_folder_links(&dir),
    };
    
    serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())
}

/// Import mode
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportMode {
    /// Replace all existing settings
    Replace,
    /// Merge with existing (don't overwrite existing items)
    Merge,
}

/// Import settings from JSON string
#[tauri::command]
pub fn import_settings(
    app: tauri::AppHandle,
    json: String,
    mode: Option<String>,
) -> Result<ImportResult, String> {
    let bundle: SettingsBundle = serde_json::from_str(&json)
        .map_err(|e| format!("Invalid settings JSON: {}", e))?;
    
    let mode = match mode.as_deref() {
        Some("replace") => ImportMode::Replace,
        _ => ImportMode::Merge,
    };
    
    let dir = app_data_dir(&app)?;
    
    let mut result = ImportResult {
        projects_imported: 0,
        profiles_imported: 0,
        sessions_imported: 0,
        folder_links_imported: 0,
    };
    
    match mode {
        ImportMode::Replace => {
            // Replace all
            save_projects(&dir, &bundle.projects)?;
            result.projects_imported = bundle.projects.len();
            
            save_profiles(&dir, &bundle.profiles)?;
            result.profiles_imported = bundle.profiles.len();
            
            save_sessions(&dir, &bundle.sessions)?;
            result.sessions_imported = bundle.sessions.len();
            
            save_folder_links(&dir, &bundle.folder_links)?;
            result.folder_links_imported = bundle.folder_links.paths.len();
        }
        ImportMode::Merge => {
            // Merge projects
            let mut existing_projects = load_projects(&dir);
            let existing_paths: std::collections::HashSet<_> = 
                existing_projects.iter().map(|p| p.path.clone()).collect();
            for p in bundle.projects {
                if !existing_paths.contains(&p.path) {
                    existing_projects.push(p);
                    result.projects_imported += 1;
                }
            }
            save_projects(&dir, &existing_projects)?;
            
            // Merge profiles
            let mut existing_profiles = load_profiles(&dir);
            for (k, v) in bundle.profiles {
                if !existing_profiles.contains_key(&k) {
                    existing_profiles.insert(k, v);
                    result.profiles_imported += 1;
                }
            }
            save_profiles(&dir, &existing_profiles)?;
            
            // Merge sessions
            let mut existing_sessions = load_sessions(&dir);
            let existing_ids: std::collections::HashSet<_> = 
                existing_sessions.iter().map(|s| s.id.clone()).collect();
            for s in bundle.sessions {
                if !existing_ids.contains(&s.id) {
                    existing_sessions.push(s);
                    result.sessions_imported += 1;
                }
            }
            save_sessions(&dir, &existing_sessions)?;
            
            // Merge folder links
            let mut existing_links = load_folder_links(&dir);
            let existing_set: std::collections::HashSet<_> = 
                existing_links.paths.iter().cloned().collect();
            for p in bundle.folder_links.paths {
                if !existing_set.contains(&p) {
                    existing_links.paths.push(p);
                    result.folder_links_imported += 1;
                }
            }
            save_folder_links(&dir, &existing_links)?;
        }
    }
    
    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub projects_imported: usize,
    pub profiles_imported: usize,
    pub sessions_imported: usize,
    pub folder_links_imported: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_bundle() -> SettingsBundle {
        SettingsBundle {
            version: "2.4.4".to_string(),
            exported_at: "2025-01-31T00:00:00Z".to_string(),
            projects: vec![Project {
                id: "test-id".to_string(),
                path: "/test/path".to_string(),
                name: "Test Project".to_string(),
                created_at: "2025-01-31T00:00:00Z".to_string(),
            }],
            profiles: HashMap::from([(
                "test-id".to_string(),
                ProjectSettings {
                    project_id: "test-id".to_string(),
                    auto_check: true,
                    max_attempts: 3,
                    max_actions: 10,
                    goal_template: Some("Test goal".to_string()),
                },
            )]),
            sessions: vec![],
            folder_links: FolderLinks {
                paths: vec!["/test/folder".to_string()],
            },
        }
    }

    #[test]
    fn test_settings_bundle_serialization() {
        let bundle = create_test_bundle();
        let json = serde_json::to_string(&bundle).unwrap();
        
        assert!(json.contains("\"version\":\"2.4.4\""));
        assert!(json.contains("\"Test Project\""));
        assert!(json.contains("\"/test/folder\""));
        
        let parsed: SettingsBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.version, "2.4.4");
        assert_eq!(parsed.projects.len(), 1);
        assert_eq!(parsed.projects[0].name, "Test Project");
    }

    #[test]
    fn test_settings_bundle_deserialization() {
        let json = r#"{
            "version": "2.4.4",
            "exported_at": "2025-01-31T00:00:00Z",
            "projects": [],
            "profiles": {},
            "sessions": [],
            "folder_links": { "paths": [] }
        }"#;
        
        let bundle: SettingsBundle = serde_json::from_str(json).unwrap();
        assert_eq!(bundle.version, "2.4.4");
        assert!(bundle.projects.is_empty());
    }

    #[test]
    fn test_import_result_default() {
        let result = ImportResult {
            projects_imported: 0,
            profiles_imported: 0,
            sessions_imported: 0,
            folder_links_imported: 0,
        };
        assert_eq!(result.projects_imported, 0);
    }
}
