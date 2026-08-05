use anyhow::Result;
use log::warn;
use rusqlite::{params, Connection};
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

        -- Performance indexes for common query patterns
        CREATE INDEX IF NOT EXISTS idx_sessions_updated_at
            ON sessions_meta(updated_at DESC);
        CREATE INDEX IF NOT EXISTS idx_sessions_cwd
            ON sessions_meta(cwd);
        CREATE INDEX IF NOT EXISTS idx_sessions_model
            ON sessions_meta(model_name);
        CREATE INDEX IF NOT EXISTS idx_user_data_starred
            ON user_data(starred) WHERE starred = 1;

        -- Tag metadata (color, description, order)
        CREATE TABLE IF NOT EXISTS tag_metadata (
            tag         TEXT PRIMARY KEY,
            color       TEXT NOT NULL DEFAULT '#334155',
            description TEXT NOT NULL DEFAULT '',
            sort_order  INTEGER NOT NULL DEFAULT 0,
            created_at  INTEGER NOT NULL DEFAULT 0,
            is_smart    INTEGER NOT NULL DEFAULT 0
        );

        -- Smart tag rules (dynamic tag definitions)
        CREATE TABLE IF NOT EXISTS smart_tag_rules (
            tag        TEXT PRIMARY KEY REFERENCES tag_metadata(tag) ON DELETE CASCADE,
            rule_type  TEXT NOT NULL,
            rule_value TEXT NOT NULL DEFAULT '{}'
        );

        -- Saved snippets (persistent, user-curated code collection)
        CREATE TABLE IF NOT EXISTS saved_snippets (
            id                TEXT PRIMARY KEY,
            title             TEXT NOT NULL DEFAULT '',
            description       TEXT NOT NULL DEFAULT '',
            language          TEXT NOT NULL DEFAULT 'text',
            code              TEXT NOT NULL,
            tags              TEXT NOT NULL DEFAULT '[]',
            starred           INTEGER NOT NULL DEFAULT 0,
            source_session_id TEXT,
            source_cwd        TEXT NOT NULL DEFAULT '',
            created_at        INTEGER NOT NULL,
            updated_at        INTEGER NOT NULL,
            use_count         INTEGER NOT NULL DEFAULT 0,
            last_used_at      INTEGER NOT NULL DEFAULT 0
        );
        CREATE VIRTUAL TABLE IF NOT EXISTS saved_snippets_fts USING fts5(
            id UNINDEXED, title, description, code, tags
        );
        CREATE INDEX IF NOT EXISTS idx_snippets_language ON saved_snippets(language);
        CREATE INDEX IF NOT EXISTS idx_snippets_starred  ON saved_snippets(starred) WHERE starred = 1;
        CREATE INDEX IF NOT EXISTS idx_snippets_updated  ON saved_snippets(updated_at DESC);
    ")?;
    Ok(())
}

