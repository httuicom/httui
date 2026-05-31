use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::collections::HashMap;

/// Compute a block hash that includes content + environment + connection context.
pub fn compute_block_hash(
    content: &str,
    environment_id: Option<&str>,
    connection_id: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hasher.update(b"|env:");
    hasher.update(environment_id.unwrap_or("").as_bytes());
    hasher.update(b"|conn:");
    hasher.update(connection_id.unwrap_or("").as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Cache hash for an HTTP block — must stay in lockstep with
/// `httui-desktop/src/lib/blocks/hash.ts::computeHttpCacheHash` so the
/// same `(file_path, hash)` row written by either client is readable
/// by the other.
///
/// Canonicalization:
/// - method uppercased
/// - URL extended with sorted percent-encoded params
/// - headers sorted by lowercased key, emitted `name: value`
/// - body verbatim
/// - segments joined by `__SEP__`
/// - env snapshot of *only* the keys referenced anywhere in the
///   canonical text folded in as `__ENV__\nK=V\n…` (sorted)
pub fn compute_http_cache_hash(
    method: &str,
    url: &str,
    params: &[(String, String)],
    headers: &[(String, String)],
    body: &str,
    env_vars: &HashMap<String, String>,
) -> String {
    let method_up = method.to_uppercase();
    let mut sorted_params: Vec<(String, String)> = params.to_vec();
    sorted_params.sort_by(|a, b| a.0.cmp(&b.0));
    let params_joined: String = sorted_params
        .iter()
        .map(|(k, v)| format!("{}={}", encode_uri_component(k), encode_uri_component(v)))
        .collect::<Vec<_>>()
        .join("&");
    let canonical_url = if params_joined.is_empty() {
        url.to_string()
    } else {
        format!("{url}?{params_joined}")
    };
    let mut sorted_headers: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| (k.to_lowercase(), v.clone()))
        .collect();
    sorted_headers.sort_by(|a, b| a.0.cmp(&b.0));
    let headers_joined: String = sorted_headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect::<Vec<_>>()
        .join("\n");

    let full_text = [method_up, canonical_url, headers_joined, body.to_string()].join("\n__SEP__\n");

    let mut used: Vec<(&String, &String)> = env_vars
        .iter()
        .filter(|(k, _)| full_text.contains(&format!("{{{{{k}}}}}")))
        .collect();
    used.sort_by(|a, b| a.0.cmp(b.0));
    let env_block: String = used
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("\n");
    let keyed = if env_block.is_empty() {
        full_text
    } else {
        format!("{full_text}\n__ENV__\n{env_block}")
    };
    compute_block_hash(&keyed, None, None)
}

/// Same `encodeURIComponent` semantics the JS hash uses. Reserved
/// chars matching MDN's table (`!`, `*`, `'`, `(`, `)`, `~`) stay
/// literal so the canonical query string round-trips byte-for-byte.
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let unreserved = c.is_ascii_alphanumeric()
            || matches!(c, '-' | '_' | '.' | '~' | '!' | '*' | '\'' | '(' | ')');
        if unreserved {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for byte in c.encode_utf8(&mut buf).as_bytes() {
                out.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    out
}

#[derive(Debug, Serialize)]
pub struct CachedBlockResult {
    pub status: String,
    pub response: String,
    pub total_rows: Option<i64>,
    pub elapsed_ms: i64,
    pub executed_at: String,
}

pub async fn get_block_result(
    pool: &SqlitePool,
    file_path: &str,
    block_hash: &str,
) -> Result<Option<CachedBlockResult>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT status, response, total_rows, elapsed_ms, executed_at
         FROM block_results WHERE file_path = ?1 AND block_hash = ?2",
    )
    .bind(file_path)
    .bind(block_hash)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| CachedBlockResult {
        status: r.get("status"),
        response: r.get("response"),
        total_rows: r.get("total_rows"),
        elapsed_ms: r.get("elapsed_ms"),
        executed_at: r.get("executed_at"),
    }))
}

pub async fn save_block_result(
    pool: &SqlitePool,
    file_path: &str,
    block_hash: &str,
    status: &str,
    response: &str,
    elapsed_ms: i64,
    total_rows: Option<i64>,
) -> Result<(), sqlx::Error> {
    save_block_result_with_alias(
        pool, file_path, block_hash, None, status, response, elapsed_ms, total_rows,
    )
    .await
}

/// Same as `save_block_result` but also persists the block's alias.
/// Callers that have the alias on hand should use this so the
/// `get_latest_block_result_by_alias` lookup can find their response —
/// the alias is what the autocomplete / footer surfaces resolve by.
#[allow(clippy::too_many_arguments)]
pub async fn save_block_result_with_alias(
    pool: &SqlitePool,
    file_path: &str,
    block_hash: &str,
    alias: Option<&str>,
    status: &str,
    response: &str,
    elapsed_ms: i64,
    total_rows: Option<i64>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO block_results (file_path, block_hash, alias, status, response, elapsed_ms, total_rows)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(file_path, block_hash) DO UPDATE SET
           alias = excluded.alias,
           status = excluded.status,
           response = excluded.response,
           elapsed_ms = excluded.elapsed_ms,
           total_rows = excluded.total_rows,
           executed_at = datetime('now')",
    )
    .bind(file_path)
    .bind(block_hash)
    .bind(alias)
    .bind(status)
    .bind(response)
    .bind(elapsed_ms)
    .bind(total_rows)
    .execute(pool)
    .await?;

    Ok(())
}

