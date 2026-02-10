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
  /** При ok=false и триггере online fallback: UI вызывает researchAnswer(query) */
  online_fallback_suggested?: string | null;
  /** true — online_context_md был принят и вставлен в prompt */
  online_context_used?: boolean | null;
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
  protocol_fallback_stage?: string | null;
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

/** Источник online research */
export interface OnlineSource {
  url: string;
  title: string;
  published_at?: string;
  snippet?: string;
}

/** Результат online research */
export interface OnlineAnswer {
  answer_md: string;
  sources: OnlineSource[];
  confidence: number;
  notes?: string;
}

/** Источник в domain note */
export interface DomainNoteSource {
  url: string;
  title: string;
}

/** Domain note (curated from online research) */
export interface DomainNote {
  id: string;
  created_at: number;
  topic: string;
  tags: string[];
  content_md: string;
  sources: DomainNoteSource[];
  confidence: number;
  ttl_days: number;
  usage_count: number;
  last_used_at?: number | null;
  pinned: boolean;
}

/** Domain notes file (.papa-yu/notes/domain_notes.json) */
export interface DomainNotes {
  schema_version: number;
  updated_at: number;
  notes: DomainNote[];
}

/** Один proposal из еженедельного отчёта (B3) */
export interface WeeklyProposal {
  kind: "prompt_change" | "setting_change" | "golden_trace_add" | "limit_tuning" | "safety_rule";
  title: string;
  why: string;
  risk: "low" | "medium" | "high";
  steps: string[];
  expected_impact: string;
  evidence?: string;
}

/** Результат еженедельного отчёта */
export interface WeeklyReportResult {
  ok: boolean;
  error?: string;
  stats_bundle?: unknown;
  llm_report?: unknown;
  report_md?: string;
}
