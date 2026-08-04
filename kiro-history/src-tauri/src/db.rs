use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json;

use crate::types::*;

// ── Search ────────────────────────────────────────────────────────────────────

pub fn search_sessions(
    conn: &Connection,
    query: &str,
    limit: u32,
    filters: &FilterParams,
) -> Result<Vec<SessionSummary>> {
    // Build WHERE clauses
    let mut conditions: Vec<String> = Vec::new();
    let mut bind_values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    // FTS match (if query non-empty)
    let use_fts = !query.trim().is_empty();
    if use_fts {
        conditions.push(
            "m.session_id IN (SELECT session_id FROM sessions_fts WHERE sessions_fts MATCH ?1)"
                .to_string(),
        );
        bind_values.push(Box::new(format!("\"{}\"*", query.replace('"', "\"\""))));
    }

    // Date filters
    if let Some(from) = filters.date_from {
        let idx = bind_values.len() + 1;
        conditions.push(format!("m.updated_at >= ?{}", idx));
        bind_values.push(Box::new(from));
    }
    if let Some(to) = filters.date_to {
        let idx = bind_values.len() + 1;
        conditions.push(format!("m.updated_at <= ?{}", idx));
        bind_values.push(Box::new(to));
    }

    // Model filter
    if let Some(ref model) = filters.model_name {
        let idx = bind_values.len() + 1;
        conditions.push(format!("m.model_name = ?{}", idx));
        bind_values.push(Box::new(model.clone()));
    }

    // Starred filter
    if filters.starred_only.unwrap_or(false) {
        conditions.push("u.starred = 1".to_string());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred, 0), COALESCE(u.tags, '[]')
         FROM sessions_meta m
         LEFT JOIN user_data u ON m.session_id = u.session_id
         {where_clause}
         ORDER BY m.updated_at DESC
         LIMIT {limit}",
    );

    let mut stmt = conn.prepare(&sql)?;

    // Execute with dynamic params
    let refs: Vec<&dyn rusqlite::ToSql> = bind_values.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_summary)?;

    let mut results: Vec<SessionSummary> = rows.flatten().collect();

    // Tags filter (post-filter since tags are JSON array in SQLite)
    if let Some(ref tag_filter) = filters.tags {
        if !tag_filter.is_empty() {
            results.retain(|s| tag_filter.iter().all(|t| s.tags.contains(t)));
        }
    }

    Ok(results)
}

fn row_to_summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionSummary> {
    let tags_json: String = row.get(13)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    Ok(SessionSummary {
        session_id: row.get(0)?,
        title: row.get(1)?,
        cwd: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        message_count: row.get(5)?,
        source: row.get(6)?,
        model_name: row.get(7)?,
        max_context_pct: row.get(8)?,
        total_tool_uses: row.get(9)?,
        total_cycles: row.get(10)?,
        total_duration_secs: row.get(11)?,
        starred: row.get::<_, i32>(12)? != 0,
        tags,
    })
}

// ── Related sessions ──────────────────────────────────────────────────────────

pub fn get_related_sessions(
    conn: &Connection,
    cwd: &str,
    exclude_id: &str,
    limit: u32,
) -> Result<Vec<SessionSummary>> {
    let mut stmt = conn.prepare(
        "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred, 0), COALESCE(u.tags, '[]')
         FROM sessions_meta m
         LEFT JOIN user_data u ON m.session_id = u.session_id
         WHERE m.cwd = ?1 AND m.session_id != ?2
         ORDER BY m.updated_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![cwd, exclude_id, limit], row_to_summary)?;
    Ok(rows.flatten().collect())
}

// ── Index stats ───────────────────────────────────────────────────────────────

pub fn get_index_stats(conn: &Connection) -> IndexStats {
    let (session_count, last_indexed_at) = crate::index::get_index_stats(conn);
    IndexStats {
        session_count,
        last_indexed_at,
    }
}

// ── Bookmarks / Tags ──────────────────────────────────────────────────────────

