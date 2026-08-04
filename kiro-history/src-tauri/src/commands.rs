use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, State};

use crate::db;
use crate::index;
use crate::models::Session;
use crate::operations;
use crate::parser::{jsonl, sqlite_source};
use crate::state::AppState;
use crate::types::*;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Load a full Session by ID (searches JSONL then SQLite sources).
fn load_session_by_id(state: &AppState, session_id: &str) -> Option<Session> {
    let sessions_dir = state.sessions_dir.clone();
    let sqlite_path = state.sqlite_db_path.clone();

    // Try JSONL: load directly by UUID (no full scan needed)
    let json_path = sessions_dir.join(format!("{}.json", session_id));
    let jsonl_path = sessions_dir.join(format!("{}.jsonl", session_id));
    if json_path.exists() && jsonl_path.exists() {
        if let Ok(messages) = jsonl::parse_jsonl_messages(&jsonl_path) {
            // Read metadata
            if let Ok(meta_str) = std::fs::read_to_string(&json_path) {
                if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&meta_str) {
                    let title = meta["title"].as_str().unwrap_or("Untitled").to_string();
                    let cwd = meta["cwd"].as_str().unwrap_or("").to_string();
                    let created_at =
                        jsonl::parse_rfc3339_to_ms(meta["created_at"].as_str().unwrap_or(""));
                    let updated_at =
                        jsonl::parse_rfc3339_to_ms(meta["updated_at"].as_str().unwrap_or(""));
                    return Some(crate::models::Session {
                        id: session_id.to_string(),
                        title,
                        cwd,
                        created_at,
                        updated_at,
                        messages,
                        model_name: None,
                        max_context_pct: None,
                        total_tool_uses: 0,
                        total_cycles: 0,
                        total_duration_secs: 0,
                        source: crate::models::SessionSource::Jsonl,
                    });
                }
            }
        }
    }

    // Try JSONL subdirectory (tasks/) format
    let tasks_dir = sessions_dir.join(session_id).join("tasks");
    if tasks_dir.exists() {
        let all = jsonl::parse_all(&sessions_dir);
        if let Some(s) = all.into_iter().find(|s| s.id == session_id) {
            return Some(s);
        }
    }

    // Try SQLite: use targeted load (no full scan)
    if sqlite_path.exists() {
        if let Some(s) = sqlite_source::load_full_session(&sqlite_path, session_id) {
            return Some(s);
        }
    }

    None
}

