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

// ---- LLM Integration ----

export interface LlmRequest {
  provider: string;       // "openai" | "anthropic" | "ollama"
  model: string;
  api_key?: string | null;
  base_url?: string | null;
  context: string;        // JSON string of llm_context
  prompt: string;
  max_tokens?: number | null;
}

export interface LlmResponse {
  ok: boolean;
  content: string;
  model: string;
  usage?: { prompt_tokens: number; completion_tokens: number; total_tokens: number } | null;
  error?: string | null;
}

export interface LlmSettings {
  provider: string;
  model: string;
  apiKey: string;
  baseUrl: string;
}

export const DEFAULT_LLM_SETTINGS: LlmSettings = {
  provider: 'openai',
  model: 'gpt-4o-mini',
  apiKey: '',
  baseUrl: '',
};

export const LLM_MODELS: Record<string, { label: string; models: { value: string; label: string }[] }> = {
  openai: {
    label: 'OpenAI',
    models: [
      { value: 'gpt-4o-mini', label: 'GPT-4o Mini (дешёвый, быстрый)' },
      { value: 'gpt-4o', label: 'GPT-4o (мощный)' },
      { value: 'gpt-4.1-mini', label: 'GPT-4.1 Mini' },
      { value: 'gpt-4.1', label: 'GPT-4.1' },
    ],
  },
  anthropic: {
    label: 'Anthropic',
    models: [
      { value: 'claude-sonnet-4-20250514', label: 'Claude Sonnet 4' },
      { value: 'claude-haiku-4-5-20251001', label: 'Claude Haiku 4.5 (быстрый)' },
    ],
  },
  ollama: {
    label: 'Ollama (локальный)',
    models: [
      { value: 'llama3.1', label: 'Llama 3.1' },
      { value: 'mistral', label: 'Mistral' },
      { value: 'codellama', label: 'Code Llama' },
      { value: 'qwen2.5-coder', label: 'Qwen 2.5 Coder' },
    ],
  },
};

export async function askLlm(
  settings: LlmSettings,
  context: LlmContext,
  prompt: string,
): Promise<LlmResponse> {
  return invoke<LlmResponse>('ask_llm', {
    request: {
      provider: settings.provider,
      model: settings.model,
      api_key: settings.apiKey || null,
      base_url: settings.baseUrl || null,
      context: JSON.stringify(context),
      prompt,
      max_tokens: 2048,
    },
  });
}