pub fn toggle_bookmark(conn: &Connection, session_id: &str) -> Result<bool> {
    conn.execute(
        "INSERT OR IGNORE INTO user_data (session_id) VALUES (?1)",
        params![session_id],
    )?;
    let current: i32 = conn.query_row(
        "SELECT starred FROM user_data WHERE session_id = ?1",
        params![session_id],
        |r| r.get(0),
    )?;
    let new_val = if current == 0 { 1 } else { 0 };
    conn.execute(
        "UPDATE user_data SET starred = ?1 WHERE session_id = ?2",
        params![new_val, session_id],
    )?;
    Ok(new_val != 0)
}

pub fn set_tags(conn: &Connection, session_id: &str, tags: Vec<String>) -> Result<()> {
    // Validate
    let tags: Vec<String> = tags
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty() && t.len() <= 20)
        .take(10)
        .collect();

    let tags_json = serde_json::to_string(&tags)?;
    conn.execute(
        "INSERT INTO user_data (session_id, tags) VALUES (?1, ?2)
         ON CONFLICT(session_id) DO UPDATE SET tags = ?2",
        params![session_id, tags_json],
    )?;
    Ok(())
}

pub fn get_all_tags(conn: &Connection) -> Result<Vec<TagStat>> {
    // Parse tags from all user_data rows and aggregate counts
    let mut stmt = conn.prepare("SELECT tags FROM user_data WHERE tags != '[]'")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;

    let mut tag_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();

    for row in rows.flatten() {
        let tags: Vec<String> = serde_json::from_str(&row).unwrap_or_default();
        for tag in tags {
            *tag_counts.entry(tag).or_insert(0) += 1;
        }
    }

    let mut stats: Vec<TagStat> = tag_counts
        .into_iter()
        .map(|(tag, count)| TagStat { tag, count })
        .collect();
    stats.sort_by(|a, b| b.count.cmp(&a.count));
    Ok(stats)
}

pub fn get_bookmarked_sessions(conn: &Connection) -> Result<Vec<SessionSummary>> {
    let mut stmt = conn.prepare(
        "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                u.starred, u.tags
         FROM sessions_meta m
         JOIN user_data u ON m.session_id = u.session_id
         WHERE u.starred = 1
         ORDER BY m.updated_at DESC",
    )?;
    let rows = stmt.query_map([], row_to_summary)?;
    Ok(rows.flatten().collect())
}

// ── Rename ────────────────────────────────────────────────────────────────────