// ── Search & Index ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn search_sessions(
    query: String,
    limit: Option<u32>,
    filters: Option<FilterParams>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<SessionSummary>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = &state.index_conn;
    let filters = filters.unwrap_or_default();
    db::search_sessions(conn, &query, limit.unwrap_or(200), &filters).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_session_detail(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<SessionDetail, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    let conn = &state.index_conn;

    // Load full messages from source
    let session = load_session_by_id(&state, &session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;

    // Build summary from meta
    let summary = {
        let mut stmt = conn
            .prepare(
                "SELECT m.session_id, m.title, m.cwd, m.created_at, m.updated_at,
                        m.message_count, m.source, m.model_name, m.max_context_pct,
                        m.total_tool_uses, m.total_cycles, m.total_duration_secs,
                        COALESCE(u.starred, 0), COALESCE(u.tags, '[]')
                 FROM sessions_meta m
                 LEFT JOIN user_data u ON m.session_id = u.session_id
                 WHERE m.session_id = ?1",
            )
            .map_err(|e| e.to_string())?;

        stmt.query_row(rusqlite::params![session_id], |row| {
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
        })
        .map_err(|e| e.to_string())?
    };

    let messages = session
        .messages
        .iter()
        .map(|m| MessageDto {
            role: m.role_str().to_string(),
            content: m.content.clone(),
            timestamp: m.timestamp,
        })
        .collect();

    Ok(SessionDetail { summary, messages })
}

#[tauri::command]
pub fn get_related_sessions(
    cwd: String,
    exclude_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<SessionSummary>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::get_related_sessions(&state.index_conn, &cwd, &exclude_id, 5).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rebuild_index(
    app: AppHandle,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let (sessions_dir, sqlite_db_path) = {
        let state = state.lock().map_err(|e| e.to_string())?;
        (state.sessions_dir.clone(), state.sqlite_db_path.clone())
    };

    // Re-open connection for this operation (can't hold lock across await)
    let index_path = {
        let state = state.lock().map_err(|e| e.to_string())?;
        state.index_db_path.clone()
    };

    let conn = index::open_index_db(&index_path).map_err(|e| e.to_string())?;
    index::rebuild_index(&conn, &sessions_dir, &sqlite_db_path, Some(&app))
        .map_err(|e| e.to_string())?;

    // Update the shared connection's stats
    Ok(())
}

#[tauri::command]
pub fn get_index_stats(state: State<'_, Mutex<AppState>>) -> Result<IndexStats, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    Ok(db::get_index_stats(&state.index_conn))
}

// ── Clipboard & Resume ────────────────────────────────────────────────────────

#[tauri::command]
pub fn copy_to_clipboard(text: String, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(text).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_session(session_id: String, cwd: String) -> Result<(), String> {
    use std::process::Command;

    #[allow(unused_variables)]
    let cwd_path = if cwd.is_empty() {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
    } else {
        PathBuf::from(&cwd)
    };

    // kiro-cli binary path
    let kiro_bin = dirs::home_dir()
        .map(|h| h.join(".local/bin/kiro-cli"))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "kiro-cli".to_string());

    // Command to run: kiro-cli chat --resume-id <session_id>
    let kiro_cmd = format!("{} chat --resume-id {}", kiro_bin, session_id);

    #[cfg(target_os = "linux")]
    {
        // Detect WSL
        let is_wsl = std::fs::read_to_string("/proc/version")
            .map(|v| v.to_lowercase().contains("microsoft"))
            .unwrap_or(false);

        if is_wsl {
            // Try Windows Terminal first (wt.exe)
            let wt_paths = [
                // User-installed Windows Terminal
                dirs::home_dir()
                    .map(|h| h.join("msHome/AppData/Local/Microsoft/WindowsApps/wt.exe"))
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default(),
                "/mnt/c/Users/wenyan/AppData/Local/Microsoft/WindowsApps/wt.exe".to_string(),
                // System-wide
                "/mnt/c/Program Files/WindowsApps/Microsoft.WindowsTerminal_1.18.3181.0_x64__8wekyb3d8bbwe/wt.exe".to_string(),
            ];

            let cwd_linux = cwd_path.to_string_lossy().to_string();

            for wt in &wt_paths {
                if std::path::Path::new(wt).exists() {
                    // wt.exe wsl.exe --cd <cwd> -- bash -c "kiro-cli chat --resume-id <id>"
                    let result = Command::new(wt)
                        .args([
                            "wsl.exe", "--cd", &cwd_linux, "--", "bash", "-i", "-c", &kiro_cmd,
                        ])
                        .spawn();
                    if result.is_ok() {
                        return Ok(());
                    }
                }
            }

            // Fallback: wsl.exe in a new cmd window
            let result = Command::new("/mnt/c/WINDOWS/system32/cmd.exe")
                .args([
                    "/c", "start", "wsl.exe", "--cd", &cwd_linux, "--", "bash", "-i", "-c",
                    &kiro_cmd,
                ])
                .spawn();
            if result.is_ok() {
                return Ok(());
            }

            return Err(
                "Could not launch Windows Terminal or WSL window. Please run manually:\n"
                    .to_string()
                    + &format!("cd {} && {}", cwd_path.display(), kiro_cmd),
            );
        }

        // Native Linux: try common terminals
        let cwd_str = cwd_path.to_string_lossy().to_string();
        let terminals: &[(&str, &[&str])] = &[
            (
                "gnome-terminal",
                &[
                    "--working-directory",
                    &cwd_str,
                    "--",
                    "bash",
                    "-i",
                    "-c",
                    &kiro_cmd,
                ],
            ),
            (
                "x-terminal-emulator",
                &["-e", &format!("bash -i -c '{}'", kiro_cmd)],
            ),
            ("xterm", &["-e", &format!("bash -i -c '{}'", kiro_cmd)]),
            (
                "konsole",
                &["--workdir", &cwd_str, "-e", "bash", "-i", "-c", &kiro_cmd],
            ),
        ];

        for (term, args) in terminals {
            if Command::new(term)
                .args(*args)
                .current_dir(&cwd_path)
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
        }

        Err(format!(
            "No terminal emulator found. Run manually:\ncd {} && {}",
            cwd_path.display(),
            kiro_cmd
        ))
    }

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            r#"tell application "Terminal" to do script "cd '{}' && {}""#,
            cwd_path.display(),
            kiro_cmd
        );
        Command::new("osascript")
            .args(["-e", &script])
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("wt.exe")
            .args(["wsl.exe", "--", "bash", "-i", "-c", &kiro_cmd])
            .spawn()
            .or_else(|_| {
                Command::new("cmd.exe")
                    .args([
                        "/c", "start", "wsl.exe", "--", "bash", "-i", "-c", &kiro_cmd,
                    ])
                    .spawn()
            })
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

// ── Bookmarks & Tags ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn toggle_bookmark(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<bool, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::toggle_bookmark(&state.index_conn, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_tags(
    session_id: String,
    tags: Vec<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::set_tags(&state.index_conn, &session_id, tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_tags(state: State<'_, Mutex<AppState>>) -> Result<Vec<TagStat>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::get_all_tags(&state.index_conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_bookmarked_sessions(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<SessionSummary>, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::get_bookmarked_sessions(&state.index_conn).map_err(|e| e.to_string())
}

// ── Delete ────────────────────────────────────────────────────────────────────

/// Remove from index only (soft delete).
#[tauri::command]
pub fn delete_session(session_id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::delete_session(&state.index_conn, &session_id).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct DeleteResult {
    pub deleted: Vec<String>,
    pub skipped: Vec<SkippedSession>,
}

#[derive(serde::Serialize)]
pub struct SkippedSession {
    pub session_id: String,
    pub reason: String,
}

/// Delete actual source files + remove from index.
#[tauri::command]
pub fn delete_sessions_files(
    session_ids: Vec<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<DeleteResult, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let sessions_dir = state_guard.sessions_dir.clone();
    let sqlite_db_path = state_guard.sqlite_db_path.clone();
    let conn = &state_guard.index_conn;

    let mut deleted = Vec::new();
    let mut skipped = Vec::new();

    for session_id in &session_ids {
        // Determine source from index
        let source: Option<String> = conn
            .query_row(
                "SELECT source FROM sessions_meta WHERE session_id = ?1",
                rusqlite::params![session_id],
                |r| r.get(0),
            )
            .ok();

        let source = source.as_deref().unwrap_or("jsonl");

        let result = if source.starts_with("sqlite") {
            delete_sqlite_session(session_id, &sqlite_db_path)
        } else {
            delete_jsonl_session(session_id, &sessions_dir)
        };

        match result {
            Ok(_) => {
                // Remove from kiro-history index too
                let _ = db::delete_session(conn, session_id);
                deleted.push(session_id.clone());
            }
            Err(reason) => {
                skipped.push(SkippedSession {
                    session_id: session_id.clone(),
                    reason,
                });
            }
        }
    }

    Ok(DeleteResult { deleted, skipped })
}

fn delete_jsonl_session(session_id: &str, sessions_dir: &std::path::Path) -> Result<(), String> {
    // Check for .lock file (session in use)
    let lock_path = sessions_dir.join(format!("{}.lock", session_id));
    if lock_path.exists() {
        return Err("セッションが使用中です (.lock ファイルが存在します)".to_string());
    }

    // Delete all related files
    for ext in &["json", "jsonl", "history", "lock"] {
        let path = sessions_dir.join(format!("{}.{}", session_id, ext));
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("ファイル削除エラー: {}", e))?;
        }
    }

    // Delete subdirectory if exists
    let dir_path = sessions_dir.join(session_id);
    if dir_path.is_dir() {
        std::fs::remove_dir_all(&dir_path).map_err(|e| format!("ディレクトリ削除エラー: {}", e))?;
    }

    Ok(())
}

fn delete_sqlite_session(session_id: &str, sqlite_db_path: &std::path::Path) -> Result<(), String> {
    if !sqlite_db_path.exists() {
        return Err("SQLiteデータベースが見つかりません".to_string());
    }

    let conn =
        rusqlite::Connection::open(sqlite_db_path).map_err(|e| format!("DB接続エラー: {}", e))?;

    // Try conversations_v2 first
    let affected = conn
        .execute(
            "DELETE FROM conversations_v2 WHERE conversation_id = ?1",
            rusqlite::params![session_id],
        )
        .map_err(|e| format!("削除エラー: {}", e))?;

    if affected == 0 {
        // Try conversations (v1: key=cwd, value contains conversation_id)
        // Find and delete rows where value JSON contains this conversation_id
        let mut stmt = conn
            .prepare("SELECT key FROM conversations WHERE value LIKE ?1")
            .map_err(|e| format!("クエリエラー: {}", e))?;
        let pattern = format!("%\"{}\"% ", session_id);
        let keys: Vec<String> = stmt
            .query_map(rusqlite::params![pattern], |r| r.get(0))
            .map_err(|e| format!("クエリエラー: {}", e))?
            .flatten()
            .collect();

        for key in keys {
            conn.execute(
                "DELETE FROM conversations WHERE key = ?1",
                rusqlite::params![key],
            )
            .map_err(|e| format!("削除エラー: {}", e))?;
        }
    }

    Ok(())
}

// ── Stats ─────────────────────────────────────────────────────────────────────

// ── Rename ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn rename_session(
    session_id: String,
    new_title: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::rename_session(&state.index_conn, &session_id, &new_title).map_err(|e| e.to_string())
}

// ── Stats ─────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_stats(state: State<'_, Mutex<AppState>>) -> Result<StatsData, String> {
    let state = state.lock().map_err(|e| e.to_string())?;
    db::get_stats(&state.index_conn).map_err(|e| e.to_string())
}

