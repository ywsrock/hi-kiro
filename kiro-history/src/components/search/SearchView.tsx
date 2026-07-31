import { useState, useEffect, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import { useApp } from "../../context/AppContext";
import { SearchBar } from "../search/SearchBar";
import { FilterBar } from "../search/FilterBar";
import { SessionList } from "../session/SessionList";
import { PreviewPane } from "../preview/PreviewPane";
import { DeleteConfirmDialog } from "../session/DeleteConfirmDialog";
import { useToast } from "../ui/Toast";
import { api } from "../../api";
import type { FilterParams, SessionSummary } from "../../types";

// Debounce hook
function useDebounce<T>(value: T, delay: number): T {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const t = setTimeout(() => setDebounced(value), delay);
    return () => clearTimeout(t);
  }, [value, delay]);
  return debounced;
}

export function SearchView() {
  const { selectedSessionId, setSelectedSessionId, setIndexing, setIndexProgress } = useApp();
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [filters, setFilters] = useState<FilterParams>({});
  const [sessions, setSessions] = useState<SessionSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [availableTags, setAvailableTags] = useState<string[]>([]);
  const [deleteTargets, setDeleteTargets] = useState<SessionSummary[]>([]);

  const debouncedQuery = useDebounce(query, 200);

  const loadSessions = useCallback(async () => {
    try {
      const results = await api.searchSessions(debouncedQuery, 500, filters);
      setSessions(results);
      setLoading(false);

      // Extract unique models for filter
      const models = [...new Set(results.map((s) => s.model_name).filter(Boolean) as string[])];
      setAvailableModels(models);
    } catch (e) {
      console.error("Search failed:", e);
      setLoading(false);
    }
  }, [debouncedQuery, filters]);

  useEffect(() => {
    loadSessions();
  }, [loadSessions]);

  // Load tags
  useEffect(() => {
    api.getAllTags().then((tags) => setAvailableTags(tags.map((t) => t.tag)));
  }, []);

  // Listen for index progress events
  useEffect(() => {
    const unlisten = listen<{ processed: number; total: number }>("index:progress", (e) => {
      setIndexing(true);
      setIndexProgress(e.payload);
    });
    const unlistenDone = listen("index:done", () => {
      setIndexing(false);
      loadSessions();
    });
    return () => {
      unlisten.then((fn) => fn());
      unlistenDone.then((fn) => fn());
    };
  }, [loadSessions]);

  const handleBookmarkToggle = (id: string, starred: boolean) => {
    setSessions((prev) =>
      prev.map((s) => (s.session_id === id ? { ...s, starred } : s))
    );
  };

  const handleRename = (id: string, newTitle: string) => {
    setSessions((prev) =>
      prev.map((s) => (s.session_id === id ? { ...s, title: newTitle } : s))
    );
  };

  const handleDelete = (id: string) => {
    setSessions((prev) => prev.filter((s) => s.session_id !== id));
    // Clear selection if deleted session was selected
    if (selectedSessionId === id) {
      setSelectedSessionId(null);
    }
  };

  // Show confirm dialog for one or multiple sessions
  const handleDeleteMultiple = (ids: string[]) => {
    const targets = sessions.filter((s) => ids.includes(s.session_id));
    setDeleteTargets(targets);
  };

  const confirmDelete = async () => {
    const ids = deleteTargets.map((s) => s.session_id);
    const result = await api.deleteSessionsFiles(ids);
    // Remove successfully deleted from list
    setSessions((prev) => prev.filter((s) => !result.deleted.includes(s.session_id)));
    if (selectedSessionId && result.deleted.includes(selectedSessionId)) {
      setSelectedSessionId(null);
    }
    // Show skipped info if any
    if (result.skipped.length > 0) {
      const msg = result.skipped
        .map((sk) => `• ${sk.session_id.slice(0, 8)}… : ${sk.reason}`)
        .join("\n");
      toast.success(`削除完了: ${result.deleted.length}件`); if (result.skipped.length > 0) { const msg = result.skipped.map((sk: {session_id: string; reason: string}) => `${sk.session_id.slice(0,8)}: ${sk.reason}`).join(", "); toast.info(`スキップ: ${result.skipped.length}件 (${msg})`); }
    }
    setDeleteTargets([]);
  };

  return (
    <>
      {deleteTargets.length > 0 && (
        <DeleteConfirmDialog
          sessions={deleteTargets}
          onConfirm={confirmDelete}
          onCancel={() => setDeleteTargets([])}
        />
      )}
      <div className="flex flex-1 overflow-hidden">
        {/* Left pane */}
        <div
          className="flex flex-col"
          style={{ width: "320px", borderRight: "1px solid var(--border)", flexShrink: 0 }}
        >
          <SearchBar value={query} onChange={setQuery} resultCount={sessions.length} />
          <FilterBar
            filters={filters}
            onChange={setFilters}
            availableModels={availableModels}
            availableTags={availableTags}
          />
          <SessionList
            sessions={sessions}
            selectedId={selectedSessionId}
            onSelect={setSelectedSessionId}
            onBookmarkToggle={handleBookmarkToggle}
            onDelete={handleDelete}
            onDeleteMultiple={handleDeleteMultiple}
            onRename={handleRename}
            loading={loading}
          />
        </div>

        {/* Right pane */}
        <div className="flex-1 flex overflow-hidden">
          <PreviewPane sessionId={selectedSessionId} onSelectSession={setSelectedSessionId} />
        </div>
      </div>
    </>
  );
}