pub fn rename_session(conn: &Connection, session_id: &str, new_title: &str) -> Result<()> {
    let new_title = new_title.trim();
    if new_title.is_empty() {
        return Err(anyhow::anyhow!("タイトルを入力してください"));
    }
    // Update sessions_meta
    conn.execute(
        "UPDATE sessions_meta SET title = ?1 WHERE session_id = ?2",
        params![new_title, session_id],
    )?;
    // Update FTS index: delete old entry and re-insert with new title
    // (FTS5 doesn't support UPDATE on content, so delete + insert)
    let (cwd, full_text): (String, String) = conn.query_row(
        "SELECT cwd, '' FROM sessions_meta WHERE session_id = ?1",
        params![session_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    conn.execute(
        "DELETE FROM sessions_fts WHERE session_id = ?1",
        params![session_id],
    )?;
    conn.execute(
        "INSERT INTO sessions_fts (session_id, title, cwd, full_text) VALUES (?1, ?2, ?3, ?4)",
        params![session_id, new_title, cwd, new_title],
    )?;
    Ok(())
}

// ── Delete ────────────────────────────────────────────────────────────────────

pub fn delete_session(conn: &Connection, session_id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();

    // Add to blacklist so it won't be re-indexed
    conn.execute(
        "INSERT OR REPLACE INTO deleted_sessions (session_id, deleted_at) VALUES (?1, ?2)",
        params![session_id, now],
    )?;

    // Remove from FTS index
    conn.execute(
        "DELETE FROM sessions_fts WHERE session_id = ?1",
        params![session_id],
    )?;

    // Remove from meta
    conn.execute(
        "DELETE FROM sessions_meta WHERE session_id = ?1",
        params![session_id],
    )?;

    // Remove from user_data
    conn.execute(
        "DELETE FROM user_data WHERE session_id = ?1",
        params![session_id],
    )?;

    Ok(())
}

pub fn restore_session(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM deleted_sessions WHERE session_id = ?1",
        params![session_id],
    )?;
    Ok(())
}

pub fn get_stats(conn: &Connection) -> Result<StatsData> {
    // ── Core ──────────────────────────────────────────────────────────────────
    let total_sessions: i64 =
        conn.query_row("SELECT COUNT(*) FROM sessions_meta", [], |r| r.get(0))?;

    let total_messages: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(message_count),0) FROM sessions_meta",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    // Sessions by model (with duration)
    let mut stmt = conn.prepare(
        "SELECT COALESCE(model_name,'unknown') as mn, COUNT(*) as cnt,
                COALESCE(SUM(total_duration_secs),0) as dur
         FROM sessions_meta GROUP BY mn ORDER BY cnt DESC",
    )?;
    let sessions_by_model_raw: Vec<(String, i64, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .flatten()
        .collect();

    // Build cost breakdown per model
    let cost_breakdown: Vec<crate::types::CostBreakdown> = sessions_by_model_raw
        .iter()
        .map(|(model, _cnt, dur)| {
            let (input_price, output_price, context_window) = model_pricing(model);
            // Estimate: avg context 15%, 70% input / 30% output split
            let avg_ctx_pct = 0.15f64;
            let est_total_tokens = (avg_ctx_pct * context_window as f64) as i64 * _cnt;
            let est_input = (est_total_tokens as f64 * 0.7) as i64;
            let est_output = (est_total_tokens as f64 * 0.3) as i64;
            let cost = (est_input as f64 / 1_000_000.0) * input_price
                + (est_output as f64 / 1_000_000.0) * output_price;
            crate::types::CostBreakdown {
                model_name: model.clone(),
                est_input_tokens: est_input,
                est_output_tokens: est_output,
                est_cost_usd: (cost * 100.0).round() / 100.0,
            }
        })
        .collect();

    let total_est_cost_usd = cost_breakdown.iter().map(|c| c.est_cost_usd).sum::<f64>();
    let est_tokens_total = cost_breakdown
        .iter()
        .map(|c| c.est_input_tokens + c.est_output_tokens)
        .sum::<i64>();

    let sessions_by_model: Vec<crate::types::ModelCount> = sessions_by_model_raw
        .iter()
        .zip(cost_breakdown.iter())
        .map(|((mn, cnt, dur), cb)| crate::types::ModelCount {
            model_name: mn.clone(),
            count: *cnt,
            est_cost_usd: cb.est_cost_usd,
            total_duration_secs: *dur,
        })
        .collect();

    // Sessions by cwd (top 10, with extra stats)
    let mut stmt = conn.prepare(
        "SELECT cwd, COUNT(*) as cnt,
                COALESCE(SUM(message_count),0) as msgs,
                COALESCE(SUM(total_duration_secs),0) as dur,
                COALESCE(SUM(total_tool_uses),0) as tools
         FROM sessions_meta GROUP BY cwd ORDER BY cnt DESC LIMIT 10",
    )?;
    let sessions_by_cwd: Vec<crate::types::CwdCount> = stmt
        .query_map([], |r| {
            Ok(crate::types::CwdCount {
                cwd: r.get(0)?,
                count: r.get(1)?,
                total_messages: r.get(2)?,
                total_duration_secs: r.get(3)?,
                total_tool_uses: r.get(4)?,
            })
        })?
        .flatten()
        .collect();

    // Sessions by date (daily, last 365 days)
    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m-%d', datetime(updated_at/1000, 'unixepoch')) as d,
                COUNT(*) as cnt
         FROM sessions_meta WHERE updated_at > (strftime('%s','now') - 365*86400) * 1000
         GROUP BY d ORDER BY d",
    )?;
    let sessions_by_date: Vec<crate::types::DateCount> = stmt
        .query_map([], |r| {
            Ok(crate::types::DateCount {
                date: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .flatten()
        .collect();

    // By hour (local time via unixepoch)
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%H', datetime(updated_at/1000, 'unixepoch')) AS INTEGER) as h,
                COUNT(*) as cnt
         FROM sessions_meta WHERE updated_at > 0
         GROUP BY h ORDER BY h",
    )?;
    let by_hour: Vec<crate::types::HourCount> = stmt
        .query_map([], |r| {
            Ok(crate::types::HourCount {
                hour: r.get::<_, u8>(0)?,
                count: r.get(1)?,
            })
        })?
        .flatten()
        .collect();

    let peak_hour = by_hour
        .iter()
        .max_by_key(|h| h.count)
        .map(|h| h.hour)
        .unwrap_or(0);

    // By weekday (0=Sun in strftime)
    let mut stmt = conn.prepare(
        "SELECT CAST(strftime('%w', datetime(updated_at/1000, 'unixepoch')) AS INTEGER) as wd,
                COUNT(*) as cnt
         FROM sessions_meta WHERE updated_at > 0
         GROUP BY wd ORDER BY wd",
    )?;
    let by_weekday: Vec<crate::types::WeekdayCount> = stmt
        .query_map([], |r| {
            Ok(crate::types::WeekdayCount {
                weekday: r.get::<_, u8>(0)?,
                count: r.get(1)?,
            })
        })?
        .flatten()
        .collect();

    // ── Productivity ──────────────────────────────────────────────────────────
    let (total_duration_secs, avg_duration_secs, longest_session_duration): (i64, f64, i64) = conn
        .query_row(
            "SELECT COALESCE(SUM(total_duration_secs),0), COALESCE(AVG(total_duration_secs),0), COALESCE(MAX(total_duration_secs),0) FROM sessions_meta",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;

    let avg_messages_per_session = if total_sessions > 0 {
        total_messages as f64 / total_sessions as f64
    } else {
        0.0
    };

    // ── AI Usage ──────────────────────────────────────────────────────────────
    let (total_tool_uses, total_cycles): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(total_tool_uses),0), COALESCE(SUM(total_cycles),0) FROM sessions_meta",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    let avg_tool_uses_per_session = if total_sessions > 0 {
        total_tool_uses as f64 / total_sessions as f64
    } else {
        0.0
    };

    let agent_sessions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sessions_meta WHERE total_cycles > 0",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let agent_session_ratio = if total_sessions > 0 {
        agent_sessions as f64 / total_sessions as f64
    } else {
        0.0
    };

    // ── Context & Tags ────────────────────────────────────────────────────────
    let avg_context_pct: f32 = conn
        .query_row(
            "SELECT AVG(max_context_pct) FROM sessions_meta WHERE max_context_pct IS NOT NULL",
            [],
            |r| r.get::<_, Option<f64>>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(0.0) as f32;

    let most_used_tags = get_all_tags(conn).unwrap_or_default();

    Ok(StatsData {
        total_sessions,
        total_messages,
        sessions_by_model,
        sessions_by_cwd,
        sessions_by_date,
        avg_context_pct,
        most_used_tags,
        total_duration_secs,
        avg_duration_secs,
        longest_session_duration,
        avg_messages_per_session,
        total_tool_uses,
        total_cycles,
        avg_tool_uses_per_session,
        agent_session_ratio,
        by_hour,
        by_weekday,
        peak_hour,
        cost_breakdown,
        total_est_cost_usd: (total_est_cost_usd * 100.0).round() / 100.0,
        est_tokens_total,
    })
}

/// Return (input_price_per_mtok, output_price_per_mtok, context_window) for a model.
fn model_pricing(model: &str) -> (f64, f64, u64) {
    let m = model.to_lowercase();
    if m.contains("opus") {
        (15.0, 75.0, 200_000)
    } else if m.contains("haiku") {
        (0.8, 4.0, 200_000)
    } else if m.contains("sonnet") {
        (3.0, 15.0, 200_000)
    } else {
        (3.0, 15.0, 200_000) // default to sonnet pricing
    }
}

// ── Tag Metadata CRUD ─────────────────────────────────────────────────────────

pub fn get_tag_metadata(conn: &Connection) -> Result<Vec<crate::types::TagMeta>> {
    // Get all tags from user_data + join with tag_metadata for enriched info
    let all_tags = get_all_tags(conn)?;

    let mut result: Vec<crate::types::TagMeta> = Vec::new();
    for ts in &all_tags {
        let meta: Option<(String, String, i64, i64, i64)> = conn
            .query_row(
                "SELECT color, description, sort_order, created_at, is_smart
                 FROM tag_metadata WHERE tag = ?1",
                params![ts.tag],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .ok();

        let (rule_type, rule_value) = if meta.as_ref().map(|m| m.4).unwrap_or(0) == 1 {
            let rule: Option<(String, String)> = conn
                .query_row(
                    "SELECT rule_type, rule_value FROM smart_tag_rules WHERE tag = ?1",
                    params![ts.tag],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            (
                rule.as_ref().map(|r| r.0.clone()),
                rule.as_ref().map(|r| r.1.clone()),
            )
        } else {
            (None, None)
        };

        result.push(crate::types::TagMeta {
            tag: ts.tag.clone(),
            color: meta
                .as_ref()
                .map(|m| m.0.clone())
                .unwrap_or_else(|| "#334155".to_string()),
            description: meta.as_ref().map(|m| m.1.clone()).unwrap_or_default(),
            sort_order: meta.as_ref().map(|m| m.2).unwrap_or(0),
            created_at: meta.as_ref().map(|m| m.3).unwrap_or(0),
            is_smart: meta.as_ref().map(|m| m.4).unwrap_or(0) == 1,
            count: ts.count,
            rule_type,
            rule_value,
        });
    }

    // Sort by sort_order, then count desc
    result.sort_by(|a, b| {
        if a.sort_order != b.sort_order {
            a.sort_order.cmp(&b.sort_order)
        } else {
            b.count.cmp(&a.count)
        }
    });

    Ok(result)
}

pub fn create_tag(conn: &Connection, params: &crate::types::CreateTagParams) -> Result<()> {
    let tag = params.tag.trim().trim_start_matches('#').to_string();
    let tag = format!("#{}", tag);
    if tag.len() < 2 || tag.len() > 30 {
        return Err(anyhow::anyhow!("タグ名は1〜29文字で入力してください"));
    }
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR IGNORE INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
         VALUES (?1, ?2, ?3, 0, ?4, 0)",
        rusqlite::params![tag, params.color, params.description, now],
    )?;
    Ok(())
}

pub fn update_tag(conn: &Connection, tag: &str, color: &str, description: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
         VALUES (?1, ?2, ?3, 0, ?4, 0)
         ON CONFLICT(tag) DO UPDATE SET color = ?2, description = ?3",
        rusqlite::params![
            tag,
            color,
            description,
            chrono::Utc::now().timestamp_millis()
        ],
    )?;
    Ok(())
}

pub fn delete_tag_full(conn: &Connection, tag: &str) -> Result<usize> {
    // Remove from all sessions
    let mut stmt = conn.prepare("SELECT session_id, tags FROM user_data WHERE tags LIKE ?1")?;
    let pattern = format!("%{}%", tag);
    let rows: Vec<(String, String)> = stmt
        .query_map(rusqlite::params![pattern], |r| Ok((r.get(0)?, r.get(1)?)))?
        .flatten()
        .collect();

    let mut affected = 0usize;
    for (session_id, tags_json) in rows {
        let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        if tags.contains(&tag.to_string()) {
            tags.retain(|t| t != tag);
            let new_json = serde_json::to_string(&tags)?;
            conn.execute(
                "UPDATE user_data SET tags = ?1 WHERE session_id = ?2",
                rusqlite::params![new_json, session_id],
            )?;
            affected += 1;
        }
    }

    // Remove metadata
    conn.execute(
        "DELETE FROM smart_tag_rules WHERE tag = ?1",
        rusqlite::params![tag],
    )?;
    conn.execute(
        "DELETE FROM tag_metadata WHERE tag = ?1",
        rusqlite::params![tag],
    )?;
    Ok(affected)
}

pub fn rename_tag(conn: &Connection, old_tag: &str, new_tag: &str) -> Result<usize> {
    let new_tag = new_tag.trim().trim_start_matches('#');
    let new_tag = format!("#{}", new_tag);

    // Update all sessions
    let mut stmt = conn.prepare("SELECT session_id, tags FROM user_data WHERE tags LIKE ?1")?;
    let pattern = format!("%{}%", old_tag);
    let rows: Vec<(String, String)> = stmt
        .query_map(rusqlite::params![pattern], |r| Ok((r.get(0)?, r.get(1)?)))?
        .flatten()
        .collect();

    let mut affected = 0usize;
    for (session_id, tags_json) in rows {
        let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        if tags.contains(&old_tag.to_string()) {
            tags = tags
                .into_iter()
                .map(|t| if t == old_tag { new_tag.clone() } else { t })
                .collect();
            // Dedup in case new_tag already exists
            tags.dedup();
            let new_json = serde_json::to_string(&tags)?;
            conn.execute(
                "UPDATE user_data SET tags = ?1 WHERE session_id = ?2",
                rusqlite::params![new_json, session_id],
            )?;
            affected += 1;
        }
    }

    // Update metadata
    conn.execute(
        "INSERT INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
         SELECT ?2, color, description, sort_order, created_at, is_smart FROM tag_metadata WHERE tag = ?1
         ON CONFLICT(tag) DO NOTHING",
        rusqlite::params![old_tag, new_tag],
    )?;
    conn.execute(
        "DELETE FROM tag_metadata WHERE tag = ?1",
        rusqlite::params![old_tag],
    )?;
    Ok(affected)
}

pub fn merge_tags(conn: &Connection, from_tag: &str, to_tag: &str) -> Result<usize> {
    // Add to_tag to all sessions that have from_tag
    let mut stmt = conn.prepare("SELECT session_id, tags FROM user_data WHERE tags LIKE ?1")?;
    let pattern = format!("%{}%", from_tag);
    let rows: Vec<(String, String)> = stmt
        .query_map(rusqlite::params![pattern], |r| Ok((r.get(0)?, r.get(1)?)))?
        .flatten()
        .collect();

    let mut affected = 0usize;
    for (session_id, tags_json) in rows {
        let mut tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
        if tags.contains(&from_tag.to_string()) {
            tags.retain(|t| t != from_tag);
            if !tags.contains(&to_tag.to_string()) {
                tags.push(to_tag.to_string());
            }
            let new_json = serde_json::to_string(&tags)?;
            conn.execute(
                "UPDATE user_data SET tags = ?1 WHERE session_id = ?2",
                rusqlite::params![new_json, session_id],
            )?;
            affected += 1;
        }
    }

    // Delete merged tag
    conn.execute(
        "DELETE FROM smart_tag_rules WHERE tag = ?1",
        rusqlite::params![from_tag],
    )?;
    conn.execute(
        "DELETE FROM tag_metadata WHERE tag = ?1",
        rusqlite::params![from_tag],
    )?;
    Ok(affected)
}

pub fn set_tag_order(conn: &Connection, tags: &[String]) -> Result<()> {
    for (i, tag) in tags.iter().enumerate() {
        conn.execute(
            "INSERT INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
             VALUES (?1, '#334155', '', ?2, ?3, 0)
             ON CONFLICT(tag) DO UPDATE SET sort_order = ?2",
            rusqlite::params![tag, i as i64, chrono::Utc::now().timestamp_millis()],
        )?;
    }
    Ok(())
}

// ── Smart Tags ────────────────────────────────────────────────────────────────

pub fn create_smart_tag(
    conn: &Connection,
    rule: &crate::types::SmartTagRule,
    color: &str,
    description: &str,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR REPLACE INTO tag_metadata (tag, color, description, sort_order, created_at, is_smart)
         VALUES (?1, ?2, ?3, 0, ?4, 1)",
        rusqlite::params![rule.tag, color, description, now],
    )?;
    conn.execute(
        "INSERT OR REPLACE INTO smart_tag_rules (tag, rule_type, rule_value) VALUES (?1, ?2, ?3)",
        rusqlite::params![rule.tag, rule.rule_type, rule.rule_value],
    )?;
    Ok(())
}

pub fn get_sessions_by_tag(
    conn: &Connection,
    tags: &[String],
    mode: &str,
) -> Result<Vec<SessionSummary>> {
    if tags.is_empty() {
        // Return untagged sessions
        let mut stmt = conn.prepare(
            "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                    m.message_count, m.source, m.model_name, m.max_context_pct,
                    m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                    COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m
             LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE COALESCE(u.tags,'[]') = '[]'
             ORDER BY m.updated_at DESC LIMIT 500",
        )?;
        let rows = stmt.query_map([], row_to_summary)?;
        return Ok(rows.flatten().collect());
    }

    // Load all, then filter by AND/OR logic
    let mut stmt = conn.prepare(
        "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
         FROM sessions_meta m
         LEFT JOIN user_data u ON m.session_id = u.session_id
         ORDER BY m.updated_at DESC LIMIT 500",
    )?;
    let rows = stmt.query_map([], row_to_summary)?;
    let all: Vec<SessionSummary> = rows.flatten().collect();

    let filtered = all
        .into_iter()
        .filter(|s| {
            if mode == "AND" {
                tags.iter().all(|t| s.tags.contains(t))
            } else {
                tags.iter().any(|t| s.tags.contains(t))
            }
        })
        .collect();

    Ok(filtered)
}

pub fn evaluate_smart_tag(
    conn: &Connection,
    rule_type: &str,
    rule_value: &str,
) -> Result<Vec<SessionSummary>> {
    let val: serde_json::Value = serde_json::from_str(rule_value).unwrap_or(serde_json::json!({}));

    let sql = match rule_type {
        "recent_days" => {
            let days = val["days"].as_i64().unwrap_or(7);
            let cutoff = chrono::Utc::now().timestamp_millis() - days * 86_400_000;
            format!(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE m.updated_at > {} ORDER BY m.updated_at DESC LIMIT 200",
                cutoff
            )
        }
        "min_duration" => {
            let secs = val["seconds"].as_i64().unwrap_or(300);
            format!(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE m.total_duration_secs >= {} ORDER BY m.total_duration_secs DESC LIMIT 200",
                secs
            )
        }
        "min_cycles" => {
            let cycles = val["cycles"].as_i64().unwrap_or(3);
            format!(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE m.total_cycles >= {} ORDER BY m.total_cycles DESC LIMIT 200",
                cycles
            )
        }
        "no_tags" => "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE COALESCE(u.tags,'[]') = '[]'
             ORDER BY m.updated_at DESC LIMIT 200"
            .to_string(),
        "cwd_prefix" => {
            let prefix = val["prefix"].as_str().unwrap_or("").to_string();
            format!(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                m.message_count, m.source, m.model_name, m.max_context_pct,
                m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                COALESCE(u.starred,0), COALESCE(u.tags,'[]')
             FROM sessions_meta m LEFT JOIN user_data u ON m.session_id = u.session_id
             WHERE m.cwd LIKE '{}%' ORDER BY m.updated_at DESC LIMIT 200",
                prefix
            )
        }
        _ => return Ok(vec![]),
    };

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_summary)?;
    Ok(rows.flatten().collect())
}

