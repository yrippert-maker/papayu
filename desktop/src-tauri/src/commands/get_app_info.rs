use serde::Serialize;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub version: String,
    pub app_data_dir: Option<String>,
    pub app_config_dir: Option<String>,
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> AppInfo {
    let version = app.package_info().version.to_string();
    let app_data_dir = app.path().app_data_dir().ok().map(|p| p.to_string_lossy().into_owned());
    let app_config_dir = app.path().app_config_dir().ok().map(|p| p.to_string_lossy().into_owned());
    AppInfo {
        version,
        app_data_dir,
        app_config_dir,
    }
}
