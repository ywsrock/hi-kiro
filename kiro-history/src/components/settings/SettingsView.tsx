import { useState, useEffect } from "react";
import { RefreshCw, Search, Save, RotateCcw } from "lucide-react";
import { api } from "../../api";
import { useToast } from "../ui/Toast";

export function SettingsView() {
  const toast = useToast();

  // Index stats
  const [stats, setStats] = useState<{ session_count: number; last_indexed_at: number } | null>(null);
  const [rebuilding, setRebuilding] = useState(false);

  // Current paths (actual resolved paths)
  const [currentPaths, setCurrentPaths] = useState<{
    sessions_dir: string;
    sqlite_db_path: string;
    index_db_path: string;
  } | null>(null);

  // Config (user overrides)
  const [sessionsDir, setSessionsDir] = useState("");
  const [sqlitePath, setSqlitePath] = useState("");
  const [detecting, setDetecting] = useState(false);
  const [saving, setSaving] = useState(false);
  const [isWindows, setIsWindows] = useState(false);

  useEffect(() => {
    // OS判定
    setIsWindows(navigator.userAgent.includes("Windows") || navigator.platform.includes("Win"));

    // 現在の設定を読み込み
    Promise.all([api.getConfig(), api.getCurrentPaths(), api.getIndexStats()])
      .then(([cfg, paths, s]) => {
        setSessionsDir(cfg.sessions_dir ?? "");
        setSqlitePath(cfg.sqlite_db_path ?? "");
        setCurrentPaths(paths);
        setStats(s);
      })
      .catch(console.error);
  }, []);

  const rebuild = async () => {
    setRebuilding(true);
    try {
      await api.rebuildIndex();
      const s = await api.getIndexStats();
      setStats(s);
      toast.success("インデックスを再構築しました");
    } catch (e) {
      toast.error(`エラー: ${e}`);
    } finally {
      setRebuilding(false);
    }
  };

  const handleDetect = async () => {
    setDetecting(true);
    try {
      const result = await api.detectWslPaths();
      if (result.sessions_dir || result.sqlite_db_path) {
        if (result.sessions_dir) setSessionsDir(result.sessions_dir);
        if (result.sqlite_db_path) setSqlitePath(result.sqlite_db_path);
        toast.success(
          result.distro
            ? `WSL2 (${result.distro}) のパスを検出しました`
            : "WSL2 パスを検出しました"
        );
      } else {
        toast.error("WSL2 のデータパスが見つかりませんでした。手動で入力してください。");
      }
    } catch (e) {
      toast.error(`検出エラー: ${e}`);
    } finally {
      setDetecting(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      await api.saveConfig({
        sessions_dir: sessionsDir.trim() || undefined,
        sqlite_db_path: sqlitePath.trim() || undefined,
      });
      // 再読み込み
      const paths = await api.getCurrentPaths();
      setCurrentPaths(paths);
      toast.success("設定を保存しました。インデックスを再構築してください。");
    } catch (e) {
      toast.error(`保存エラー: ${e}`);
    } finally {
      setSaving(false);
    }
  };

  const handleReset = async () => {
    setSessionsDir("");
    setSqlitePath("");
    await api.saveConfig({ sessions_dir: undefined, sqlite_db_path: undefined });
    const paths = await api.getCurrentPaths();
    setCurrentPaths(paths);
    toast.success("デフォルトパスにリセットしました");
  };

  const inputStyle = {
    background: "var(--bg)",
    border: "1px solid var(--border)",
    borderRadius: "6px",
    color: "var(--text-primary)",
    fontSize: "11px",
    fontFamily: "'JetBrains Mono', monospace",
    padding: "6px 10px",
    width: "100%",
    outline: "none",
  };

  return (
    <div className="flex-1 overflow-auto p-6 max-w-2xl">
      <h2
        className="text-lg font-semibold mb-6"
        style={{ fontFamily: "'JetBrains Mono', monospace", color: "var(--accent)" }}
      >
        Settings
      </h2>

      {/* ── Data Source Paths ── */}
      <div
        className="rounded-lg p-4 mb-4"
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
      >
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-medium" style={{ color: "var(--text-secondary)" }}>
            Data Source Paths
          </h3>
          {/* Windows のみ自動検出ボタンを表示 */}
          {isWindows && (
            <button
              onClick={handleDetect}
              disabled={detecting}
              className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded cursor-pointer"
              style={{
                background: "var(--accent)",
                color: "#000",
                opacity: detecting ? 0.6 : 1,
              }}
            >
              <Search size={12} className={detecting ? "animate-pulse" : ""} />
              {detecting ? "検出中..." : "WSL2 パスを自動検出"}
            </button>
          )}
        </div>

        {/* Windows 向け説明 */}
        {isWindows && (
          <div
            className="text-xs rounded p-2 mb-3"
            style={{
              background: "rgba(34,197,94,0.08)",
              border: "1px solid rgba(34,197,94,0.2)",
              color: "var(--text-secondary)",
            }}
          >
            Windows 側でアプリを起動している場合、kiro-cli のデータは WSL2 内にあります。
            「WSL2 パスを自動検出」ボタンで自動設定できます。
          </div>
        )}

        <div className="space-y-3">
          <div>
            <label className="text-xs mb-1 block" style={{ color: "var(--text-muted)" }}>
              Sessions Directory (JSONL)
            </label>
            <input
              value={sessionsDir}
              onChange={(e) => setSessionsDir(e.target.value)}
              placeholder="空白 = デフォルト (~/.kiro/sessions/cli)"
              style={inputStyle}
            />
          </div>
          <div>
            <label className="text-xs mb-1 block" style={{ color: "var(--text-muted)" }}>
              SQLite DB Path
            </label>
            <input
              value={sqlitePath}
              onChange={(e) => setSqlitePath(e.target.value)}
              placeholder="空白 = デフォルト (~/.local/share/kiro-cli/data.sqlite3)"
              style={inputStyle}
            />
          </div>
        </div>

        <div className="flex items-center gap-2 mt-3">
          <button
            onClick={handleSave}
            disabled={saving}
            className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded cursor-pointer"
            style={{ background: "var(--accent)", color: "#000", opacity: saving ? 0.6 : 1 }}
          >
            <Save size={12} />
            {saving ? "保存中..." : "保存"}
          </button>
          <button
            onClick={handleReset}
            className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded cursor-pointer"
            style={{
              background: "var(--surface)",
              border: "1px solid var(--border)",
              color: "var(--text-secondary)",
            }}
          >
            <RotateCcw size={12} />
            デフォルトに戻す
          </button>
        </div>
      </div>

      {/* ── 現在の実際のパス ── */}
      {currentPaths && (
        <div
          className="rounded-lg p-4 mb-4"
          style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
        >
          <h3 className="text-sm font-medium mb-3" style={{ color: "var(--text-secondary)" }}>
            Current Resolved Paths
          </h3>
          <div className="space-y-2">
            {[
              ["Sessions (JSONL)", currentPaths.sessions_dir],
              ["SQLite DB", currentPaths.sqlite_db_path],
              ["Index DB", currentPaths.index_db_path],
            ].map(([label, path]) => (
              <div key={label}>
                <p className="text-xs" style={{ color: "var(--text-muted)" }}>{label}</p>
                <p
                  className="text-xs font-mono break-all"
                  style={{ color: "var(--text-secondary)" }}
                >
                  {path}
                </p>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* ── Index ── */}
      <div
        className="rounded-lg p-4"
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
      >
        <h3 className="text-sm font-medium mb-3" style={{ color: "var(--text-secondary)" }}>
          Index
        </h3>
        {stats && (
          <div className="space-y-1 mb-4">
            <p className="text-xs" style={{ color: "var(--text-muted)" }}>
              Sessions indexed:{" "}
              <span style={{ color: "var(--text-primary)" }}>{stats.session_count}</span>
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
    </div>
  );
}
