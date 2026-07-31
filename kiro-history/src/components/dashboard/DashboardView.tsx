import { useState, useEffect } from "react";
import {
  BarChart, Bar, PieChart, Pie, Cell, XAxis, YAxis,
  Tooltip, ResponsiveContainer, CartesianGrid,
} from "recharts";
import CalendarHeatmap from "react-calendar-heatmap";
import "react-calendar-heatmap/dist/styles.css";
import { api } from "../../api";
import type { StatsData } from "../../types";

const ACCENT = "#22C55E";
const COLORS = ["#22C55E", "#16A34A", "#15803D", "#166534", "#14532D", "#052E16"];

export function DashboardView() {
  const [stats, setStats] = useState<StatsData | null>(null);

  useEffect(() => {
    api.getStats().then(setStats).catch(console.error);
  }, []);

  if (!stats) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p style={{ color: "var(--text-muted)" }}>Loading stats...</p>
      </div>
    );
  }

  const heatmapValues = stats.sessions_by_date.map((d) => ({
    date: d.date,
    count: d.count,
  }));

  const endDate = new Date();
  const startDate = new Date(endDate);
  startDate.setFullYear(startDate.getFullYear() - 1);

  return (
    <div className="flex-1 overflow-auto p-6 space-y-8">
      <h2
        className="text-lg font-semibold"
        style={{ fontFamily: "'JetBrains Mono', monospace", color: "var(--accent)" }}
      >
        Dashboard
      </h2>

      {/* Summary cards */}
      <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
        {[
          { label: "Sessions", value: stats.total_sessions },
          { label: "Messages", value: stats.total_messages },
          {
            label: "Top Model",
            value: stats.sessions_by_model[0]?.model_name
              ?.replace("claude-", "")
              ?.replace("-latest", "") ?? "—",
          },
          {
            label: "Avg Context",
            value: `${stats.avg_context_pct.toFixed(1)}%`,
          },
        ].map(({ label, value }) => (
          <div
            key={label}
            className="rounded-lg p-4"
            style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
          >
            <p className="text-xs mb-1" style={{ color: "var(--text-muted)" }}>
              {label}
            </p>
            <p
              className="text-xl font-semibold"
              style={{ color: "var(--text-primary)", fontFamily: "'JetBrains Mono', monospace" }}
            >
              {value}
            </p>
          </div>
        ))}
      </div>

      {/* Charts row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Model donut */}
        {stats.sessions_by_model.length > 0 && (
          <div
            className="rounded-lg p-4"
            style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
          >
            <h3 className="text-sm font-medium mb-3" style={{ color: "var(--text-secondary)" }}>
              Sessions by Model
            </h3>
            <ResponsiveContainer width="100%" height={200}>
              <PieChart>
                <Pie
                  data={stats.sessions_by_model}
                  dataKey="count"
                  nameKey="model_name"
                  cx="50%"
                  cy="50%"
                  innerRadius={50}
                  outerRadius={80}
                  label={({ model_name, count }) =>
                    `${model_name?.replace("claude-", "")} (${count})`
                  }
                  labelLine={false}
                >
                  {stats.sessions_by_model.map((_, idx) => (
                    <Cell key={idx} fill={COLORS[idx % COLORS.length]} />
                  ))}
                </Pie>
                <Tooltip
                  contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", fontSize: 12 }}
                />
              </PieChart>
            </ResponsiveContainer>
          </div>
        )}

        {/* Top projects */}
        {stats.sessions_by_cwd.length > 0 && (
          <div
            className="rounded-lg p-4"
            style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
          >
            <h3 className="text-sm font-medium mb-3" style={{ color: "var(--text-secondary)" }}>
              Top Projects
            </h3>
            <ResponsiveContainer width="100%" height={200}>
              <BarChart
                data={stats.sessions_by_cwd.slice(0, 8).map((d) => ({
                  ...d,
                  short: d.cwd.split("/").slice(-2).join("/"),
                }))}
                layout="vertical"
              >
                <CartesianGrid strokeDasharray="3 3" stroke="var(--border)" />
                <XAxis type="number" tick={{ fontSize: 11, fill: "var(--text-muted)" }} />
                <YAxis
                  type="category"
                  dataKey="short"
                  width={100}
                  tick={{ fontSize: 11, fill: "var(--text-muted)" }}
                />
                <Tooltip
                  contentStyle={{ background: "var(--surface)", border: "1px solid var(--border)", fontSize: 12 }}
                />
                <Bar dataKey="count" fill={ACCENT} radius={[0, 3, 3, 0]} />
              </BarChart>
            </ResponsiveContainer>
          </div>
        )}
      </div>

      {/* Calendar Heatmap */}
      <div
        className="rounded-lg p-4"
        style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
      >
        <h3 className="text-sm font-medium mb-3" style={{ color: "var(--text-secondary)" }}>
          Activity (Last 12 months)
        </h3>
        <div className="overflow-x-auto">
          <CalendarHeatmap
            startDate={startDate}
            endDate={endDate}
            values={heatmapValues}
            classForValue={(value) => {
              const c = value?.count ?? 0;
              if (!value || c === 0) return "color-empty";
              if (c >= 10) return "color-scale-4";
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
          .react-calendar-heatmap text { fill: var(--text-muted); font-size: 10px; }
          .react-calendar-heatmap rect { rx: 2; }
        `}</style>
      </div>

      {/* Tags ranking */}
      {stats.most_used_tags.length > 0 && (
        <div
          className="rounded-lg p-4"
          style={{ background: "var(--surface)", border: "1px solid var(--border)" }}
        >
          <h3 className="text-sm font-medium mb-3" style={{ color: "var(--text-secondary)" }}>
            Top Tags
          </h3>
          <div className="flex flex-wrap gap-2">
            {stats.most_used_tags.slice(0, 20).map((t) => (
              <span
                key={t.tag}
                className="flex items-center gap-1 text-xs px-2 py-1 rounded-full"
                style={{ background: "var(--border)", color: "var(--text-secondary)" }}
              >
                {t.tag}
                <span style={{ color: "var(--accent)" }}>{t.count}</span>
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
