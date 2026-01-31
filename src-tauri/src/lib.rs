mod commands;
mod context;
mod online_research;
mod memory;
mod patch;
mod protocol;
mod store;
mod tx;
mod types;
mod verify;

use commands::{add_project, agentic_run, analyze_project, analyze_weekly_reports, append_session_event, apply_actions, apply_actions_tx, export_settings, fetch_trends_recommendations, generate_actions, generate_actions_from_report, get_project_profile, get_project_settings, get_trends_recommendations, get_undo_redo_state_cmd, import_settings, list_projects, list_sessions, load_folder_links, preview_actions, propose_actions, redo_last, run_batch, save_folder_links, save_report_to_file, set_project_settings, undo_available, undo_last, undo_last_tx, undo_status};
use tauri::Manager;
use commands::FolderLinks;
use types::{ApplyPayload, BatchPayload};

#[tauri::command]
fn analyze_project_cmd(paths: Vec<String>, attached_files: Option<Vec<String>>) -> Result<types::AnalyzeReport, String> {
    analyze_project(paths, attached_files)
}

#[tauri::command]
fn preview_actions_cmd(payload: ApplyPayload) -> Result<types::PreviewResult, String> {
    preview_actions(payload)
}

#[tauri::command]
fn apply_actions_cmd(app: tauri::AppHandle, payload: ApplyPayload) -> types::ApplyResult {
    apply_actions(app, payload)
}

#[tauri::command]
async fn run_batch_cmd(app: tauri::AppHandle, payload: BatchPayload) -> Result<Vec<types::BatchEvent>, String> {
    run_batch(app, payload).await
}

#[tauri::command]
fn get_folder_links(app: tauri::AppHandle) -> Result<FolderLinks, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(load_folder_links(&dir))
}

#[tauri::command]
fn set_folder_links(app: tauri::AppHandle, links: FolderLinks) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    save_folder_links(&dir, &links)
}

/// Проверка целостности проекта (типы, сборка, тесты). Вызывается автоматически после применений или вручную.
#[tauri::command]
fn verify_project(path: String) -> types::VerifyResult {
    verify::verify_project(&path)
}

/// Анализ еженедельных отчётов: агрегация трасс и генерация отчёта через LLM.
#[tauri::command]
async fn analyze_weekly_reports_cmd(
    project_path: String,
    from: Option<String>,
    to: Option<String>,
) -> commands::WeeklyReportResult {
    analyze_weekly_reports(std::path::Path::new(&project_path), from, to).await
}

/// Online research: поиск + fetch + LLM summarize.
#[tauri::command]
async fn research_answer_cmd(query: String) -> Result<online_research::OnlineAnswer, String> {
    online_research::research_answer(&query).await
}

/// Сохранить отчёт в docs/reports/weekly_YYYY-MM-DD.md.
#[tauri::command]
fn save_report_cmd(project_path: String, report_md: String, date: Option<String>) -> Result<String, String> {
    save_report_to_file(
        std::path::Path::new(&project_path),
        &report_md,
        date.as_deref(),
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            analyze_project_cmd,
            preview_actions_cmd,
            apply_actions_cmd,
            undo_last,
            undo_available,
            redo_last,
            get_undo_redo_state_cmd,
            generate_actions,
            run_batch_cmd,
            get_folder_links,
            set_folder_links,
            apply_actions_tx,
            verify_project,
            undo_last_tx,
            undo_status,
            propose_actions,
            generate_actions_from_report,
            agentic_run,
            list_projects,
            add_project,
            get_project_profile,
            get_project_settings,
            set_project_settings,
            list_sessions,
            append_session_event,
            get_trends_recommendations,
            fetch_trends_recommendations,
            export_settings,
            import_settings,
            analyze_weekly_reports_cmd,
            save_report_cmd,
            research_answer_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
