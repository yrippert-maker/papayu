//! v2.5: Projects & sessions store (JSON in app_data_dir).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::types::{Project, ProjectSettings, Session, SessionEvent};

const PROJECTS_FILE: &str = "projects.json";
const PROFILES_FILE: &str = "project_profiles.json";
const SESSIONS_FILE: &str = "sessions.json";

const MAX_SESSIONS_PER_PROJECT: usize = 50;
const MAX_EVENTS_PER_SESSION: usize = 200;

pub fn load_projects(app_data_dir: &Path) -> Vec<Project> {
    let p = app_data_dir.join(PROJECTS_FILE);
    if let Ok(s) = fs::read_to_string(&p) {
        if let Ok(v) = serde_json::from_str::<Vec<Project>>(&s) {
            return v;
        }
    }
    vec![]
}

pub fn save_projects(app_data_dir: &Path, projects: &[Project]) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let p = app_data_dir.join(PROJECTS_FILE);
    fs::write(
        &p,
        serde_json::to_string_pretty(projects).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn load_profiles(app_data_dir: &Path) -> HashMap<String, ProjectSettings> {
    let p = app_data_dir.join(PROFILES_FILE);
    if let Ok(s) = fs::read_to_string(&p) {
        if let Ok(m) = serde_json::from_str::<HashMap<String, ProjectSettings>>(&s) {
            return m;
        }
    }
    HashMap::new()
}

pub fn save_profiles(
    app_data_dir: &Path,
    profiles: &HashMap<String, ProjectSettings>,
) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let p = app_data_dir.join(PROFILES_FILE);
    fs::write(
        &p,
        serde_json::to_string_pretty(profiles).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn load_sessions(app_data_dir: &Path) -> Vec<Session> {
    let p = app_data_dir.join(SESSIONS_FILE);
    if let Ok(s) = fs::read_to_string(&p) {
        if let Ok(v) = serde_json::from_str::<Vec<Session>>(&s) {
            return v;
        }
    }
    vec![]
}

pub fn save_sessions(app_data_dir: &Path, sessions: &[Session]) -> Result<(), String> {
    fs::create_dir_all(app_data_dir).map_err(|e| e.to_string())?;
    let p = app_data_dir.join(SESSIONS_FILE);
    fs::write(
        &p,
        serde_json::to_string_pretty(sessions).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn add_session_event(
    app_data_dir: &Path,
    project_id: &str,
    event: SessionEvent,
) -> Result<Session, String> {
    let mut sessions = load_sessions(app_data_dir);
    let now = chrono::Utc::now().to_rfc3339();
    let idx = sessions
        .iter()
        .enumerate()
        .filter(|(_, s)| s.project_id == project_id)
        .max_by_key(|(_, s)| s.updated_at.as_str())
        .map(|(i, _)| i);

    if let Some(i) = idx {
        let s = &mut sessions[i];
        s.updated_at = now.clone();
        s.events.push(event);
        if s.events.len() > MAX_EVENTS_PER_SESSION {
            let n = s.events.len() - MAX_EVENTS_PER_SESSION;
            s.events.drain(..n);
        }
        save_sessions(app_data_dir, &sessions)?;
        return Ok(sessions[i].clone());
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let session = Session {
        id: session_id.clone(),
        project_id: project_id.to_string(),
        created_at: now.clone(),
        updated_at: now,
        events: vec![event],
    };
    sessions.push(session.clone());
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    if sessions.len() > MAX_SESSIONS_PER_PROJECT * 10 {
        sessions.truncate(MAX_SESSIONS_PER_PROJECT * 10);
    }
    save_sessions(app_data_dir, &sessions)?;
    Ok(session)
}
