import { useState, useEffect, useMemo } from "react";
import { ViewTabs, type CalendarView } from "./ViewTabs";
import {
  BarChart, Bar, PieChart, Pie, Cell, XAxis, YAxis,
  Tooltip, ResponsiveContainer, CartesianGrid, RadarChart,
  Radar, PolarGrid, PolarAngleAxis, AreaChart, Area,
} from "recharts";
import CalendarHeatmap from "react-calendar-heatmap";
import "react-calendar-heatmap/dist/styles.css";
import { api } from "../../api";
import type { StatsData } from "../../types";

// ── Design tokens ──────────────────────────────────────────────────────────────
const ACCENT = "#22C55E";

function formatDuration(secs: number): string {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (h > 0) return `${h}時間${m > 0 ? m + "分" : ""}`;
  return `${m}分`;
}
const ACCENT2 = "#16A34A";
const MODEL_COLORS = ["#22C55E", "#60A5FA", "#F59E0B", "#A78BFA", "#F472B6", "#34D399"];
const WEEKDAY_LABELS = ["日", "月", "火", "水", "木", "金", "土"];

// ── Helpers ────────────────────────────────────────────────────────────────────
function fmtDuration(secs: number): string {
  if (secs < 60) return `${secs}秒`;
  if (secs < 3600) return `${Math.round(secs / 60)}分`;
  const h = Math.floor(secs / 3600);
  const m = Math.round((secs % 3600) / 60);
  return `${h}時間${m > 0 ? ` ${m}分` : ""}`;
}

function fmtCost(usd: number): string {
  if (usd < 0.01) return "< $0.01";
  return `$${usd.toFixed(2)}`;
}

