use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::path::Path;

use crate::models::{Message, MessageRole, Session, SessionSource};

/// Load all sessions from kiro-cli's SQLite database (v1 + v2).
pub fn parse_all(db_path: &Path) -> Vec<Session> {
    let mut sessions = Vec::new();

    let conn = match Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(_) => return sessions,
    };

    sessions.extend(parse_v1(&conn));
    sessions.extend(parse_v2(&conn));
    sessions
}

/// Parse v1 conversations table: key=cwd, value=JSON blob
fn parse_v1(conn: &Connection) -> Vec<Session> {
    let mut sessions = Vec::new();

    let mut stmt = match conn.prepare(
        "SELECT key, value FROM conversations",
    ) {
        Ok(s) => s,
        Err(_) => return sessions,
    };

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
            ))
        })
        .unwrap_or_else(|_| panic!("query failed"));

    for row in rows.flatten() {
        let (cwd, json_str) = row;
        if let Ok(data) = serde_json::from_str::<Value>(&json_str) {
            if let Some(session) = build_session_from_value(&data, &cwd, SessionSource::SqliteV1) {
                sessions.push(session);
            }
        }
    }
    sessions
}

/// Parse v2 conversations_v2 table
fn parse_v2(conn: &Connection) -> Vec<Session> {
    let mut sessions = Vec::new();

    let mut stmt = match conn.prepare(
        "SELECT key, conversation_id, created_at, updated_at, value FROM conversations_v2",
    ) {
        Ok(s) => s,
        Err(_) => return sessions,
    };

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .unwrap_or_else(|_| panic!("query failed"));

    for row in rows.flatten() {
        let (cwd, conversation_id, created_at, updated_at, json_str) = row;
        if let Ok(data) = serde_json::from_str::<Value>(&json_str) {
            if let Some(mut session) =
                build_session_from_value(&data, &cwd, SessionSource::SqliteV2)
            {
                session.id = conversation_id;
                session.created_at = created_at;
                session.updated_at = updated_at;
                sessions.push(session);
            }
        }
    }
    sessions
}

/// Build a Session from the JSON value blob stored in SQLite.
fn build_session_from_value(data: &Value, cwd: &str, source: SessionSource) -> Option<Session> {
    let conversation_id = data["conversation_id"].as_str()?.to_string();
    let history = data["history"].as_array()?;

    let mut messages: Vec<Message> = Vec::new();

    for turn in history {
        // User message
        if let Some(user) = turn.get("user") {
            let text = extract_user_text(user);
            if !text.is_empty() {
                let ts = user["timestamp"].as_i64();
                messages.push(Message {
                    role: MessageRole::User,
                    content: text,
                    timestamp: ts,
                });
            }
        }

        // Assistant message
        if let Some(assistant) = turn.get("assistant") {
            let text = extract_assistant_text(assistant);
            if !text.is_empty() {
                messages.push(Message {
                    role: MessageRole::Assistant,
                    content: text,
                    timestamp: None,
                });
            }
        }
    }

    if messages.is_empty() {
        return None;
    }

    // Title: latest_summary → first user message truncated
    let title = data["latest_summary"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            messages
                .iter()
                .find(|m| m.role == MessageRole::User)
                .map(|m| {
                    let s = m.content.chars().take(50).collect::<String>();
                    if m.content.len() > 50 { format!("{}…", s) } else { s }
                })
                .unwrap_or_else(|| "Untitled".to_string())
        });

    // Model name
    let model_name = data["model_info"]["model_name"]
        .as_str()
        .map(|s| s.to_string());

    // Metadata from user_turn_metadata
    let (max_context_pct, total_tool_uses, total_cycles, total_duration_secs) =
        extract_turn_metadata(data);

    Some(Session {
        id: conversation_id,
        title,
        cwd: cwd.to_string(),
        created_at: 0, // overridden for v2
        updated_at: 0,
        messages,
        model_name,
        max_context_pct,
        total_tool_uses,
        total_cycles,
        total_duration_secs,
        source,
    })
}

/// Extract user text from history[n].user
fn extract_user_text(user: &Value) -> String {
    // content.Prompt.prompt
    if let Some(prompt) = user["content"]["Prompt"]["prompt"].as_str() {
        return prompt.to_string();
    }
    // content is a string directly
    if let Some(s) = user["content"].as_str() {
        return s.to_string();
    }
    String::new()
}

/// Extract assistant text from history[n].assistant
fn extract_assistant_text(assistant: &Value) -> String {
    if let Some(response) = assistant.get("Response") {
        // Case 1: Response.content is a direct string (SQLite format)
        if let Some(s) = response["content"].as_str() {
            return s.to_string();
        }
        // Case 2: Response.content is an array of content blocks
        if let Some(arr) = response["content"].as_array() {
            let text: String = arr
                .iter()
                .filter_map(|c| c["text"].as_str().or_else(|| c["data"].as_str()))
                .collect::<Vec<_>>()
                .join("\n");
            if !text.is_empty() {
                return text;
            }
        }
        // Case 3: Response itself is a string
        if let Some(s) = response.as_str() {
            return s.to_string();
        }
    }
    // Case 4: direct content string
    if let Some(s) = assistant["content"].as_str() {
        return s.to_string();
    }
    String::new()
}

/// Extract aggregate turn metadata
fn extract_turn_metadata(data: &Value) -> (Option<f32>, i64, i64, i64) {
    let turns = data["user_turn_metadata"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut max_context_pct: Option<f32> = None;
    let mut total_tool_uses: i64 = 0;
    let mut total_cycles: i64 = 0;
    let mut total_duration_secs: i64 = 0;

    for turn in &turns {
        if let Some(pct) = turn["context_usage_percentage"].as_f64() {
            let pct = pct as f32;
            max_context_pct = Some(max_context_pct.map_or(pct, |m: f32| m.max(pct)));
        }
        total_tool_uses += turn["builtin_tool_uses"].as_i64().unwrap_or(0);
        total_cycles += turn["number_of_cycles"].as_i64().unwrap_or(0);
        total_duration_secs += turn["turn_duration"]["secs"].as_i64().unwrap_or(0);
    }

    (max_context_pct, total_tool_uses, total_cycles, total_duration_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db_path() -> std::path::PathBuf {
        dirs::data_dir()
            .unwrap()
            .join("kiro-cli/data.sqlite3")
    }

    #[test]
    fn test_parse_sqlite() {
        let path = db_path();
        if !path.exists() {
            return;
        }
        let sessions = parse_all(&path);
        assert!(!sessions.is_empty(), "Expected sessions from SQLite");
        let s = &sessions[0];
        assert!(!s.id.is_empty());
        assert!(!s.title.is_empty());
    }
}
