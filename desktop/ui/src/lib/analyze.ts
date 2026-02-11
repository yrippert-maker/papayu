import { invoke } from '@tauri-apps/api/core';

export type ActionKind =
  | 'create_file'
  | 'update_file'
  | 'delete_file'
  | 'create_dir'
  | 'delete_dir';

export interface Action {
  id: string;
  title: string;
  description: string;
  kind: ActionKind;
  path: string;
  content?: string | null;
}

export interface ApplyResult {
  ok: boolean;
  session_id: string;
  applied: string[];
  skipped: string[];
  error?: string | null;
  error_code?: string | null;
  undo_available: boolean;
}

export interface UndoResult {
  ok: boolean;
  session_id: string;
  restored: string[];
  error?: string | null;
  error_code?: string | null;
}

export type DiffItem = {
  path: string;
  kind: 'create' | 'update' | 'delete' | 'mkdir' | 'rmdir' | string;
  before?: string | null;
  after?: string | null;
  summary: string;
};

export type PreviewResult = {
  ok: boolean;
  diffs: DiffItem[];
  error?: string | null;
  error_code?: string | null;
};

export interface ProjectStructure {
  project_type: string;
  architecture: string;
  structure_notes: string[];
}

export interface ProjectContext {
  stack: string[];
  domain: string;
  maturity: string;
  complexity: string;
  risk_level: string;
}

export interface LlmContext {
  concise_summary: string;
  key_risks: string[];
  top_recommendations: string[];
  signals: ProjectSignal[];
}

export interface ProjectSignal {
  category: string;
  level: string;
  message: string;
}

export interface Recommendation {
  title: string;
  details: string;
  priority: string;
  effort: string;
  impact: string;
}

export interface AnalyzeReport {
  path: string;
  narrative: string;
  stats: {
    file_count: number;
    dir_count: number;
    total_size_bytes: number;
    top_extensions: [string, number][];
    max_depth: number;
  };
  structure: ProjectStructure;
  project_context: ProjectContext;
  findings: { severity: string; title: string; details: string }[];
  recommendations: Recommendation[];
  actions?: Action[];
  signals: ProjectSignal[];
  report_md: string;
  llm_context: LlmContext;
}

export async function analyzeProject(path: string): Promise<AnalyzeReport> {
  return invoke<AnalyzeReport>('analyze_project', { path });
}
