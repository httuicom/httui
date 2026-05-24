//! HTTP block execution. Mirrors the DB module's flow:
//! `apply_run_http_block` → resolve refs → spawn task → AppEvent →
//! `handle_http_block_result` folds the response into the block.

use std::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::app::{App, RunningKind, RunningQuery, StatusKind};
use crate::buffer::block::ExecutionState;
use crate::buffer::Segment;
use crate::commands::db::{load_active_env_vars, resolve_one_ref};
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
}

/// Best-effort lookup of the active tab's document path, formatted
/// as a relative-or-absolute string. Returns `None` for in-memory
/// docs (no file backing) — those don't get history rows.
fn active_file_path_string(app: &App) -> Option<String> {
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
/// Useful enough for spotting "huh, that was a 4MB POST" in the
/// history modal; not a substitute for raw socket counters.
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
    // + blank line + body. Mirrors what `to_http_file` emits, which
    // is already a faithful HTTP-message representation. We don't
    // resolve refs here — the snapshot runs before the executor
    // resolves them — so the count is "what the user wrote", not
    // "what reqwest sent". Close enough for the history modal.
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

// ─── Direct cURL copy (Ctrl+Shift+C) ─────────────────────────────

/// `<C-S-c>` on an HTTP block — resolve `{{refs}}` and copy a cURL
/// command to the clipboard. Same flow as the gx export picker's
/// HTTP path but without the picker — the "express" route.
/// Surfaces failures (no HTTP block / empty URL / clipboard down /
/// ref resolution failed) as status messages.
pub fn copy_as_curl(app: &mut crate::app::App) {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(crate::buffer::Cursor::InBlock { segment_idx, .. })
        | Some(crate::buffer::Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => {
            app.set_status(
                crate::app::StatusKind::Info,
                "place the cursor on an HTTP block first",
            );
            return;
        }
    };
    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => {
            app.set_status(crate::app::StatusKind::Info, "no block at cursor");
            return;
        }
    };
    if !block.is_http() {
        app.set_status(
            crate::app::StatusKind::Info,
            format!("`{}` blocks don't have a cURL form", block.block_type),
        );
        return;
    }
    let url_ok = block
        .params
        .get("url")
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !url_ok {
        app.set_status(
            crate::app::StatusKind::Info,
            "set a URL on the block before copying",
        );
        return;
    }

    // Resolve refs the same way the run path does. Failure stays
    // soft — surface it via the status line, don't crash.
    let env_vars = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(crate::commands::db::load_active_env_vars(
            &app.environments_store,
        ))
    })
    .unwrap_or_default();
    let segments_snapshot: Vec<Segment> = app
        .document()
        .map(|d| d.segments().to_vec())
        .unwrap_or_default();
    let mut resolved = block.params.clone();
    if let Err(msg) =
        resolve_in_http_params(&mut resolved, &segments_snapshot, segment_idx, &env_vars)
    {
        app.set_status(
            crate::app::StatusKind::Error,
            format!("ref resolution failed: {msg}"),
        );
        return;
    }

    let payload = httui_core::blocks::http_codegen::to_curl(&resolved);
    match crate::clipboard::set_text(&payload) {
        Ok(()) => {
            app.set_status(
                crate::app::StatusKind::Info,
                format!("copied as cURL ({} bytes) to clipboard", payload.len()),
            );
        }
        Err(e) => {
            app.set_status(crate::app::StatusKind::Error, e);
        }
    }
}

// ─── History modal (gh chord) ────────────────────────────────────

