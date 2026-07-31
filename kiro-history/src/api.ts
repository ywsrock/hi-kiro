import { invoke } from "@tauri-apps/api/core";
import type {
  CodeSnippetWithSession,
  DiffResult,
  ExportFormat,
  FileRef,
  FilterParams,
  IndexStats,
  SessionDetail,
  SessionSummary,
  StatsData,
  TagStat,
} from "./types";

export const api = {
  searchSessions: (query: string, limit?: number, filters?: FilterParams) =>
    invoke<SessionSummary[]>("search_sessions", { query, limit, filters }),

  getSessionDetail: (sessionId: string) =>
    invoke<SessionDetail>("get_session_detail", { sessionId }),

  getRelatedSessions: (cwd: string, excludeId: string) =>
    invoke<SessionSummary[]>("get_related_sessions", { cwd, excludeId }),

  rebuildIndex: () => invoke<void>("rebuild_index"),

  getIndexStats: () => invoke<IndexStats>("get_index_stats"),

  copyToClipboard: (text: string) =>
    invoke<void>("copy_to_clipboard", { text }),

  resumeSession: (sessionId: string, cwd: string) =>
    invoke<void>("resume_session", { sessionId, cwd }),

  toggleBookmark: (sessionId: string) =>
    invoke<boolean>("toggle_bookmark", { sessionId }),

  setTags: (sessionId: string, tags: string[]) =>
    invoke<void>("set_tags", { sessionId, tags }),

  getAllTags: () => invoke<TagStat[]>("get_all_tags"),

  getBookmarkedSessions: () => invoke<SessionSummary[]>("get_bookmarked_sessions"),

  getStats: () => invoke<StatsData>("get_stats"),

  getSnippets: (sessionId: string) =>
    invoke<{ language: string; code: string }[]>("get_snippets", { sessionId }),

  getAllSnippets: (query?: string, langFilter?: string) =>
    invoke<CodeSnippetWithSession[]>("get_all_snippets", {
      query: query ?? "",
      langFilter: langFilter ?? "",
    }),

  getFileRefs: (sessionId: string) =>
    invoke<FileRef[]>("get_file_refs", { sessionId }),

  openInEditor: (path: string) => invoke<void>("open_in_editor", { path }),

  getSessionDiff: (sessionIdA: string, sessionIdB: string) =>
    invoke<DiffResult>("get_session_diff", { sessionIdA, sessionIdB }),

  exportSession: (sessionId: string, format: ExportFormat, outputPath: string) =>
    invoke<void>("export_session_cmd", { sessionId, format, outputPath }),

  exportSessionsZip: (
    sessionIds: string[],
    format: ExportFormat,
    outputPath: string
  ) => invoke<void>("export_sessions_zip_cmd", { sessionIds, format, outputPath }),

  deleteSession: (sessionId: string) =>
    invoke<void>("delete_session", { sessionId }),

  deleteSessionsFiles: (sessionIds: string[]) =>
    invoke<{ deleted: string[]; skipped: { session_id: string; reason: string }[] }>(
      "delete_sessions_files",
      { sessionIds }
    ),

  renameSession: (sessionId: string, newTitle: string) =>
    invoke<void>("rename_session", { sessionId, newTitle }),
};
