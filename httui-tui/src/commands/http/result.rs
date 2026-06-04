//! Post-run helpers for the HTTP runner — translate the executor's
//! `HttpResponse` to the JSON shape the BLOCKS view renderer reads,
//! pull canonical metadata (alias/method/url/size) off the
//! in-document block, and persist a metadata-only history row.

use crate::app::App;
use crate::buffer::Segment;
use httui_core::executor::http::types::HttpResponse;

/// Best-effort lookup of the active tab's document path, formatted
/// as a relative-or-absolute string. Returns `None` for in-memory
/// docs (no file backing) — those don't get history rows.
pub(super) fn active_file_path_string(app: &App) -> Option<String> {
    let tab = app.tabs.tabs.get(app.tabs.active())?;
    let path = tab.active_leaf().document_path.as_ref()?;
    Some(path.to_string_lossy().to_string())
}

/// Pull `(alias, method, url+query, request_size)` out of the
/// block at `segment_idx`. Returns `None` when there's no doc /
/// no block / wrong type. URL is rebuilt from
/// `url + ?key=value&...` so the canonical form stays stable
/// regardless of whether the source used inline or
/// continuation-line query syntax.
///
/// `request_size` is a coarse approximation of bytes sent on the
/// wire: serialized request line + headers + body separator + body.
pub(super) fn snapshot_block_meta(
    app: &App,
    segment_idx: usize,
) -> Option<(Option<String>, String, String, Option<i64>)> {
    let doc = app.document()?;
    let block = match doc.segments().get(segment_idx)? {
        Segment::Block(b) => b,
        _ => return None,
    };
    if !block.is_http() {
        return None;
    }
    let alias = block.alias.clone();
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
    let mut canonical = url.clone();
    if let Some(arr) = block.params.get("params").and_then(|v| v.as_array()) {
        let parts: Vec<String> = arr
            .iter()
            .filter_map(|p| {
                let k = p
                    .get("key")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())?;
                let v = p.get("value").and_then(|v| v.as_str()).unwrap_or("");
                if v.is_empty() {
                    Some(k.to_string())
                } else {
                    Some(format!("{k}={v}"))
                }
            })
            .collect();
        if !parts.is_empty() {
            let sep = if canonical.contains('?') { '&' } else { '?' };
            canonical.push(sep);
            canonical.push_str(&parts.join("&"));
        }
    }

    // Approximate request size: request line + per-header `K: V\r\n`
    // + blank line + body.
    let mut size = method.len() + 1 + canonical.len() + 2; // METHOD URL\r\n
    if let Some(headers) = block.params.get("headers").and_then(|v| v.as_array()) {
        for h in headers {
            let k = h.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let v = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
            if k.is_empty() {
                continue;
            }
            size += k.len() + 2 + v.len() + 2; // "K: V\r\n"
        }
    }
    let body = block
        .params
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !body.is_empty() {
        size += 2; // blank line "\r\n"
        size += body.len();
    }

    Some((alias, method, canonical, Some(size as i64)))
}

/// Spawn the SQLite insert in the background. We don't `await` here
/// — handle_http_block_result is called from the (synchronous) main
/// event loop and a SQLite write should never block the UI. Failures
/// are logged via `tracing::warn` and don't surface as user-visible
/// errors (history is best-effort by design).
pub(super) fn record_history_async(
    app: &App,
    file_path: Option<String>,
    block_alias: Option<String>,
    method: String,
    url_canonical: String,
    request_size: Option<i64>,
    outcome: Result<HttpResponse, String>,
) {
    let (Some(file_path), Some(block_alias)) = (file_path, block_alias) else {
        return; // in-memory doc or anonymous block — no history key.
    };
    let pool = app.pool_manager.app_pool().clone();
    let entry = match outcome {
        Ok(response) => httui_core::block_history::InsertEntry {
            file_path,
            block_alias,
            method,
            url_canonical,
            status: Some(response.status_code as i64),
            request_size,
            response_size: Some(response.size_bytes as i64),
            elapsed_ms: Some(response.elapsed_ms as i64),
            outcome: "success".into(),
            plan: None,
        },
        Err(msg) => httui_core::block_history::InsertEntry {
            file_path,
            block_alias,
            method,
            url_canonical,
            status: None,
            request_size,
            response_size: None,
            elapsed_ms: None,
            // Differentiate cancel from real failures so the modal
            // can dim the row (cancelled runs aren't bugs).
            outcome: if msg.to_lowercase().contains("cancel") {
                "cancelled"
            } else {
                "error"
            }
            .into(),
            plan: None,
        },
    };
    tokio::spawn(async move {
        if let Err(e) = httui_core::block_history::insert_history_entry(&pool, entry).await {
            tracing::warn!("block history insert failed: {e}");
        }
    });
}

/// Convert the executor's `HttpResponse` to the JSON shape the
/// renderer expects: headers as `[{key, value}]` array (vs the
/// executor's `HashMap`), `status` field aliased from `status_code`,
/// `timing.total_ms` derived from `elapsed_ms`.
pub(super) fn http_response_to_json(r: &HttpResponse) -> serde_json::Value {
    let headers: Vec<serde_json::Value> = r
        .headers
        .iter()
        .map(|(k, v)| serde_json::json!({ "key": k, "value": v }))
        .collect();
    let cookies: Vec<serde_json::Value> = r
        .cookies
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name,
                "value": c.value,
                "domain": c.domain,
                "path": c.path,
            })
        })
        .collect();
    serde_json::json!({
        "status": r.status_code,
        "status_text": r.status_text,
        "headers": headers,
        "cookies": cookies,
        "body": r.body,
        "size_bytes": r.size_bytes,
        "timing": {
            "total_ms": r.elapsed_ms,
            "ttfb_ms": r.timing.ttfb_ms,
        },
    })
}
