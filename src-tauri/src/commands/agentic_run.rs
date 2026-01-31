//! v2.4: Agentic Loop — analyze → plan → preview → apply → verify → auto-rollback → retry.

use std::path::Path;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, Window};

use crate::commands::{analyze_project, apply_actions_tx, generate_actions_from_report, get_project_profile, preview_actions, undo_last_tx};
use crate::types::{
    Action, ActionKind, AgenticRunRequest, AgenticRunResult, AttemptResult,
    ApplyOptions, ApplyPayload, VerifyResult,
};
use crate::verify::verify_project;

const AGENTIC_PROGRESS: &str = "agentic_progress";

#[derive(Clone, Serialize, Deserialize)]
pub struct AgenticProgressPayload {
    pub stage: String,
    pub message: String,
    pub attempt: u8,
}

fn emit_progress(window: &Window, stage: &str, message: &str, attempt: u8) {
    let _ = window.emit(
        AGENTIC_PROGRESS,
        AgenticProgressPayload {
            stage: stage.to_string(),
            message: message.to_string(),
            attempt,
        },
    );
}

fn has_readme(root: &Path) -> bool {
    ["README.md", "README.MD", "README.txt", "README"]
        .iter()
        .any(|f| root.join(f).exists())
}

fn has_gitignore(root: &Path) -> bool {
    root.join(".gitignore").exists()
}

fn has_src(root: &Path) -> bool {
    root.join("src").is_dir()
}

fn has_tests(root: &Path) -> bool {
    root.join("tests").is_dir()
}

fn has_editorconfig(root: &Path) -> bool {
    root.join(".editorconfig").exists()
}

/// v2.4.0: эвристический план (без LLM). README, .gitignore, tests/README.md, .editorconfig.
fn build_plan(
    path: &str,
    _goal: &str,
    max_actions: u16,
) -> (String, Vec<Action>) {
    let root = Path::new(path);
    let mut actions: Vec<Action> = vec![];
    let mut plan_parts: Vec<String> = vec![];

    if !has_readme(root) {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: "README.md".to_string(),
            content: Some(
                "# Project\n\n## Описание\n\nКратко опишите проект.\n\n## Запуск\n\n- dev: ...\n- build: ...\n\n## Структура\n\n- src/\n- tests/\n".into(),
            ),
        });
        plan_parts.push("README.md".into());
    }

    if !has_gitignore(root) {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: ".gitignore".to_string(),
            content: Some(
                "node_modules/\ndist/\nbuild/\n.next/\ncoverage/\n.env\n.env.*\n.DS_Store\n.target/\n".into(),
            ),
        });
        plan_parts.push(".gitignore".into());
    }

    if has_src(root) && !has_tests(root) {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: "tests/README.md".to_string(),
            content: Some("# Тесты\n\nДобавьте unit- и интеграционные тесты.\n".into()),
        });
        plan_parts.push("tests/README.md".into());
    }

    if !has_editorconfig(root) {
        actions.push(Action {
            kind: ActionKind::CreateFile,
            path: ".editorconfig".to_string(),
            content: Some(
                "root = true\n\n[*]\nindent_style = space\nindent_size = 2\nend_of_line = lf\n".into(),
            ),
        });
        plan_parts.push(".editorconfig".into());
    }

    let n = max_actions as usize;
    if actions.len() > n {
        actions.truncate(n);
    }

    let plan = if plan_parts.is_empty() {
        "Нет безопасных правок для применения.".to_string()
    } else {
        format!("План: добавить {}", plan_parts.join(", "))
    };

    (plan, actions)
}

