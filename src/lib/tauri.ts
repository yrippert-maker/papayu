import { invoke } from "@tauri-apps/api/core";
import type {
  Action,
  AgenticRunRequest,
  AgenticRunResult,
  AnalyzeReport,
  ApplyTxResult,
  BatchEvent,
  GenerateActionsResult,
  PreviewResult,
  ProjectProfile,
  Session,
  TrendsResult,
  UndoStatus,
  VerifyResult,
} from "./types";

export interface UndoRedoState {
  undo_available: boolean;
  redo_available: boolean;
}

export interface RunBatchPayload {
  paths: string[];
  confirm_apply: boolean;
  auto_check: boolean;
  selected_actions?: Action[];
  user_confirmed?: boolean;
  attached_files?: string[];
}

export interface ApplyActionsTxOptions {
  auto_check: boolean;
  user_confirmed: boolean;
}

export interface ProjectItem {
  id: string;
  path: string;
}

export interface AddProjectResult {
  id: string;
}

export interface UndoLastResult {
  ok: boolean;
  error_code?: string;
  error?: string;
}

export async function getUndoRedoState(): Promise<UndoRedoState> {
  return invoke<UndoRedoState>("get_undo_redo_state_cmd");
}

export async function getUndoStatus(): Promise<UndoStatus> {
  return invoke<UndoStatus>("undo_status").catch(() => ({ available: false } as UndoStatus));
}

export async function getFolderLinks(): Promise<{ paths: string[] }> {
  return invoke<{ paths: string[] }>("get_folder_links");
}

export async function setFolderLinks(paths: string[]): Promise<void> {
  return invoke("set_folder_links", { links: { paths } });
}

export async function getProjectProfile(path: string): Promise<ProjectProfile> {
  return invoke<ProjectProfile>("get_project_profile", { path });
}

export async function runBatchCmd(payload: RunBatchPayload): Promise<BatchEvent[]> {
  return invoke<BatchEvent[]>("run_batch_cmd", { payload });
}

/** Предпросмотр diff для actions (CREATE/UPDATE/DELETE) без записи на диск. */
export async function previewActions(rootPath: string, actions: Action[]): Promise<PreviewResult> {
  return invoke<PreviewResult>("preview_actions_cmd", {
    payload: {
      root_path: rootPath,
      actions,
      auto_check: null,
      label: null,
      user_confirmed: false,
    },
  });
}

export async function applyActionsTx(
  path: string,
  actions: Action[],
  options: ApplyActionsTxOptions | boolean
): Promise<ApplyTxResult> {
  const opts: ApplyActionsTxOptions =
    typeof options === "boolean"
      ? { auto_check: options, user_confirmed: true }
      : options;
  return invoke<ApplyTxResult>("apply_actions_tx", {
    path,
    actions,
    options: opts,
  });
}

export async function generateActionsFromReport(
  path: string,
  report: AnalyzeReport,
  mode: string
): Promise<GenerateActionsResult> {
  return invoke<GenerateActionsResult>("generate_actions_from_report", {
    path,
    report,
    mode,
  });
}

export async function agenticRun(payload: AgenticRunRequest): Promise<AgenticRunResult> {
  return invoke<AgenticRunResult>("agentic_run", { payload });
}

export async function listProjects(): Promise<ProjectItem[]> {
  return invoke<ProjectItem[]>("list_projects");
}

export async function addProject(path: string, name: string | null): Promise<AddProjectResult> {
  return invoke<AddProjectResult>("add_project", { path, name });
}

export async function listSessions(projectId?: string): Promise<Session[]> {
  return invoke<Session[]>("list_sessions", { projectId: projectId ?? null });
}

export async function appendSessionEvent(
  projectId: string,
  kind: string,
  role: string,
  text: string
): Promise<void> {
  return invoke("append_session_event", {
    project_id: projectId,
    kind,
    role,
    text,
  });
}

export interface AgentPlanResult {
  ok: boolean;
  summary: string;
  actions: Action[];
  error?: string;
  error_code?: string;
  plan_json?: string;
  plan_context?: string;
}

export async function proposeActions(
  path: string,
  reportJson: string,
  userGoal: string,
  designStyle?: string | null,
  trendsContext?: string | null,
  lastPlanJson?: string | null,
  lastContext?: string | null
): Promise<AgentPlanResult> {
  return invoke<AgentPlanResult>("propose_actions", {
    path,
    reportJson,
    userGoal,
    designStyle: designStyle ?? null,
    trendsContext: trendsContext ?? null,
    lastPlanJson: lastPlanJson ?? null,
    lastContext: lastContext ?? null,
  });
}

export async function undoLastTx(path: string): Promise<boolean> {
  return invoke<boolean>("undo_last_tx", { path });
}

export async function undoLast(): Promise<UndoLastResult> {
  return invoke<UndoLastResult>("undo_last");
}

export async function redoLast(): Promise<UndoLastResult> {
  return invoke<UndoLastResult>("redo_last");
}

/** Проверка целостности проекта (типы, сборка, тесты). Вызывается автоматически после применений или вручную. */
export async function verifyProject(path: string): Promise<VerifyResult> {
  return invoke<VerifyResult>("verify_project", { path });
}

/** Тренды и рекомендации: последнее обновление и список. should_update === true если прошло >= 30 дней. */
export async function getTrendsRecommendations(): Promise<TrendsResult> {
  return invoke<TrendsResult>("get_trends_recommendations");
}

/** Обновить тренды и рекомендации (запрос к внешним ресурсам по allowlist). */
export async function fetchTrendsRecommendations(): Promise<TrendsResult> {
  return invoke<TrendsResult>("fetch_trends_recommendations");
}

// Settings export/import

export interface ImportResult {
  projects_imported: number;
  profiles_imported: number;
  sessions_imported: number;
  folder_links_imported: number;
}

/** Export all settings as JSON string */
export async function exportSettings(): Promise<string> {
  return invoke<string>("export_settings");
}

/** Import settings from JSON string */
export async function importSettings(json: string, mode?: "replace" | "merge"): Promise<ImportResult> {
  return invoke<ImportResult>("import_settings", { json, mode: mode ?? "merge" });
}
