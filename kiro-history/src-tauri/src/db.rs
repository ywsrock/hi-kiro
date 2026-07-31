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
         LIMIT {limit}"
    );

    let mut stmt = conn.prepare(&sql)?;

    // Execute with dynamic params
    let refs: Vec<&dyn rusqlite::ToSql> = bind_values.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(refs.as_slice(), row_to_summary)?;

    let mut results: Vec<SessionSummary> = rows.flatten().collect();

    // Tags filter (post-filter since tags are JSON array in SQLite)
    if let Some(ref tag_filter) = filters.tags {
        if !tag_filter.is_empty() {
            results.retain(|s| {
                tag_filter.iter().all(|t| s.tags.contains(t))
            });
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
    IndexStats { session_count, last_indexed_at }
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

    let mut tag_counts: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();

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
    conn.execute("DELETE FROM sessions_fts WHERE session_id = ?1", params![session_id])?;
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
    let total_sessions: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions_meta", [], |r| r.get(0))?;

    let total_messages: i64 = conn
        .query_row("SELECT SUM(message_count) FROM sessions_meta", [], |r| {
            r.get(0)
        })
        .unwrap_or(0);

    // By model
    let mut stmt = conn.prepare(
        "SELECT COALESCE(model_name, 'unknown') as mn, COUNT(*) as cnt
         FROM sessions_meta GROUP BY mn ORDER BY cnt DESC",
    )?;
    let sessions_by_model: Vec<ModelCount> = stmt
        .query_map([], |r| {
            Ok(ModelCount {
                model_name: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .flatten()
        .collect();

    // By cwd (top 10)
    let mut stmt = conn.prepare(
        "SELECT cwd, COUNT(*) as cnt FROM sessions_meta
         GROUP BY cwd ORDER BY cnt DESC LIMIT 10",
    )?;
    let sessions_by_cwd: Vec<CwdCount> = stmt
        .query_map([], |r| {
            Ok(CwdCount {
                cwd: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .flatten()
        .collect();

    // By date (daily, last 365 days)
    let mut stmt = conn.prepare(
        "SELECT strftime('%Y-%m-%d', datetime(updated_at/1000, 'unixepoch')) as d,
                COUNT(*) as cnt
         FROM sessions_meta
         WHERE updated_at > (strftime('%s','now') - 365*86400) * 1000
         GROUP BY d ORDER BY d",
    )?;
    let sessions_by_date: Vec<DateCount> = stmt
        .query_map([], |r| {
            Ok(DateCount {
                date: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .flatten()
        .collect();

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
    })
}
