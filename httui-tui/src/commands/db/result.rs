//! Async DB run result handlers: outcome fold, cancel, load-more.

use tokio_util::sync::CancellationToken;

use crate::app::{App, StatusKind};
use crate::buffer::block::ExecutionState;
use crate::buffer::Segment;

use super::{
    derive_db_history_stats, load_active_env_vars, record_db_history_async, resolve_block_refs,
    resolve_connection_id, save_db_cache_async, snapshot_db_history_meta, spawn_db_query,
    summarize_db_response,
};

/// Fold the outcome of a backgrounded DB query (kicked off by
/// `apply_run_block` or the load-more prefetch) into the matching
/// block. Called by the main loop on `AppEvent::DbBlockResult`.
/// Always clears `app.running_query` so the next run / Ctrl-C
/// behave correctly.
pub fn handle_db_block_result(
    app: &mut App,
    segment_idx: usize,
    kind: crate::event::DbBlockResultKind,
    outcome: Result<httui_core::executor::db::types::DbResponse, String>,
) {
    // Cache key was stored on the running-query handle when
    // `apply_run_block` decided this was a cacheable read. Take it
    // *before* clearing the slot so the success branch below can
    // write back without re-deriving the hash.
    let cache_key = app.running_query.take().and_then(|q| q.cache_key);

    // Snapshot history-relevant metadata once, before we descend into
    // the per-kind match. Block fields (alias, query, connection)
    // don't change between Ok/Err and we need them in both branches
    // to record an entry. Only `DbBlockResultKind::Run` actually
    // emits a row — load-more / explain are not user "runs".
    let history_meta = if matches!(kind, crate::event::DbBlockResultKind::Run) {
        snapshot_db_history_meta(app, segment_idx)
    } else {
        None
    };

    use crate::event::DbBlockResultKind;
    use httui_core::executor::db::types::DbResult;
    // Set inside Run arms; consumed below to advance the auto-exec
    // chain. `None` for LoadMore / Explain (chain ignores them).
    let mut run_success: Option<bool> = None;
    match kind {
        DbBlockResultKind::Run => match outcome {
            Ok(response) => {
                let first_was_error =
                    matches!(response.results.first(), Some(DbResult::Error { .. }));
                let summary = summarize_db_response(&response);
                let value = serde_json::to_value(&response).ok();
                if let Some(doc) = app.tabs.active_document_mut() {
                    if let Some(b) = doc.block_at_mut(segment_idx) {
                        b.state = if first_was_error {
                            ExecutionState::Error(summary.clone())
                        } else {
                            ExecutionState::Success
                        };
                        b.cached_result = value.clone();
                    }
                }
                // Save to cache only on success — error responses
                // shouldn't poison subsequent runs (user fixes the
                // query and re-runs; we don't want to serve the old
                // error). Mirrors desktop behavior.
                if !first_was_error {
                    if let (Some((file_path, hash)), Some(value)) = (cache_key, value) {
                        let alias = history_meta.as_ref().map(|m| m.block_alias.clone());
                        save_db_cache_async(
                            app.pool_manager.app_pool().clone(),
                            file_path,
                            hash,
                            alias,
                            value,
                            response.stats.elapsed_ms,
                            &response.results,
                        );
                    }
                }
                // Record run history (metadata only — never the
                // result rows). Outcome distinguishes a SELECT/
                // mutation success from a per-statement error.
                if let Some(meta) = history_meta.as_ref() {
                    let elapsed = response.stats.elapsed_ms;
                    let (status, response_size) = derive_db_history_stats(&response);
                    let outcome_str = if first_was_error { "error" } else { "success" };
                    record_db_history_async(
                        app.pool_manager.app_pool().clone(),
                        meta.clone(),
                        Some(elapsed as i64),
                        status,
                        response_size,
                        outcome_str,
                    );
                }

                if first_was_error {
                    app.set_status(StatusKind::Error, summary);
                } else {
                    app.set_status(StatusKind::Info, summary);
                }
                run_success = Some(!first_was_error);
            }
            Err(msg) => {
                // Without a synthetic result the panel stays empty and
                // the error is only on the status bar — which scrolls
                // off on the next keystroke.
                let synthetic = httui_core::executor::db::types::DbResponse {
                    results: vec![httui_core::executor::db::types::DbResult::Error {
                        message: msg.clone(),
                        line: None,
                        column: None,
                    }],
                    messages: Vec::new(),
                    plan: None,
                    stats: httui_core::executor::db::types::DbStats {
                        elapsed_ms: 0,
                        rows_streamed: None,
                    },
                };
                let value = serde_json::to_value(&synthetic).ok();
                if let Some(doc) = app.tabs.active_document_mut() {
                    if let Some(b) = doc.block_at_mut(segment_idx) {
                        b.state = ExecutionState::Error(msg.clone());
                        b.cached_result = value;
                    }
                }
                if let Some(meta) = history_meta.as_ref() {
                    let outcome_str = if msg.to_lowercase().contains("cancel") {
                        "cancelled"
                    } else {
                        "error"
                    };
                    record_db_history_async(
                        app.pool_manager.app_pool().clone(),
                        meta.clone(),
                        None,
                        None,
                        None,
                        outcome_str,
                    );
                }
                app.set_status(StatusKind::Error, msg);
                run_success = Some(false);
            }
        },
        DbBlockResultKind::LoadMore => match outcome {
            Ok(response) => {
                let (new_rows, new_has_more) = match response.results.first() {
                    Some(DbResult::Select { rows, has_more, .. }) => (rows.clone(), *has_more),
                    Some(DbResult::Error { message, .. }) => {
                        app.set_status(StatusKind::Error, format!("load more: {message}"));
                        return;
                    }
                    _ => {
                        app.set_status(StatusKind::Error, "load more: unexpected response shape");
                        return;
                    }
                };
                let new_total = if let Some(doc) = app.tabs.active_document_mut() {
                    if let Some(b) = doc.block_at_mut(segment_idx) {
                        if let Some(cached) = b.cached_result.as_mut() {
                            if let Some(first) = cached
                                .get_mut("results")
                                .and_then(|v| v.as_array_mut())
                                .and_then(|a| a.first_mut())
                            {
                                if let Some(rows) =
                                    first.get_mut("rows").and_then(|v| v.as_array_mut())
                                {
                                    rows.extend(new_rows);
                                    let total = rows.len();
                                    if let Some(slot) = first.get_mut("has_more") {
                                        *slot = serde_json::Value::Bool(new_has_more);
                                    }
                                    total
                                } else {
                                    0
                                }
                            } else {
                                0
                            }
                        } else {
                            0
                        }
                    } else {
                        0
                    }
                } else {
                    0
                };
                let suffix = if new_has_more { "+" } else { "" };
                app.set_status(StatusKind::Info, format!("loaded {new_total}{suffix} rows"));
            }
            Err(msg) => {
                app.set_status(StatusKind::Error, format!("load more: {msg}"));
            }
        },
        DbBlockResultKind::Explain => match outcome {
            Ok(response) => {
                // Stuff the EXPLAIN response under `cached_result.plan`
                // without touching `cached_result.results` — the
                // user's last `r` output stays visible. If the block
                // never ran a `r` (no cached_result yet), seed a
                // minimal envelope so the Plan tab has somewhere to
                // hang.
                let plan_value = serde_json::to_value(&response).ok();
                let first_was_error =
                    matches!(response.results.first(), Some(DbResult::Error { .. }));
                let block_id = app
                    .tabs
                    .active_document()
                    .and_then(|d| d.block_at(segment_idx))
                    .map(|b| b.id);
                if let Some(doc) = app.tabs.active_document_mut() {
                    if let Some(b) = doc.block_at_mut(segment_idx) {
                        let target = b.cached_result.get_or_insert_with(|| {
                            serde_json::json!({
                                "results": [],
                                "messages": [],
                                "stats": { "elapsed_ms": 0 }
                            })
                        });
                        if let Some(obj) = target.as_object_mut() {
                            obj.insert(
                                "plan".into(),
                                plan_value.unwrap_or(serde_json::Value::Null),
                            );
                        }
                    }
                }
                if first_was_error {
                    let msg = summarize_db_response(&response);
                    app.set_status(StatusKind::Error, format!("explain: {msg}"));
                } else {
                    // Auto-switch to the Plan tab so the user sees the
                    // result without an extra `gt`-cycle through tabs.
                    if let Some(id) = block_id {
                        app.set_result_tab(id, crate::app::ResultPanelTab::Plan);
                    }
                    app.set_status(
                        StatusKind::Info,
                        format!("plan · {}ms", response.stats.elapsed_ms),
                    );
                }
            }
            Err(msg) => {
                app.set_status(StatusKind::Error, format!("explain: {msg}"));
            }
        },
    }

    if let Some(success) = run_success {
        crate::commands::refs::on_block_complete(app, segment_idx, success);
    }
}

