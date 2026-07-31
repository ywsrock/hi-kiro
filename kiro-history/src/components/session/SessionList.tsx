import { useRef, useCallback, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { Star, Copy, Check, Trash2 } from "lucide-react";
import type { SessionSummary } from "../../types";
import { api } from "../../api";

interface SessionListProps {
  sessions: SessionSummary[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onBookmarkToggle?: (id: string, starred: boolean) => void;
  onDelete?: (id: string) => void;
  onDeleteMultiple?: (ids: string[]) => void;
  onRename?: (id: string, newTitle: string) => void;
  loading?: boolean;
}

function formatDate(ms: number): string {
  if (!ms) return "";
  return new Date(ms).toLocaleDateString("ja-JP", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}

function shortenCwd(cwd: string): string {
  const home = "/home/";
  if (cwd.startsWith(home)) {
    const rest = cwd.slice(cwd.indexOf("/", home.length));
    return "~" + rest;
  }
  return cwd;
}

function ContextBar({ pct }: { pct?: number }) {
  if (!pct) return null;
  const color = pct >= 95 ? "#EF4444" : pct >= 80 ? "#F59E0B" : "var(--accent)";
  return (
    <div className="mt-1 h-0.5 rounded-full overflow-hidden" style={{ background: "var(--border)" }}>
      <div style={{ width: `${Math.min(pct, 100)}%`, background: color, height: "100%" }} />
    </div>
  );
}

interface InlineTitleProps {
  sessionId: string;
  title: string;
  onRename: (id: string, newTitle: string) => void;
}

function InlineTitle({ sessionId, title, onRename }: InlineTitleProps) {
  const [editing, setEditing] = useState(false);
  const divRef = useRef<HTMLDivElement>(null);

  const startEdit = (e: React.MouseEvent) => {
    e.stopPropagation();
    setEditing(true);
    setTimeout(() => {
      if (divRef.current) {
        divRef.current.focus();
        // select all text
        const range = document.createRange();
        range.selectNodeContents(divRef.current);
        const sel = window.getSelection();
        sel?.removeAllRanges();
        sel?.addRange(range);
      }
    }, 0);
  };

  const commit = async (e?: React.MouseEvent) => {
    e?.stopPropagation();
    const trimmed = (divRef.current?.innerText ?? "").trim();
    if (trimmed && trimmed !== title) {
      try {
        await api.renameSession(sessionId, trimmed);
        onRename(sessionId, trimmed);
      } catch (err) {
        alert(`リネームに失敗しました: ${err}`);
      }
    }
    setEditing(false);
  };

  const cancel = (e?: React.MouseEvent) => {
    e?.stopPropagation();
    setEditing(false);
  };

  if (editing) {
    return (
      <div className="flex items-center gap-1 flex-1 min-w-0" onClick={(e) => e.stopPropagation()}>
        <div
          ref={divRef}
          contentEditable
          suppressContentEditableWarning
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.nativeEvent.isComposing) return;
            if (e.key === "Enter") { e.preventDefault(); commit(); }
            if (e.key === "Escape") cancel();
          }}
          className="flex-1 text-sm font-medium rounded px-1 outline-none min-w-0"
          style={{
            background: "var(--bg)",
            border: "1px solid var(--accent)",
            color: "var(--text-primary)",
            minHeight: "1.5rem",
            lineHeight: "1.5rem",
            whiteSpace: "nowrap",
            overflow: "hidden",
          }}
          aria-label="セッションタイトルを編集"
        >
          {title}
        </div>
        <button
          onClick={commit}
          className="text-xs px-1.5 py-0.5 rounded cursor-pointer flex-shrink-0"
          style={{ background: "var(--accent)", color: "#000", fontWeight: 600 }}
          title="確定"
        >✓</button>
        <button
          onClick={cancel}
          className="text-xs px-1.5 py-0.5 rounded cursor-pointer flex-shrink-0"
          style={{ background: "var(--border)", color: "var(--text-muted)" }}
          title="キャンセル"
        >✕</button>
      </div>
    );
  }

  return (
    <span
      className="text-sm font-medium truncate flex-1"
      style={{ color: "var(--text-primary)" }}
      title={title}
    >
      {title || "Untitled"}
    </span>
  );
}

