import { useState, useEffect } from "react";
import { RefreshCw } from "lucide-react";
import { api } from "../../api";

export function SettingsView() {
  const [stats, setStats] = useState<{ session_count: number; last_indexed_at: number } | null>(null);
  const [rebuilding, setRebuilding] = useState(false);

  useEffect(() => {
    api.getIndexStats().then(setStats).catch(console.error);
  }, []);

  const rebuild = async () => {
    setRebuilding(true);
    try {
      await api.rebuildIndex();
      const s = await api.getIndexStats();
      setStats(s);
    } finally {
      setRebuilding(false);
    }
  };

  return (
    <div className="flex-1 overflow-auto p-6 max-w-lg">
      <h2
        className="text-lg font-semibold mb-6"
        style={{ fontFamily: "'JetBrains Mono', monospace", color: "var(--accent)" }}
      >
        Settings
      </h2>

      {/* Index info */}
      <div
        className="rounded-lg p-4 mb-6"
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
      >
        <h3 className="text-sm font-medium mb-3" style={{ color: "var(--text-secondary)" }}>
          Index
        </h3>
        {stats && (
          <div className="space-y-1 mb-4">
            <p className="text-xs" style={{ color: "var(--text-muted)" }}>
              Sessions indexed: <span style={{ color: "var(--text-primary)" }}>{stats.session_count}</span>
            </p>
            {stats.last_indexed_at > 0 && (
              <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                Last indexed:{" "}
                <span style={{ color: "var(--text-primary)" }}>
                  {new Date(stats.last_indexed_at).toLocaleString("ja-JP")}
                </span>
              </p>
            )}
          </div>
        )}
        <button
          onClick={rebuild}
          disabled={rebuilding}
          className="flex items-center gap-2 text-sm px-3 py-2 rounded cursor-pointer"
          style={{
            background: "var(--accent)",
            color: "#000",
            opacity: rebuilding ? 0.6 : 1,
          }}
        >
          <RefreshCw size={14} className={rebuilding ? "animate-spin" : ""} />
          {rebuilding ? "Rebuilding..." : "Rebuild Index"}
        </button>
      </div>

      {/* Data paths info */}
      <div
        className="rounded-lg p-4"
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
      >
        <h3 className="text-sm font-medium mb-3" style={{ color: "var(--text-secondary)" }}>
          Data Sources (Read-only)
        </h3>
        <div className="space-y-2">
          {[
            ["Sessions (JSONL)", "~/.kiro/sessions/cli/"],
            ["SQLite DB", "~/.local/share/kiro-cli/data.sqlite3"],
            ["Index DB", "~/.local/share/kiro-history/index.db"],
          ].map(([label, path]) => (
            <div key={label}>
              <p className="text-xs" style={{ color: "var(--text-muted)" }}>{label}</p>
              <p
                className="text-xs font-mono"
                style={{ color: "var(--text-secondary)" }}
              >
                {path}
              </p>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
