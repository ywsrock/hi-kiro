use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{AppHandle, Emitter};

use crate::models::Session;
use crate::parser::{jsonl, sqlite_source};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexProgress {
    pub processed: u32,
    pub total: u32,
}

/// Open (or create) the kiro-history index database.
pub fn open_index_db(db_path: &Path) -> Result<Connection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(db_path)?;
    init_schema(&conn)?;
    Ok(conn)
}

/// Initialize all tables.
fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE VIRTUAL TABLE IF NOT EXISTS sessions_fts USING fts5(
            session_id UNINDEXED,
            title,
            cwd,
            full_text
        );

        CREATE TABLE IF NOT EXISTS sessions_meta (
            session_id      TEXT PRIMARY KEY,
            title           TEXT NOT NULL DEFAULT '',
            cwd             TEXT NOT NULL DEFAULT '',
            created_at      INTEGER NOT NULL DEFAULT 0,
            updated_at      INTEGER NOT NULL DEFAULT 0,
            message_count   INTEGER NOT NULL DEFAULT 0,
            source          TEXT NOT NULL DEFAULT '',
            model_name      TEXT,
            max_context_pct REAL,
            total_tool_uses INTEGER NOT NULL DEFAULT 0,
            total_cycles    INTEGER NOT NULL DEFAULT 0,
            total_duration_secs INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS user_data (
            session_id  TEXT PRIMARY KEY,
            starred     INTEGER NOT NULL DEFAULT 0,
            tags        TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS index_meta (
            key   TEXT PRIMARY KEY,
            value TEXT
        );

        CREATE TABLE IF NOT EXISTS deleted_sessions (
            session_id TEXT PRIMARY KEY,
            deleted_at INTEGER NOT NULL
        );
    ")?;
    Ok(())
}

/// Rebuild index for all sessions from all sources.
pub fn rebuild_index(
    conn: &Connection,
    sessions_dir: &Path,
    sqlite_db_path: &Path,
    app: Option<&AppHandle>,
) -> Result<()> {
    // Load all sessions
    let mut all_sessions: Vec<Session> = Vec::new();
    all_sessions.extend(jsonl::parse_all(sessions_dir));
    if sqlite_db_path.exists() {
        all_sessions.extend(sqlite_source::parse_all(sqlite_db_path));
    }

    let total = all_sessions.len() as u32;

    for (i, session) in all_sessions.iter().enumerate() {
        // Skip sessions in the deleted blacklist
        let is_deleted: bool = conn
            .query_row(
                "SELECT 1 FROM deleted_sessions WHERE session_id = ?1",
                params![session.id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if is_deleted {
            continue;
        }

        upsert_session(conn, session)?;

        // Emit progress every 5 sessions
        if i % 5 == 0 {
            if let Some(app) = app {
                let _ = app.emit(
                    "index:progress",
                    IndexProgress {
                        processed: i as u32 + 1,
                        total,
                    },
                );
            }
        }
    }

    // Final progress event
    if let Some(app) = app {
        let _ = app.emit("index:progress", IndexProgress { processed: total, total });
    }

    // Update last_indexed_at
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR REPLACE INTO index_meta (key, value) VALUES ('last_indexed_at', ?1)",
        params![now.to_string()],
    )?;

    Ok(())
}

/// Insert or update a session in the index.
fn upsert_session(conn: &Connection, session: &Session) -> Result<()> {
    // Check if update is needed via updated_at
    let existing: Option<i64> = conn
        .query_row(
            "SELECT updated_at FROM sessions_meta WHERE session_id = ?1",
            params![session.id],
            |row| row.get(0),
        )
        .ok();

    if let Some(existing_updated) = existing {
        if existing_updated >= session.updated_at && session.updated_at > 0 {
            return Ok(()); // Skip: no changes
        }
        // Delete old FTS entry
        conn.execute(
            "DELETE FROM sessions_fts WHERE session_id = ?1",
            params![session.id],
        )?;
    }

    // Build full_text for FTS
    let full_text = build_full_text(session);

    // Upsert sessions_meta
    conn.execute(
        "INSERT OR REPLACE INTO sessions_meta
         (session_id, title, cwd, created_at, updated_at, message_count, source,
          model_name, max_context_pct, total_tool_uses, total_cycles, total_duration_secs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            session.id,
            session.title,
            session.cwd,
            session.created_at,
            session.updated_at,
            session.messages.len() as i64,
            session.source.to_string(),
            session.model_name,
            session.max_context_pct,
            session.total_tool_uses,
            session.total_cycles,
            session.total_duration_secs,
        ],
    )?;

    // Insert into FTS
    conn.execute(
        "INSERT INTO sessions_fts (session_id, title, cwd, full_text)
         VALUES (?1, ?2, ?3, ?4)",
        params![session.id, session.title, session.cwd, full_text],
    )?;

    // Ensure user_data row exists
    conn.execute(
        "INSERT OR IGNORE INTO user_data (session_id) VALUES (?1)",
        params![session.id],
    )?;

    Ok(())
}

/// Combine title + cwd + all message content for FTS indexing.
fn build_full_text(session: &Session) -> String {
    let mut parts = vec![session.title.clone(), session.cwd.clone()];
    for msg in &session.messages {
        parts.push(msg.content.clone());
    }
    parts.join(" ")
}

/// Get the last indexed timestamp and session count.
pub fn get_index_stats(conn: &Connection) -> (i64, i64) {
    let session_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM sessions_meta", [], |r| r.get(0))
        .unwrap_or(0);

    let last_indexed: i64 = conn
        .query_row(
            "SELECT value FROM index_meta WHERE key = 'last_indexed_at'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    (session_count, last_indexed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_init_schema() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let conn = open_index_db(&db_path).unwrap();

        // Verify tables exist
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='sessions_meta'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_rebuild_index() {
        let sessions_dir = dirs::home_dir()
            .unwrap()
            .join(".kiro/sessions/cli");
        let sqlite_db = dirs::data_dir()
            .unwrap()
            .join("kiro-cli/data.sqlite3");

        if !sessions_dir.exists() {
            return;
        }

        let dir = tempdir().unwrap();
        let db_path = dir.path().join("index.db");
        let conn = open_index_db(&db_path).unwrap();

        rebuild_index(&conn, &sessions_dir, &sqlite_db, None).unwrap();

        let (count, _) = get_index_stats(&conn);
        assert!(count > 0, "Expected sessions in index");
    }
}