function SessionIdBadge({ id }: { id: string }) {
  const [copied, setCopied] = useState(false);
  const short = id.slice(0, 8);

  const copy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.copyToClipboard(id);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <button
      onClick={copy}
      className="flex items-center gap-1 text-xs px-1.5 py-0.5 rounded font-mono cursor-pointer"
      style={{ background: "var(--border)", color: "var(--text-muted)" }}
      title={`Session ID: ${id}\nClick to copy`}
    >
      {copied ? (
        <Check size={10} style={{ color: "var(--accent)" }} />
      ) : (
        <Copy size={10} />
      )}
      {short}…
    </button>
  );
}

export function SessionList({
  sessions,
  selectedId,
  onSelect,
  onBookmarkToggle,
  onDelete,
  onDeleteMultiple,
  onRename,
  loading,
}: SessionListProps) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [selectMode, setSelectMode] = useState(false);
  const [checkedIds, setCheckedIds] = useState<Set<string>>(new Set());

  const virtualizer = useVirtualizer({
    count: sessions.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 80,
    overscan: 5,
  });

  const handleStar = useCallback(
    async (e: React.MouseEvent, session: SessionSummary) => {
      e.stopPropagation();
      const newVal = await api.toggleBookmark(session.session_id);
      onBookmarkToggle?.(session.session_id, newVal);
    },
    [onBookmarkToggle]
  );

  const handleDelete = useCallback(
    async (e: React.MouseEvent, session: SessionSummary) => {
      e.stopPropagation();
      onDeleteMultiple?.([session.session_id]);
    },
    [onDeleteMultiple]
  );

  const toggleCheck = (id: string) => {
    setCheckedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const selectAll = () => setCheckedIds(new Set(sessions.map((s) => s.session_id)));
  const deselectAll = () => setCheckedIds(new Set());

  const exitSelectMode = () => {
    setSelectMode(false);
    setCheckedIds(new Set());
  };

  if (loading) {
    return (
      <div className="flex-1 p-3 space-y-2">
        {[...Array(8)].map((_, i) => (
          <div
            key={i}
            className="h-16 rounded animate-pulse"
            style={{ background: "var(--surface)" }}
          />
        ))}
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="text-sm" style={{ color: "var(--text-muted)" }}>
          No sessions found
        </p>
      </div>
    );
  }

  if (sessions.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="text-sm" style={{ color: "var(--text-muted)" }}>
          No sessions found
        </p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Toolbar */}
      <div
        className="flex items-center gap-2 px-3 py-1.5 flex-shrink-0"
        style={{ borderBottom: "1px solid var(--border)" }}
      >
        {!selectMode ? (
          <button
            onClick={() => setSelectMode(true)}
            className="text-xs px-2 py-1 rounded cursor-pointer"
            style={{ background: "var(--border)", color: "var(--text-secondary)" }}
          >
            選択
          </button>
        ) : (
          <>
            <button
              onClick={selectAll}
              className="text-xs px-2 py-1 rounded cursor-pointer"
              style={{ background: "var(--border)", color: "var(--text-secondary)" }}
            >
              全選択
            </button>
            <button
              onClick={deselectAll}
              className="text-xs px-2 py-1 rounded cursor-pointer"
              style={{ background: "var(--border)", color: "var(--text-secondary)" }}
            >
              解除
            </button>
            <span className="text-xs flex-1" style={{ color: "var(--text-muted)" }}>
              {checkedIds.size} 件選択中
            </span>
            {checkedIds.size > 0 && (
              <button
                onClick={() => onDeleteMultiple?.(Array.from(checkedIds))}
                className="text-xs px-2 py-1 rounded cursor-pointer font-medium"
                style={{ background: "#EF4444", color: "#fff" }}
              >
                削除 ({checkedIds.size})
              </button>
            )}
            <button
              onClick={exitSelectMode}
              className="text-xs px-2 py-1 rounded cursor-pointer"
              style={{ color: "var(--text-muted)" }}
            >
              ✕
            </button>
          </>
        )}
      </div>

      {/* List */}
      <div ref={parentRef} className="flex-1 overflow-auto">
        <div
          style={{ height: `${virtualizer.getTotalSize()}px`, position: "relative" }}
        >
          {virtualizer.getVirtualItems().map((item) => {
            const s = sessions[item.index];
            const isSelected = s.session_id === selectedId;
            const isChecked = checkedIds.has(s.session_id);

            return (
              <div
                key={s.session_id}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  transform: `translateY(${item.start}px)`,
                }}
              >
                <div
                  onClick={() => {
                    if (selectMode) {
                      toggleCheck(s.session_id);
                    } else {
                      onSelect(s.session_id);
                    }
                  }}
                  className="group mx-2 my-1 px-3 py-2 rounded cursor-pointer"
                  style={{
                    background: isSelected && !selectMode
                      ? "rgba(34,197,94,0.08)"
                      : isChecked
                      ? "rgba(239,68,68,0.08)"
                      : "transparent",
                    border:
                      isSelected && !selectMode
                        ? "1px solid rgba(34,197,94,0.4)"
                        : isChecked
                        ? "1px solid rgba(239,68,68,0.4)"
                        : "1px solid transparent",
                  }}
                  role="button"
                  tabIndex={0}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      selectMode ? toggleCheck(s.session_id) : onSelect(s.session_id);
                    }
                  }}
                >
                  {/* Title row */}
                  <div className="flex items-start justify-between gap-2">
                    {selectMode && (
                      <input
                        type="checkbox"
                        checked={isChecked}
                        onChange={() => toggleCheck(s.session_id)}
                        onClick={(e) => e.stopPropagation()}
                        className="mt-0.5 flex-shrink-0 cursor-pointer"
                        aria-label={`Select ${s.title}`}
                      />
                    )}
                    <InlineTitle
                      sessionId={s.session_id}
                      title={s.title || "Untitled"}
                      onRename={(id, t) => onRename?.(id, t)}
                    />
                    {!selectMode && (
                      <div className="flex items-center gap-1 flex-shrink-0 opacity-0 group-hover:opacity-100">
                        <button
                          onClick={(e) => handleStar(e, s)}
                          className="cursor-pointer p-0.5 rounded"
                          style={{ color: s.starred ? "#F59E0B" : "var(--text-muted)" }}
                          aria-label={s.starred ? "Remove bookmark" : "Bookmark"}
                        >
                          <Star size={13} fill={s.starred ? "currentColor" : "none"} />
                        </button>
                        <button
                          onClick={(e) => handleDelete(e, s)}
                          className="cursor-pointer p-0.5 rounded"
                          style={{ color: "var(--text-muted)" }}
                          onMouseEnter={(e) => (e.currentTarget.style.color = "#EF4444")}
                          onMouseLeave={(e) => (e.currentTarget.style.color = "var(--text-muted)")}
                          aria-label="Delete session"
                          title="削除"
                        >
                          <Trash2 size={13} />
                        </button>
                      </div>
                    )}
                  </div>

                  {/* Meta row */}
                  <div className="flex items-center gap-2 mt-0.5">
                    <span className="text-xs truncate flex-1" style={{ color: "var(--text-muted)" }}>
                      {shortenCwd(s.cwd)}
                    </span>
                    <span className="text-xs flex-shrink-0" style={{ color: "var(--text-muted)" }}>
                      {formatDate(s.updated_at)}
                    </span>
                  </div>

                  {/* Badges row */}
                  <div className="flex items-center gap-1.5 mt-1 flex-wrap">
                    <SessionIdBadge id={s.session_id} />
                    <span
                      className="text-xs px-1.5 py-0.5 rounded"
                      style={{ background: "var(--border)", color: "var(--text-secondary)" }}
                    >
                      {s.message_count} msgs
                    </span>
                    {s.model_name && (
                      <span
                        className="text-xs px-1.5 py-0.5 rounded truncate max-w-24"
                        style={{ background: "rgba(34,197,94,0.1)", color: "var(--accent)" }}
                        title={s.model_name}
                      >
                        {s.model_name.replace("claude-", "").replace("-latest", "")}
                      </span>
                    )}
                    {s.tags.slice(0, 2).map((tag) => (
                      <span
                        key={tag}
                        className="text-xs px-1.5 py-0.5 rounded"
                        style={{ background: "var(--border)", color: "var(--text-secondary)" }}
                      >
                        {tag}
                      </span>
                    ))}
                  </div>

                  <ContextBar pct={s.max_context_pct} />
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
