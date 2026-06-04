//! Rehydrate `BlockNode.cached_result` on document load.
//!
//! Each `Segment::Block` in the just-parsed `Document` is keyed back
//! against `block_results` (the same SQLite table the desktop writes)
//! and, when a row exists, the deserialized response is restored to
//! the in-memory node so the BLOCKS view's Response/Result cards
//! paint without a re-run and so `{{alias.body.…}}` autocomplete can
//! materialize the captured value.
//!
//! Best-effort: a missing row / serialization failure / pool error is
//! swallowed (logged via `tracing::warn`), the block keeps
//! `cached_result = None`. The function never errors.

use std::collections::HashMap;
use std::path::Path;

use sqlx::sqlite::SqlitePool;

use crate::buffer::{Document, Segment};

/// Walk `doc`'s blocks and load any persisted response into
/// `cached_result`. `file_path` is the canonical key used by the
/// runners (relative or absolute — caller's choice, both clients use
/// the same shape).
pub async fn hydrate_document(
    pool: &SqlitePool,
    doc: &mut Document,
    env_vars: &HashMap<String, String>,
    file_path: &Path,
) {
    let _ = env_vars; // alias-keyed lookup ignores env folding
    let file_key = file_path.to_string_lossy().to_string();
    let segment_count = doc.segments().len();
    for idx in 0..segment_count {
        let alias = match doc.segments().get(idx) {
            Some(Segment::Block(b)) => match b.alias.as_deref() {
                Some(a) if !a.trim().is_empty() => a.to_string(),
                _ => continue,
            },
            _ => continue,
        };
        let row = match httui_core::block_results::get_latest_block_result_by_alias(
            pool, &file_key, &alias,
        )
        .await
        {
            Ok(row) => row,
            Err(e) => {
                tracing::warn!("hydrate: latest_by_alias {file_key} failed: {e}");
                continue;
            }
        };
        let Some(cached) = row else { continue };
        let value: serde_json::Value = match serde_json::from_str(&cached.response) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("hydrate: parse cached body failed: {e}");
                continue;
            }
        };
        if let Some(b) = doc.block_at_mut(idx) {
            b.cached_result = Some(value);
        }
    }
}

#[allow(dead_code)]
fn compute_hash_for(
    block: &crate::buffer::block::BlockNode,
    env_vars: &HashMap<String, String>,
) -> Option<String> {
    if block.block_type == "http" {
        let method = block
            .params
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_string();
        let url = block
            .params
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let params = enabled_pairs(block.params.get("params"));
        let headers = enabled_pairs(block.params.get("headers"));
        let body = block
            .params
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        return Some(httui_core::block_results::compute_http_cache_hash(
            &method, &url, &params, &headers, &body, env_vars,
        ));
    }
    if block.block_type.starts_with("db") {
        let body = block
            .params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let conn_id = block
            .params
            .get("connection")
            .or_else(|| block.params.get("connection_id"))
            .and_then(|v| v.as_str())
            .map(str::to_string);
        return Some(crate::commands::db::compute_db_cache_hash(
            &body,
            conn_id.as_deref(),
            env_vars,
        ));
    }
    None
}

#[allow(dead_code)]
fn enabled_pairs(value: Option<&serde_json::Value>) -> Vec<(String, String)> {
    let Some(arr) = value.and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    arr.iter()
        .filter(|item| {
            item.get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true)
        })
        .map(|item| {
            let k = item
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let v = item
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (k, v)
        })
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

/// Re-attach `cached_result` to each block in an owned `segments`
/// vec by looking up the canonical hash in `block_results`. Used by
/// the ref-completion path so the popup sees the latest captured
/// value even when another pane (or this same pane in a different
/// session) just refreshed it. Best-effort — skips on any error.
pub async fn hydrate_segments(
    pool: &SqlitePool,
    segments: &mut [crate::buffer::Segment],
    env_vars: &HashMap<String, String>,
    file_path: &Path,
) {
    let _ = env_vars; // alias-keyed lookup ignores env folding
    let file_key = file_path.to_string_lossy().to_string();
    for seg in segments.iter_mut() {
        let crate::buffer::Segment::Block(b) = seg else {
            continue;
        };
        let Some(alias) = b.alias.as_deref().filter(|a| !a.trim().is_empty()) else {
            continue;
        };
        let row = match httui_core::block_results::get_latest_block_result_by_alias(
            pool, &file_key, alias,
        )
        .await
        {
            Ok(r) => r,
            Err(_) => continue,
        };
        let Some(cached) = row else { continue };
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&cached.response) {
            b.cached_result = Some(value);
        }
    }
}

