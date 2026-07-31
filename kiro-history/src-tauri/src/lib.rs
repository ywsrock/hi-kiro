mod commands;
mod db;
mod index;
mod models;
mod operations;
mod parser;
mod state;
mod types;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // GDK_BACKEND / WEBKIT env vars must be set before process start.
    // Use ~/kiro-history/dev.sh to launch with proper WSLg/IME settings.
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Load config and resolve paths
            let config = state::load_config();
            let (sessions_dir, sqlite_db_path, index_db_path) = state::resolve_paths(&config);

            // Open index DB
            let conn = index::open_index_db(&index_db_path)
                .expect("Failed to open index database");

            let app_state = state::AppState::new(
                conn,
                index_db_path.clone(),
                sessions_dir.clone(),
                sqlite_db_path.clone(),
                config,
            );
            app.manage(Mutex::new(app_state));

            // Kick off background index rebuild
            let app_handle: AppHandle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let index_conn = index::open_index_db(&index_db_path)
                    .expect("Failed to open index db for background rebuild");
                let _ = index::rebuild_index(
                    &index_conn,
                    &sessions_dir,
                    &sqlite_db_path,
                    Some(&app_handle),
                );
                // Signal frontend that indexing is complete
                let _ = app_handle.emit("index:done", ());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::search_sessions,
            commands::get_session_detail,
            commands::get_related_sessions,
            commands::rebuild_index,
            commands::get_index_stats,
            commands::copy_to_clipboard,
            commands::resume_session,
            commands::toggle_bookmark,
            commands::set_tags,
            commands::get_all_tags,
            commands::get_bookmarked_sessions,
            commands::get_stats,
            commands::get_snippets,
            commands::get_all_snippets,
            commands::get_file_refs,
            commands::open_in_editor,
            commands::get_session_diff,
            commands::export_session_cmd,
            commands::export_sessions_zip_cmd,
            commands::delete_session,
            commands::delete_sessions_files,
            commands::rename_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
