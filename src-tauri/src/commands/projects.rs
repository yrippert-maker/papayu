//! v2.5: Projects & sessions — list/add projects, profiles, session history.

use crate::store::{
    add_session_event as store_add_session_event, load_profiles, load_projects, load_sessions,
    save_profiles, save_projects,
};
use crate::types::{Project, ProjectSettings, Session, SessionEvent};
use tauri::Manager;

fn app_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_projects(app: tauri::AppHandle) -> Result<Vec<Project>, String> {
    let dir = app_data_dir(&app)?;
    Ok(load_projects(&dir))
}

#[tauri::command]
pub fn add_project(app: tauri::AppHandle, path: String, name: Option<String>) -> Result<Project, String> {
    let dir = app_data_dir(&app)?;
    let mut projects = load_projects(&dir);
    let name = name.unwrap_or_else(|| {
        std::path::Path::new(&path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Project")
            .to_string()
    });
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    let project = Project {
        id: id.clone(),
        path: path.clone(),
        name,
        created_at: created_at.clone(),
    };
    if projects.iter().any(|p| p.path == path) {
        return Err("Project with this path already exists".to_string());
    }
    projects.push(project.clone());
    save_projects(&dir, &projects)?;
    Ok(project)
}

#[tauri::command]
pub fn get_project_settings(app: tauri::AppHandle, project_id: String) -> Result<ProjectSettings, String> {
    let dir = app_data_dir(&app)?;
    let profiles = load_profiles(&dir);
    Ok(profiles
        .get(&project_id)
        .cloned()
        .unwrap_or_else(|| ProjectSettings {
            project_id: project_id.clone(),
            auto_check: true,
            max_attempts: 2,
            max_actions: 12,
            goal_template: None,
        }))
}

#[tauri::command]
pub fn set_project_settings(app: tauri::AppHandle, profile: ProjectSettings) -> Result<(), String> {
    let dir = app_data_dir(&app)?;
    let mut profiles = load_profiles(&dir);
    profiles.insert(profile.project_id.clone(), profile);
    save_profiles(&dir, &profiles)?;
    Ok(())
}

#[tauri::command]
pub fn list_sessions(app: tauri::AppHandle, project_id: Option<String>) -> Result<Vec<Session>, String> {
    let dir = app_data_dir(&app)?;
    let mut sessions = load_sessions(&dir);
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    if let Some(pid) = project_id {
        sessions.retain(|s| s.project_id == pid);
    }
    Ok(sessions)
}

#[tauri::command]
pub fn append_session_event(
    app: tauri::AppHandle,
    project_id: String,
    kind: String,
    role: Option<String>,
    text: Option<String>,
) -> Result<Session, String> {
    let dir = app_data_dir(&app)?;
    let at = chrono::Utc::now().to_rfc3339();
    let event = SessionEvent {
        kind,
        role,
        text,
        at,
    };
    store_add_session_event(&dir, &project_id, event)
}
