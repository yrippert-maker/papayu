//! v3.1: транзакция — snapshot + apply + autocheck + autorollback (history/tx, history/snapshots)

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::commands::get_project_profile::get_project_limits;
use crate::tx::{normalize_content_for_write, safe_join, sort_actions_for_apply};
use crate::types::{Action, ActionKind, ApplyOptions, ApplyTxResult, CheckStageResult};

const PROGRESS_EVENT: &str = "analyze_progress";

fn clip(s: String, n: usize) -> String {
    if s.len() <= n {
        s
    } else {
        format!("{}…", &s[..n])
    }
}

fn emit_progress(app: &AppHandle, msg: &str) {
    let _ = app.emit(PROGRESS_EVENT, msg);
}

fn write_tx_record(
    app: &AppHandle,
    tx_id: &str,
    record: &serde_json::Value,
) -> Result<(), String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let tx_dir = dir.join("history").join("tx");
    fs::create_dir_all(&tx_dir).map_err(|e| e.to_string())?;
    let p = tx_dir.join(format!("{tx_id}.json"));
    let bytes =
        serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
    fs::write(&p, bytes).map_err(|e| e.to_string())
}

fn copy_dir_recursive(
    src: &Path,
    dst: &Path,
    exclude: &[&str],
) -> Result<(), String> {
    if exclude
        .iter()
        .any(|x| src.file_name().map(|n| n == *x).unwrap_or(false))
    {
        return Ok(());
    }
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        let name = entry.file_name();
        let dstp = dst.join(name);
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        if ft.is_dir() {
            copy_dir_recursive(&p, &dstp, exclude)?;
        } else if ft.is_file() {
            fs::copy(&p, &dstp).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn snapshot_project(
    app: &AppHandle,
    project_root: &Path,
    tx_id: &str,
) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let snap_dir = dir.join("history").join("snapshots").join(tx_id);
    if snap_dir.exists() {
        fs::remove_dir_all(&snap_dir).map_err(|e| e.to_string())?;
    }
    fs::create_dir_all(&snap_dir).map_err(|e| e.to_string())?;

    let exclude = [
        ".git", "node_modules", "dist", "build", ".next", "target", ".cache", "coverage",
    ];
    copy_dir_recursive(project_root, &snap_dir, &exclude)?;
    Ok(snap_dir)
}

fn restore_snapshot(project_root: &Path, snap_dir: &Path) -> Result<(), String> {
    let exclude = [
        ".git", "node_modules", "dist", "build", ".next", "target", ".cache", "coverage",
    ];

    for entry in fs::read_dir(project_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let p = entry.path();
        let name = entry.file_name();
        if exclude
            .iter()
            .any(|x| name.to_string_lossy().as_ref() == *x)
        {
            continue;
        }
        if p.is_dir() {
            fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
        } else {
            fs::remove_file(&p).map_err(|e| e.to_string())?;
        }
    }

    copy_dir_recursive(snap_dir, project_root, &[])?;
    Ok(())
}

fn apply_one_action(root: &Path, action: &Action) -> Result<(), String> {
    let p = safe_join(root, &action.path)?;
    match action.kind {
        ActionKind::CreateFile | ActionKind::UpdateFile => {
            let content = action.content.as_deref().unwrap_or("");
            let normalized = normalize_content_for_write(content, &p);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::write(&p, normalized.as_bytes()).map_err(|e| e.to_string())?;
            Ok(())
        }
        ActionKind::DeleteFile => {
            if p.exists() {
                fs::remove_file(&p).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        ActionKind::CreateDir => {
            fs::create_dir_all(&p).map_err(|e| e.to_string())
        }
        ActionKind::DeleteDir => {
            if p.exists() {
                fs::remove_dir_all(&p).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }
}

fn run_cmd_allowlisted(
    cwd: &Path,
    exe: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<String, String> {
    let start = Instant::now();
    let mut cmd = std::process::Command::new(exe);
    cmd.current_dir(cwd);
    cmd.args(args);
    cmd.env("CI", "1");
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    loop {
        if start.elapsed() > timeout {
            let _ = child.kill();
            return Err("TIMEOUT".into());
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(_status) => {
                let out = child.wait_with_output().map_err(|e| e.to_string())?;
                let mut text = String::new();
                text.push_str(&String::from_utf8_lossy(&out.stdout));
                text.push_str(&String::from_utf8_lossy(&out.stderr));
                let text = clip(text, 20_000);
                if out.status.success() {
                    return Ok(text);
                }
                return Err(text);
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn auto_check(project_root: &Path, timeout_sec: u32) -> Vec<CheckStageResult> {
    let mut res: Vec<CheckStageResult> = vec![];
    let timeout = Duration::from_secs(timeout_sec as u64);

    let cargo = project_root.join("Cargo.toml").exists();
    let pkg = project_root.join("package.json").exists();

    if cargo {
        match run_cmd_allowlisted(project_root, "cargo", &["check"], timeout) {
            Ok(out) => res.push(CheckStageResult {
                stage: "verify".into(),
                ok: true,
                output: out,
            }),
            Err(out) => res.push(CheckStageResult {
                stage: "verify".into(),
                ok: false,
                output: out,
            }),
        }
    } else if pkg {
        match run_cmd_allowlisted(project_root, "npm", &["run", "-s", "typecheck"], timeout) {
            Ok(out) => res.push(CheckStageResult {
                stage: "verify".into(),
                ok: true,
                output: out,
            }),
            Err(out) => res.push(CheckStageResult {
                stage: "verify".into(),
                ok: false,
                output: out,
            }),
        }
    }

    if pkg {
        let build_timeout = Duration::from_secs((timeout_sec as u64).max(120));
        match run_cmd_allowlisted(project_root, "npm", &["run", "-s", "build"], build_timeout) {
            Ok(out) => res.push(CheckStageResult {
                stage: "build".into(),
                ok: true,
                output: out,
            }),
            Err(out) => res.push(CheckStageResult {
                stage: "build".into(),
                ok: false,
                output: out,
            }),
        }
    } else if cargo {
        let build_timeout = Duration::from_secs((timeout_sec as u64).max(120));
        match run_cmd_allowlisted(project_root, "cargo", &["build"], build_timeout) {
            Ok(out) => res.push(CheckStageResult {
                stage: "build".into(),
                ok: true,
                output: out,
            }),
            Err(out) => res.push(CheckStageResult {
                stage: "build".into(),
                ok: false,
                output: out,
            }),
        }
    }

    if pkg {
        match run_cmd_allowlisted(project_root, "npm", &["test"], timeout) {
            Ok(out) => res.push(CheckStageResult {
                stage: "smoke".into(),
                ok: true,
                output: out,
            }),
            Err(out) => res.push(CheckStageResult {
                stage: "smoke".into(),
                ok: false,
                output: out,
            }),
        }
    } else if cargo {
        match run_cmd_allowlisted(project_root, "cargo", &["test"], timeout) {
            Ok(out) => res.push(CheckStageResult {
                stage: "smoke".into(),
                ok: true,
                output: out,
            }),
            Err(out) => res.push(CheckStageResult {
                stage: "smoke".into(),
                ok: false,
                output: out,
            }),
        }
    }

    res
}

#[tauri::command]
pub async fn apply_actions_tx(
    app: AppHandle,
    path: String,
    actions: Vec<Action>,
    options: ApplyOptions,
) -> ApplyTxResult {
    let root = PathBuf::from(&path);
    if !root.exists() || !root.is_dir() {
        return ApplyTxResult {
            ok: false,
            tx_id: None,
            applied: false,
            rolled_back: false,
            checks: vec![],
            error: Some("path not found".into()),
            error_code: Some("PATH_NOT_FOUND".into()),
        };
    }

    if !options.user_confirmed {
        return ApplyTxResult {
            ok: false,
            tx_id: None,
            applied: false,
            rolled_back: false,
            checks: vec![],
            error: Some("confirmation required".into()),
            error_code: Some("CONFIRM_REQUIRED".into()),
        };
    }

    let limits = get_project_limits(&root);
    if actions.len() > limits.max_actions_per_tx as usize {
        return ApplyTxResult {
            ok: false,
            tx_id: None,
            applied: false,
            rolled_back: false,
            checks: vec![],
            error: Some(format!(
                "too many actions: {} > {}",
                actions.len(),
                limits.max_actions_per_tx
            )),
            error_code: Some("TOO_MANY_ACTIONS".into()),
        };
    }

    for a in &actions {
        let rel = a.path.as_str();
        if is_protected_file(rel) || !is_text_allowed(rel) {
            return ApplyTxResult {
                ok: false,
                tx_id: None,
                applied: false,
                rolled_back: false,
                checks: vec![],
                error: Some(format!("protected or non-text file: {}", rel)),
                error_code: Some("PROTECTED_PATH".into()),
            };
        }
    }

    let tx_id = Uuid::new_v4().to_string();

    emit_progress(&app, "Сохраняю точку отката…");
    let snap_dir = match snapshot_project(&app, &root, &tx_id) {
        Ok(p) => p,
        Err(e) => {
            return ApplyTxResult {
                ok: false,
                tx_id: Some(tx_id),
                applied: false,
                rolled_back: false,
                checks: vec![],
                error: Some(e),
                error_code: Some("SNAPSHOT_FAILED".into()),
            };
        }
    };

    emit_progress(&app, "Применяю изменения…");
    let mut actions = actions;
    sort_actions_for_apply(&mut actions);
    for a in &actions {
        if let Err(e) = apply_one_action(&root, a) {
            let _ = restore_snapshot(&root, &snap_dir);
            eprintln!("[APPLY_ROLLBACK] tx_id={} path={} reason={}", tx_id, path, e);
            return ApplyTxResult {
                ok: false,
                tx_id: Some(tx_id.clone()),
                applied: false,
                rolled_back: true,
                checks: vec![],
                error: Some(e),
                error_code: Some("APPLY_FAILED_ROLLED_BACK".into()),
            };
        }
    }

    let mut checks: Vec<CheckStageResult> = vec![];
    if options.auto_check {
        emit_progress(&app, "Проверяю типы…");
        checks = auto_check(&root, limits.timeout_sec);

        let any_fail = checks.iter().any(|c| !c.ok);
        if any_fail {
            emit_progress(&app, "Обнаружены ошибки. Откатываю изменения…");
            let _ = restore_snapshot(&root, &snap_dir);
            eprintln!("[APPLY_ROLLBACK] tx_id={} path={} reason=autoCheck_failed", tx_id, path);

            let record = json!({
                "txId": tx_id,
                "path": path,
                "rolledBack": true,
                "checks": checks,
            });
            let _ = write_tx_record(&app, &tx_id, &record);

            return ApplyTxResult {
                ok: false,
                tx_id: Some(tx_id),
                applied: true,
                rolled_back: true,
                checks,
                error: Some("autoCheck failed — rolled back".into()),
                error_code: Some("AUTO_CHECK_FAILED_ROLLED_BACK".into()),
            };
        }
    }

    let record = json!({
        "txId": tx_id,
        "path": path,
        "rolledBack": false,
        "checks": checks,
    });
    let _ = write_tx_record(&app, &tx_id, &record);

    eprintln!("[APPLY_SUCCESS] tx_id={} path={} actions={}", tx_id, path, actions.len());

    ApplyTxResult {
        ok: true,
        tx_id: Some(tx_id),
        applied: true,
        rolled_back: false,
        checks,
        error: None,
        error_code: None,
    }
}

fn is_protected_file(p: &str) -> bool {
    let lower = p.to_lowercase().replace('\\', "/");
    // Секреты и ключи (denylist)
    if lower == ".env" || lower.ends_with("/.env") { return true; }
    if lower.ends_with(".pem") || lower.ends_with(".key") || lower.ends_with(".p12") { return true; }
    if lower.contains("id_rsa") { return true; }
    if lower.contains("/secrets/") || lower.starts_with("secrets/") { return true; }
    // Lock-файлы
    if lower.ends_with("cargo.lock") { return true; }
    if lower.ends_with("package-lock.json") { return true; }
    if lower.ends_with("pnpm-lock.yaml") { return true; }
    if lower.ends_with("yarn.lock") { return true; }
    if lower.ends_with("composer.lock") { return true; }
    if lower.ends_with("poetry.lock") { return true; }
    if lower.ends_with("pipfile.lock") { return true; }
    let bin_ext = [
        ".png", ".jpg", ".jpeg", ".gif", ".webp", ".svg",
        ".pdf", ".zip", ".7z", ".rar", ".dmg", ".pkg",
        ".exe", ".dll", ".so", ".dylib", ".bin",
        ".mp3", ".mp4", ".mov", ".avi",
        ".wasm", ".class",
    ];
    for ext in bin_ext {
        if lower.ends_with(ext) { return true; }
    }
    false
}

fn is_text_allowed(p: &str) -> bool {
    let lower = p.to_lowercase();
    let ok_ext = [
        ".ts", ".tsx", ".js", ".jsx", ".json", ".md", ".txt", ".toml", ".yaml", ".yml",
        ".rs", ".py", ".go", ".java", ".kt", ".c", ".cpp", ".h", ".hpp",
        ".css", ".scss", ".html", ".env", ".gitignore", ".editorconfig",
    ];
    ok_ext.iter().any(|e| lower.ends_with(e)) || !lower.contains('.')
}

#[cfg(test)]
mod tests {
    use super::{is_protected_file, is_text_allowed};

    #[test]
    fn test_is_protected_file_secrets() {
        assert!(is_protected_file(".env"));
        assert!(is_protected_file("config/.env"));
        assert!(is_protected_file("key.pem"));
        assert!(is_protected_file("id_rsa"));
        assert!(is_protected_file(".ssh/id_rsa"));
        assert!(is_protected_file("secrets/secret.json"));
    }

    #[test]
    fn test_is_protected_file_lock_and_binary() {
        assert!(is_protected_file("Cargo.lock"));
        assert!(is_protected_file("package-lock.json"));
        assert!(is_protected_file("node_modules/foo/package-lock.json"));
        assert!(is_protected_file("image.PNG"));
        assert!(is_protected_file("file.pdf"));
        assert!(is_protected_file("lib.so"));
    }

    #[test]
    fn test_is_protected_file_allows_source() {
        assert!(!is_protected_file("src/main.rs"));
        assert!(!is_protected_file("src/App.tsx"));
        assert!(!is_protected_file("package.json"));
    }

    #[test]
    fn test_is_text_allowed_extensions() {
        assert!(is_text_allowed("src/main.rs"));
        assert!(is_text_allowed("App.tsx"));
        assert!(is_text_allowed("config.json"));
        assert!(is_text_allowed("README.md"));
        assert!(is_text_allowed(".env"));
        assert!(is_text_allowed(".gitignore"));
    }

    #[test]
    fn test_is_text_allowed_no_extension() {
        assert!(is_text_allowed("Dockerfile"));
        assert!(is_text_allowed("Makefile"));
    }

    #[test]
    fn test_is_text_allowed_rejects_binary_ext() {
        assert!(!is_text_allowed("photo.png"));
        assert!(!is_text_allowed("doc.pdf"));
    }
}