// ── Tag Suggestions ───────────────────────────────────────────────────────────

pub fn suggest_tags(conn: &Connection, session_id: &str) -> Result<Vec<String>> {
    // Get session info
    let (title, cwd): (String, String) = conn
        .query_row(
            "SELECT title, cwd FROM sessions_meta WHERE session_id = ?1",
            rusqlite::params![session_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_default();

    let mut suggestions = Vec::new();
    let text = format!("{} {}", title.to_lowercase(), cwd.to_lowercase());

    // Rule-based tag suggestion from title+cwd keywords
    let rules: &[(&[&str], &str)] = &[
        (
            &[
                "aws",
                "cloudformation",
                "cfn",
                "s3",
                "ec2",
                "ecs",
                "rds",
                "lambda",
                "sqs",
                "sns",
            ],
            "#aws",
        ),
        (
            &[
                "infra",
                "infrastructure",
                "terraform",
                "ansible",
                "k8s",
                "kubernetes",
                "docker",
            ],
            "#infra",
        ),
        (
            &[
                "react",
                "vue",
                "svelte",
                "frontend",
                "css",
                "tailwind",
                "component",
                "ui",
            ],
            "#frontend",
        ),
        (
            &[
                "python",
                "rust",
                "typescript",
                "javascript",
                "golang",
                "java",
                "ruby",
            ],
            "#code",
        ),
        (
            &[
                "bug",
                "fix",
                "error",
                "issue",
                "crash",
                "null",
                "exception",
                "エラー",
                "修正",
            ],
            "#bug-fix",
        ),
        (
            &[
                "test",
                "spec",
                "jest",
                "vitest",
                "cargo test",
                "pytest",
                "e2e",
                "テスト",
            ],
            "#testing",
        ),
        (
            &[
                "deploy",
                "release",
                "build",
                "ci",
                "cd",
                "pipeline",
                "デプロイ",
                "リリース",
            ],
            "#devops",
        ),
        (
            &[
                "db",
                "database",
                "sql",
                "sqlite",
                "postgres",
                "mysql",
                "migration",
                "データ",
            ],
            "#database",
        ),
        (
            &[
                "api", "endpoint", "rest", "graphql", "openapi", "swagger", "http",
            ],
            "#api",
        ),
        (
            &[
                "doc",
                "readme",
                "document",
                "spec",
                "設計",
                "仕様",
                "ドキュメント",
            ],
            "#docs",
        ),
        (
            &[
                "refactor",
                "cleanup",
                "rename",
                "リファクタ",
                "整理",
                "改善",
            ],
            "#refactor",
        ),
        (
            &[
                "plan",
                "design",
                "architecture",
                "設計",
                "計画",
                "アーキテクチャ",
            ],
            "#design",
        ),
        (
            &[
                "security",
                "auth",
                "token",
                "credential",
                "セキュリティ",
                "認証",
            ],
            "#security",
        ),
        (
            &[
                "perf",
                "performance",
                "optimize",
                "speed",
                "slow",
                "パフォーマンス",
                "最適化",
            ],
            "#performance",
        ),
    ];

    for (keywords, tag) in rules {
        if keywords.iter().any(|kw| text.contains(kw)) {
            suggestions.push(tag.to_string());
        }
    }

    // Also suggest existing tags from same cwd (context-aware)
    let cwd_prefix = cwd.trim_end_matches('/');
    if !cwd_prefix.is_empty() {
        let mut stmt = conn.prepare(
            "SELECT DISTINCT u.tags FROM sessions_meta m
             JOIN user_data u ON m.session_id = u.session_id
             WHERE m.cwd = ?1 AND u.tags != '[]' AND m.session_id != ?2
             LIMIT 10",
        )?;
        let cwd_tags: Vec<String> = stmt
            .query_map(rusqlite::params![cwd_prefix, session_id], |r| r.get(0))?
            .flatten()
            .flat_map(|s: String| serde_json::from_str::<Vec<String>>(&s).unwrap_or_default())
            .collect();

        for t in cwd_tags {
            if !suggestions.contains(&t) {
                suggestions.push(t);
            }
        }
    }

    // Get current tags to exclude
    let current_tags: Vec<String> = conn
        .query_row(
            "SELECT tags FROM user_data WHERE session_id = ?1",
            rusqlite::params![session_id],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    suggestions.retain(|t| !current_tags.contains(t));
    suggestions.dedup();
    Ok(suggestions.into_iter().take(6).collect())
}