// ── Snippets ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_snippets(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<CodeSnippet>, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let session = load_session_by_id(&state_guard, &session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    Ok(operations::extract_snippets(&session))
}

#[tauri::command]
pub fn get_all_snippets(
    query: Option<String>,
    lang_filter: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<CodeSnippetWithSession>, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let sessions_dir = state_guard.sessions_dir.clone();
    let sqlite_db_path = state_guard.sqlite_db_path.clone();
    drop(state_guard);

    let mut all: Vec<CodeSnippetWithSession> = Vec::new();
    let q = query.as_deref().unwrap_or("");
    let lang = lang_filter.as_deref();

    for session in jsonl::parse_all(&sessions_dir) {
        all.extend(operations::extract_snippets_with_session(&session, q, lang));
    }
    if sqlite_db_path.exists() {
        for session in sqlite_source::parse_all(&sqlite_db_path) {
            all.extend(operations::extract_snippets_with_session(&session, q, lang));
        }
    }
    Ok(all)
}

// ── File References ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_file_refs(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<FileRef>, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let session = load_session_by_id(&state_guard, &session_id)
        .ok_or_else(|| format!("Session not found: {}", session_id))?;
    Ok(operations::extract_file_refs(&session))
}

#[tauri::command]
pub fn open_in_editor(path: String) -> Result<(), String> {
    use std::process::Command;

    let expanded = if path.starts_with('~') {
        dirs::home_dir()
            .map(|h| h.join(&path[2..]).to_string_lossy().into_owned())
            .unwrap_or(path.clone())
    } else {
        path.clone()
    };

    // Try editors in order
    #[cfg(target_os = "linux")]
    {
        let editor = std::env::var("EDITOR").unwrap_or_default();
        if !editor.is_empty() && Command::new(&editor).arg(&expanded).spawn().is_ok() {
            return Ok(());
        }
        for ed in &["code", "vim", "nano", "gedit"] {
            if Command::new(ed).arg(&expanded).spawn().is_ok() {
                return Ok(());
            }
        }
        Command::new("xdg-open")
            .arg(&expanded)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        let editor = std::env::var("EDITOR").unwrap_or_default();
        if !editor.is_empty() && Command::new(&editor).arg(&expanded).spawn().is_ok() {
            return Ok(());
        }
        Command::new("open")
            .arg(&expanded)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("code")
            .arg(&expanded)
            .spawn()
            .or_else(|_| Command::new("notepad").arg(&expanded).spawn())
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

// ── Diff ──────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_session_diff(
    session_id_a: String,
    session_id_b: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<DiffResult, String> {
    let state_guard = state.lock().map_err(|e| e.to_string())?;
    let session_a = load_session_by_id(&state_guard, &session_id_a)
        .ok_or_else(|| format!("Session A not found: {}", session_id_a))?;
    let session_b = load_session_by_id(&state_guard, &session_id_b)
        .ok_or_else(|| format!("Session B not found: {}", session_id_b))?;
    Ok(operations::diff_sessions(&session_a, &session_b))
}

// ── Export ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn export_session_cmd(
    session_id: String,
    format: ExportFormat,
    output_path: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let session = {
        let state_guard = state.lock().map_err(|e| e.to_string())?;
        load_session_by_id(&state_guard, &session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?
    };

    let content = operations::export_session(&session, &format).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, content).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn export_sessions_zip_cmd(
    session_ids: Vec<String>,
    format: ExportFormat,
    output_path: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let sessions: Vec<Session> = {
        let state_guard = state.lock().map_err(|e| e.to_string())?;
        session_ids
            .iter()
            .filter_map(|id| load_session_by_id(&state_guard, id))
            .collect()
    };

    let bytes = operations::export_sessions_zip(&sessions, &format).map_err(|e| e.to_string())?;
    std::fs::write(&output_path, bytes).map_err(|e| e.to_string())?;
    Ok(())
}

// ── Tag Management Commands ───────────────────────────────────────────────────

#[tauri::command]
pub fn get_tag_metadata(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::TagMeta>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::get_tag_metadata(&s.index_conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_tag(
    params: crate::types::CreateTagParams,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::create_tag(&s.index_conn, &params).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_tag(
    tag: String,
    color: String,
    description: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::update_tag(&s.index_conn, &tag, &color, &description).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_tag_full(tag: String, state: State<'_, Mutex<AppState>>) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::delete_tag_full(&s.index_conn, &tag).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_tag(
    old_tag: String,
    new_tag: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::rename_tag(&s.index_conn, &old_tag, &new_tag).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn merge_tags(
    from_tag: String,
    to_tag: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<usize, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::merge_tags(&s.index_conn, &from_tag, &to_tag).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_tag_order(tags: Vec<String>, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::set_tag_order(&s.index_conn, &tags).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_smart_tag(
    rule: crate::types::SmartTagRule,
    color: String,
    description: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::create_smart_tag(&s.index_conn, &rule, &color, &description).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_sessions_by_tag(
    tags: Vec<String>,
    mode: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SessionSummary>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::get_sessions_by_tag(&s.index_conn, &tags, &mode).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn evaluate_smart_tag(
    rule_type: String,
    rule_value: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SessionSummary>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::evaluate_smart_tag(&s.index_conn, &rule_type, &rule_value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn suggest_tags(
    session_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<String>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    db::suggest_tags(&s.index_conn, &session_id).map_err(|e| e.to_string())
}

// ── Saved Snippets Commands ───────────────────────────────────────────────────

#[tauri::command]
pub fn save_snippet(
    params: crate::types::SaveSnippetParams,
    state: State<'_, Mutex<AppState>>,
) -> Result<crate::types::SavedSnippet, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::save_snippet(&s.index_conn, &params).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_snippet(
    id: String,
    title: String,
    description: String,
    language: String,
    code: String,
    tags: Vec<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::update_snippet(
        &s.index_conn,
        &id,
        &title,
        &description,
        &language,
        &code,
        &tags,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_snippet(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::delete_snippet(&s.index_conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_snippet_star(id: String, state: State<'_, Mutex<AppState>>) -> Result<bool, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::toggle_snippet_star(&s.index_conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn increment_snippet_use(id: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::increment_use_count(&s.index_conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_saved_snippets(
    search_params: crate::types::SnippetSearchParams,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SavedSnippet>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::search_saved_snippets(&s.index_conn, &search_params).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn find_similar_snippets(
    code: String,
    language: String,
    exclude_id: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::types::SimilarSnippet>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::find_similar_snippets(&s.index_conn, &code, &language, exclude_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn suggest_snippet_title(language: String, code: String) -> Result<String, String> {
    Ok(crate::snippets::suggest_snippet_title(&language, &code))
}

#[tauri::command]
pub fn get_snippet_stats(
    state: State<'_, Mutex<AppState>>,
) -> Result<crate::types::SnippetStats, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    crate::snippets::get_snippet_stats(&s.index_conn).map_err(|e| e.to_string())
}

// ── Config / WSL Path Detection ───────────────────────────────────────────────

#[tauri::command]
pub fn get_config() -> Result<crate::state::AppConfig, String> {
    Ok(crate::state::load_config())
}

#[tauri::command]
pub fn save_config_cmd(config: crate::state::AppConfig) -> Result<(), String> {
    // theme が None の場合は既存の値を保持する
    let existing = crate::state::load_config();
    let merged = crate::state::AppConfig {
        sessions_dir: config.sessions_dir,
        sqlite_db_path: config.sqlite_db_path,
        theme: config
            .theme
            .or(existing.theme)
            .or(Some("system".to_string())),
    };
    crate::state::save_config(&merged).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn detect_wsl_paths() -> Result<crate::state::DetectedPaths, String> {
    Ok(crate::state::detect_wsl_paths())
}

#[tauri::command]
pub fn get_current_paths(state: State<'_, Mutex<AppState>>) -> Result<serde_json::Value, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "sessions_dir": s.sessions_dir.to_string_lossy(),
        "sqlite_db_path": s.sqlite_db_path.to_string_lossy(),
        "index_db_path": s.index_db_path.to_string_lossy(),
    }))
}
