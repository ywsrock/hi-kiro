//! Saved snippets management — CRUD, FTS5 search, similarity detection,
//! title suggestion, and usage statistics.

use anyhow::Result;
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::types::{
    SaveSnippetParams, SavedSnippet, SimilarSnippet, SnippetSearchParams, SnippetStats,
};

// ── Row mapper ────────────────────────────────────────────────────────────────

fn row_to_snippet(row: &rusqlite::Row<'_>) -> rusqlite::Result<SavedSnippet> {
    let tags_json: String = row.get(5)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    Ok(SavedSnippet {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        language: row.get(3)?,
        code: row.get(4)?,
        tags,
        starred: row.get::<_, i32>(6)? != 0,
        source_session_id: row.get(7)?,
        source_cwd: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        use_count: row.get(11)?,
        last_used_at: row.get(12)?,
    })
}

const SELECT: &str = "SELECT id, title, description, language, code, tags, starred,
    source_session_id, source_cwd, created_at, updated_at, use_count, last_used_at
    FROM saved_snippets";

// ── CRUD ──────────────────────────────────────────────────────────────────────

pub fn save_snippet(conn: &Connection, p: &SaveSnippetParams) -> Result<SavedSnippet> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let tags_json = serde_json::to_string(&p.tags)?;

    conn.execute(
        "INSERT INTO saved_snippets
         (id, title, description, language, code, tags, starred, source_session_id,
          source_cwd, created_at, updated_at, use_count, last_used_at)
         VALUES (?1,?2,?3,?4,?5,?6,0,?7,?8,?9,?9,0,0)",
        params![
            id,
            p.title,
            p.description,
            p.language,
            p.code,
            tags_json,
            p.source_session_id,
            p.source_cwd,
            now,
        ],
    )?;

    // FTS index
    conn.execute(
        "INSERT INTO saved_snippets_fts (id, title, description, code, tags)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, p.title, p.description, p.code, tags_json],
    )?;

    get_snippet_by_id(conn, &id)?.ok_or_else(|| anyhow::anyhow!("insert failed"))
}

pub fn get_snippet_by_id(conn: &Connection, id: &str) -> Result<Option<SavedSnippet>> {
    let mut stmt = conn.prepare(&format!("{} WHERE id = ?1", SELECT))?;
    Ok(stmt.query_row(params![id], row_to_snippet).ok())
}

pub fn update_snippet(
    conn: &Connection,
    id: &str,
    title: &str,
    description: &str,
    language: &str,
    code: &str,
    tags: &[String],
) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let tags_json = serde_json::to_string(tags)?;

    conn.execute(
        "UPDATE saved_snippets SET title=?1, description=?2, language=?3, code=?4,
         tags=?5, updated_at=?6 WHERE id=?7",
        params![title, description, language, code, tags_json, now, id],
    )?;

    // Rebuild FTS
    conn.execute("DELETE FROM saved_snippets_fts WHERE id = ?1", params![id])?;
    conn.execute(
        "INSERT INTO saved_snippets_fts (id, title, description, code, tags)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, title, description, code, tags_json],
    )?;
    Ok(())
}

