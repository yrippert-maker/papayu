# Контракты UI ↔ Tauri

Единый источник правды для вызовов команд и форматов ответов. PAPA YU v2.4.5.

---

## Стандарт ответов

- **Успех:** `{ ok: true, ...data }` или возврат типа `AnalyzeReport`, `PreviewResult`, `ApplyResult`, `UndoResult`.
- **Ошибка:** `Result::Err(String)` или поле `ok: false` с `error`, `error_code`, при необходимости `details`.

---

## Команды (invoke)

| Команда | Вход | Выход | Слой UI |
|---------|------|-------|---------|
| `analyze_project_cmd` | `paths`, `attached_files?` | `AnalyzeReport` | lib/tauri.ts |
| `preview_actions_cmd` | `ApplyPayload` | `PreviewResult` | lib/tauri.ts |
| `apply_actions_cmd` | `ApplyPayload` | `ApplyResult` | lib/tauri.ts |
| `apply_actions_tx` | `ApplyPayload` | `ApplyTxResult` | lib/tauri.ts |
| `run_batch_cmd` | `BatchPayload` | `BatchEvent[]` | lib/tauri.ts |
| `undo_last` | — | `UndoResult` | lib/tauri.ts |
| `undo_last_tx` | `path` | `UndoResult` | lib/tauri.ts |
| `undo_available` | — | `UndoRedoState` | lib/tauri.ts |
| `get_undo_redo_state_cmd` | — | `UndoRedoState` | lib/tauri.ts |
| `redo_last` | — | `RedoResult` | lib/tauri.ts |
| `undo_status` | — | `UndoStatus` | lib/tauri.ts |
| `generate_actions` | payload | `GenerateActionsResult` | lib/tauri.ts |
| `generate_actions_from_report` | payload | `Action[]` | lib/tauri.ts |
| `propose_actions` | payload | `AgentPlan` | lib/tauri.ts |
| `agentic_run` | `AgenticRunRequest` | `AgenticRunResult` | lib/tauri.ts |
| `get_folder_links` | — | `{ paths }` | lib/tauri.ts |
| `set_folder_links` | `{ links: { paths } }` | `void` | lib/tauri.ts |
| `verify_project` | `path` | `VerifyResult` | lib/tauri.ts |
| `get_project_profile` | `path` | `ProjectProfile` | lib/tauri.ts |
| `list_projects` | — | `ProjectItem[]` | lib/tauri.ts |
| `add_project` | `path` | `AddProjectResult` | lib/tauri.ts |
| `list_sessions` | `projectPath` | `Session[]` | lib/tauri.ts |
| `append_session_event` | payload | `void` | lib/tauri.ts |
| `get_project_settings` | `projectPath` | `ProjectSettings` | lib/tauri.ts |
| `set_project_settings` | payload | `void` | lib/tauri.ts |
| `apply_project_setting_cmd` | `projectPath`, `key`, `value` | `void` | lib/tauri.ts |
| `get_trends_recommendations` | — | `TrendsResult` | lib/tauri.ts |
| `fetch_trends_recommendations` | — | `TrendsResult` | lib/tauri.ts |
| `export_settings` | — | `string` (JSON) | lib/tauri.ts |
| `import_settings` | `json` | `void` | lib/tauri.ts |
| `analyze_weekly_reports_cmd` | `projectPath`, `from?`, `to?` | `WeeklyReportResult` | lib/tauri.ts |
| `save_report_cmd` | `projectPath`, `reportMd`, `date?` | `string` | lib/tauri.ts |
| `research_answer_cmd` | `query`, `projectPath?` | `OnlineAnswer` | lib/tauri.ts |
| `load_domain_notes_cmd` | `projectPath` | `DomainNotes` | lib/tauri.ts |
| `save_domain_notes_cmd` | `projectPath`, `data` | `void` | lib/tauri.ts |
| `delete_domain_note_cmd` | `projectPath`, `noteId` | `bool` | lib/tauri.ts |
| `clear_expired_domain_notes_cmd` | `projectPath` | `usize` | lib/tauri.ts |
| `pin_domain_note_cmd` | `projectPath`, `noteId`, `pinned` | `bool` | lib/tauri.ts |
| `distill_and_save_domain_note_cmd` | payload | `DomainNote` | lib/tauri.ts |

---

## События (listen)

| Событие | Payload | Где эмитится | Где слушается |
|---------|---------|--------------|----------------|
| `analyze_progress` | `string` | analyze_project, apply, preview | Tasks.tsx |
| `batch_event` | `BatchEvent` | run_batch | Tasks.tsx |
| `agentic_progress` | `{ stage, message, attempt }` | agentic_run | Tasks.tsx |

---

## Транзакционность (Apply / Undo)

- **apply_actions_tx:** snapshot → apply → (auto_check при включённом) → rollback при ошибке. Манифест в `userData/history/<txId>/`.
- **undo_last_tx:** откат последней транзакции из undo_stack.
- **redo_last:** повтор из redo_stack.
- Двухстековая модель: undo_stack + redo_stack.

---

*См. также `lib/tauri.ts` и `src-tauri/src/lib.rs`.*