/// Rebuild index using streaming — handles 20GB+ SQLite without loading all data to memory.
pub fn rebuild_index(
    conn: &Connection,
    sessions_dir: &Path,
    sqlite_db_path: &Path,
    app: Option<&AppHandle>,
) -> Result<()> {
    // First pass: メタデータのみでカウント（メッセージ本文はロードしない）
    let mut jsonl_count: u32 = 0;
    jsonl::stream_meta(sessions_dir, |_| {
        jsonl_count += 1;
    });

    // Count SQLite sessions quickly (no full parse)
    // Windows UNCパス対応: 一時ファイルコピー（最新のみ、キャッシュは呼び出し元で管理）
    let sqlite_count: u32 = if sqlite_db_path.exists() {
        #[cfg(target_os = "windows")]
        let sqlite_open_path = {
            let tmp = std::env::temp_dir().join("hi-kiro-sqlite-shared-tmp.db");
            if !tmp.exists() {
                std::fs::copy(sqlite_db_path, &tmp).ok();
            }
            tmp
        };
        #[cfg(not(target_os = "windows"))]
        let sqlite_open_path = sqlite_db_path.to_path_buf();

        if let Ok(c) = rusqlite::Connection::open_with_flags(
            &sqlite_open_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) {
            let v1: i64 = c
                .query_row("SELECT COUNT(*) FROM conversations", [], |r| r.get(0))
                .unwrap_or(0);
            let v2: i64 = c
                .query_row("SELECT COUNT(*) FROM conversations_v2", [], |r| r.get(0))
                .unwrap_or(0);
            (v1 + v2) as u32
        } else {
            0
        }
    } else {
        0
    };
    let total = jsonl_count + sqlite_count;

    let mut processed: u32 = 0;
    const BATCH_SIZE: u32 = 1000; // 1000件ごとにコミット

    // トランザクション開始（自動コミットを防ぎ書き込みを高速化）
    conn.execute_batch("BEGIN")?;

    // Index JSONL sessions (stream_metaでメッセージ本文をロードしない)
    jsonl::stream_meta(sessions_dir, |meta| {
        let is_deleted: bool = conn
            .query_row(
                "SELECT 1 FROM deleted_sessions WHERE session_id = ?1",
                params![meta.id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if !is_deleted {
            let session = Session {
                id: meta.id.clone(),
                title: meta.title.clone(),
                cwd: meta.cwd.clone(),
                created_at: meta.created_at,
                updated_at: meta.updated_at,
                messages: vec![],
                model_name: meta.model_name.clone(),
                max_context_pct: meta.max_context_pct,
                total_tool_uses: meta.total_tool_uses,
                total_cycles: meta.total_cycles,
                total_duration_secs: meta.total_duration_secs,
                source: meta.source.clone(),
            };
            let full_text = format!("{} {}", meta.title, meta.cwd);
            // meta.message_count を使って message_count=0 リグレッションを防ぐ
            if let Err(e) = upsert_session_with_text_and_count(
                conn,
                &session,
                &full_text,
                meta.message_count as i64,
            ) {
                warn!("Failed to index JSONL session {}: {}", session.id, e);
            }
        }
        processed += 1;
        if processed.is_multiple_of(BATCH_SIZE) {
            if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                warn!("Batch commit failed at {}: {}", processed, e);
            }
        }
        if processed.is_multiple_of(10) {
            if let Some(app) = app {
                let _ = app.emit("index:progress", IndexProgress { processed, total });
            }
        }
    });

    // Stream SQLite sessions — never loads all sessions into memory at once
    if sqlite_db_path.exists() {
        sqlite_source::stream_all(sqlite_db_path, |meta| {
            let is_deleted: bool = conn
                .query_row(
                    "SELECT 1 FROM deleted_sessions WHERE session_id = ?1",
                    params![meta.id],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            if !is_deleted {
                let session = Session {
                    id: meta.id.clone(),
                    title: meta.title.clone(),
                    cwd: meta.cwd.clone(),
                    created_at: meta.created_at,
                    updated_at: meta.updated_at,
                    messages: vec![], // not needed for index
                    model_name: meta.model_name.clone(),
                    max_context_pct: meta.max_context_pct,
                    total_tool_uses: meta.total_tool_uses,
                    total_cycles: meta.total_cycles,
                    total_duration_secs: meta.total_duration_secs,
                    source: meta.source.clone(),
                };
                // Build full_text from meta (title + cwd + first message snippet)
                let full_text = format!("{} {} {}", meta.title, meta.cwd, meta.first_user_message);
                if let Err(e) = upsert_session_with_text(conn, &session, &full_text) {
                    warn!("Failed to index session {}: {}", session.id, e);
                }
            }
            processed += 1;
            // バッチコミット（クロージャ内なのでエラーは無視してログのみ）
            if processed.is_multiple_of(BATCH_SIZE) {
                if let Err(e) = conn.execute_batch("COMMIT; BEGIN") {
                    warn!("Batch commit failed at {}: {}", processed, e);
                }
            }
            if processed.is_multiple_of(50) {
                if let Some(app) = app {
                    let _ = app.emit("index:progress", IndexProgress { processed, total });
                }
            }
        });
    }

    // 最終コミット
    conn.execute_batch("COMMIT")?;

    if let Some(app) = app {
        let _ = app.emit(
            "index:progress",
            IndexProgress {
                processed: total,
                total,
            },
        );
    }

    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR REPLACE INTO index_meta (key, value) VALUES ('last_indexed_at', ?1)",
        params![now.to_string()],
    )?;

    Ok(())
}

/// Upsert with externally provided full_text (for streaming, avoids message re-parse).
pub fn upsert_session_with_text(
    conn: &Connection,
    session: &Session,
    full_text: &str,
) -> Result<()> {
    upsert_session_with_text_and_count(conn, session, full_text, session.messages.len() as i64)
}

/// message_count を外から指定できるバージョン（stream_meta 経由のストリーミング用）
pub fn upsert_session_with_text_and_count(
    conn: &Connection,
    session: &Session,
    full_text: &str,
    message_count: i64,
) -> Result<()> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT updated_at FROM sessions_meta WHERE session_id = ?1",
            params![session.id],
            |row| row.get(0),
        )
        .ok();

    if let Some(existing_updated) = existing {
        if existing_updated >= session.updated_at && session.updated_at > 0 {
            return Ok(());
        }
        conn.execute(
            "DELETE FROM sessions_fts WHERE session_id = ?1",
            params![session.id],
        )?;
    }

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
            message_count,
            session.source.to_string(),
            session.model_name,
            session.max_context_pct,
            session.total_tool_uses,
            session.total_cycles,
            session.total_duration_secs,
        ],
    )?;

    conn.execute(
        "INSERT INTO sessions_fts (session_id, title, cwd, full_text) VALUES (?1, ?2, ?3, ?4)",
        params![session.id, session.title, session.cwd, full_text],
    )?;

    conn.execute(
        "INSERT OR IGNORE INTO user_data (session_id) VALUES (?1)",
        params![session.id],
    )?;
    Ok(())
}

/// Insert or update a session in the index.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
        let sessions_dir = dirs::home_dir().unwrap().join(".kiro/sessions/cli");
        let sqlite_db = dirs::data_dir().unwrap().join("kiro-cli/data.sqlite3");

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