pub fn hydrate_segments_blocking(
    pool: &SqlitePool,
    segments: &mut [crate::buffer::Segment],
    env_vars: &HashMap<String, String>,
    file_path: &Path,
) {
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(hydrate_segments(pool, segments, env_vars, file_path))
    });
}

/// Sync wrapper for callers that live on the dispatch thread (event
/// loop, document loader). Swallows the panic path of
/// `block_in_place` outside a Tokio runtime by treating it as "no
/// hydration this load" — the next event with a runtime context will
/// pick it up.
pub fn hydrate_document_blocking(
    pool: &SqlitePool,
    doc: &mut Document,
    env_vars: &HashMap<String, String>,
    file_path: &Path,
) {
    if tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(hydrate_document(pool, doc, env_vars, file_path))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::block_results::{compute_http_cache_hash, save_block_result_with_alias};
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn pool() -> (SqlitePool, TempDir) {
        let tmp = TempDir::new().unwrap();
        let p = init_db(tmp.path()).await.unwrap();
        (p, tmp)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn http_block_picks_up_persisted_response() {
        let (pool, _tmp) = pool().await;
        let envs: HashMap<String, String> = HashMap::new();
        let md = "```http alias=ping\nGET https://example.com/health\n```\n";
        let mut doc = Document::from_markdown(md).unwrap();
        let file_path = std::path::PathBuf::from("api.md");

        // Pre-save a fake response keyed by alias so hydration's
        // latest-by-alias lookup has something to find.
        let hash =
            compute_http_cache_hash("GET", "https://example.com/health", &[], &[], "", &envs);
        save_block_result_with_alias(
            &pool,
            "api.md",
            &hash,
            Some("ping"),
            "success",
            r#"{"status":200,"body":{"ok":true}}"#,
            42,
            None,
        )
        .await
        .unwrap();

        hydrate_document(&pool, &mut doc, &envs, &file_path).await;

        let cached = doc
            .segments()
            .iter()
            .find_map(|s| match s {
                Segment::Block(b) if b.alias.as_deref() == Some("ping") => b.cached_result.clone(),
                _ => None,
            })
            .expect("cached result populated");
        assert_eq!(cached.get("status").and_then(|v| v.as_i64()), Some(200));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn missing_row_leaves_cached_result_none() {
        let (pool, _tmp) = pool().await;
        let envs: HashMap<String, String> = HashMap::new();
        let md = "```http alias=ping\nGET https://nowhere\n```\n";
        let mut doc = Document::from_markdown(md).unwrap();
        hydrate_document(&pool, &mut doc, &envs, std::path::Path::new("api.md")).await;
        let has_cache = doc.segments().iter().any(|s| match s {
            Segment::Block(b) => b.cached_result.is_some(),
            _ => false,
        });
        assert!(!has_cache);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enabled_pairs_filters_disabled_and_empty() {
        let v = serde_json::json!([
            {"key": "Accept", "value": "json"},
            {"key": "X-Off", "value": "v", "enabled": false},
            {"key": "", "value": "noop"},
        ]);
        let out = enabled_pairs(Some(&v));
        assert_eq!(out, vec![("Accept".into(), "json".into())]);
        assert!(enabled_pairs(None).is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn compute_hash_for_returns_none_for_unknown_block_type() {
        // Any block_type the registry doesn't know (e.g. a hypothetical
        // `grpc` block) has no cacheable result — `compute_hash_for`
        // returns `None` so hydration silently skips it.
        let node = crate::buffer::block::BlockNode {
            id: crate::buffer::block::BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "grpc".into(),
            alias: None,
            display_mode: None,
            params: serde_json::json!({}),
            state: crate::buffer::block::ExecutionState::Idle,
            cached_result: None,
        };
        assert!(compute_hash_for(&node, &HashMap::new()).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn corrupt_cache_body_is_skipped_not_panic() {
        let (pool, _tmp) = pool().await;
        let envs: HashMap<String, String> = HashMap::new();
        let md = "```http alias=ping\nGET https://example.com/health\n```\n";
        let mut doc = Document::from_markdown(md).unwrap();
        let hash =
            compute_http_cache_hash("GET", "https://example.com/health", &[], &[], "", &envs);
        save_block_result_with_alias(
            &pool,
            "api.md",
            &hash,
            Some("ping"),
            "success",
            "not json{{{",
            1,
            None,
        )
        .await
        .unwrap();
        hydrate_document(&pool, &mut doc, &envs, std::path::Path::new("api.md")).await;
        let has_cache = doc.segments().iter().any(|s| match s {
            Segment::Block(b) => b.cached_result.is_some(),
            _ => false,
        });
        assert!(!has_cache);
    }
}