function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`;
  return String(n);
}

function shortenCwd(cwd: string): string {
  const home = cwd.match(/^\/home\/[^/]+/)?.[0] ?? "";
  if (home) return cwd.replace(home, "~").split("/").slice(-2).join("/");
  return cwd.split("/").slice(-2).join("/") || cwd;
}

// ── Sub-components ─────────────────────────────────────────────────────────────

function SummaryCard({ label, value, sub, accent }: { label: string; value: string; sub?: string; accent?: boolean }) {
  return (
    <div
      className="rounded-xl p-4 flex flex-col gap-1"
      style={{
        background: accent ? "rgba(34,197,94,0.08)" : "var(--surface)",
        border: `1px solid ${accent ? "rgba(34,197,94,0.3)" : "var(--border)"}`,
      }}
    >
      <p className="text-xs" style={{ color: "var(--text-muted)" }}>{label}</p>
      <p className="text-2xl font-bold" style={{ fontFamily: "'JetBrains Mono', monospace", color: accent ? ACCENT : "var(--text-primary)" }}>
        {value}
      </p>
      {sub && <p className="text-xs" style={{ color: "var(--text-muted)" }}>{sub}</p>}
    </div>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="text-xs font-semibold uppercase tracking-widest mb-3" style={{ color: "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
      {children}
    </h3>
  );
}

function Card({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={`rounded-xl p-4 ${className}`} style={{ background: "var(--surface)", border: "1px solid var(--border)" }}>
      {children}
    </div>
  );
}

const CustomTooltip = ({ active, payload, label }: any) => {
  if (!active || !payload?.length) return null;
  return (
    <div className="rounded-lg px-3 py-2 text-xs shadow-xl" style={{ background: "var(--surface)", border: "1px solid var(--border)", color: "var(--text-primary)" }}>
      <p className="font-medium mb-1">{label}</p>
      {payload.map((p: any, i: number) => (
        <p key={i} style={{ color: p.color ?? ACCENT }}>{p.name}: {p.value}</p>
      ))}
    </div>
  );
};

// ── Main Dashboard ─────────────────────────────────────────────────────────────

export function DashboardView() {
  const [stats, setStats] = useState<StatsData | null>(null);
  const [calendarView, setCalendarView] = useState<CalendarView>("day");
  const [durationView, setDurationView] = useState<CalendarView>("day");

  useEffect(() => {
    api.getStats().then(setStats).catch(console.error);
  }, []);

  // 汎用週次集計（月曜始まり）
  function aggregateByWeek(daily: { date: string; value: number }[]) {
    const map = new Map<string, number>();
    for (const d of daily) {
      const date = new Date(d.date + "T00:00:00");
      const monday = new Date(date);
      monday.setDate(date.getDate() - ((date.getDay() + 6) % 7));
      const key = monday.toISOString().slice(0, 10);
      map.set(key, (map.get(key) ?? 0) + d.value);
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([week, value]) => ({ week, value }));
  }

  // 汎用月次集計
  function aggregateByMonth(daily: { date: string; value: number }[]) {
    const map = new Map<string, number>();
    for (const d of daily) {
      const key = d.date.slice(0, 7);
      map.set(key, (map.get(key) ?? 0) + d.value);
    }
    return [...map.entries()].sort(([a], [b]) => a.localeCompare(b)).map(([month, value]) => ({ month, value }));
  }

  const sessionDailyBase = useMemo(
    () => (stats?.sessions_by_date ?? []).map((d) => ({ date: d.date, value: d.count })),
    [stats]
  );
  const weeklyData = useMemo(() => aggregateByWeek(sessionDailyBase), [sessionDailyBase]);
  const monthlyData = useMemo(() => aggregateByMonth(sessionDailyBase), [sessionDailyBase]);

  const durationDailyBase = useMemo(
    () => (stats?.duration_by_date ?? []).map((d) => ({ date: d.date, value: d.duration_secs })),
    [stats]
  );
  const durationWeekly = useMemo(() => aggregateByWeek(durationDailyBase), [durationDailyBase]);
  const durationMonthly = useMemo(() => aggregateByMonth(durationDailyBase), [durationDailyBase]);

  if (!stats) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="flex items-center gap-3" style={{ color: "var(--text-muted)" }}>
          <div className="w-5 h-5 rounded-full border-2 border-t-transparent animate-spin" style={{ borderColor: ACCENT, borderTopColor: "transparent" }} />
          統計を読み込み中...
        </div>
      </div>
    );
  }

  const endDate = new Date();
  const startDate = new Date(endDate);
  startDate.setFullYear(startDate.getFullYear() - 1);

  const heatmapValues = stats.sessions_by_date.map((d) => ({ date: d.date, count: d.count }));

  // Fill weekday data (ensure all 7 days shown)
  const weekdayData = WEEKDAY_LABELS.map((label, i) => ({
    label,
    count: stats.by_weekday.find((w) => w.weekday === i)?.count ?? 0,
  }));

  // Fill hour data (0-23)
  const hourData = Array.from({ length: 24 }, (_, h) => ({
    hour: `${h}:00`,
    count: stats.by_hour.find((b) => b.hour === h)?.count ?? 0,
  }));

  // Radar data for AI usage
  const aiRadarData = [
    { subject: "ツール使用", A: Math.min(stats.avg_tool_uses_per_session * 10, 100) },
    { subject: "エージェント率", A: stats.agent_session_ratio * 100 },
    { subject: "平均メッセージ", A: Math.min(stats.avg_messages_per_session * 3, 100) },
    { subject: "コンテキスト使用", A: stats.avg_context_pct },
    { subject: "セッション時間", A: Math.min(stats.avg_duration_secs / 3, 100) },
  ];

  const topProject = stats.sessions_by_cwd[0];

  return (
    <div className="flex-1 overflow-auto">
      <div className="p-6 space-y-8 max-w-7xl mx-auto">

        {/* ── Header ─────────────────────────────────────────────────────── */}
        <div>
          <h2 className="text-xl font-bold mb-1" style={{ fontFamily: "'JetBrains Mono', monospace", color: ACCENT }}>
            Dashboard
          </h2>
          <p className="text-xs" style={{ color: "var(--text-muted)" }}>
            kiro-cli 利用状況・コスト・生産性の分析
          </p>
        </div>

        {/* ── Summary Cards ──────────────────────────────────────────────── */}
        <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
          <SummaryCard label="総セッション数" value={String(stats.total_sessions)} sub={`総${stats.total_messages}メッセージ`} />
          <SummaryCard label="総作業時間" value={fmtDuration(stats.total_duration_secs)} sub={`平均 ${fmtDuration(Math.round(stats.avg_duration_secs))}/session`} />
          <SummaryCard label="推定コスト" value={fmtCost(stats.total_est_cost_usd)} sub={`推定 ${fmtTokens(stats.est_tokens_total)} tokens`} accent />
          <SummaryCard label="AIエージェント率" value={`${(stats.agent_session_ratio * 100).toFixed(0)}%`} sub={`ツール使用 ${stats.total_tool_uses}回 / ${stats.total_cycles}ループ`} />
        </div>

        {/* ── Cost Breakdown + Model Usage ───────────────────────────────── */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <Card>
            <SectionTitle>💰 モデル別コスト内訳（推定）</SectionTitle>
            <p className="text-xs mb-3" style={{ color: "var(--text-muted)" }}>
              ※ context_usage%とモデル単価から算出した目安です
            </p>
            {stats.cost_breakdown.length === 0 ? (
              <p className="text-xs" style={{ color: "var(--text-muted)" }}>データなし</p>
            ) : (
              <div className="space-y-2">
                {stats.cost_breakdown.map((cb, i) => {
                  const pct = stats.total_est_cost_usd > 0
                    ? (cb.est_cost_usd / stats.total_est_cost_usd) * 100
                    : 0;
                  return (
                    <div key={i}>
                      <div className="flex items-center justify-between text-xs mb-1">
                        <span style={{ color: "var(--text-secondary)" }}>
                          {cb.model_name.replace("claude-", "")}
                        </span>
                        <div className="flex items-center gap-3">
                          <span style={{ color: "var(--text-muted)" }}>{fmtTokens(cb.est_input_tokens + cb.est_output_tokens)} tokens</span>
                          <span className="font-mono font-semibold" style={{ color: ACCENT }}>{fmtCost(cb.est_cost_usd)}</span>
                        </div>
                      </div>
                      <div className="h-1.5 rounded-full overflow-hidden" style={{ background: "var(--border)" }}>
                        <div className="h-full rounded-full transition-all duration-500"
                          style={{ width: `${pct}%`, background: MODEL_COLORS[i % MODEL_COLORS.length] }} />
                      </div>
                    </div>
                  );
                })}
                <div className="flex items-center justify-between pt-2 text-xs border-t" style={{ borderColor: "var(--border)", color: "var(--text-secondary)" }}>
                  <span>合計</span>
                  <span className="font-mono font-bold" style={{ color: ACCENT }}>{fmtCost(stats.total_est_cost_usd)}</span>
                </div>
              </div>
            )}
          </Card>

          <Card>
            <SectionTitle>🤖 モデル別セッション数</SectionTitle>
            <ResponsiveContainer width="100%" height={200}>
              <PieChart>
                <Pie data={stats.sessions_by_model} dataKey="count" nameKey="model_name"
                  cx="50%" cy="50%" innerRadius={45} outerRadius={75}>
                  {stats.sessions_by_model.map((_, idx) => (
                    <Cell key={idx} fill={MODEL_COLORS[idx % MODEL_COLORS.length]} />
                  ))}
                </Pie>
                <Tooltip contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", fontSize: 12 }}
                  formatter={(v: number, name: string) => [v, name.replace("claude-", "")]} />
              </PieChart>
            </ResponsiveContainer>
            <div className="flex flex-wrap gap-2 mt-1">
              {stats.sessions_by_model.map((m, i) => (
                <div key={i} className="flex items-center gap-1 text-xs">
                  <div className="w-2 h-2 rounded-full" style={{ background: MODEL_COLORS[i % MODEL_COLORS.length] }} />
                  <span style={{ color: "var(--text-muted)" }}>{m.model_name.replace("claude-", "")}</span>
                </div>
              ))}
            </div>
          </Card>
        </div>

        {/* ── Productivity ───────────────────────────────────────────────── */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
          <div className="rounded-xl p-4" style={{ background: "var(--surface)", border: "1px solid var(--border)" }}>
            <SectionTitle>⏱ 生産性メトリクス</SectionTitle>
            <div className="space-y-3">
              {[
                { label: "総作業時間", value: fmtDuration(stats.total_duration_secs) },
                { label: "平均セッション時間", value: fmtDuration(Math.round(stats.avg_duration_secs)) },
                { label: "最長セッション", value: fmtDuration(stats.longest_session_duration) },
                { label: "平均メッセージ数/session", value: stats.avg_messages_per_session.toFixed(1) },
                { label: "ピーク時間帯", value: `${stats.peak_hour}:00 〜 ${stats.peak_hour + 1}:00` },
              ].map(({ label, value }) => (
                <div key={label} className="flex items-center justify-between">
                  <span className="text-xs" style={{ color: "var(--text-muted)" }}>{label}</span>
                  <span className="text-sm font-semibold" style={{ fontFamily: "'JetBrains Mono', monospace", color: "var(--text-primary)" }}>{value}</span>
                </div>
              ))}
            </div>
          </div>

          <Card>
            <SectionTitle>📅 曜日別利用パターン</SectionTitle>
            <ResponsiveContainer width="100%" height={160}>
              <BarChart data={weekdayData} margin={{ top: 0, right: 0, bottom: 0, left: -20 }}>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                <XAxis dataKey="label" tick={{ fontSize: 11, fill: "var(--text-muted)" }} />
                <YAxis tick={{ fontSize: 10, fill: "var(--text-muted)" }} />
                <Tooltip content={<CustomTooltip />} />
                <Bar dataKey="count" name="sessions" fill={ACCENT} radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </Card>

          <Card>
            <SectionTitle>🕐 時間帯別利用パターン</SectionTitle>
            <ResponsiveContainer width="100%" height={160}>
              <AreaChart data={hourData} margin={{ top: 0, right: 0, bottom: 0, left: -20 }}>
                <defs>
                  <linearGradient id="hourGrad" x1="0" y1="0" x2="0" y2="1">
                    <stop offset="5%" stopColor={ACCENT} stopOpacity={0.3} />
                    <stop offset="95%" stopColor={ACCENT} stopOpacity={0} />
                  </linearGradient>
                </defs>
                <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                <XAxis dataKey="hour" tick={{ fontSize: 9, fill: "var(--text-muted)" }}
                  interval={3} />
                <YAxis tick={{ fontSize: 10, fill: "var(--text-muted)" }} />
                <Tooltip content={<CustomTooltip />} />
                <Area type="monotone" dataKey="count" name="sessions"
                  stroke={ACCENT} fill="url(#hourGrad)" strokeWidth={2} />
              </AreaChart>
            </ResponsiveContainer>
          </Card>
        </div>

        {/* ── AI Usage Radar ─────────────────────────────────────────────── */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          <Card>
            <SectionTitle>🧠 AI活用度スコア</SectionTitle>
            <div className="flex items-start gap-6">
              <ResponsiveContainer width="55%" height={200}>
                <RadarChart data={aiRadarData} margin={{ top: 10, right: 10, bottom: 10, left: 10 }}>
                  <PolarGrid stroke="var(--border)" />
                  <PolarAngleAxis dataKey="subject" tick={{ fontSize: 10, fill: "var(--text-muted)" }} />
                  <Radar name="usage" dataKey="A" stroke={ACCENT} fill={ACCENT} fillOpacity={0.2} strokeWidth={2} />
                </RadarChart>
              </ResponsiveContainer>
              <div className="flex-1 space-y-2 pt-4">
                {[
                  { label: "ツール使用/session", value: stats.avg_tool_uses_per_session.toFixed(1) },
                  { label: "エージェントloop/session", value: (stats.total_cycles / Math.max(stats.total_sessions, 1)).toFixed(1) },
                  { label: "エージェント活用率", value: `${(stats.agent_session_ratio * 100).toFixed(0)}%` },
                  { label: "平均Context使用率", value: `${stats.avg_context_pct.toFixed(1)}%` },
                ].map(({ label, value }) => (
                  <div key={label} className="flex items-center justify-between">
                    <span className="text-xs" style={{ color: "var(--text-muted)" }}>{label}</span>
                    <span className="text-sm font-mono font-semibold" style={{ color: ACCENT }}>{value}</span>
                  </div>
                ))}
              </div>
            </div>
          </Card>

          <Card>
            <SectionTitle>📁 プロジェクト別分析 Top10</SectionTitle>
            <div className="space-y-1.5 max-h-52 overflow-auto">
              {stats.sessions_by_cwd.map((c, i) => (
                <div key={i} className="rounded-lg px-3 py-2" style={{ background: "var(--bg)", border: "1px solid var(--border)" }}>
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-medium truncate max-w-40" style={{ color: "var(--text-primary)", fontFamily: "'JetBrains Mono', monospace" }}
                      title={c.cwd}>
                      {shortenCwd(c.cwd)}
                    </span>
                    <span className="text-xs px-1.5 py-0.5 rounded ml-2 flex-shrink-0"
                      style={{ background: "rgba(34,197,94,0.1)", color: ACCENT }}>
                      {c.count}回
                    </span>
                  </div>
                  <div className="flex items-center gap-3 text-xs" style={{ color: "var(--text-muted)" }}>
                    <span>{c.total_messages}msgs</span>
                    <span>{fmtDuration(c.total_duration_secs)}</span>
                    {c.total_tool_uses > 0 && <span>🔧{c.total_tool_uses}</span>}
                  </div>
                </div>
              ))}
            </div>
          </Card>
        </div>

        {/* ── Activity Calendar ────────────────────────────────────────────── */}
        <Card>
          <div className="flex items-center justify-between mb-3">
            <SectionTitle>📊 活動カレンダー</SectionTitle>
            {/* 日/週/月 切り替えタブ */}
              <ViewTabs value={calendarView} onChange={setCalendarView} />
          </div>

          {/* 日ビュー: GitHub風ヒートマップ */}
          {calendarView === "day" && (
            <>
              <div className="overflow-x-auto">
                <CalendarHeatmap
                  startDate={startDate}
                  endDate={endDate}
                  values={heatmapValues}
                  classForValue={(value) => {
                    const c = value?.count ?? 0;
                    if (!value || c === 0) return "color-empty";
                    if (c >= 8) return "color-scale-4";
                    if (c >= 5) return "color-scale-3";
                    if (c >= 3) return "color-scale-2";
                    return "color-scale-1";
                  }}
                  titleForValue={(value) =>
                    value ? `${value.date}: ${value.count ?? 0} session(s)` : "No sessions"
                  }
                />
              </div>
              <style>{`
                .color-empty { fill: var(--border); }
                .color-scale-1 { fill: #166534; }
                .color-scale-2 { fill: #15803D; }
                .color-scale-3 { fill: #16A34A; }
                .color-scale-4 { fill: #22C55E; }
                .react-calendar-heatmap text { fill: var(--text-muted); font-size: 9px; }
                .react-calendar-heatmap rect { rx: 2; }
              `}</style>
              <div className="flex items-center gap-4 mt-3 text-xs" style={{ color: "var(--text-muted)" }}>
                <span>少</span>
                {["#166534","#15803D","#16A34A","#22C55E"].map((c) => (
                  <div key={c} className="w-3 h-3 rounded-sm" style={{ background: c }} />
                ))}
                <span>多</span>
              </div>
            </>
          )}

          {/* 週ビュー: 棒グラフ（recharts） */}
          {calendarView === "week" && (
            <ResponsiveContainer width="100%" height={200}>
              <BarChart data={weeklyData.map((d) => ({ ...d, count: d.value }))} margin={{ top: 4, right: 8, bottom: 0, left: -20 }}>
                <XAxis
                  dataKey="week"
                  tick={{ fontSize: 10, fill: "var(--text-muted)" }}
                  tickFormatter={(w: string) => w.slice(5)}
                />
                <YAxis tick={{ fontSize: 10, fill: "var(--text-muted)" }} allowDecimals={false} />
                <Tooltip
                  contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", fontSize: 12 }}
                  formatter={(v: number) => [v, "セッション数"]}
                  labelFormatter={(w: string) => `週: ${w}`}
                />
                <Bar dataKey="count" name="セッション数" fill={ACCENT} radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          )}

          {/* 月ビュー: 棒グラフ（recharts） */}
          {calendarView === "month" && (
            <ResponsiveContainer width="100%" height={200}>
              <BarChart data={monthlyData.map((d) => ({ ...d, count: d.value }))} margin={{ top: 4, right: 8, bottom: 0, left: -20 }}>
                <XAxis
                  dataKey="month"
                  tick={{ fontSize: 10, fill: "var(--text-muted)" }}
                  tickFormatter={(m: string) => {
                    const [y, mo] = m.split("-");
                    return `${y.slice(2)}/${mo}`;
                  }}
                />
                <YAxis tick={{ fontSize: 10, fill: "var(--text-muted)" }} allowDecimals={false} />
                <Tooltip
                  contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", fontSize: 12 }}
                  formatter={(v: number) => [v, "セッション数"]}
                  labelFormatter={(m: string) => {
                    const [y, mo] = m.split("-");
                    return `${y}年${parseInt(mo)}月`;
                  }}
                />
                <Bar dataKey="count" name="セッション数" fill={ACCENT} radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          )}
        </Card>

        {/* ── Duration Calendar ───────────────────────────────────────────── */}
        <Card>
          <div className="flex items-center justify-between mb-3">
            <SectionTitle>⏱️ 総作業時間</SectionTitle>
            <ViewTabs value={durationView} onChange={setDurationView} />
          </div>

          {/* 日ビュー: ヒートマップ（秒単位のしきい値） */}
          {durationView === "day" && (
            <>
              <div className="overflow-x-auto">
                <CalendarHeatmap
                  startDate={startDate}
                  endDate={endDate}
                  values={(stats.duration_by_date ?? []).map((d) => ({ date: d.date, count: d.duration_secs }))}
                  classForValue={(value) => {
                    const s = value?.count ?? 0;
                    if (!value || s === 0) return "color-empty";
                    if (s >= 3600 * 3) return "color-scale-4"; // 3時間以上
                    if (s >= 3600)      return "color-scale-3"; // 1時間以上
                    if (s >= 1800)      return "color-scale-2"; // 30分以上
                    return "color-scale-1";
                  }}
                  titleForValue={(value) =>
                    value ? `${value.date}: ${formatDuration(value.count ?? 0)}` : "活動なし"
                  }
                />
              </div>
              <div className="flex items-center gap-4 mt-3 text-xs" style={{ color: "var(--text-muted)" }}>
                <span>少</span>
                {["#166534","#15803D","#16A34A","#22C55E"].map((c) => (
                  <div key={c} className="w-3 h-3 rounded-sm" style={{ background: c }} />
                ))}
                <span>多</span>
              </div>
            </>
          )}

          {/* 週ビュー */}
          {durationView === "week" && (
            <ResponsiveContainer width="100%" height={200}>
              <BarChart data={durationWeekly} margin={{ top: 4, right: 8, bottom: 0, left: 0 }}>
                <XAxis
                  dataKey="week"
                  tick={{ fontSize: 10, fill: "var(--text-muted)" }}
                  tickFormatter={(w: string) => w.slice(5)}
                />
                <YAxis
                  tick={{ fontSize: 10, fill: "var(--text-muted)" }}
                  tickFormatter={(v: number) => v >= 3600 ? `${Math.round(v / 3600)}h` : `${Math.round(v / 60)}m`}
                />
                <Tooltip
                  contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", fontSize: 12 }}
                  formatter={(v: number) => [formatDuration(v), "作業時間"]}
                  labelFormatter={(w: string) => `週: ${w}`}
                />
                <Bar dataKey="value" name="作業時間" fill={ACCENT} radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          )}

          {/* 月ビュー */}
          {durationView === "month" && (
            <ResponsiveContainer width="100%" height={200}>
              <BarChart data={durationMonthly} margin={{ top: 4, right: 8, bottom: 0, left: 0 }}>
                <XAxis
                  dataKey="month"
                  tick={{ fontSize: 10, fill: "var(--text-muted)" }}
                  tickFormatter={(m: string) => {
                    const [y, mo] = m.split("-");
                    return `${y.slice(2)}/${mo}`;
                  }}
                />
                <YAxis
                  tick={{ fontSize: 10, fill: "var(--text-muted)" }}
                  tickFormatter={(v: number) => v >= 3600 ? `${Math.round(v / 3600)}h` : `${Math.round(v / 60)}m`}
                />
                <Tooltip
                  contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", fontSize: 12 }}
                  formatter={(v: number) => [formatDuration(v), "作業時間"]}
                  labelFormatter={(m: string) => {
                    const [y, mo] = m.split("-");
                    return `${y}年${parseInt(mo)}月`;
                  }}
                />
                <Bar dataKey="value" name="作業時間" fill={ACCENT2} radius={[3, 3, 0, 0]} />
              </BarChart>
            </ResponsiveContainer>
          )}
        </Card>

        {/* ── Tags ───────────────────────────────────────────────────────── */}
        {stats.most_used_tags.length > 0 && (
          <Card>
            <SectionTitle>🏷 タグランキング</SectionTitle>
            <div className="flex flex-wrap gap-2">
              {stats.most_used_tags.slice(0, 30).map((t, i) => (
                <span key={t.tag} className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-full"
                  style={{
                    background: i === 0 ? "rgba(34,197,94,0.15)" : "var(--border)",
                    color: i === 0 ? ACCENT : "var(--text-secondary)",
                    border: `1px solid ${i === 0 ? "rgba(34,197,94,0.3)" : "transparent"}`,
                    fontWeight: i < 3 ? 600 : 400,
                  }}>
                  {t.tag}
                  <span style={{ color: i === 0 ? ACCENT : "var(--text-muted)", fontFamily: "'JetBrains Mono', monospace" }}>
                    {t.count}
                  </span>
                </span>
              ))}
            </div>
          </Card>
        )}

      </div>
    </div>
  );
}