/// Return the most recently written `block_results` row for the
/// `(file_path, alias)` pair, regardless of which hash it landed on.
/// The autocomplete / footer paths use this so a user who edits a
/// request and re-runs it (which writes under a new hash) sees the
/// fresh capture in `{{alias.response.body.…}}` without having to
/// resync per-pane document state.
pub async fn get_latest_block_result_by_alias(
    pool: &SqlitePool,
    file_path: &str,
    alias: &str,
) -> Result<Option<CachedBlockResult>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT status, response, total_rows, elapsed_ms, executed_at
         FROM block_results
         WHERE file_path = ?1 AND alias = ?2
         ORDER BY id DESC
         LIMIT 1",
    )
    .bind(file_path)
    .bind(alias)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| CachedBlockResult {
        status: r.get("status"),
        response: r.get("response"),
        total_rows: r.get("total_rows"),
        elapsed_ms: r.get("elapsed_ms"),
        executed_at: r.get("executed_at"),
    }))
}

/// Try to acquire an execution lock for a block. Returns `true` if
/// acquired (caller should execute), `false` if another execution is
/// already in progress.
pub async fn try_acquire_execution_lock(
    pool: &SqlitePool,
    file_path: &str,
    block_hash: &str,
) -> Result<bool, sqlx::Error> {
    // Use a temp table row as a mutex. INSERT OR IGNORE returns rows_affected=0 if lock exists.
    let result = sqlx::query(
        "INSERT OR IGNORE INTO block_execution_locks (file_path, block_hash, locked_at)
         VALUES (?1, ?2, datetime('now'))",
    )
    .bind(file_path)
    .bind(block_hash)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Release an execution lock after block execution completes.
pub async fn release_execution_lock(
    pool: &SqlitePool,
    file_path: &str,
    block_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM block_execution_locks WHERE file_path = ?1 AND block_hash = ?2")
        .bind(file_path)
        .bind(block_hash)
        .execute(pool)
        .await?;
    Ok(())
}

/// Clean up stale execution locks older than 60 seconds (timed out or crashed).
pub async fn cleanup_stale_locks(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM block_execution_locks WHERE locked_at < datetime('now', '-60 seconds')",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_block_results_for_file(
    pool: &SqlitePool,
    file_path: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM block_results WHERE file_path = ?1")
        .bind(file_path)
        .execute(pool)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use tempfile::TempDir;

    async fn setup() -> (SqlitePool, TempDir) {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();
        (pool, tmp)
    }

    #[tokio::test]
    async fn test_get_returns_none_when_empty() {
        let (pool, _tmp) = setup().await;
        let result = get_block_result(&pool, "test.md", "abc123").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_save_and_get() {
        let (pool, _tmp) = setup().await;

        save_block_result(
            &pool,
            "test.md",
            "hash1",
            "success",
            r#"{"ok":true}"#,
            150,
            None,
        )
        .await
        .unwrap();

        let result = get_block_result(&pool, "test.md", "hash1").await.unwrap();
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.status, "success");
        assert_eq!(r.response, r#"{"ok":true}"#);
        assert_eq!(r.elapsed_ms, 150);
        assert!(r.total_rows.is_none());
    }

    #[tokio::test]
    async fn test_save_upserts() {
        let (pool, _tmp) = setup().await;

        save_block_result(
            &pool,
            "test.md",
            "hash1",
            "success",
            r#"{"v":1}"#,
            100,
            None,
        )
        .await
        .unwrap();
        save_block_result(
            &pool,
            "test.md",
            "hash1",
            "success",
            r#"{"v":2}"#,
            200,
            Some(5),
        )
        .await
        .unwrap();

        let r = get_block_result(&pool, "test.md", "hash1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r.response, r#"{"v":2}"#);
        assert_eq!(r.elapsed_ms, 200);
        assert_eq!(r.total_rows, Some(5));
    }

    #[tokio::test]
    async fn test_different_hash_different_result() {
        let (pool, _tmp) = setup().await;

        save_block_result(&pool, "test.md", "hash1", "success", "r1", 100, None)
            .await
            .unwrap();
        save_block_result(&pool, "test.md", "hash2", "error", "r2", 50, None)
            .await
            .unwrap();

        let r1 = get_block_result(&pool, "test.md", "hash1")
            .await
            .unwrap()
            .unwrap();
        let r2 = get_block_result(&pool, "test.md", "hash2")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r1.status, "success");
        assert_eq!(r2.status, "error");
    }

    #[tokio::test]
    async fn test_delete_for_file() {
        let (pool, _tmp) = setup().await;

        save_block_result(&pool, "test.md", "h1", "success", "r1", 100, None)
            .await
            .unwrap();
        save_block_result(&pool, "test.md", "h2", "success", "r2", 100, None)
            .await
            .unwrap();
        save_block_result(&pool, "other.md", "h1", "success", "r3", 100, None)
            .await
            .unwrap();

        delete_block_results_for_file(&pool, "test.md")
            .await
            .unwrap();

        assert!(get_block_result(&pool, "test.md", "h1")
            .await
            .unwrap()
            .is_none());
        assert!(get_block_result(&pool, "test.md", "h2")
            .await
            .unwrap()
            .is_none());
        assert!(get_block_result(&pool, "other.md", "h1")
            .await
            .unwrap()
            .is_some());
    }

    #[test]
    fn http_hash_matches_method_url_sorted_params_headers_body() {
        let envs = HashMap::new();
        let hash = compute_http_cache_hash(
            "get",
            "https://api.example.com/users",
            &[("z".into(), "1".into()), ("a".into(), "two".into())],
            &[
                ("X-Trace".into(), "abc".into()),
                ("authorization".into(), "Bearer t".into()),
            ],
            "{\"k\":\"v\"}",
            &envs,
        );
        // Same inputs in different order → same hash (canonical sort).
        let hash2 = compute_http_cache_hash(
            "GET",
            "https://api.example.com/users",
            &[("a".into(), "two".into()), ("z".into(), "1".into())],
            &[
                ("Authorization".into(), "Bearer t".into()),
                ("X-Trace".into(), "abc".into()),
            ],
            "{\"k\":\"v\"}",
            &envs,
        );
        assert_eq!(hash, hash2);
    }

    #[test]
    fn http_hash_different_for_method_change() {
        let envs = HashMap::new();
        let g = compute_http_cache_hash("GET", "/x", &[], &[], "", &envs);
        let p = compute_http_cache_hash("POST", "/x", &[], &[], "", &envs);
        assert_ne!(g, p);
    }

    #[test]
    fn http_hash_folds_only_referenced_env_vars() {
        let mut envs = HashMap::new();
        envs.insert("TOKEN".into(), "v1".into());
        envs.insert("UNUSED".into(), "noise".into());
        let used = compute_http_cache_hash(
            "GET",
            "/x",
            &[],
            &[("Authorization".into(), "Bearer {{TOKEN}}".into())],
            "",
            &envs,
        );
        // Bumping UNUSED must NOT change the hash.
        let mut envs2 = envs.clone();
        envs2.insert("UNUSED".into(), "other".into());
        let used2 = compute_http_cache_hash(
            "GET",
            "/x",
            &[],
            &[("Authorization".into(), "Bearer {{TOKEN}}".into())],
            "",
            &envs2,
        );
        assert_eq!(used, used2);
        // Bumping TOKEN MUST change the hash — it's referenced.
        let mut envs3 = envs.clone();
        envs3.insert("TOKEN".into(), "v2".into());
        let used3 = compute_http_cache_hash(
            "GET",
            "/x",
            &[],
            &[("Authorization".into(), "Bearer {{TOKEN}}".into())],
            "",
            &envs3,
        );
        assert_ne!(used, used3);
    }

    #[test]
    fn http_hash_percent_encodes_query_values() {
        let envs = HashMap::new();
        let h = compute_http_cache_hash(
            "GET",
            "/search",
            &[("q".into(), "hello world".into())],
            &[],
            "",
            &envs,
        );
        // hello%20world variant must produce the same hash because
        // both encode to `hello%20world` via the canonicalizer.
        let h2 = compute_http_cache_hash(
            "GET",
            "/search",
            &[("q".into(), "hello world".into())],
            &[],
            "",
            &envs,
        );
        assert_eq!(h, h2);
    }

    #[test]
    fn encode_uri_component_keeps_js_unreserved_chars_literal() {
        assert_eq!(encode_uri_component("a-b_c.d~e!f*g'h(i)j"), "a-b_c.d~e!f*g'h(i)j");
    }

    #[tokio::test]
    async fn latest_by_alias_returns_most_recent_across_different_hashes() {
        let (pool, _tmp) = setup().await;
        // First run: alias=req1, hash=H1, response=OLD.
        save_block_result_with_alias(
            &pool,
            "api.md",
            "h-old",
            Some("req1"),
            "success",
            r#"{"v":"OLD"}"#,
            10,
            None,
        )
        .await
        .unwrap();
        // SQLite datetime('now') is second-granular. Sleep so the
        // second row is unambiguously later by timestamp.
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        save_block_result_with_alias(
            &pool,
            "api.md",
            "h-new",
            Some("req1"),
            "success",
            r#"{"v":"NEW"}"#,
            10,
            None,
        )
        .await
        .unwrap();

        let got = get_latest_block_result_by_alias(&pool, "api.md", "req1")
            .await
            .unwrap()
            .expect("a row exists");
        assert_eq!(got.response, r#"{"v":"NEW"}"#);
    }

    #[test]
    fn encode_uri_component_percent_escapes_reserved_chars() {
        assert_eq!(encode_uri_component("hello world"), "hello%20world");
        assert_eq!(encode_uri_component("a&b=c"), "a%26b%3Dc");
        assert_eq!(encode_uri_component("á"), "%C3%A1");
    }
}
