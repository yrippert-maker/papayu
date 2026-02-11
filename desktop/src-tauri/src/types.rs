use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind {
    CreateFile,
    UpdateFile,
    DeleteFile,
    CreateDir,
    DeleteDir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub title: String,
    pub description: String,
    pub kind: ActionKind,
    pub path: String,
    pub content: Option<String>, // для create/update
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub ok: bool,
    pub session_id: String,
    pub applied: Vec<String>,
    pub skipped: Vec<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
    pub undo_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResult {
    pub ok: bool,
    pub session_id: String,
    pub restored: Vec<String>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffItem {
    pub path: String,
    pub kind: String, // "create" | "update" | "delete" | "mkdir" | "rmdir"
    pub before: Option<String>,
    pub after: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewResult {
    pub ok: bool,
    pub diffs: Vec<DiffItem>,
    pub error: Option<String>,
    pub error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectContext {
    pub stack: Vec<String>,
    pub domain: String,
    pub maturity: String,   // Prototype | MVP | Production-like
    pub complexity: String, // Low | Medium | High
    pub risk_level: String, // Low | Medium | High
}

#[derive(Debug, Clone, Serialize)]
pub struct LlmContext {
    pub concise_summary: String,
    pub key_risks: Vec<String>,
    pub top_recommendations: Vec<String>,
    pub signals: Vec<ProjectSignal>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportStats {
    pub file_count: u64,
    pub dir_count: u64,
    pub total_size_bytes: u64,
    pub top_extensions: Vec<(String, u64)>,
    pub max_depth: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub severity: String, // info|warn|high
    pub title: String,
    pub details: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Recommendation {
    pub title: String,
    pub details: String,
    pub priority: String, // high|medium|low
    pub effort: String,   // low|medium|high
    pub impact: String,   // low|medium|high
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStructure {
    pub project_type: String,
    pub architecture: String,
    pub structure_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectSignal {
    pub category: String, // security|quality|structure
    pub level: String,    // info|warn|high
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalyzeReport {
    pub path: String,
    pub narrative: String,
    pub stats: ReportStats,
    pub structure: ProjectStructure,
    pub signals: Vec<ProjectSignal>,
    pub findings: Vec<Finding>,
    pub recommendations: Vec<Recommendation>,
    pub actions: Vec<Action>,
    pub project_context: ProjectContext,
    pub report_md: String,
    pub llm_context: LlmContext,
}
