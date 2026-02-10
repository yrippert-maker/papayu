//! v2.4: verify_project — проверка сборки/типов после apply (allowlisted, timeout 60s).
//! v2.4.5: allowlist команд загружается из config/verify_allowlist.json (или встроенный дефолт).

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::types::{CheckItem, VerifyResult};

/// Одна разрешённая команда из конфига.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct VerifyAllowlistEntry {
    pub exe: String,
    pub args: Vec<String>,
    pub name: String,
    #[serde(default)]
    pub timeout_sec: Option<u64>,
}

fn default_timeout() -> u64 {
    60
}

fn load_verify_allowlist() -> HashMap<String, Vec<VerifyAllowlistEntry>> {
    const DEFAULT_JSON: &str = include_str!("../config/verify_allowlist.json");
    serde_json::from_str(DEFAULT_JSON).unwrap_or_else(|_| HashMap::new())
}

fn run_check(cwd: &Path, exe: &str, args: &[&str], name: &str, timeout_secs: u64) -> CheckItem {
    let timeout = Duration::from_secs(timeout_secs);
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .current_dir(cwd)
        .env("CI", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let (ok, output_str) = match cmd.spawn() {
        Ok(mut child) => {
            let start = Instant::now();
            let result = loop {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    break (false, format!("TIMEOUT ({}s)", timeout_secs));
                }
                match child.try_wait() {
                    Ok(Some(_status)) => {
                        let out = child.wait_with_output();
                        let (success, combined) = match out {
                            Ok(o) => {
                                let out_str = String::from_utf8_lossy(&o.stdout);
                                let err_str = String::from_utf8_lossy(&o.stderr);
                                let combined = format!("{}{}", out_str, err_str);
                                let combined = if combined.len() > 8000 {
                                    format!("{}…", &combined[..8000])
                                } else {
                                    combined
                                };
                                (o.status.success(), combined)
                            }
                            Err(e) => (false, e.to_string()),
                        };
                        break (success, combined);
                    }
                    Ok(None) => {
                        thread::sleep(Duration::from_millis(100));
                    }
                    Err(e) => break (false, e.to_string()),
                }
            };
            result
        }
        Err(e) => (false, e.to_string()),
    };

    CheckItem {
        name: name.to_string(),
        ok,
        output: output_str,
    }
}

/// Определение типа проекта по наличию файлов в корне.
fn project_type(root: &Path) -> &'static str {
    if root.join("Cargo.toml").exists() {
        return "rust";
    }
    if root.join("package.json").exists() {
        return "node";
    }
    if root.join("setup.py").exists() || root.join("pyproject.toml").exists() {
        return "python";
    }
    "unknown"
}

/// Выполняет одну команду из allowlist (exe + args из конфига).
fn run_check_from_entry(cwd: &Path, entry: &VerifyAllowlistEntry) -> CheckItem {
    let timeout = entry.timeout_sec.unwrap_or_else(default_timeout);
    let args: Vec<&str> = entry.args.iter().map(|s| s.as_str()).collect();
    run_check(cwd, &entry.exe, &args, &entry.name, timeout)
}

/// v2.4: проверка проекта после apply. Allowlist из config/verify_allowlist.json.
pub fn verify_project(path: &str) -> VerifyResult {
    let root = Path::new(path);
    if !root.exists() || !root.is_dir() {
        return VerifyResult {
            ok: false,
            checks: vec![],
            error: Some("path not found".to_string()),
            error_code: Some("PATH_NOT_FOUND".into()),
        };
    }

    let pt = project_type(root);
    let allowlist = load_verify_allowlist();
    let mut checks: Vec<CheckItem> = vec![];

    match pt {
        "rust" => {
            if let Some(entries) = allowlist.get("rust") {
                if let Some(entry) = entries.first() {
                    checks.push(run_check_from_entry(root, entry));
                }
            }
            if checks.is_empty() {
                checks.push(run_check(root, "cargo", &["check"], "cargo check", 60));
            }
        }
        "node" => {
            let (exe, args, name): (String, Vec<String>, String) = {
                let pkg = root.join("package.json");
                if pkg.exists() {
                    if let Ok(s) = std::fs::read_to_string(&pkg) {
                        if s.contains("\"test\"") {
                            (
                                "npm".into(),
                                vec!["run".into(), "-s".into(), "test".into()],
                                "npm test".into(),
                            )
                        } else if s.contains("\"build\"") {
                            (
                                "npm".into(),
                                vec!["run".into(), "-s".into(), "build".into()],
                                "npm run build".into(),
                            )
                        } else if s.contains("\"lint\"") {
                            (
                                "npm".into(),
                                vec!["run".into(), "-s".into(), "lint".into()],
                                "npm run lint".into(),
                            )
                        } else {
                            (
                                "npm".into(),
                                vec!["run".into(), "-s".into(), "build".into()],
                                "npm run build".into(),
                            )
                        }
                    } else {
                        (
                            "npm".into(),
                            vec!["run".into(), "-s".into(), "build".into()],
                            "npm run build".into(),
                        )
                    }
                } else {
                    (
                        "npm".into(),
                        vec!["run".into(), "-s".into(), "build".into()],
                        "npm run build".into(),
                    )
                }
            };
            let allowed = allowlist
                .get("node")
                .and_then(|entries| entries.iter().find(|e| e.exe == exe && e.args == args));
            let timeout = allowed.and_then(|e| e.timeout_sec).unwrap_or(60);
            let name_str = allowed.map(|e| e.name.as_str()).unwrap_or(name.as_str());
            let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            checks.push(run_check(root, &exe, &args_ref, name_str, timeout));
        }
        "python" => {
            if let Some(entries) = allowlist.get("python") {
                if let Some(entry) = entries.first() {
                    checks.push(run_check_from_entry(root, entry));
                }
            }
            if checks.is_empty() {
                checks.push(run_check(
                    root,
                    "python3",
                    &["-m", "compileall", ".", "-q"],
                    "python -m compileall",
                    60,
                ));
            }
        }
        _ => {
            return VerifyResult {
                ok: true,
                checks: vec![],
                error: None,
                error_code: None,
            };
        }
    }

    let ok = checks.iter().all(|c| c.ok);
    VerifyResult {
        ok,
        checks,
        error: if ok {
            None
        } else {
            Some("verify failed".to_string())
        },
        error_code: if ok {
            None
        } else {
            Some("VERIFY_FAILED".into())
        },
    }
}
