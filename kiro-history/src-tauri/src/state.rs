use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub sessions_dir: Option<String>,
    pub sqlite_db_path: Option<String>,
    pub theme: Option<String>, // "system" | "dark" | "light"
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            sessions_dir: None,
            sqlite_db_path: None,
            theme: Some("system".to_string()),
        }
    }
}

/// Shared application state managed by Tauri.
pub struct AppState {
    pub index_conn: Connection,
    pub index_db_path: PathBuf,
    pub sessions_dir: PathBuf,
    pub sqlite_db_path: PathBuf,
    #[allow(dead_code)]
    pub config: AppConfig,
}

impl AppState {
    pub fn new(
        index_conn: Connection,
        index_db_path: PathBuf,
        sessions_dir: PathBuf,
        sqlite_db_path: PathBuf,
        config: AppConfig,
    ) -> Self {
        AppState {
            index_conn,
            index_db_path,
            sessions_dir,
            sqlite_db_path,
            config,
        }
    }
}

/// Resolve platform-specific paths.
pub fn resolve_paths(config: &AppConfig) -> (PathBuf, PathBuf, PathBuf) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let data = dirs::data_dir().unwrap_or_else(|| home.join(".local/share"));

    let sessions_dir = config
        .sessions_dir
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".kiro/sessions/cli"));

    let sqlite_db = config
        .sqlite_db_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| data.join("kiro-cli/data.sqlite3"));

    let index_db = data.join("kiro-history/index.db");

    (sessions_dir, sqlite_db, index_db)
}

/// Load config from disk.
pub fn load_config() -> AppConfig {
    let config_path = dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
        .join("kiro-history/config.json");

    if config_path.exists() {
        if let Ok(s) = std::fs::read_to_string(&config_path) {
            if let Ok(c) = serde_json::from_str::<AppConfig>(&s) {
                return c;
            }
        }
    }
    AppConfig::default()
}

/// Save config to disk.
pub fn save_config(config: &AppConfig) -> anyhow::Result<()> {
    let config_path = dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
        .join("kiro-history/config.json");

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(config_path, json)?;
    Ok(())
}
