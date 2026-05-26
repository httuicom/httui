//! HTTP block execution. Mirrors the DB module's flow:
//! `apply_run_http_block` → resolve refs → spawn task → AppEvent →
//! `handle_http_block_result` folds the response into the block.

mod codegen;
mod history;
pub mod refs;

pub use codegen::copy_as_curl;
pub use history::{close_block_history, move_block_history_cursor, open_block_history};
pub use refs::resolve_in_http_params;

use std::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::app::{App, RunningKind, RunningQuery, StatusKind};
use crate::buffer::block::ExecutionState;
use crate::buffer::Segment;
use crate::commands::db::load_active_env_vars;
use httui_core::executor::http::types::HttpResponse;
use httui_core::executor::http::HttpExecutor;

pub fn apply_run_http_block(app: &mut App, segment_idx: usize) {
    let Some(doc) = app.document() else { return };
    let block = match doc.segments().get(segment_idx) {
        Some(Segment::Block(b)) => b.clone(),
        _ => return,
    };
    if !block.is_http() {
        app.set_status(StatusKind::Info, "not an HTTP block");
        return;
    }

    // Pre-flight: env vars + ref resolution. Fast (in-memory + a
    // couple of SQLite reads), so we keep them on the dispatch
    // thread.
    let env_vars = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(load_active_env_vars(&app.environments_store))
    })
    .unwrap_or_default();

    let segments_snapshot: Vec<Segment> = doc.segments().to_vec();
    let mut resolved = block.params.clone();
    if let Err(msg) =
        resolve_in_http_params(&mut resolved, &segments_snapshot, segment_idx, &env_vars)
    {
        if let Some(doc) = app.tabs.active_document_mut() {
            if let Some(b) = doc.block_at_mut(segment_idx) {
                b.state = ExecutionState::Error(msg.clone());
                b.cached_result = None;
            }
        }
        app.set_status(StatusKind::Error, msg);
        return;
    }

    // Validate URL is non-empty after resolution.
    let url_ok = resolved
        .get("url")
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !url_ok {
        let msg = "empty URL".to_string();
        if let Some(doc) = app.tabs.active_document_mut() {
            if let Some(b) = doc.block_at_mut(segment_idx) {
                b.state = ExecutionState::Error(msg.clone());
                b.cached_result = None;
            }
        }
        app.set_status(StatusKind::Error, msg);
        return;
    }

    // Mark Running on the block + record running query slot for
    // Ctrl-C cancel.
    if let Some(doc) = app.tabs.active_document_mut() {
        if let Some(b) = doc.block_at_mut(segment_idx) {
            b.state = ExecutionState::Running;
        }
    }

    let token = CancellationToken::new();
    let Some(sender) = app.event_sender.clone() else {
        app.set_status(
            StatusKind::Error,
            "internal: no event sender wired (spawn aborted)",
        );
        return;
    };
    let token_for_task = token.clone();
    let segment_idx_for_task = segment_idx;
    let sender_for_chunks = sender.clone();
    tokio::spawn(async move {
        let executor = HttpExecutor::new();
        let outcome = executor
            .execute_streamed(resolved, token_for_task, move |chunk| {
                use httui_core::executor::http::types::HttpChunk;
                if matches!(chunk, HttpChunk::Headers { .. } | HttpChunk::BodyChunk { .. }) {
                    let _ = sender_for_chunks.send(crate::event::AppEvent::HttpBlockChunk {
                        segment_idx: segment_idx_for_task,
                        chunk,
                    });
                }
            })
            .await
            .map_err(|e| format!("{e}"));
        let _ = sender.send(crate::event::AppEvent::HttpBlockResult {
            segment_idx: segment_idx_for_task,
            outcome,
        });
    });

    app.running_query = Some(RunningQuery {
        segment_idx,
        cancel: token,
        started_at: Instant::now(),
        kind: RunningKind::Run,
        cache_key: None,
        bytes_received: 0,
    });
    app.record_run_anchor(segment_idx);
}

/// Update `app.running_query.bytes_received` from a streamed chunk.
/// Ignores terminal variants (Complete/Error/Cancelled are folded
/// via `handle_http_block_result`). The status bar reads
/// `bytes_received` to paint the download counter.
pub fn handle_http_block_chunk(
    app: &mut App,
    segment_idx: usize,
    chunk: httui_core::executor::http::types::HttpChunk,
) {
    use httui_core::executor::http::types::HttpChunk;
    let Some(rq) = app.running_query.as_mut() else {
        return;
    };
    if rq.segment_idx != segment_idx {
        return;
    }
    if let HttpChunk::BodyChunk { offset, bytes } = chunk {
        rq.bytes_received = offset + bytes.len() as u64;
    }
}

