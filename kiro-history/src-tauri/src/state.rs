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

/// Windows: wsl.exe --list でディストリビューション一覧を取得
#[allow(dead_code)]
pub fn list_wsl_distros() -> Vec<String> {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("wsl.exe")
            .args(["--list", "--quiet"])
            .output();
        if let Ok(out) = output {
            // wsl --list は UTF-16LE で出力する
            let raw = out.stdout;
            let utf16: Vec<u16> = raw
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let s = String::from_utf16_lossy(&utf16);
            return s
                .lines()
                .map(|l| {
                    // BOM (U+FEFF) と null文字を除去
                    l.trim()
                        .trim_start_matches('\u{FEFF}')
                        .trim_end_matches('\0')
                        .to_string()
                })
                .filter(|l| !l.is_empty())
                .collect();
        }
        vec![]
    }
    #[cfg(not(target_os = "windows"))]
    {
        vec![]
    }
}

/// Windows: WSL内のユーザーホームを取得
#[allow(dead_code)]
pub fn get_wsl_home(distro: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("wsl.exe")
            .args(["-d", distro, "--", "sh", "-c", "echo $HOME"])
            .output()
            .ok()?;
        let home = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if home.is_empty() {
            None
        } else {
            Some(home)
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = distro;
        None
    }
}

/// Windows UNCパスを構築するヘルパー
/// \\wsl$\<distro>\home\user\.kiro\sessions\cli
#[allow(dead_code)]
fn wsl_unc_path(prefix: &str, distro: &str, linux_path: &str) -> PathBuf {
    // linux path: /home/user/.kiro/... → \home\user\.kiro\...
    let win_relative = linux_path.replace('/', r"\");
    PathBuf::from(format!(r"\\{}\{}{}", prefix, distro, win_relative))
}

/// データパス自動検出結果
#[derive(Debug, Serialize, Deserialize)]
pub struct DetectedPaths {
    pub sessions_dir: Option<String>,
    pub sqlite_db_path: Option<String>,
    pub distro: Option<String>,
}

/// Windows: WSLパスを検出して DetectedPaths を返す
pub fn detect_wsl_paths() -> DetectedPaths {
    #[cfg(target_os = "windows")]
    {
        for distro in list_wsl_distros() {
            if let Some(home) = get_wsl_home(&distro) {
                let sessions_linux = format!("{}/.kiro/sessions/cli", home);
                let sqlite_linux = format!("{}/.local/share/kiro-cli/data.sqlite3", home);

                // \\wsl$ と \\wsl.localhost の両方を試す
                let prefixes = ["wsl$", "wsl.localhost"];
                let mut found_sessions: Option<PathBuf> = None;
                let mut found_sqlite: Option<PathBuf> = None;

                'outer: for prefix in &prefixes {
                    let s = wsl_unc_path(prefix, &distro, &sessions_linux);
                    let db = wsl_unc_path(prefix, &distro, &sqlite_linux);
                    if s.exists() || db.exists() {
                        found_sessions = Some(s);
                        found_sqlite = Some(db);
                        break 'outer;
                    }
                }

                if found_sessions.is_some() || found_sqlite.is_some() {
                    return DetectedPaths {
                        sessions_dir: found_sessions.map(|p| p.to_string_lossy().to_string()),
                        sqlite_db_path: found_sqlite.map(|p| p.to_string_lossy().to_string()),
                        distro: Some(distro),
                    };
                }
            }
        }
        DetectedPaths {
            sessions_dir: None,
            sqlite_db_path: None,
            distro: None,
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        DetectedPaths {
            sessions_dir: None,
            sqlite_db_path: None,
            distro: None,
        }
    }
}

/// Resolve platform-specific paths.
pub fn resolve_paths(config: &AppConfig) -> (PathBuf, PathBuf, PathBuf) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let data = dirs::data_dir().unwrap_or_else(|| home.join(".local/share"));

    // config で明示指定されている場合はそれを優先
    // Windows の場合: WSL2 パスを1回だけ検出して使い回す
    #[cfg(target_os = "windows")]
    let detected = detect_wsl_paths();

    let sessions_dir = if let Some(ref p) = config.sessions_dir {
        PathBuf::from(p)
    } else {
        #[cfg(target_os = "windows")]
        {
            detected
                .sessions_dir
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".kiro/sessions/cli"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            home.join(".kiro/sessions/cli")
        }
    };

    let sqlite_db = if let Some(ref p) = config.sqlite_db_path {
        PathBuf::from(p)
    } else {
        #[cfg(target_os = "windows")]
        {
            detected
                .sqlite_db_path
                .as_deref()
                .map(PathBuf::from)
                .unwrap_or_else(|| data.join("kiro-cli/data.sqlite3"))
        }
        #[cfg(not(target_os = "windows"))]
        {
            data.join("kiro-cli/data.sqlite3")
        }
    };

    let index_db = data.join("hi-kiro/index.db");

    (sessions_dir, sqlite_db, index_db)
}

/// Load config from disk.
pub fn load_config() -> AppConfig {
    let config_path = dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
        .join("hi-kiro/config.json");

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
#[allow(dead_code)]
pub fn save_config(config: &AppConfig) -> anyhow::Result<()> {
    let config_path = dirs::data_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from("/")))
        .join("hi-kiro/config.json");

    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(config_path, json)?;
    Ok(())
}
