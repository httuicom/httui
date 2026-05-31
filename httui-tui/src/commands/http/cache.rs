//! HTTP block cache write — fire-and-forget save of the JSON response
//! to `block_results` after a successful run, keyed by the canonical
//! hash from `httui_core::block_results::compute_http_cache_hash`.
//!
//! Read path (rehydrate on file open) lives in `crate::block_hydrate`.

use std::collections::HashMap;

use crate::app::App;
use crate::buffer::Segment;
use crate::commands::db::load_active_env_vars;

/// Tuple form of `(method, url, params, headers, body)` ready for
/// `compute_http_cache_hash`. Drops disabled params/headers — matches
/// the desktop's filter so the same `(file, hash)` row is written by
/// both clients.
pub(super) type HttpCacheInputs = (
    String,
    String,
    Vec<(String, String)>,
    Vec<(String, String)>,
    String,
);

/// Snapshot the cache-input slice of an HTTP block. Returns `None`
/// when the segment is missing, not HTTP, or has no document context.
pub(super) fn http_block_cache_inputs(app: &App, segment_idx: usize) -> Option<HttpCacheInputs> {
    let doc = app.document()?;
    let block = match doc.segments().get(segment_idx)? {
        Segment::Block(b) => b,
        _ => return None,
    };
    if !block.is_http() {
        return None;
    }
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
    let params = enabled_kv_pairs(block.params.get("params"));
    let headers = enabled_kv_pairs(block.params.get("headers"));
    let body = block
        .params
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Some((method, url, params, headers, body))
}

fn enabled_kv_pairs(value: Option<&serde_json::Value>) -> Vec<(String, String)> {
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

/// True for methods that intentionally bypass the cache write —
/// idempotent reads are cacheable, side-effect mutations aren't.
/// Matches the desktop's `MUTATION_METHODS` set.
pub(super) fn is_mutation_method(method: &str) -> bool {
    matches!(
        method.to_ascii_uppercase().as_str(),
        "POST" | "PUT" | "PATCH" | "DELETE"
    )
}

/// Compute the cache hash + spawn the SQLite save. The current env's
/// `key->value` snapshot is read on the calling thread (cheap,
/// already cached in the store) so the spawned task only does the
/// hash + insert. Failure is logged via `tracing::warn` — cache
/// writes are best-effort.
pub(super) fn persist_http_cache_async(
    app: &App,
    file_path: String,
    alias: Option<String>,
    inputs: HttpCacheInputs,
    response_json: serde_json::Value,
    elapsed_ms: i64,
) {
    let env_vars: HashMap<String, String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(load_active_env_vars(&app.environments_store))
    })
    .unwrap_or_default();
    let pool = app.pool_manager.app_pool().clone();
    tokio::spawn(async move {
        let (method, url, params, headers, body) = inputs;
        let hash = httui_core::block_results::compute_http_cache_hash(
            &method, &url, &params, &headers, &body, &env_vars,
        );
        let response_str = match serde_json::to_string(&response_json) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("http cache: serialize failed: {e}");
                return;
            }
        };
        if let Err(e) = httui_core::block_results::save_block_result_with_alias(
            &pool,
            &file_path,
            &hash,
            alias.as_deref(),
            "success",
            &response_str,
            elapsed_ms,
            None,
        )
        .await
        {
            tracing::warn!("http cache: save failed: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_method_set_matches_desktop() {
        assert!(is_mutation_method("POST"));
        assert!(is_mutation_method("put"));
        assert!(is_mutation_method("Patch"));
        assert!(is_mutation_method("DELETE"));
        assert!(!is_mutation_method("GET"));
        assert!(!is_mutation_method("HEAD"));
        assert!(!is_mutation_method("OPTIONS"));
    }

    #[test]
    fn enabled_kv_pairs_drops_disabled_and_empty_keys() {
        let v = serde_json::json!([
            {"key": "Authorization", "value": "Bearer t", "enabled": true},
            {"key": "X-Debug", "value": "on", "enabled": false},
            {"key": "", "value": "noop"},
            {"key": "Accept", "value": "*/*"}, // omitted-enabled = true
        ]);
        let out = enabled_kv_pairs(Some(&v));
        assert_eq!(
            out,
            vec![
                ("Authorization".into(), "Bearer t".into()),
                ("Accept".into(), "*/*".into()),
            ]
        );
    }

    #[test]
    fn enabled_kv_pairs_handles_missing_value() {
        assert!(enabled_kv_pairs(None).is_empty());
        let null = serde_json::Value::Null;
        assert!(enabled_kv_pairs(Some(&null)).is_empty());
    }
}