/// Cancel an in-flight DB query, if any. Called from the
/// dispatcher when `Ctrl-C` arrives while `app.running_query` is
/// `Some`. The actual abort is reported back via the regular
/// `DbBlockResult` path (the executor's cancel-aware future
/// resolves to `Err("Request cancelled")`).
pub fn cancel_running_query(app: &mut App) -> bool {
    let Some(rq) = app.running_query.as_ref() else {
        return false;
    };
    rq.cancel.cancel();
    app.set_status(StatusKind::Info, "cancelling query…");
    true
}

/// Fire the next page of rows for a paginated DB block. Mirrors
/// `apply_run_block` but with `offset = rows.len()` and merge-on-
/// completion (the result handler appends instead of replacing the
/// `cached_result`). Returns `Ok(())` on dispatch, `Err(msg)` if
/// the pre-flight (no cache, no connection, ref resolution …)
/// failed — the caller surfaces that as a status hint.
pub(crate) fn load_more_db_block(app: &mut App, segment_idx: usize) -> Result<(), String> {
    if app.running_query.is_some() {
        return Err("another query is already running".into());
    }
    // Snapshot the block; release the immutable doc borrow before
    // any later mutation.
    let block = {
        let doc = app.document().ok_or_else(|| "no document".to_string())?;
        match doc.segments().get(segment_idx) {
            Some(Segment::Block(b)) => b.clone(),
            _ => return Err("block missing".into()),
        }
    };
    if !block.is_db() {
        return Err("not a DB block".into());
    }

    let cached = block
        .cached_result
        .as_ref()
        .ok_or_else(|| "no result cached yet".to_string())?;
    let first = cached
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| "result has no rows".to_string())?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return Err("not a select result".into());
    }
    let has_more = first
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !has_more {
        return Err("no more rows".into());
    }
    let current_offset = first
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|a| a.len() as u64)
        .unwrap_or(0);

    let raw_query = block
        .params
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if raw_query.is_empty() {
        return Err("empty SQL".into());
    }
    let connection_id_raw = block
        .params
        .get("connection_id")
        .or_else(|| block.params.get("connection"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if connection_id_raw.is_empty() {
        return Err("no connection on block".into());
    }
    let limit = block
        .params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100);
    let timeout_ms = block.params.get("timeout_ms").and_then(|v| v.as_u64());

    let env_vars: std::collections::HashMap<String, String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(load_active_env_vars(&app.environments_store))
    })
    .unwrap_or_default();
    let (query, bind_values) = match app.document() {
        Some(d) => resolve_block_refs(d.segments(), segment_idx, &raw_query, &env_vars)?,
        None => (raw_query.clone(), Vec::new()),
    };
    let store = app.connections_store.clone();
    let connection_id = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(resolve_connection_id(&store, &connection_id_raw))
    })?;

    let token = CancellationToken::new();
    // Pagination doesn't write to the cache — the on-disk entry is
    // keyed by the original query+conn+env, not by `(query, offset)`.
    // Bumping cache on every load-more page would either bloat with
    // partial responses or, worse, overwrite the canonical entry
    // with a partial one.
    spawn_db_query(
        app,
        segment_idx,
        crate::app::RunningKind::LoadMore,
        token,
        connection_id,
        query,
        bind_values,
        limit,
        current_offset,
        timeout_ms,
        None,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, RunningKind, RunningQuery};
    use crate::buffer::Document;
    use crate::config::Config;
    use crate::event::DbBlockResultKind;
    use crate::pane::{Pane, TabState};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use httui_core::executor::db::types::{DbResponse, DbResult, DbStats};
    use std::time::Instant;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    async fn app_with_block(md: &str) -> (App, usize, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let note = vault.path().join("note.md");
        std::fs::write(&note, md).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
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
            .expect("block");
        (app, idx, data, vault)
    }

    fn stats() -> DbStats {
        DbStats {
            elapsed_ms: 12,
            rows_streamed: None,
        }
    }

    fn select_response(rows: usize, has_more: bool) -> DbResponse {
        let cols = vec![httui_core::db::connections::ColumnInfo {
            name: "id".into(),
            type_name: "INTEGER".into(),
        }];
        let rows: Vec<_> = (0..rows).map(|i| serde_json::json!([i])).collect();
        DbResponse {
            results: vec![DbResult::Select {
                columns: cols,
                rows,
                has_more,
            }],
            messages: vec![],
            plan: None,
            stats: stats(),
        }
    }

    fn error_response(msg: &str) -> DbResponse {
        DbResponse {
            results: vec![DbResult::Error {
                message: msg.into(),
                line: None,
                column: None,
            }],
            messages: vec![],
            plan: None,
            stats: stats(),
        }
    }

    fn make_running_query(idx: usize) -> RunningQuery {
        RunningQuery {
            segment_idx: idx,
            cancel: CancellationToken::new(),
            started_at: Instant::now(),
            kind: RunningKind::Run,
            cache_key: None,
            bytes_received: 0,
            http_cache_meta: None,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_ok_success_updates_block_state_and_status() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::Run,
            Ok(select_response(2, false)),
        );
        assert!(app.running_query.is_none());
        let block = app.document().unwrap().block_at(idx).unwrap().clone();
        assert!(matches!(
            block.state,
            crate::buffer::block::ExecutionState::Success
        ));
        assert!(block.cached_result.is_some());
        assert!(app.status_message.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_ok_first_is_error_marks_block_error() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::Run,
            Ok(error_response("syntax")),
        );
        let block = app.document().unwrap().block_at(idx).unwrap().clone();
        assert!(matches!(
            block.state,
            crate::buffer::block::ExecutionState::Error(_)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_err_sets_synthetic_error_block_and_status() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::Run,
            Err("connection refused".into()),
        );
        let block = app.document().unwrap().block_at(idx).unwrap().clone();
        assert!(matches!(
            block.state,
            crate::buffer::block::ExecutionState::Error(_)
        ));
        assert!(block.cached_result.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_err_cancelled_uses_cancelled_outcome_for_history() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        // outcome message contains "cancel" → history outcome = cancelled
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::Run,
            Err("Request cancelled".into()),
        );
        assert!(app.status_message.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_ok_appends_rows_and_updates_has_more() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        // seed cache with one initial row
        let initial = select_response(1, true);
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.cached_result = serde_json::to_value(&initial).ok();
        }
        // running_query exists for LoadMore too
        app.running_query = Some(make_running_query(idx));
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::LoadMore,
            Ok(select_response(2, false)),
        );
        let block = app.document().unwrap().block_at(idx).unwrap().clone();
        let rows = block
            .cached_result
            .as_ref()
            .unwrap()
            .get("results")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|r| r.get("rows"))
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap();
        assert_eq!(rows, 3); // 1 + 2
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_err_sets_error_status() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::LoadMore,
            Err("network down".into()),
        );
        let s = app.status_message.expect("status");
        assert!(s.text.contains("load more"), "got {:?}", s.text);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_ok_with_error_result_returns_early_with_status() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::LoadMore,
            Ok(error_response("oops")),
        );
        let s = app.status_message.expect("status");
        assert!(s.text.contains("load more"), "got {:?}", s.text);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn explain_ok_writes_plan_and_switches_tab() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::Explain,
            Ok(select_response(1, false)),
        );
        let block = app.document().unwrap().block_at(idx).unwrap().clone();
        let cache = block.cached_result.as_ref().unwrap();
        assert!(cache.get("plan").is_some(), "plan slot must be set");
        let s = app.status_message.expect("status");
        assert!(s.text.contains("plan"), "got {:?}", s.text);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn explain_ok_with_error_result_sets_error_status() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::Explain,
            Ok(error_response("bad explain")),
        );
        let s = app.status_message.expect("status");
        assert!(s.text.contains("explain"), "got {:?}", s.text);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn explain_err_sets_status() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        handle_db_block_result(
            &mut app,
            idx,
            DbBlockResultKind::Explain,
            Err("nope".into()),
        );
        let s = app.status_message.expect("status");
        assert!(s.text.contains("explain"), "got {:?}", s.text);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_running_query_without_active_returns_false() {
        let md = "prose\n";
        let (mut app, _idx, _d, _v) = app_with_block_or_prose(md).await;
        assert!(!cancel_running_query(&mut app));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_running_query_with_active_cancels_token_and_returns_true() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        let token = CancellationToken::new();
        app.running_query = Some(RunningQuery {
            segment_idx: idx,
            cancel: token.clone(),
            started_at: Instant::now(),
            kind: RunningKind::Run,
            cache_key: None,
            bytes_received: 0,
            http_cache_meta: None,
        });
        assert!(cancel_running_query(&mut app));
        assert!(token.is_cancelled());
    }

    async fn app_with_block_or_prose(md: &str) -> (App, usize, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let note = vault.path().join("note.md");
        std::fs::write(&note, md).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        let doc = Document::from_markdown(md).unwrap();
        let pane = Pane::new(doc, note);
        app.tabs.tabs.clear();
        app.tabs.tabs.push(TabState::new(pane));
        app.tabs.active = 0;
        (app, 0, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_db_block_with_running_query_errors() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        app.running_query = Some(make_running_query(idx));
        let err = load_more_db_block(&mut app, idx).unwrap_err();
        assert!(err.contains("already running"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_db_block_non_db_block_errors() {
        let md = "```http alias=a\nGET /x\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        let err = load_more_db_block(&mut app, idx).unwrap_err();
        assert!(err.contains("not a DB"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_db_block_missing_block_errors() {
        let (mut app, _idx, _d, _v) = app_with_block_or_prose("prose\n").await;
        let err = load_more_db_block(&mut app, 999).unwrap_err();
        assert!(
            err.contains("missing") || err.contains("block"),
            "got {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_db_block_no_cache_errors() {
        let md = "```db-sqlite alias=q connection=c\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        let err = load_more_db_block(&mut app, idx).unwrap_err();
        assert!(err.contains("no result cached"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_db_block_no_has_more_errors() {
        let md = "```db-sqlite alias=q connection=c\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.cached_result = serde_json::to_value(select_response(1, false)).ok();
        }
        let err = load_more_db_block(&mut app, idx).unwrap_err();
        assert!(err.contains("no more rows"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_db_block_empty_sql_errors() {
        let md = "```db-sqlite alias=q connection=c\n\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.cached_result = serde_json::to_value(select_response(1, true)).ok();
        }
        let err = load_more_db_block(&mut app, idx).unwrap_err();
        assert!(err.contains("empty SQL"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_db_block_no_connection_errors() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.cached_result = serde_json::to_value(select_response(1, true)).ok();
        }
        let err = load_more_db_block(&mut app, idx).unwrap_err();
        assert!(err.contains("connection"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_more_db_block_not_a_select_errors() {
        let md = "```db-sqlite alias=q connection=c\nSELECT 1;\n```\n";
        let (mut app, idx, _d, _v) = app_with_block(md).await;
        let mutation_cache = serde_json::json!({
            "results":[{"kind":"mutation","rows_affected":2}],
            "messages":[],"stats":{"elapsed_ms":1}
        });
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.cached_result = Some(mutation_cache);
        }
        let err = load_more_db_block(&mut app, idx).unwrap_err();
        assert!(
            err.contains("not a select") || err.contains("no more"),
            "got {err:?}"
        );
    }
}
