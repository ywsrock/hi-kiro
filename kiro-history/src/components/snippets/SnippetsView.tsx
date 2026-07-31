import { useState, useEffect } from "react";
import { Copy, Check } from "lucide-react";
import { api } from "../../api";
import type { CodeSnippetWithSession } from "../../types";

const LANGUAGES = ["", "typescript", "javascript", "python", "rust", "bash", "sql", "json", "yaml", "go", "java"];

export function SnippetsView() {
  const [snippets, setSnippets] = useState<CodeSnippetWithSession[]>([]);
  const [query, setQuery] = useState("");
  const [lang, setLang] = useState("");
  const [loading, setLoading] = useState(true);
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null);

  useEffect(() => {
    setLoading(true);
    api
      .getAllSnippets(query, lang)
      .then(setSnippets)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, [query, lang]);

  const copy = async (code: string, idx: number) => {
    await api.copyToClipboard(code);
    setCopiedIdx(idx);
    setTimeout(() => setCopiedIdx(null), 1500);
  };

  return (
    <div className="flex-1 flex flex-col overflow-hidden">
      {/* Toolbar */}
      <div
        className="flex items-center gap-3 px-4 py-2 flex-shrink-0"
        style={{ borderBottom: "1px solid var(--border)", background: "var(--surface)" }}
      >
        <input
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search snippets..."
          className="flex-1 bg-transparent outline-none text-sm"
          style={{ color: "var(--text-primary)" }}
        />
        <select
          value={lang}
          onChange={(e) => setLang(e.target.value)}
          className="text-xs px-2 py-1 rounded cursor-pointer"
          style={{
            background: "var(--bg)",
            border: "1px solid var(--border)",
            color: "var(--text-secondary)",
          }}
          aria-label="Filter by language"
        >
          <option value="">All languages</option>
          {LANGUAGES.filter(Boolean).map((l) => (
            <option key={l} value={l}>{l}</option>
          ))}
        </select>
        <span className="text-xs" style={{ color: "var(--text-muted)" }}>
          {snippets.length} snippets
        </span>
      </div>

      {/* List */}
      <div className="flex-1 overflow-auto p-4 space-y-4">
        {loading && (
          <p className="text-sm" style={{ color: "var(--text-muted)" }}>Loading...</p>
        )}
        {!loading && snippets.length === 0 && (
          <p className="text-sm" style={{ color: "var(--text-muted)" }}>No snippets found</p>
        )}
        {snippets.map((s, i) => (
          <div
            key={i}
            className="rounded-lg overflow-hidden"
            style={{ border: "1px solid var(--border)" }}
          >
            {/* Snippet header */}
            <div
              className="flex items-center justify-between px-3 py-2"
              style={{ background: "var(--surface)" }}
            >
              <div className="flex items-center gap-2">
                <span
                  className="text-xs px-2 py-0.5 rounded font-mono"
                  style={{ background: "rgba(34,197,94,0.1)", color: "var(--accent)" }}
                >
                  {s.language}
                </span>
                <span className="text-xs truncate max-w-48" style={{ color: "var(--text-muted)" }}>
                  {s.session_title}
                </span>
              </div>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => copy(s.code, i)}
                  className="flex items-center gap-1 text-xs px-2 py-1 rounded cursor-pointer"
                  style={{ background: "var(--border)", color: "var(--text-secondary)" }}
                >
                  {copiedIdx === i ? (
                    <Check size={11} style={{ color: "var(--accent)" }} />
                  ) : (
                    <Copy size={11} />
                  )}
                  {copiedIdx === i ? "Copied!" : "Copy"}
                </button>
              </div>
            </div>
            {/* Code */}
            <pre
              className="p-3 text-xs overflow-x-auto"
              style={{
                background: "var(--bg)",
                color: "var(--text-secondary)",
                fontFamily: "'JetBrains Mono', monospace",
                margin: 0,
                maxHeight: "300px",
              }}
            >
              <code>{s.code}</code>
            </pre>
          </div>
        ))}
      </div>
    </div>
  );
}
