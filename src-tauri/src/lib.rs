mod agent_sync;
mod audit_log;
mod commands;
mod policy_engine;
mod secrets_guard;
mod snyk_sync;
mod context;
mod domain_notes;
mod memory;
mod net;
mod online_research;
mod patch;
mod protocol;
mod store;
mod tx;
mod types;
mod verify;

use commands::FolderLinks;
use commands::{
    add_project, agentic_run, analyze_project, analyze_weekly_reports, append_session_event,
    apply_actions, apply_actions_tx, apply_project_setting_cmd, chat_on_project, export_settings,
    fetch_narrative_for_report, fetch_trends_recommendations, generate_actions,
    generate_actions_from_report, get_project_profile, get_project_settings,
    get_trends_recommendations, get_undo_redo_state_cmd, import_settings, list_projects,
    list_sessions, load_folder_links, preview_actions, propose_actions, redo_last, run_batch,
    save_folder_links, save_report_to_file, set_project_settings, undo_available, undo_last,
    undo_last_tx, undo_status,
};
use tauri::Manager;
use types::{ApplyPayload, BatchPayload};

#[tauri::command]
async fn analyze_project_cmd(
    app: tauri::AppHandle,
    paths: Vec<String>,
    attached_files: Option<Vec<String>>,
) -> Result<types::AnalyzeReport, String> {
    let mut report = analyze_project(paths.clone(), attached_files)?;
    if commands::is_llm_configured() {
        if let Ok(narrative) = fetch_narrative_for_report(&report).await {
            report.narrative = narrative;
        }
    }
    let snyk_findings = if snyk_sync::is_snyk_sync_enabled() {
        snyk_sync::fetch_snyk_code_issues().await.ok()
    } else {
        None
    };
    agent_sync::write_agent_sync_if_enabled(&report, snyk_findings);
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = audit_log::log_event(
            &dir,
            "analyze",
            paths.first().map(String::as_str),
            Some("ok"),
            Some(&format!("findings={}", report.findings.len())),
        );
    }
    Ok(report)
}

#[tauri::command]
fn preview_actions_cmd(payload: ApplyPayload) -> Result<types::PreviewResult, String> {
    preview_actions(payload)
}

#[tauri::command]
fn apply_actions_cmd(app: tauri::AppHandle, payload: ApplyPayload) -> types::ApplyResult {
    let result = apply_actions(app.clone(), payload.clone());
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = audit_log::log_event(
            &dir,
            "apply",
            Some(&payload.root_path),
            if result.ok { Some("ok") } else { Some("fail") },
            result.error.as_deref(),
        );
    }
    result
}

#[tauri::command]
async fn run_batch_cmd(
    app: tauri::AppHandle,
    payload: BatchPayload,
) -> Result<Vec<types::BatchEvent>, String> {
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

/// Online research: поиск + fetch + LLM summarize. Optional project_path → cache in project .papa-yu/cache/.
#[tauri::command]
async fn research_answer_cmd(
    query: String,
    project_path: Option<String>,
) -> Result<online_research::OnlineAnswer, String> {
    let path_ref = project_path.as_deref().map(std::path::Path::new);
    online_research::research_answer(&query, path_ref).await
}

/// Domain notes: load for project.
#[tauri::command]
fn load_domain_notes_cmd(project_path: String) -> domain_notes::DomainNotes {
    domain_notes::load_domain_notes(std::path::Path::new(&project_path))
}

/// Domain notes: save (after UI edit).
#[tauri::command]
fn save_domain_notes_cmd(
    project_path: String,
    data: domain_notes::DomainNotes,
) -> Result<(), String> {
    domain_notes::save_domain_notes(std::path::Path::new(&project_path), data)
}

/// Domain notes: delete note by id.
#[tauri::command]
fn delete_domain_note_cmd(project_path: String, note_id: String) -> Result<bool, String> {
    domain_notes::delete_note(std::path::Path::new(&project_path), &note_id)
}

/// Domain notes: clear expired (non-pinned). Returns count removed.
#[tauri::command]
fn clear_expired_domain_notes_cmd(project_path: String) -> Result<usize, String> {
    domain_notes::clear_expired_notes(std::path::Path::new(&project_path))
}

/// Domain notes: set pinned.
#[tauri::command]
fn pin_domain_note_cmd(
    project_path: String,
    note_id: String,
    pinned: bool,
) -> Result<bool, String> {
    domain_notes::pin_note(std::path::Path::new(&project_path), &note_id, pinned)
}

/// Domain notes: distill OnlineAnswer into a short note and save.
#[tauri::command]
async fn distill_and_save_domain_note_cmd(
    project_path: String,
    query: String,
    answer_md: String,
    sources: Vec<domain_notes::NoteSource>,
    confidence: f64,
) -> Result<domain_notes::DomainNote, String> {
    let path = std::path::Path::new(&project_path);
    let sources_tuples: Vec<(String, String)> =
        sources.into_iter().map(|s| (s.url, s.title)).collect();
    domain_notes::distill_and_save_note(path, &query, &answer_md, &sources_tuples, confidence).await
}

/// Журнал аудита: последние события.
#[tauri::command]
fn audit_log_list_cmd(app: tauri::AppHandle, limit: Option<usize>) -> Result<Vec<audit_log::AuditEvent>, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(audit_log::read_events(&dir, limit.unwrap_or(100)))
}

/// Сканирование проекта на подозрительные секреты.
#[tauri::command]
fn scan_secrets_cmd(project_path: String) -> Vec<secrets_guard::SecretSuspicion> {
    secrets_guard::scan_secrets(std::path::Path::new(&project_path))
}

/// Список правил политик.
#[tauri::command]
fn get_policies_cmd() -> Vec<policy_engine::PolicyRule> {
    policy_engine::get_policies()
}

/// Проверка проекта по правилам.
#[tauri::command]
fn run_policy_check_cmd(project_path: String) -> Vec<policy_engine::PolicyCheckResult> {
    policy_engine::run_policy_check(std::path::Path::new(&project_path))
}

/// RAG: вопрос по коду проекта (контекст из файлов + LLM).
#[tauri::command]
async fn rag_query_cmd(project_path: String, question: String) -> Result<String, String> {
    chat_on_project(std::path::Path::new(&project_path), &question).await
}

/// Сохранить отчёт в docs/reports/weekly_YYYY-MM-DD.md.
#[tauri::command]
fn save_report_cmd(
    project_path: String,
    report_md: String,
    date: Option<String>,
) -> Result<String, String> {
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
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            analyze_project_cmd,
            audit_log_list_cmd,
            scan_secrets_cmd,
            get_policies_cmd,
            run_policy_check_cmd,
            rag_query_cmd,
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
            commands::design_trends::research_design_trends,
            export_settings,
            import_settings,
            analyze_weekly_reports_cmd,
            save_report_cmd,
            research_answer_cmd,
            load_domain_notes_cmd,
            save_domain_notes_cmd,
            delete_domain_note_cmd,
            clear_expired_domain_notes_cmd,
            pin_domain_note_cmd,
            distill_and_save_domain_note_cmd,
            apply_project_setting_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
