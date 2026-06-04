//! SQL classifiers + cache helpers (hash, summary, on-disk save).

/// Strip leading whitespace + line/block comments so query classifiers
/// see the first *real* statement word.
pub fn strip_leading_sql_comments(query: &str) -> &str {
    let mut s = query.trim_start();
    loop {
        if let Some(rest) = s.strip_prefix("--") {
            s = match rest.find('\n') {
                Some(idx) => rest[idx + 1..].trim_start(),
                None => "",
            };
        } else if let Some(rest) = s.strip_prefix("/*") {
            s = match rest.find("*/") {
                Some(idx) => rest[idx + 2..].trim_start(),
                None => "",
            };
        } else {
            break;
        }
    }
    s
}

/// Read-only statements cache (SELECT/EXPLAIN/WITH/SHOW/PRAGMA/DESC);
/// anything else (UPDATE/DELETE/INSERT/DDL) bypasses the cache.
pub fn is_cacheable_query(query: &str) -> bool {
    let s = strip_leading_sql_comments(query);
    let first_word: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    matches!(
        first_word.to_ascii_uppercase().as_str(),
        "SELECT" | "WITH" | "EXPLAIN" | "SHOW" | "PRAGMA" | "DESC" | "DESCRIBE"
    )
}

/// Read-only gate uses this — strict allowlist. Anything not
/// recognized as a write counts as a read (safer default: let a weird
/// read through rather than block one).
pub fn is_writing_query(query: &str) -> bool {
    let s = strip_leading_sql_comments(query);
    let first_word: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    matches!(
        first_word.to_ascii_uppercase().as_str(),
        "UPDATE"
            | "DELETE"
            | "INSERT"
            | "REPLACE"
            | "MERGE"
            | "CREATE"
            | "DROP"
            | "ALTER"
            | "TRUNCATE"
            | "GRANT"
            | "REVOKE"
            | "VACUUM"
    )
}

/// `UPDATE`/`DELETE` without a `WHERE` clause — the kind of slip that
/// nukes an entire table. Word-boundary aware so `whereabouts` doesn't
/// register as `WHERE`.
pub fn is_unscoped_destructive(query: &str) -> bool {
    let s = strip_leading_sql_comments(query);
    let first_word: String = s.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let kind = first_word.to_ascii_uppercase();
    if kind != "UPDATE" && kind != "DELETE" {
        return false;
    }
    let stmt_end = s.find(';').unwrap_or(s.len());
    let stmt = &s[..stmt_end];
    let upper = stmt.to_ascii_uppercase();
    let mut start = 0;
    while let Some(pos) = upper[start..].find("WHERE") {
        let abs = start + pos;
        let before_ok = abs == 0
            || !upper.as_bytes()[abs - 1].is_ascii_alphanumeric()
                && upper.as_bytes()[abs - 1] != b'_';
        let after = abs + 5;
        let after_ok = after >= upper.len()
            || (!upper.as_bytes()[after].is_ascii_alphanumeric()
                && upper.as_bytes()[after] != b'_');
        if before_ok && after_ok {
            return false;
        }
        start = abs + 5;
    }
    true
}

/// Hash text is the raw SQL body plus, when any env vars are
/// referenced via `{{KEY}}`, a sorted `KEY=VALUE` snapshot of just
/// those vars. Connection id is a separate hash input so the same
/// query against two connections can't collide.
pub fn compute_db_cache_hash(
    body: &str,
    conn_id: Option<&str>,
    env_vars: &std::collections::HashMap<String, String>,
) -> String {
    let mut used: Vec<(&String, &String)> = env_vars
        .iter()
        .filter(|(k, _)| body.contains(&format!("{{{{{k}}}}}")))
        .collect();
    used.sort_by(|a, b| a.0.cmp(b.0));
    let env_block: String = used
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");
    let keyed = if env_block.is_empty() {
        body.to_string()
    } else {
        format!("{body}\n__ENV__\n{env_block}")
    };
    httui_core::block_results::compute_block_hash(&keyed, None, conn_id)
}

/// One-liner status (`⛁ cached · …`) driven by the deserialized cache
/// `Value`. Errors with position get an ` at L:C` suffix matching
/// `summarize_db_response`.
pub fn db_summary_from_value(value: Option<&serde_json::Value>, elapsed: u64) -> String {
    let Some(v) = value else {
        return format!("ok · {elapsed}ms");
    };
    let results = v.get("results").and_then(|r| r.as_array());
    let extras = match results.map(|r| r.len()).unwrap_or(0) {
        0 | 1 => String::new(),
        n => format!(" (+{} more)", n - 1),
    };
    let first = results.and_then(|r| r.first());
    let kind = first.and_then(|f| f.get("kind")).and_then(|k| k.as_str());
    match kind {
        Some("select") => {
            let rows = first
                .and_then(|f| f.get("rows"))
                .and_then(|r| r.as_array())
                .map(|r| r.len())
                .unwrap_or(0);
            let has_more = first
                .and_then(|f| f.get("has_more"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let suffix = if has_more { "+" } else { "" };
            format!("{rows}{suffix} rows · {elapsed}ms{extras}")
        }
        Some("mutation") => {
            let affected = first
                .and_then(|f| f.get("rows_affected"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            format!("{affected} affected · {elapsed}ms{extras}")
        }
        Some("error") => first
            .and_then(|f| f.get("message"))
            .and_then(|v| v.as_str())
            .map(|m| {
                let pos = first
                    .and_then(|f| f.get("line"))
                    .and_then(|l| l.as_u64())
                    .map(|line| {
                        let col = first
                            .and_then(|f| f.get("column"))
                            .and_then(|c| c.as_u64())
                            .unwrap_or(1);
                        format!(" at {line}:{col}")
                    })
                    .unwrap_or_default();
                format!("error: {m}{pos}{extras}")
            })
            .unwrap_or_else(|| format!("error · {elapsed}ms")),
        _ => format!("ok · {elapsed}ms{extras}"),
    }
}

/// Fire-and-forget save to the on-disk cache. Failure is logged but
/// never surfaces — cache writes are best-effort. `alias` is optional
/// — anonymous blocks still get a hash-keyed row but won't be findable
/// by `get_latest_block_result_by_alias`.
#[allow(clippy::too_many_arguments)]
pub fn save_db_cache_async(
    pool: sqlx::SqlitePool,
    file_path: String,
    hash: String,
    alias: Option<String>,
    value: serde_json::Value,
    elapsed_ms: u64,
    results: &[httui_core::executor::db::types::DbResult],
) {
    use httui_core::executor::db::types::DbResult;
    let total_rows: Option<i64> = results.first().and_then(|r| match r {
        DbResult::Select { rows, .. } => Some(rows.len() as i64),
        _ => None,
    });
    let response_str = match serde_json::to_string(&value) {
        Ok(s) => s,
        Err(_) => return,
    };
    tokio::spawn(async move {
        let _ = httui_core::block_results::save_block_result_with_alias(
            &pool,
            &file_path,
            &hash,
            alias.as_deref(),
            "success",
            &response_str,
            elapsed_ms as i64,
            total_rows,
        )
        .await;
    });
}
