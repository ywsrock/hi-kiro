// 将来の使用のために定義されている定数も含む
#![allow(dead_code)]

//! アプリ全体で使用する定数
//! マジックナンバーを一箇所に集約し、変更を追いやすくする

// ── タグ ──────────────────────────────────────────────────────────────────────

/// タグのデフォルトカラー（ダーク系グレー）
pub const TAG_COLOR_DEFAULT: &str = "#334155";

/// タグ名の最大文字数（# を含まない）
pub const TAG_NAME_MAX_LEN: usize = 29;

/// タグ名の最小文字数（# を含まない）
pub const TAG_NAME_MIN_LEN: usize = 1;

/// 1セッションに付けられるタグの最大数
pub const TAG_MAX_PER_SESSION: usize = 10;

// ── 検索 ──────────────────────────────────────────────────────────────────────

/// デフォルトの検索件数上限
pub const SEARCH_LIMIT_DEFAULT: u32 = 200;

// ── エクスポート ──────────────────────────────────────────────────────────────

/// ZIPファイル名に付与するセッションIDプレフィックス長
pub const EXPORT_ID_PREFIX_LEN: usize = 8;

// ── インデックス ──────────────────────────────────────────────────────────────

/// インデックス進捗ログの出力間隔（件数）
pub const INDEX_LOG_INTERVAL: usize = 10;

/// 大規模インデックスの進捗ログ間隔（件数）
pub const INDEX_LOG_INTERVAL_LARGE: usize = 50;

// ── SQLite ────────────────────────────────────────────────────────────────────

/// FTS5キャッシュサイズ（バイト、負値=KB）
pub const SQLITE_CACHE_SIZE_KB: i32 = -32768; // 32MB

/// SQLite mmap サイズ（バイト）
pub const SQLITE_MMAP_SIZE: u64 = 268_435_456; // 256MB

// ── ログ ──────────────────────────────────────────────────────────────────────

/// ログファイルのローテーションサイズ上限（バイト）
pub const LOG_MAX_FILE_SIZE: u128 = 10 * 1024 * 1024; // 10MB

/// 保持するログファイルの最大数
pub const LOG_MAX_FILES: usize = 5;

// ── モデル価格表 ──────────────────────────────────────────────────────────────
// 最終更新: 2025-08 (Anthropic公式価格に基づく)
// 価格改定時はこのセクションを更新してください
// 参考: https://www.anthropic.com/pricing
// 単位: $/1M tokens

/// Claude 3 Opus: 入力単価 $/1M tokens
pub const PRICE_OPUS_INPUT: f64 = 15.0;
/// Claude 3 Opus: 出力単価 $/1M tokens
pub const PRICE_OPUS_OUTPUT: f64 = 75.0;
/// Claude 3 Opus: コンテキストウィンドウ
pub const CTX_OPUS: u64 = 200_000;

/// Claude 3 Haiku: 入力単価 $/1M tokens
pub const PRICE_HAIKU_INPUT: f64 = 0.8;
/// Claude 3 Haiku: 出力単価 $/1M tokens
pub const PRICE_HAIKU_OUTPUT: f64 = 4.0;
/// Claude 3 Haiku: コンテキストウィンドウ
pub const CTX_HAIKU: u64 = 200_000;

/// Claude 3 Sonnet (デフォルト): 入力単価 $/1M tokens
pub const PRICE_SONNET_INPUT: f64 = 3.0;
/// Claude 3 Sonnet (デフォルト): 出力単価 $/1M tokens
pub const PRICE_SONNET_OUTPUT: f64 = 15.0;
/// Claude 3 Sonnet: コンテキストウィンドウ
pub const CTX_SONNET: u64 = 200_000;