/// `gh` on an HTTP block — open the read-only history modal. Reads
/// `httui-core::block_history::list_history(file, alias)` synchronously
/// (the read is cheap; opening can briefly block). Validates:
///   1. cursor sits on an HTTP block
///   2. block has an alias (history rows are keyed by alias)
///   3. active doc has a file path on disk
///   4. there's at least one row to show
///
/// Each failure surfaces a status hint instead of opening an empty
/// modal — empty modals waste a keystroke.
pub fn open_block_history(app: &mut crate::app::App) -> Result<(), String> {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(crate::buffer::Cursor::InBlock { segment_idx, .. })
        | Some(crate::buffer::Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => return Err("place the cursor on a block first".into()),
    };
    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => return Err("place the cursor on a block first".into()),
    };
    if !block.is_http() && !block.is_db() {
        return Err(format!(
            "`{}` blocks don't record runs yet",
            block.block_type
        ));
    }
    let alias = match block.alias.as_deref().filter(|s| !s.is_empty()) {
        Some(a) => a.to_string(),
        None => return Err("anonymous block has no history (give it an `alias=`)".into()),
    };
    let file_path = active_file_path_string(app)
        .ok_or_else(|| "save the file first — history is keyed by file path".to_string())?;

    let pool = app.pool_manager.app_pool().clone();
    let entries: Vec<httui_core::block_history::HistoryEntry> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move {
            httui_core::block_history::list_history(&pool, &file_path, &alias).await
        })
    })
    .map_err(|e| format!("history read failed: {e}"))?;

    if entries.is_empty() {
        return Err("no history yet — run the block at least once".into());
    }

    // Title format differs by block type so users see context
    // matching their mental model:
    // - HTTP: `<METHOD> <alias>` (`GET myreq`)
    // - DB:   `DB <alias>`       (`DB userlist`)
    // The driver kind is already encoded in each row's `method`
    // column so we don't repeat it in the title.
    let title = if block.is_http() {
        format!(
            "{} {}",
            block
                .params
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("GET"),
            block.alias.as_deref().unwrap_or(""),
        )
    } else {
        format!("DB {}", block.alias.as_deref().unwrap_or(""))
    };

    app.modal = Some(crate::modal::Modal::BlockHistory(
        crate::app::BlockHistoryState {
            segment_idx,
            title,
            entries,
            selected: 0,
        },
    ));
    app.vim.mode = crate::vim::mode::Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub fn close_block_history(app: &mut crate::app::App) {
    if matches!(app.modal, Some(crate::modal::Modal::BlockHistory(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub fn move_block_history_cursor(app: &mut crate::app::App, delta: i32) {
    let Some(crate::modal::Modal::BlockHistory(state)) = app.modal.as_mut() else {
        return;
    };
    if state.entries.is_empty() {
        return;
    }
    let last = state.entries.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
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
    // Clone the SqlitePool so the spawned task owns its handle —
    // SqlitePool is `Arc`-backed, so this is cheap (single ref-count
    // bump). Without the clone the pool would borrow from `&App` and
    // can't escape the spawn.
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

/// Walk the HTTP block's params object and replace every
/// `{{...}}` placeholder in URL / headers / params / body with its
/// resolved text value. Block refs come from `resolve_one_ref`
/// (same source the SQL path uses); env vars resolve as plain
/// strings.
pub(crate) fn resolve_in_http_params(
    params: &mut serde_json::Value,
    segments: &[Segment],
    current_segment: usize,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    if let Some(s) = params.get("url").and_then(|v| v.as_str()).map(String::from) {
        let resolved = resolve_text_refs(&s, segments, current_segment, env_vars)?;
        if let Some(slot) = params.get_mut("url") {
            *slot = serde_json::Value::String(resolved);
        }
    }
    if let Some(arr) = params.get_mut("headers").and_then(|v| v.as_array_mut()) {
        for h in arr.iter_mut() {
            resolve_kv_in_place(h, segments, current_segment, env_vars)?;
        }
    }
    if let Some(arr) = params.get_mut("params").and_then(|v| v.as_array_mut()) {
        for p in arr.iter_mut() {
            resolve_kv_in_place(p, segments, current_segment, env_vars)?;
        }
    }
    if let Some(s) = params
        .get("body")
        .and_then(|v| v.as_str())
        .map(String::from)
    {
        let resolved = resolve_text_refs(&s, segments, current_segment, env_vars)?;
        if let Some(slot) = params.get_mut("body") {
            *slot = serde_json::Value::String(resolved);
        }
    }
    Ok(())
}

fn resolve_kv_in_place(
    obj: &mut serde_json::Value,
    segments: &[Segment],
    current_segment: usize,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    for field in ["key", "value"] {
        if let Some(s) = obj.get(field).and_then(|v| v.as_str()).map(String::from) {
            let resolved = resolve_text_refs(&s, segments, current_segment, env_vars)?;
            if let Some(slot) = obj.get_mut(field) {
                *slot = serde_json::Value::String(resolved);
            }
        }
    }
    Ok(())
}

/// Substitute `{{ref}}` placeholders in `text` with their resolved
/// value as plain text. Strings unquote; other JSON values use
/// their JSON form. Used by HTTP for URL / header / param / body
/// substitution (DB uses `?`-bind placeholders instead).
pub(crate) fn resolve_text_refs(
    text: &str,
    segments: &[Segment],
    current_segment: usize,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let close = match find_close(&bytes[i + 2..]) {
                Some(rel) => i + 2 + rel,
                None => {
                    out.push('{');
                    i += 1;
                    continue;
                }
            };
            let inner = std::str::from_utf8(&bytes[i + 2..close])
                .map_err(|_| "invalid utf-8 inside reference".to_string())?
                .trim();
            let value = resolve_one_ref(segments, current_segment, inner, env_vars)?;
            let s = match value {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            };
            out.push_str(&s);
            i = close + 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    Ok(out)
}

fn find_close(bytes: &[u8]) -> Option<usize> {
    (0..bytes.len().saturating_sub(1)).find(|&i| bytes[i] == b'}' && bytes[i + 1] == b'}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::executor::http::types::{Cookie, TimingBreakdown};
    use std::collections::HashMap;

    fn empty_segs() -> Vec<Segment> {
        Vec::new()
    }

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn resolve_text_refs_substitutes_env_vars() {
        let segs = empty_segs();
        let env = env(&[("TOKEN", "abc123"), ("HOST", "api.x.com")]);
        let out = resolve_text_refs("https://{{HOST}}/v1?t={{TOKEN}}", &segs, 0, &env).unwrap();
        assert_eq!(out, "https://api.x.com/v1?t=abc123");
    }

    #[test]
    fn resolve_text_refs_passes_through_text_without_refs() {
        let segs = empty_segs();
        let env = env(&[]);
        let out = resolve_text_refs("plain text", &segs, 0, &env).unwrap();
        assert_eq!(out, "plain text");
    }

    #[test]
    fn resolve_text_refs_keeps_unmatched_open_brace() {
        // A bare `{{` with no `}}` close is treated as literal text
        // (defensive — protects from runaway substitution mid-edit).
        let segs = empty_segs();
        let env = env(&[]);
        let out = resolve_text_refs("oops {{ nope", &segs, 0, &env).unwrap();
        assert!(out.contains("{ nope"), "got: {out}");
    }

    #[test]
    fn resolve_text_refs_errors_on_missing_env_var() {
        let segs = empty_segs();
        let env = env(&[]);
        let err = resolve_text_refs("{{MISSING}}", &segs, 0, &env).unwrap_err();
        assert!(
            err.contains("MISSING") || err.to_lowercase().contains("missing"),
            "got: {err}"
        );
    }

    #[test]
    fn resolve_in_http_params_walks_url_headers_body() {
        let segs = empty_segs();
        let env = env(&[("TOKEN", "secret")]);
        let mut params = serde_json::json!({
            "method": "POST",
            "url": "https://api.x.com?key={{TOKEN}}",
            "headers": [
                { "key": "Authorization", "value": "Bearer {{TOKEN}}" }
            ],
            "params": [
                { "key": "tok", "value": "{{TOKEN}}" }
            ],
            "body": "{\"t\":\"{{TOKEN}}\"}"
        });
        resolve_in_http_params(&mut params, &segs, 0, &env).unwrap();
        assert_eq!(
            params.get("url").and_then(|v| v.as_str()),
            Some("https://api.x.com?key=secret")
        );
        let auth = params
            .get("headers")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|h| h.get("value"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(auth, "Bearer secret");
        let body = params.get("body").and_then(|v| v.as_str()).unwrap();
        assert_eq!(body, "{\"t\":\"secret\"}");
    }

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
}
