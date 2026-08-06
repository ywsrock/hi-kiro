import { useState, useEffect, useCallback, useRef } from "react";
import { Search, LayoutGrid, List, Star, Plus, BarChart2, X, Copy, Check } from "lucide-react";
import { api } from "../../api";
import { useToast } from "../ui/Toast";
import { SnippetCard, SaveSnippetModal, langColor, LANG_COLORS } from "./SnippetCard";
import type { SavedSnippet, CodeSnippetWithSession, SnippetStats } from "../../types";

type Tab = "saved" | "all";
type SortBy = "recent" | "used" | "starred";
type View = "grid" | "list";

const LANGUAGES = Object.keys(LANG_COLORS).filter(Boolean).sort();

function useDebounce<T>(value: T, delay: number): T {
  const [d, setD] = useState(value);
  useEffect(() => { const t = setTimeout(() => setD(value), delay); return () => clearTimeout(t); }, [value, delay]);
  return d;
}

export function SnippetsView() {
  const toast = useToast();
  const searchRef = useRef<HTMLInputElement>(null);

  const [tab, setTab] = useState<Tab>("saved");
  const [query, setQuery] = useState("");
  const [lang, setLang] = useState("");
  const [sortBy, setSortBy] = useState<SortBy>("recent");
  const [starredOnly, setStarredOnly] = useState(false);
  const [view, setView] = useState<View>("grid");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const [savedSnippets, setSavedSnippets] = useState<SavedSnippet[]>([]);
  const [allSnippets, setAllSnippets] = useState<CodeSnippetWithSession[]>([]);
  const [stats, setStats] = useState<SnippetStats | null>(null);
  const [showStats, setShowStats] = useState(false);
  const [loading, setLoading] = useState(false);

  // Save modal state
  const [saveModal, setSaveModal] = useState<CodeSnippetWithSession | null>(null);

  const debouncedQuery = useDebounce(query, 200);

  // Load saved snippets
  const loadSaved = useCallback(async () => {
    setLoading(true);
    try {
      const r = await api.searchSavedSnippets({
        query: debouncedQuery,
        language: lang || undefined,
        starred_only: starredOnly || undefined,
        sort_by: sortBy,
      });
      setSavedSnippets(r);
    } finally { setLoading(false); }
  }, [debouncedQuery, lang, starredOnly, sortBy]);

  // Load all (session) snippets
  const loadAll = useCallback(async () => {
    setLoading(true);
    try {
      const r = await api.getAllSnippets(debouncedQuery, lang);
      setAllSnippets(r);
    } finally { setLoading(false); }
  }, [debouncedQuery, lang]);

  useEffect(() => {
    if (tab === "saved") loadSaved();
    else loadAll();
  }, [tab, loadSaved, loadAll]);

  useEffect(() => {
    api.getSnippetStats().then(setStats).catch(() => {});
  }, [savedSnippets]);

  // Keyboard shortcuts
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "k") { e.preventDefault(); searchRef.current?.focus(); }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  const countLabel = tab === "saved" ? `${savedSnippets.length} saved` : `${allSnippets.length} snippets`;

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* ── Toolbar ──────────────────────────────────────────────────────── */}
      <div className="flex-shrink-0" style={{ borderBottom: "1px solid var(--border)", background: "var(--surface)" }}>
        {/* Tab bar */}
        <div className="flex items-center gap-0 px-4 pt-3">
          {(["saved", "all"] as Tab[]).map((t) => (
            <button key={t} onClick={() => setTab(t)}
              className="px-4 py-2 text-sm font-medium cursor-pointer rounded-t-lg"
              style={{
                background: tab === t ? "var(--bg)" : "transparent",
                color: tab === t ? "var(--accent)" : "var(--text-muted)",
                borderBottom: tab === t ? "2px solid var(--accent)" : "2px solid transparent",
              }}>
              {t === "saved" ? `保存済み${stats ? ` (${stats.total_saved})` : ""}` : "全セッション"}
            </button>
          ))}
          <div className="flex-1" />
          <button onClick={() => setShowStats(!showStats)}
            className="flex items-center gap-1 text-xs px-2 py-1 rounded cursor-pointer"
            style={{ color: showStats ? "var(--accent)" : "var(--text-muted)", background: showStats ? "rgba(34,197,94,0.1)" : "transparent" }}>
            <BarChart2 size={13} /> 統計
          </button>
        </div>

        {/* Stats panel */}
        {showStats && stats && (
          <div className="px-4 py-3 flex items-center gap-6" style={{ borderTop: "1px solid var(--border)", background: "var(--bg)" }}>
            {[
              { label: "保存済み", value: stats.total_saved },
              { label: "総使用回数", value: stats.total_uses },
            ].map(({ label, value }) => (
              <div key={label}>
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>{label}</p>
                <p className="text-lg font-bold" style={{ fontFamily: "'JetBrains Mono',monospace", color: "var(--accent)" }}>{value}</p>
              </div>
            ))}
            <div className="flex items-center gap-2 flex-wrap">
              {stats.by_language.slice(0, 6).map(([l, c]: [string, number]) => (
                <span key={l} className="flex items-center gap-1 text-xs px-2 py-0.5 rounded-full"
                  style={{ background: `${langColor(l)}18`, color: langColor(l), border: `1px solid ${langColor(l)}30` }}>
                  {l} <strong>{c}</strong>
                </span>
              ))}
            </div>
          </div>
        )}

        {/* Search row */}
        <div className="flex items-center gap-2 px-4 py-2">
          <div className="flex items-center gap-2 flex-1 px-3 py-1.5 rounded-lg"
            style={{ background: "var(--bg)", border: "1px solid var(--border)" }}>
            <Search size={13} style={{ color: "var(--text-muted)", flexShrink: 0 }} />
            <input ref={searchRef} type="text" value={query} onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && setQuery("")}
              placeholder="スニペットを検索... (Ctrl+K)"
              className="flex-1 bg-transparent outline-none text-sm"
              style={{ color: "var(--text-primary)" }} />
            {query && <button onClick={() => setQuery("")} className="cursor-pointer" style={{ color: "var(--text-muted)" }}><X size={12} /></button>}
          </div>
          <select value={lang} onChange={(e) => setLang(e.target.value)}
            className="text-xs px-2 py-1.5 rounded-lg cursor-pointer"
            style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-secondary)" }}>
            <option value="">全言語</option>
            {LANGUAGES.map((l) => <option key={l} value={l}>{l}</option>)}
          </select>
          {tab === "saved" && (
            <>
              <select value={sortBy} onChange={(e) => setSortBy(e.target.value as SortBy)}
                className="text-xs px-2 py-1.5 rounded-lg cursor-pointer"
                style={{ background: "var(--bg)", border: "1px solid var(--border)", color: "var(--text-secondary)" }}>
                <option value="recent">最新順</option>
                <option value="used">使用頻度</option>
                <option value="starred">スター</option>
              </select>
              <button onClick={() => setStarredOnly(!starredOnly)}
                className="p-1.5 rounded-lg cursor-pointer"
                style={{ background: starredOnly ? "rgba(245,158,11,0.15)" : "var(--bg)", border: "1px solid var(--border)", color: starredOnly ? "#F59E0B" : "var(--text-muted)" }}
                title="スターのみ">
                <Star size={13} fill={starredOnly ? "currentColor" : "none"} />
              </button>
            </>
          )}
          <div className="flex items-center rounded-lg overflow-hidden" style={{ border: "1px solid var(--border)" }}>
            {(["grid", "list"] as View[]).map((v) => (
              <button key={v} onClick={() => setView(v)}
                className="p-1.5 cursor-pointer"
                style={{ background: view === v ? "var(--accent)" : "var(--bg)", color: view === v ? "#000" : "var(--text-muted)" }}>
                {v === "grid" ? <LayoutGrid size={13} /> : <List size={13} />}
              </button>
            ))}
          </div>
          <span className="text-xs flex-shrink-0" style={{ color: "var(--text-muted)" }}>{countLabel}</span>
        </div>
      </div>

      {/* ── Content ───────────────────────────────────────────────────────── */}
      <div className="flex-1 overflow-auto p-4">
        {loading && (
          <div className="flex items-center justify-center gap-2 py-12" style={{ color: "var(--text-muted)" }}>
            <div className="w-4 h-4 rounded-full border-2 border-t-transparent animate-spin"
              style={{ borderColor: "var(--accent)", borderTopColor: "transparent" }} />
            読み込み中...
          </div>
        )}

        {/* Saved snippets */}
        {!loading && tab === "saved" && (
          <>
            {savedSnippets.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-4 py-16">
                <div style={{ color: "var(--text-muted)", opacity: 0.3 }}>
                  <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                    <polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/>
                  </svg>
                </div>
                <p className="text-sm font-medium" style={{ color: "var(--text-secondary)" }}>保存済みスニペットがありません</p>
                <p className="text-xs text-center" style={{ color: "var(--text-muted)" }}>
                  「全セッション」タブでセッション内のコードを見つけ、<br />「保存」ボタンでコレクションに追加できます
                </p>
                <button onClick={() => setTab("all")}
                  className="flex items-center gap-2 text-sm px-4 py-2 rounded-lg cursor-pointer"
                  style={{ background: "var(--accent)", color: "#000" }}>
                  <Plus size={14} /> セッションから追加
                </button>
              </div>
            ) : view === "grid" ? (
              <div className="grid gap-4" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))" }}>
                {savedSnippets.map((s) => (
                  <SnippetCard key={s.id} snippet={s} view="grid"
                    isSelected={selectedId === s.id}
                    onSelect={() => setSelectedId(selectedId === s.id ? null : s.id)}
                    onChange={loadSaved} />
                ))}
              </div>
            ) : (
              <div>
                {savedSnippets.map((s) => (
                  <SnippetCard key={s.id} snippet={s} view="list"
                    isSelected={selectedId === s.id}
                    onSelect={() => setSelectedId(selectedId === s.id ? null : s.id)}
                    onChange={loadSaved} />
                ))}
              </div>
            )}
          </>
        )}

        {/* All session snippets */}
        {!loading && tab === "all" && (
          <>
            {allSnippets.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-3 py-16">
                <p className="text-sm" style={{ color: "var(--text-muted)" }}>スニペットが見つかりません</p>
              </div>
            ) : (
              <div className={view === "grid" ? "grid gap-4" : "space-y-0"} style={view === "grid" ? { gridTemplateColumns: "repeat(auto-fill, minmax(280px, 1fr))" } : {}}>
                {allSnippets.map((s, i) => (
                  <SessionSnippetCard key={i} snippet={s} view={view}
                    isSelected={selectedId === `all-${i}`}
                    onSelect={() => setSelectedId(selectedId === `all-${i}` ? null : `all-${i}`)}
                    onSave={() => setSaveModal(s)} />
                ))}
              </div>
            )}
          </>
        )}
      </div>

      {/* Save Modal */}
      {saveModal && (
        <SaveSnippetModal
          language={saveModal.language}
          code={saveModal.code}
          sessionId={saveModal.session_id}
          sessionTitle={saveModal.session_title}
          onClose={() => setSaveModal(null)}
          onSaved={() => { loadSaved(); setTab("saved"); }}
        />
      )}
    </div>
  );
}

