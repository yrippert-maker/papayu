export interface Action {
  kind: string;
  path: string;
  content?: string;
}

export interface Finding {
  title: string;
  details: string;
  path?: string;
}

export interface ActionGroup {
  id: string;
  title: string;
  description: string;
  actions: Action[];
}

export interface FixPack {
  id: string;
  title: string;
  description: string;
  group_ids: string[];
}

export interface AnalyzeReport {
  path: string;
  narrative: string;
  findings: Finding[];
  recommendations: unknown[];
  actions: Action[];
  action_groups?: ActionGroup[];
  fix_packs?: FixPack[];
  recommended_pack_ids?: string[];
}

export interface DiffItem {
  kind: string;
  path: string;
  old_content?: string;
  new_content?: string;
  /** v2.4.2: BLOCKED — защищённый/не-текстовый файл */
  summary?: string;
}

export interface PreviewResult {
  diffs: DiffItem[];
  summary: string;
}

export interface ApplyResult {
  ok: boolean;
  tx_id?: string;
  error?: string;
  error_code?: string;
}

/** v2.9.3: доступен ли откат транзакции */
export interface UndoStatus {
  available: boolean;
  tx_id?: string;
}

/** v3.0: план агента (propose_actions) */
export interface AgentPlan {
  ok: boolean;
  summary: string;
  actions: Action[];
  error?: string;
  error_code?: string;
  /** JSON плана для передачи в Apply */
  plan_json?: string;
  /** Собранный контекст для Apply */
  plan_context?: string;
}

/** Тренды и рекомендации (мониторинг не реже раз в месяц) */
export interface TrendsRecommendation {
  title: string;
  summary?: string;
  url?: string;
  source?: string;
}

export interface TrendsResult {
  last_updated: string;
  recommendations: TrendsRecommendation[];
  should_update: boolean;
}

/** v3.1: результат apply_actions_tx с autocheck и откатом */
export interface ApplyTxResult {
  ok: boolean;
  tx_id?: string | null;
  applied: boolean;
  rolled_back: boolean;
  checks: { stage: string; ok: boolean; output: string }[];
  error?: string;
  error_code?: string;
}

/** v3.2: результат generate_actions_from_report */
export interface GenerateActionsResult {
  ok: boolean;
  actions: Action[];
  skipped: string[];
  error?: string;
  error_code?: string;
}

/** v2.4: Agentic Loop */
export interface AgenticConstraints {
  auto_check: boolean;
  max_attempts: number;
  max_actions: number;
}

export interface AgenticRunRequest {
  path: string;
  goal: string;
  constraints: AgenticConstraints;
}

export interface CheckItem {
  name: string;
  ok: boolean;
  output: string;
}

export interface VerifyResult {
  ok: boolean;
  checks: CheckItem[];
  error?: string;
  error_code?: string;
}

export interface AttemptResult {
  attempt: number;
  plan: string;
  actions: Action[];
  preview: PreviewResult;
  apply: ApplyTxResult;
  verify: VerifyResult;
}

export interface AgenticRunResult {
  ok: boolean;
  attempts: AttemptResult[];
  final_summary: string;
  error?: string;
  error_code?: string;
}

/** v2.4.3: detected profile (by path) */
export type ProjectType = "react_vite" | "next_js" | "node" | "rust" | "python" | "unknown";

export interface ProjectLimits {
  max_files: number;
  timeout_sec: number;
  max_actions_per_tx: number;
}

export interface ProjectProfile {
  path: string;
  project_type: ProjectType;
  safe_mode: boolean;
  max_attempts: number;
  goal_template: string;
  limits: ProjectLimits;
}

export interface BatchEvent {
  kind: string;
  report?: AnalyzeReport;
  preview?: PreviewResult;
  apply_result?: ApplyResult;
  message?: string;
  undo_available?: boolean;
}

export type ChatRole = "system" | "user" | "assistant";

export interface ChatMessage {
  role: ChatRole;
  text: string;
  report?: AnalyzeReport;
  preview?: PreviewResult;
  applyResult?: ApplyResult;
}

/** Событие сессии (agentic_run, message, analyze, apply) */
export interface SessionEvent {
  kind: string;
  role?: string;
  text?: string;
  at: string;
}

/** Сессия по проекту */
export interface Session {
  id: string;
  project_id: string;
  created_at: string;
  updated_at: string;
  events: SessionEvent[];
}
