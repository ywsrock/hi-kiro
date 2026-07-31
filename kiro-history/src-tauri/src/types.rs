use serde::{Deserialize, Serialize};

// ── Search / Filter ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct FilterParams {
    pub date_from: Option<i64>,
    pub date_to: Option<i64>,
    pub model_name: Option<String>,
    pub tags: Option<Vec<String>>,
    pub starred_only: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionSummary {
    pub session_id: String,
    pub title: String,
    pub cwd: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub message_count: i64,
    pub source: String,
    pub model_name: Option<String>,
    pub max_context_pct: Option<f32>,
    pub total_tool_uses: i64,
    pub total_cycles: i64,
    pub total_duration_secs: i64,
    pub starred: bool,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionDetail {
    pub summary: SessionSummary,
    pub messages: Vec<MessageDto>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageDto {
    pub role: String,
    pub content: String,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexStats {
    pub session_count: i64,
    pub last_indexed_at: i64,
}

// ── Tags ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct TagStat {
    pub tag: String,
    pub count: i64,
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelCount {
    pub model_name: String,
    pub count: i64,
    pub est_cost_usd: f64,
    pub total_duration_secs: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CwdCount {
    pub cwd: String,
    pub count: i64,
    pub total_messages: i64,
    pub total_duration_secs: i64,
    pub total_tool_uses: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DateCount {
    pub date: String, // "YYYY-MM-DD"
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HourCount {
    pub hour: u8,
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WeekdayCount {
    pub weekday: u8, // 0=Sun ... 6=Sat
    pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub model_name: String,
    pub est_input_tokens: i64,
    pub est_output_tokens: i64,
    pub est_cost_usd: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsData {
    // ── Core ──────────────────────────────────────
    pub total_sessions: i64,
    pub total_messages: i64,
    pub sessions_by_model: Vec<ModelCount>,
    pub sessions_by_cwd: Vec<CwdCount>,
    pub sessions_by_date: Vec<DateCount>,
    pub avg_context_pct: f32,
    pub most_used_tags: Vec<TagStat>,

    // ── Productivity ──────────────────────────────
    pub total_duration_secs: i64,       // total working time
    pub avg_duration_secs: f64,         // avg session length
    pub longest_session_duration: i64,  // single longest session
    pub avg_messages_per_session: f64,

    // ── AI Usage ──────────────────────────────────
    pub total_tool_uses: i64,
    pub total_cycles: i64,              // agentic loop count
    pub avg_tool_uses_per_session: f64,
    pub agent_session_ratio: f64,       // sessions with cycles > 0

    // ── Time Patterns ────────────────────────────
    pub by_hour: Vec<HourCount>,        // 0-23 usage distribution
    pub by_weekday: Vec<WeekdayCount>,  // 0-6 usage distribution
    pub peak_hour: u8,                  // most active hour

    // ── Cost Estimate ────────────────────────────
    pub cost_breakdown: Vec<CostBreakdown>,
    pub total_est_cost_usd: f64,
    pub est_tokens_total: i64,
}

// ── Snippets ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct CodeSnippet {
    pub language: String,
    pub code: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodeSnippetWithSession {
    pub session_id: String,
    pub session_title: String,
    pub language: String,
    pub code: String,
}

// ── File refs ─────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    pub exists: bool,
}

// ── Diff ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffResult {
    pub session_id_a: String,
    pub session_id_b: String,
    pub diff: String,
}

// ── Export ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Markdown,
    Html,
    Pdf,
}