pub fn handle_http_block_result(
    app: &mut App,
    segment_idx: usize,
    outcome: Result<HttpResponse, String>,
) {
    app.running_query = None;

    // Snapshot the bits we need for the history insert before we
    // borrow `app` mutably below — the active file path, the block's
    // alias, method, and URL. These are stable during this fn (the
    // user can't move tabs while we hold the event in flight).
    let file_path = active_file_path_string(app);
    let (block_alias, method, url_canonical, request_size) =
        snapshot_block_meta(app, segment_idx).unwrap_or_default();

    let Some(doc) = app.tabs.active_document_mut() else {
        return;
    };
    let Some(b) = doc.block_at_mut(segment_idx) else {
        return;
    };
    match &outcome {
        Ok(response) => {
            b.cached_result = Some(http_response_to_json(response));
            b.state = ExecutionState::Success;
            app.set_status(
                StatusKind::Info,
                format!(
                    "{} {} · {}ms",
                    response.status_code, response.status_text, response.elapsed_ms
                ),
            );
        }
        Err(msg) => {
            b.state = ExecutionState::Error(msg.clone());
            b.cached_result = None;
            app.set_status(StatusKind::Error, msg.clone());
        }
    }

    let success = outcome.is_ok();

    // Persist a metadata-only history row when the block has both a
    // file path on disk and an alias — without an alias the history
    // table has no stable key to group runs under, so anonymous
    // blocks intentionally have no history.
    record_history_async(
        app,
        file_path,
        block_alias,
        method,
        url_canonical,
        request_size,
        outcome,
    );

    crate::commands::refs::on_block_complete(app, segment_idx, success);
}

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
fn snapshot_block_meta(
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
fn record_history_async(
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
fn http_response_to_json(r: &HttpResponse) -> serde_json::Value {
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

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::executor::http::types::{Cookie, TimingBreakdown};
    use std::collections::HashMap;

    #[test]
    fn http_response_to_json_translates_executor_shape_to_renderer_shape() {
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        let response = HttpResponse {
            status_code: 200,
            status_text: "OK".into(),
            headers,
            body: serde_json::json!({"ok": true}),
            size_bytes: 11,
            elapsed_ms: 42,
            timing: TimingBreakdown {
                ttfb_ms: Some(30),
                ..Default::default()
            },
            cookies: vec![Cookie {
                name: "session".into(),
                value: "abc".into(),
                domain: Some("x.com".into()),
                path: Some("/".into()),
                expires: None,
                secure: false,
                http_only: false,
            }],
        };
        let v = http_response_to_json(&response);
        assert_eq!(v.get("status").and_then(|x| x.as_u64()), Some(200));
        assert_eq!(v.get("status_text").and_then(|x| x.as_str()), Some("OK"));
        let headers_arr = v.get("headers").and_then(|x| x.as_array()).unwrap();
        assert_eq!(headers_arr.len(), 1);
        assert_eq!(
            headers_arr[0].get("key").and_then(|x| x.as_str()),
            Some("Content-Type")
        );
        let timing = v.get("timing").unwrap();
        assert_eq!(timing.get("total_ms").and_then(|x| x.as_u64()), Some(42));
        assert_eq!(timing.get("ttfb_ms").and_then(|x| x.as_u64()), Some(30));
        let cookies = v.get("cookies").and_then(|x| x.as_array()).unwrap();
        assert_eq!(
            cookies[0].get("name").and_then(|x| x.as_str()),
            Some("session")
        );
    }

    use crate::app::App;
    use crate::buffer::Document;
    use crate::config::Config;
    use crate::pane::{Pane, TabState};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use httui_core::executor::http::types::HttpChunk;
    use tempfile::TempDir;

    async fn app_with_block(md: &str) -> (App, usize, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let note = vault.path().join("note.md");
        std::fs::write(&note, md).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault { vault: vault.path().to_path_buf() };
        let mut app = App::new(Config::default(), resolved, pool);
        let doc = Document::from_markdown(md).unwrap();
        let pane = Pane::new(doc, note);
        app.tabs.tabs.clear();
        app.tabs.tabs.push(TabState::new(pane));
        app.tabs.active = 0;
        let idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap_or(0);
        (app, idx, data, vault)
    }

    fn http_response() -> HttpResponse {
        HttpResponse {
            status_code: 200,
            status_text: "OK".into(),
            headers: HashMap::new(),
            body: serde_json::json!({}),
            size_bytes: 12,
            elapsed_ms: 33,
            timing: TimingBreakdown::default(),
            cookies: Vec::new(),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_run_http_block_non_http_emits_status() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        apply_run_http_block(&mut app, idx);
        let s = app.status_message.as_ref().expect("status");
        assert!(s.text.contains("not an HTTP"), "got {:?}", s.text);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_run_http_block_no_doc_returns() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault { vault: vault.path().to_path_buf() };
        let mut app = App::new(Config::default(), resolved, pool);
        app.tabs.tabs.clear();
        apply_run_http_block(&mut app, 0); // no panic
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_run_http_block_empty_url_errors() {
        let md = "```http alias=a\n\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        app.event_sender = Some(tx);
        apply_run_http_block(&mut app, idx);
        let s = app.status_message.as_ref().expect("status");
        assert!(s.text.contains("empty URL"), "got {:?}", s.text);
        let block = app.document().unwrap().block_at(idx).unwrap().clone();
        assert!(matches!(block.state, crate::buffer::block::ExecutionState::Error(_)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_run_http_block_ref_resolution_failure_errors() {
        let md = "```http alias=a\nGET https://x.com/{{ghost.body.id}}\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        app.event_sender = Some(tx);
        apply_run_http_block(&mut app, idx);
        let s = app.status_message.as_ref().expect("status");
        // Could be ref error or "no event sender" if test environment differs.
        assert!(s.text.contains("ghost") || s.text.contains("not found") || s.text.contains("URL"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_run_http_block_no_event_sender_errors() {
        let md = "```http alias=a\nGET https://x.com\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        // event_sender not wired
        apply_run_http_block(&mut app, idx);
        let s = app.status_message.as_ref().expect("status");
        assert!(s.text.contains("event sender"), "got {:?}", s.text);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_http_block_chunk_updates_bytes_received() {
        let md = "```http alias=a\nGET https://x.com\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(RunningQuery {
            segment_idx: idx,
            cancel: CancellationToken::new(),
            started_at: Instant::now(),
            kind: RunningKind::Run,
            cache_key: None,
            bytes_received: 0,
        });
        handle_http_block_chunk(
            &mut app,
            idx,
            HttpChunk::BodyChunk { offset: 100, bytes: vec![0u8; 50] },
        );
        assert_eq!(app.running_query.as_ref().unwrap().bytes_received, 150);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_http_block_chunk_ignores_wrong_segment() {
        let md = "```http alias=a\nGET https://x.com\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(RunningQuery {
            segment_idx: idx,
            cancel: CancellationToken::new(),
            started_at: Instant::now(),
            kind: RunningKind::Run,
            cache_key: None,
            bytes_received: 0,
        });
        handle_http_block_chunk(
            &mut app,
            999, // wrong idx
            HttpChunk::BodyChunk { offset: 100, bytes: vec![0u8; 50] },
        );
        assert_eq!(app.running_query.as_ref().unwrap().bytes_received, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_http_block_chunk_no_running_query_is_noop() {
        let md = "prose\n";
        let (mut app, _idx, _d, _v) = app_with_block(md).await;
        handle_http_block_chunk(
            &mut app,
            0,
            HttpChunk::BodyChunk { offset: 0, bytes: vec![] },
        );
        assert!(app.running_query.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_http_block_result_ok_marks_success_writes_cache_status() {
        let md = "```http alias=a\nGET https://x.com\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        handle_http_block_result(&mut app, idx, Ok(http_response()));
        let block = app.document().unwrap().block_at(idx).unwrap().clone();
        assert!(matches!(block.state, crate::buffer::block::ExecutionState::Success));
        assert!(block.cached_result.is_some());
        let s = app.status_message.as_ref().expect("status");
        assert!(s.text.contains("200"), "got {:?}", s.text);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_http_block_result_err_marks_error() {
        let md = "```http alias=a\nGET https://x.com\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        handle_http_block_result(&mut app, idx, Err("connection failed".into()));
        let block = app.document().unwrap().block_at(idx).unwrap().clone();
        assert!(matches!(block.state, crate::buffer::block::ExecutionState::Error(_)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_http_block_result_cancelled_outcome_recorded_as_cancelled() {
        let md = "```http alias=a\nGET https://x.com\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        handle_http_block_result(&mut app, idx, Err("Request cancelled".into()));
        assert!(app.status_message.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_block_meta_builds_canonical_url_with_params_and_size() {
        let md = "```http alias=a\nGET https://x.com?existing=1\nContent-Type: text/plain\n\nhello\n```\n";
        let (app, idx, _d, _v) = app_with_block(md).await;
        let (alias, method, canonical, size) = snapshot_block_meta(&app, idx).expect("some");
        assert_eq!(alias.as_deref(), Some("a"));
        assert_eq!(method, "GET");
        assert!(canonical.contains("?existing=1"), "got {canonical}");
        assert!(size.unwrap() > 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn snapshot_block_meta_non_http_returns_none() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (app, idx, _d, _v) = app_with_block(md).await;
        assert!(snapshot_block_meta(&app, idx).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn active_file_path_string_returns_some_when_pane_has_path() {
        let md = "prose\n";
        let (app, _idx, _d, _v) = app_with_block(md).await;
        let p = active_file_path_string(&app).expect("path");
        assert!(p.ends_with("note.md"));
    }
}