// コードから意味あるタイトルを生成（指示文を除外してコードの特徴行を使う）
function inferSnippetTitle(language: string, code: string): string {
  const lines = code.split("\n").map((l) => l.trim()).filter(Boolean);
  if (!lines.length) return `${language} snippet`;

  // 指示文パターン（自然言語）を検出
  const isInstruction = (line: string) =>
    /[：:。]$/.test(line) ||
    /(して|する|せよ|ください|通訳|翻訳|変換|説明|教えて|とは)/.test(line) ||
    (!/[({=;]/.test(line) && line.length < 60);

  // コードらしい行を優先
  const codeLine = lines.find((l) => !isInstruction(l) && /[({=;#]|^(def |fn |function |class |import |from |use |const |let |var )/.test(l));
  const best = codeLine ?? lines[0];
  return best.length > 50 ? best.slice(0, 50) + "…" : best;
}

// ── Session snippet card (read-only, with Save button) ────────────────────────
interface SessionSnippetCardProps {
  snippet: CodeSnippetWithSession;
  view: View;
  isSelected: boolean;
  onSelect: () => void;
  onSave: () => void;
}
function SessionSnippetCard({ snippet: s, view, isSelected, onSelect, onSave }: SessionSnippetCardProps) {
  const toast = useToast();
  const [copied, setCopied] = useState(false);
  const color = langColor(s.language);

  const copy = async (e: React.MouseEvent) => {
    e.stopPropagation();
    await api.copyToClipboard(s.code);
    setCopied(true);
    toast.success("コピーしました");
    setTimeout(() => setCopied(false), 1500);
  };

  if (view === "list") {
    return (
      <div onClick={onSelect} className="flex items-center gap-3 px-4 py-2.5 cursor-pointer group rounded-lg"
        style={{ background: isSelected ? "rgba(34,197,94,0.06)" : "transparent", border: isSelected ? "1px solid rgba(34,197,94,0.3)" : "1px solid var(--border)", marginBottom: "4px" }}>
        <span className="w-2 h-2 rounded-full flex-shrink-0" style={{ background: color }} />
        <span className="text-xs font-mono px-1.5 py-0.5 rounded flex-shrink-0" style={{ background: `${color}15`, color }}>{s.language}</span>
        <div className="flex flex-col min-w-0 flex-1">
          <span className="text-sm truncate" style={{ color: "var(--text-primary)" }}>{inferSnippetTitle(s.language, s.code)}</span>
          <div className="flex flex-col min-w-0">
            <span className="text-xs truncate font-medium" style={{ color: "var(--text-secondary)" }}>{inferSnippetTitle(s.language, s.code)}</span>
            <span className="text-xs truncate" style={{ color: "var(--text-muted)" }}>{s.session_title}</span>
          </div>
        </div>
        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100">
          <button onClick={(e) => { e.stopPropagation(); onSave(); }}
            className="flex items-center gap-1 text-xs px-2 py-1 rounded cursor-pointer"
            style={{ background: "rgba(34,197,94,0.1)", color: "var(--accent)" }}>保存</button>
          <button onClick={copy} className="p-1 rounded cursor-pointer" style={{ color: "var(--text-muted)" }}>
            {copied ? <Check size={13} style={{ color: "var(--accent)" }} /> : <Copy size={13} />}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div onClick={onSelect} className="rounded-xl overflow-hidden cursor-pointer group flex flex-col"
      style={{ background: "var(--surface)", border: isSelected ? "1px solid rgba(34,197,94,0.5)" : "1px solid var(--border)", transition: "border-color 0.15s" }}>
      <div className="flex items-center justify-between px-3 py-2.5" style={{ borderBottom: "1px solid var(--border)" }}>
        <div className="flex items-center gap-2 min-w-0">
          <span className="text-xs px-2 py-0.5 rounded font-mono font-semibold flex-shrink-0" style={{ background: `${color}18`, color }}>{s.language}</span>
          <div className="flex flex-col min-w-0">
            <span className="text-xs truncate font-medium" style={{ color: "var(--text-secondary)" }}>{inferSnippetTitle(s.language, s.code)}</span>
            <span className="text-xs truncate" style={{ color: "var(--text-muted)" }}>{s.session_title}</span>
          </div>
        </div>
      </div>
      <pre className="px-3 py-2.5 text-xs overflow-hidden flex-1"
        style={{ background: "var(--bg)", color: "var(--text-secondary)", fontFamily: "'JetBrains Mono',monospace", margin: 0, maxHeight: "140px", WebkitMaskImage: "linear-gradient(to bottom, black 60%, transparent)" }}>
        {s.code}
      </pre>
      <div className="flex items-center justify-end gap-2 px-3 py-2" style={{ borderTop: "1px solid var(--border)" }}>
        <button onClick={(e) => { e.stopPropagation(); onSave(); }}
          className="flex items-center gap-1 text-xs px-2.5 py-1 rounded cursor-pointer opacity-0 group-hover:opacity-100"
          style={{ background: "rgba(34,197,94,0.1)", color: "var(--accent)", border: "1px solid rgba(34,197,94,0.3)" }}>
          <Plus size={11} /> 保存
        </button>
        <button onClick={copy} className="flex items-center gap-1 text-xs px-2.5 py-1 rounded cursor-pointer"
          style={{ background: copied ? "rgba(34,197,94,0.15)" : "var(--accent)", color: copied ? "var(--accent)" : "#000" }}>
          {copied ? <Check size={11} /> : <Copy size={11} />} {copied ? "Copied!" : "Copy"}
        </button>
      </div>
    </div>
  );
}