pub fn delete_snippet(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM saved_snippets_fts WHERE id = ?1", params![id])?;
    conn.execute("DELETE FROM saved_snippets WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn toggle_snippet_star(conn: &Connection, id: &str) -> Result<bool> {
    let current: i32 = conn.query_row(
        "SELECT starred FROM saved_snippets WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    let new_val = if current == 0 { 1 } else { 0 };
    conn.execute(
        "UPDATE saved_snippets SET starred=?1 WHERE id=?2",
        params![new_val, id],
    )?;
    Ok(new_val != 0)
}

pub fn increment_use_count(conn: &Connection, id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE saved_snippets SET use_count = use_count + 1, last_used_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

// ── Search ────────────────────────────────────────────────────────────────────

pub fn search_saved_snippets(
    conn: &Connection,
    params_in: &SnippetSearchParams,
) -> Result<Vec<SavedSnippet>> {
    let query = params_in.query.trim();
    let use_fts = !query.is_empty();

    let base = if use_fts {
        format!(
            "{} WHERE id IN (SELECT id FROM saved_snippets_fts WHERE saved_snippets_fts MATCH ?1)",
            SELECT
        )
    } else {
        SELECT.to_string()
    };

    let mut conditions: Vec<String> = Vec::new();
    if let Some(ref lang) = params_in.language {
        if !lang.is_empty() {
            conditions.push(format!("language = '{}'", lang.replace('\'', "''")));
        }
    }
    if params_in.starred_only.unwrap_or(false) {
        conditions.push("starred = 1".to_string());
    }

    let where_extra = if conditions.is_empty() {
        String::new()
    } else if use_fts {
        format!(" AND {}", conditions.join(" AND "))
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    let order = match params_in.sort_by.as_deref().unwrap_or("recent") {
        "used" => "ORDER BY use_count DESC, updated_at DESC",
        "starred" => "ORDER BY starred DESC, updated_at DESC",
        _ => "ORDER BY updated_at DESC",
    };

    let sql = format!("{}{} {} LIMIT 500", base, where_extra, order);
    let mut stmt = conn.prepare(&sql)?;

    let rows = if use_fts {
        let fts_query = format!("\"{}\"*", query.replace('"', "\"\""));
        stmt.query_map(params![fts_query], row_to_snippet)?
            .flatten()
            .collect::<Vec<_>>()
    } else {
        stmt.query_map([], row_to_snippet)?
            .flatten()
            .collect::<Vec<_>>()
    };

    // Tag filter (post-filter)
    let result = if let Some(ref tag_filter) = params_in.tags {
        if tag_filter.is_empty() {
            rows
        } else {
            rows.into_iter()
                .filter(|s| tag_filter.iter().all(|t| s.tags.contains(t)))
                .collect()
        }
    } else {
        rows
    };

    Ok(result)
}

// ── Similarity ────────────────────────────────────────────────────────────────

/// Find snippets similar to given code using normalized edit distance.
/// Only compares snippets of the same language for performance.
pub fn find_similar_snippets(
    conn: &Connection,
    code: &str,
    language: &str,
    exclude_id: Option<&str>,
) -> Result<Vec<SimilarSnippet>> {
    let mut stmt = conn.prepare(&format!(
        "{} WHERE language = ?1 ORDER BY updated_at DESC LIMIT 100",
        SELECT
    ))?;
    let candidates: Vec<SavedSnippet> = stmt
        .query_map(params![language], row_to_snippet)?
        .flatten()
        .collect();

    let code_norm = normalize_code(code);
    let mut similar: Vec<SimilarSnippet> = candidates
        .into_iter()
        .filter(|s| exclude_id.is_none_or(|id| s.id != id))
        .filter_map(|s| {
            let sim = jaccard_similarity(&code_norm, &normalize_code(&s.code));
            if sim > 0.4 {
                Some(SimilarSnippet {
                    snippet: s,
                    similarity: sim,
                })
            } else {
                None
            }
        })
        .collect();

    similar.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(similar.into_iter().take(3).collect())
}

/// Normalize code for comparison: strip comments, whitespace, lowercase.
fn normalize_code(code: &str) -> Vec<String> {
    code.lines()
        .map(|l| l.trim().to_lowercase())
        .filter(|l| {
            !l.is_empty() && !l.starts_with("//") && !l.starts_with("#") && !l.starts_with("--")
        })
        .flat_map(|l| {
            l.split_whitespace()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Jaccard similarity between two token sets.
fn jaccard_similarity(a: &[String], b: &[String]) -> f32 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let set_a: std::collections::HashSet<&String> = a.iter().collect();
    let set_b: std::collections::HashSet<&String> = b.iter().collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    intersection as f32 / union as f32
}

// ── Title Suggestion ──────────────────────────────────────────────────────────

/// Generate a descriptive title from code + language using rule-based extraction.
pub fn suggest_snippet_title(language: &str, code: &str) -> String {
    let lines: Vec<&str> = code.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return format!("{} snippet", language);
    }

    let first = lines[0].trim();

    match language {
        "typescript" | "javascript" => {
            if let Some(name) = extract_fn_name(
                first,
                &[
                    "function ",
                    "const ",
                    "async function ",
                    "export function ",
                    "export const ",
                    "class ",
                ],
            ) {
                return format!("{} - {}", lang_label(language), name);
            }
        }
        "python" => {
            if let Some(name) = extract_fn_name(first, &["def ", "class ", "async def "]) {
                return format!("Python - {}", name);
            }
        }
        "rust" => {
            if let Some(name) = extract_fn_name(
                first,
                &[
                    "fn ",
                    "pub fn ",
                    "pub async fn ",
                    "struct ",
                    "enum ",
                    "impl ",
                ],
            ) {
                return format!("Rust - {}", name);
            }
        }
        "sql" => {
            let upper = first.to_uppercase();
            for kw in &[
                "CREATE TABLE",
                "CREATE INDEX",
                "SELECT",
                "INSERT",
                "UPDATE",
                "DELETE",
                "ALTER",
            ] {
                if upper.starts_with(kw) {
                    let table = extract_sql_target(first);
                    return format!("SQL {} {}", kw, table).trim().to_string();
                }
            }
        }
        "yaml" | "json" => {
            // Extract top-level key as context
            if let Some(key) = first.split(':').next() {
                let key = key.trim().trim_matches('"').trim_matches('\'');
                if !key.is_empty() && key.len() < 30 {
                    return format!("{} - {}", lang_label(language), key);
                }
            }
        }
        "bash" | "shell" => {
            if first.starts_with('#') {
                let comment = first.trim_start_matches('#').trim();
                if !comment.is_empty() && comment.len() < 60 {
                    return comment.to_string();
                }
            }
            if let Some(cmd) = first.split_whitespace().next() {
                return format!("Shell - {} command", cmd);
            }
        }
        _ => {}
    }

    // 言語マッチ失敗時: コードらしい行を探して意味あるタイトルを生成
    // 指示文（自然言語）を除外してコードの特徴的な行を優先する
    let code_line = lines.iter().find(|line| {
        let t = line.trim();
        // 指示文パターンを除外
        let is_instruction = t.ends_with('：')
            || t.ends_with(':')
            || t.ends_with("して")
            || t.ends_with("する")
            || t.ends_with("せよ")
            || t.ends_with("ください")
            || t.ends_with("通訳")
            || t.ends_with("翻訳")
            || t.ends_with("変換")
            || t.ends_with("説明")
            || (t
                .chars()
                .all(|c| c.is_alphanumeric() || c == ' ' || c == '.' || c == ',' || c == '?')
                && !t.contains('{')
                && !t.contains('(')
                && !t.contains('='));
        // コードらしい特徴: 記号・括弧・演算子を含む
        let has_code_chars = t.contains('(')
            || t.contains('{')
            || t.contains('=')
            || t.contains(';')
            || t.starts_with('#')
            || t.starts_with("//")
            || t.starts_with("def ")
            || t.starts_with("fn ")
            || t.starts_with("function ")
            || t.starts_with("class ")
            || t.starts_with("import ")
            || t.starts_with("from ")
            || t.starts_with("use ")
            || t.starts_with("var ")
            || t.starts_with("let ")
            || t.starts_with("const ");
        !is_instruction && (has_code_chars || t.len() > 5)
    });

    let best_line = code_line.unwrap_or(&first);
    let truncated: String = best_line.trim().chars().take(50).collect();
    if truncated.len() < best_line.trim().len() {
        format!("{}…", truncated)
    } else if truncated.is_empty() {
        format!("{} snippet", language)
    } else {
        truncated
    }
}

fn lang_label(lang: &str) -> &str {
    match lang {
        "typescript" => "TypeScript",
        "javascript" => "JavaScript",
        _ => lang,
    }
}

fn extract_fn_name(line: &str, prefixes: &[&str]) -> Option<String> {
    for prefix in prefixes {
        if let Some(rest) = line.strip_prefix(prefix) {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

fn extract_sql_target(line: &str) -> String {
    let words: Vec<&str> = line.split_whitespace().collect();
    // e.g. "CREATE TABLE users" → "users", "CREATE INDEX idx_email ON users" → "users(idx_email)"
    if words.len() >= 3 {
        return words[2].trim_end_matches('(').to_string();
    }
    String::new()
}

// ── Statistics ────────────────────────────────────────────────────────────────

pub fn get_snippet_stats(conn: &Connection) -> Result<SnippetStats> {
    let total_saved: i64 = conn
        .query_row("SELECT COUNT(*) FROM saved_snippets", [], |r| r.get(0))
        .unwrap_or(0);

    let total_uses: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(use_count),0) FROM saved_snippets",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT language, COUNT(*) as cnt FROM saved_snippets GROUP BY language ORDER BY cnt DESC LIMIT 10",
    )?;
    let by_language: Vec<(String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .flatten()
        .collect();

    let mut stmt = conn.prepare(&format!("{} ORDER BY use_count DESC LIMIT 5", SELECT))?;
    let most_used: Vec<SavedSnippet> = stmt.query_map([], row_to_snippet)?.flatten().collect();

    let mut stmt = conn.prepare(&format!("{} ORDER BY created_at DESC LIMIT 5", SELECT))?;
    let recently_added: Vec<SavedSnippet> = stmt.query_map([], row_to_snippet)?.flatten().collect();

    Ok(SnippetStats {
        total_saved,
        total_uses,
        by_language,
        most_used,
        recently_added,
    })
}

/// 保存済みスニペットのタグ一覧を頻度順で返す（オートコンプリート用）
pub fn get_snippet_tags(conn: &Connection) -> anyhow::Result<Vec<(String, i64)>> {
    let mut stmt = conn.prepare(
        r#"SELECT value, COUNT(*) as cnt
           FROM saved_snippets, json_each(saved_snippets.tags)
           GROUP BY value
           ORDER BY cnt DESC, value ASC
           LIMIT 100"#,
    )?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<Vec<_>>();
    Ok(rows.into_iter().flatten().collect())
}