#[tauri::command]
pub async fn agentic_run(window: Window, payload: AgenticRunRequest) -> AgenticRunResult {
    let path = payload.path.clone();
    let user_goal = payload.goal.clone();
    let constraints = payload.constraints.clone();
    let app = window.app_handle();

    let profile = match get_project_profile(window.clone(), path.clone()).await {
        Ok(p) => p,
        Err(e) => {
            return AgenticRunResult {
                ok: false,
                attempts: vec![],
                final_summary: format!("Ошибка профиля: {}", e),
                error: Some(e.clone()),
                error_code: Some("PROFILE_ERROR".into()),
            };
        }
    };

    let max_attempts = profile.max_attempts.max(1);
    let max_actions = (profile.limits.max_actions_per_tx as u16).max(1);
    let goal = profile.goal_template.replace("{goal}", &user_goal);
    let _safe_mode = profile.safe_mode;

    let mut attempts: Vec<AttemptResult> = vec![];

    for attempt in 1..=max_attempts {
        let attempt_u8 = attempt.min(255) as u8;
        emit_progress(&window, "analyze", "Сканирую проект…", attempt_u8);

        let report = match analyze_project(vec![path.clone()], None) {
            Ok(r) => r,
            Err(e) => {
                emit_progress(&window, "failed", "Ошибка анализа.", attempt_u8);
                return AgenticRunResult {
                    ok: false,
                    attempts,
                    final_summary: format!("Ошибка анализа: {}", e),
                    error: Some(e),
                    error_code: Some("ANALYZE_FAILED".into()),
                };
            }
        };

        emit_progress(&window, "plan", "Составляю план исправлений…", attempt_u8);
        let gen = generate_actions_from_report(
            path.clone(),
            report.clone(),
            "safe_create_only".to_string(),
        )
        .await;
        let (plan, actions) = if gen.ok && !gen.actions.is_empty() {
            let n = max_actions as usize;
            let mut a = gen.actions;
            if a.len() > n {
                a.truncate(n);
            }
            (
                format!("План из отчёта: {} действий.", a.len()),
                a,
            )
        } else {
            build_plan(&path, &goal, max_actions)
        };

        if actions.is_empty() {
            emit_progress(&window, "done", "Готово.", attempt_u8);
            return AgenticRunResult {
                ok: true,
                attempts,
                final_summary: plan.clone(),
                error: None,
                error_code: None,
            };
        }

        emit_progress(&window, "preview", "Показываю, что изменится…", attempt_u8);
        let preview = match preview_actions(ApplyPayload {
            root_path: path.clone(),
            actions: actions.clone(),
            auto_check: None,
            label: None,
            user_confirmed: false,
        }) {
            Ok(p) => p,
            Err(e) => {
                emit_progress(&window, "failed", "Ошибка предпросмотра.", attempt_u8);
                return AgenticRunResult {
                    ok: false,
                    attempts,
                    final_summary: format!("Ошибка предпросмотра: {}", e),
                    error: Some(e),
                    error_code: Some("PREVIEW_FAILED".into()),
                };
            }
        };

        emit_progress(&window, "apply", "Применяю изменения…", attempt_u8);
        let apply_result = apply_actions_tx(
            app.clone(),
            path.clone(),
            actions.clone(),
            ApplyOptions {
                auto_check: false,
                user_confirmed: true,
            },
        )
        .await;

        if !apply_result.ok {
            emit_progress(&window, "failed", "Не удалось безопасно применить изменения.", attempt_u8);
            let err = apply_result.error.clone();
            let code = apply_result.error_code.clone();
            attempts.push(AttemptResult {
                attempt: attempt_u8,
                plan: plan.clone(),
                actions: actions.clone(),
                preview,
                apply: apply_result,
                verify: VerifyResult {
                    ok: false,
                    checks: vec![],
                    error: None,
                    error_code: None,
                },
            });
            return AgenticRunResult {
                ok: false,
                attempts,
                final_summary: "Apply не выполнен.".to_string(),
                error: err,
                error_code: code,
            };
        }

        let verify = if constraints.auto_check {
            emit_progress(&window, "verify", "Проверяю сборку/типы…", attempt_u8);
            let v = verify_project(&path);
            if !v.ok {
                emit_progress(&window, "revert", "Обнаружены ошибки. Откатываю изменения…", attempt_u8);
                let _ = undo_last_tx(app.clone(), path.clone()).await;
                attempts.push(AttemptResult {
                    attempt: attempt_u8,
                    plan: plan.clone(),
                    actions: actions.clone(),
                    preview,
                    apply: apply_result,
                    verify: v,
                });
                continue;
            }
            v
        } else {
            VerifyResult {
                ok: true,
                checks: vec![],
                error: None,
                error_code: None,
            }
        };

        attempts.push(AttemptResult {
            attempt: attempt_u8,
            plan: plan.clone(),
            actions: actions.clone(),
            preview,
            apply: apply_result,
            verify: verify.clone(),
        });

        emit_progress(&window, "done", "Готово.", attempt_u8);
        return AgenticRunResult {
            ok: true,
            attempts,
            final_summary: plan,
            error: None,
            error_code: None,
        };
    }

    emit_progress(&window, "failed", "Не удалось безопасно применить изменения.", max_attempts.min(255) as u8);
    AgenticRunResult {
        ok: false,
        attempts,
        final_summary: "Превышено число попыток. Изменения откачены.".to_string(),
        error: Some("max_attempts exceeded".into()),
        error_code: Some("MAX_ATTEMPTS_EXCEEDED".into()),
    }
}
