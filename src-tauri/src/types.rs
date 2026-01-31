use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub kind: ActionKind,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionKind {
    CreateFile,
    CreateDir,
    UpdateFile,
    DeleteFile,
    DeleteDir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyPayload {
    pub root_path: String,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub auto_check: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// v2.4.2: обязательное подтверждение перед apply
    #[serde(default)]
    pub user_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<usize>, // v2.3.3: index where apply failed (before rollback)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxTouchedItem {
    pub rel_path: String,
    pub kind: String,   // "file" | "dir"
    pub existed: bool,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxManifest {
    pub tx_id: String,
    pub root_path: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub status: String, // "pending" | "committed" | "rolled_back"
    #[serde(default)]
    pub applied_actions: Vec<Action>,
    #[serde(default)]
    pub touched: Vec<TxTouchedItem>,
    #[serde(default)]
    pub auto_check: bool,
    /// Legacy: old manifests had snapshot_items only
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot_items: Option<Vec<TxSnapshotItem>>,
}

/// Legacy alias for rollback reading old manifests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxSnapshotItem {
    pub rel_path: String,
    pub kind: String,
    pub existed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoAvailableResult {
    pub ok: bool,
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedoResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRedoState {
    pub undo_available: bool,
    pub redo_available: bool,
}

/// v2.4: action with metadata for plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub id: String,
    pub kind: ActionKind,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    pub summary: String,
    pub rationale: String,
    pub tags: Vec<String>,
    pub risk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionPlan {
    pub plan_id: String,
    pub root_path: String,
    pub title: String,
    pub actions: Vec<ActionItem>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateActionsPayload {
    pub path: String,
    pub selected: Vec<String>,
    pub mode: String, // "safe" | "balanced"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffItem {
    pub kind: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_content: Option<String>,
    /// v2.4.2: BLOCKED — защищённый/не-текстовый файл
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewResult {
    pub diffs: Vec<DiffItem>,
    pub summary: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzePayload {
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub title: String,
    pub details: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub title: String,
    pub details: String,
    pub priority: Option<String>,
    pub effort: Option<String>,
    pub impact: Option<String>,
}

/// v2.9.2: сигнал по проекту (категория + уровень для recommended_pack_ids)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSignal {
    pub category: String, // "security" | "quality" | "structure"
    pub level: String,   // "warn" | "high" | "critical"
}

/// v2.9.1: группа действий (readme, gitignore, tests, …)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionGroup {
    pub id: String,
    pub title: String,
    pub description: String,
    pub actions: Vec<Action>,
}

/// v2.9.2: пакет улучшений (security, quality, structure)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixPack {
    pub id: String,
    pub title: String,
    pub description: String,
    pub group_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeReport {
    pub path: String,
    pub narrative: String,
    pub findings: Vec<Finding>,
    pub recommendations: Vec<Recommendation>,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub action_groups: Vec<ActionGroup>,
    #[serde(default)]
    pub fix_packs: Vec<FixPack>,
    #[serde(default)]
    pub recommended_pack_ids: Vec<String>,
    /// v2.4.5: прикреплённые файлы, переданные при анализе (контекст для UI/планировщика)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attached_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchPayload {
    pub paths: Vec<String>,
    pub confirm_apply: bool,
    pub auto_check: bool,
    pub selected_actions: Option<Vec<Action>>,
    /// v2.4.2: передаётся в ApplyPayload при confirm_apply
    #[serde(default)]
    pub user_confirmed: bool,
    /// v2.4.5: прикреплённые файлы для контекста при анализе
    #[serde(default)]
    pub attached_files: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchEvent {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<AnalyzeReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<PreviewResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_result: Option<ApplyResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub undo_available: Option<bool>,
}

/// v2.9.3: транзакционное применение (path + actions + auto_check)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionSpec {
    pub path: String,
    pub actions: Vec<Action>,
    pub auto_check: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    pub applied_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoStatus {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
}

/// v3.0: сообщение агента (user / system / assistant)
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: String,
    pub text: String,
}

/// v3.0: план агента (propose_actions)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPlan {
    pub ok: bool,
    pub summary: String,
    pub actions: Vec<Action>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    /// JSON плана для передачи в Apply (при Plan-режиме).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_json: Option<String>,
    /// Собранный контекст для передачи в Apply вместе с plan_json.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_context: Option<String>,
}

/// v3.1: опции применения (auto_check). v2.4.2: user_confirmed для apply_actions_tx.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyOptions {
    pub auto_check: bool,
    #[serde(default)]
    pub user_confirmed: bool,
}

/// v3.1: результат этапа проверки (verify / build / smoke)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckStageResult {
    pub stage: String,
    pub ok: bool,
    pub output: String,
}

/// v3.1: результат транзакционного apply с авто-проверкой и откатом
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyTxResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    pub applied: bool,
    pub rolled_back: bool,
    pub checks: Vec<CheckStageResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

/// v3.2: результат генерации действий из отчёта (generate_actions_from_report)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateActionsResult {
    pub ok: bool,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub skipped: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

// --- v2.4 Agentic Loop ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticConstraints {
    pub auto_check: bool,
    pub max_attempts: u8,
    pub max_actions: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticRunRequest {
    pub path: String,
    pub goal: String,
    pub constraints: AgenticConstraints,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckItem {
    pub name: String,
    pub ok: bool,
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub ok: bool,
    pub checks: Vec<CheckItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptResult {
    pub attempt: u8,
    pub plan: String,
    pub actions: Vec<Action>,
    pub preview: PreviewResult,
    pub apply: ApplyTxResult,
    pub verify: VerifyResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticRunResult {
    pub ok: bool,
    pub attempts: Vec<AttemptResult>,
    pub final_summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

// --- Тренды и рекомендации (мониторинг не реже раз в месяц) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendsRecommendation {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendsResult {
    pub last_updated: String,
    pub recommendations: Vec<TrendsRecommendation>,
    /// true если прошло >= 30 дней с last_updated — рекомендуется обновить
    pub should_update: bool,
}

// --- Projects & sessions (v2.5: entities, history, profiles) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub path: String,
    pub name: String,
    pub created_at: String,
}

/// v2.5: сохранённые настройки проекта (store)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSettings {
    pub project_id: String,
    #[serde(default)]
    pub auto_check: bool,
    #[serde(default)]
    pub max_attempts: u8,
    #[serde(default)]
    pub max_actions: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_template: Option<String>,
}

// --- v2.4.3: detected profile (by path) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    ReactVite,
    NextJs,
    Node,
    Rust,
    Python,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLimits {
    pub max_files: u32,
    pub timeout_sec: u32,
    pub max_actions_per_tx: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProfile {
    pub path: String,
    pub project_type: ProjectType,
    pub safe_mode: bool,
    pub max_attempts: u32,
    pub goal_template: String,
    pub limits: ProjectLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEvent {
    pub kind: String, // "message" | "analyze" | "agentic_run" | "apply"
    pub role: Option<String>,
    pub text: Option<String>,
    pub at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub events: Vec<SessionEvent>,
}
