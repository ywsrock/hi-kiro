// Type definitions mirroring Rust structs

export interface FilterParams {
  date_from?: number;
  date_to?: number;
  model_name?: string;
  tags?: string[];
  starred_only?: boolean;
}

export interface SessionSummary {
  session_id: string;
  title: string;
  cwd: string;
  created_at: number;
  updated_at: number;
  message_count: number;
  source: "jsonl" | "sqlite_v1" | "sqlite_v2";
  model_name?: string;
  max_context_pct?: number;
  total_tool_uses: number;
  total_cycles: number;
  total_duration_secs: number;
  starred: boolean;
  tags: string[];
}

export interface MessageDto {
  role: "User" | "Kiro";
  content: string;
  timestamp?: number;
}

export interface SessionDetail {
  summary: SessionSummary;
  messages: MessageDto[];
}

export interface IndexStats {
  session_count: number;
  last_indexed_at: number;
}

export interface TagStat {
  tag: string;
  count: number;
}

export interface ModelCount {
  model_name: string;
  count: number;
}

export interface CwdCount {
  cwd: string;
  count: number;
}

export interface DateCount {
  date: string;
  count: number;
}

export interface StatsData {
  total_sessions: number;
  total_messages: number;
  sessions_by_model: ModelCount[];
  sessions_by_cwd: CwdCount[];
  sessions_by_date: DateCount[];
  avg_context_pct: number;
  most_used_tags: TagStat[];
}

export interface CodeSnippet {
  language: string;
  code: string;
}

export interface CodeSnippetWithSession extends CodeSnippet {
  session_id: string;
  session_title: string;
}

export interface FileRef {
  path: string;
  exists: boolean;
}

export interface DiffResult {
  session_id_a: string;
  session_id_b: string;
  diff: string;
}

export type ExportFormat = "markdown" | "html" | "pdf";
