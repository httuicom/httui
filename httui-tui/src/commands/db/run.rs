//! DB block run pipeline: executor params, summary, and the apply→inner→spawn chain.

use tokio_util::sync::CancellationToken;

use crate::app::{App, StatusKind};
use crate::buffer::block::ExecutionState;
use crate::buffer::Segment;

use super::{
    compute_db_cache_hash, db_summary_from_value, is_cacheable_query, is_unscoped_destructive,
    is_writing_query, load_active_env_vars, resolve_block_refs, resolve_connection_id,
    strip_leading_sql_comments,
};

// ───────────── executor params + response summary ─────────────

/// Assemble the JSON `serde_json::Value` that `DbExecutor`
/// deserializes into its `DbParams`. Pure function — extracted from
/// `spawn_db_query` so it's testable in isolation. Stays in lockstep
/// with `httui_core::executor::db::DbParams`: any new field there
/// has to be threaded through here.
pub fn build_db_executor_params(
    connection_id: &str,
    query: &str,
    bind_values: &[serde_json::Value],
    offset: u64,
    limit: u64,
    timeout_ms: Option<u64>,
    session_override: Option<&crate::session_overrides::ConnectionOverride>,
) -> serde_json::Value {
    serde_json::json!({
        "connection_id": connection_id,
        "query": query,
        "bind_values": bind_values,
        "offset": offset,
        "fetch_size": limit,
        // `timeout_ms` is `Option<u64>`; serde maps `None` → `null`
        // → executor's `Option<u64>` deserializes back to `None`,
        // which falls through to the connection's default timeout
        // and ultimately the 30 s fallback in `execute_with_cancel`.
        "timeout_ms": timeout_ms,
        // `None` ⇒ both fields serialize as `null` and the executor
        // leaves `HostPortOverride` unset (base pool, untouched).
        "session_host_override": session_override.and_then(|o| o.host.clone()),
        "session_port_override": session_override.and_then(|o| o.port).map(|p| p as i64),
    })
}

/// Compact one-liner for the status bar: `5 rows · 12ms` /
/// `mutation: 3 affected · 8ms` / `error: …`. Multi-statement
/// queries get a `(+N more)` suffix so users know the renderer is
/// only surfacing `results[0]` for now (ships tabs).
pub fn summarize_db_response(resp: &httui_core::executor::db::types::DbResponse) -> String {
    use httui_core::executor::db::types::DbResult;
    let elapsed = resp.stats.elapsed_ms;
    let extras = match resp.results.len() {
        0 | 1 => String::new(),
        n => format!(" (+{} more)", n - 1),
    };
    if let Some(first) = resp.results.first() {
        match first {
            DbResult::Select { rows, has_more, .. } => {
                let suffix = if *has_more { "+" } else { "" };
                format!("{}{} rows · {}ms{}", rows.len(), suffix, elapsed, extras)
            }
            DbResult::Mutation { rows_affected } => {
                format!("{} affected · {}ms{}", rows_affected, elapsed, extras)
            }
            DbResult::Error {
                message,
                line,
                column,
            } => {
                // Append `at L:C` when the executor enriched the
                // error with positional info (Postgres always; MySQL
                // when the parser produced one). Same suffix the
                // renderer's `db_summary` paints inside the block.
                let pos = line
                    .map(|l| format!(" at {l}:{}", column.unwrap_or(1)))
                    .unwrap_or_default();
                format!("error: {message}{pos}{extras}")
            }
        }
    } else {
        format!("ok · {}ms", elapsed)
    }
}

// ───────────── block execution (`r` in normal) ─────────────

/// Run the block at the cursor. Phase 1 only handles `db` / `db-*`
/// blocks — everything else surfaces a status hint and bails. The
/// query runs in a `tokio::spawn` task so the UI stays responsive
/// (and `Ctrl-C` can cancel it via the stored `CancellationToken`).
/// When the task finishes it pushes an `AppEvent::DbBlockResult`
/// back to the main loop, which folds the outcome into the block
/// via `handle_db_block_result`.
pub fn apply_run_block(app: &mut App) {
    crate::commands::refs::apply_run_block(app);
}

/// Run the DB block at `segment_idx`. Shared entry for the
/// cursor-based `r` keypress, the confirm-modal `y`, and `<C-x>`.
/// The `force_unscoped` flag bypasses the unscoped-destructive gate
/// once — set only when the user explicitly confirmed the run, or
/// when the call is internal (EXPLAIN doesn't actually mutate).
/// `query_override` lets callers (currently `run_explain`) substitute
/// the SQL that's actually sent to the executor while keeping the
/// block's `params["query"]` text untouched.
///
/// `as_explain` re-routes the run as an EXPLAIN side-channel:
/// - skips the read-only gate (EXPLAIN is read-only by definition),
/// - skips the unscoped-destructive confirm (EXPLAIN doesn't mutate),
/// - skips the cache (EXPLAIN output is dialect-specific and small —
///   not worth poisoning the main cache slot),
/// - leaves `block.state` and `block.cached_result.results` untouched
///   so the block keeps showing whatever the last `r` produced,
/// - tags the spawn `RunningKind::Explain` so the result handler
///   merges into `cached_result["plan"]` and auto-switches to the
///   Plan tab.
pub fn run_db_block_inner(
    app: &mut App,
    segment_idx: usize,
    force_unscoped: bool,
    query_override: Option<String>,
    as_explain: bool,
) {
    let Some(doc) = app.document() else { return };
    // Snapshot the block so we can release the immutable doc borrow
    // before mutating later.
    let block = match doc.segments().get(segment_idx) {
        Some(Segment::Block(b)) => b.clone(),
        _ => return,
    };

    if !block.is_db() {
        app.set_status(
            StatusKind::Info,
            format!("`{}` blocks aren't runnable yet", block.block_type),
        );
        return;
    }

    // Build DbParams from the block's params blob. The fence parser
    // accepts both `connection` (info-string) and `connection_id`
    // (legacy JSON body); we accept either.
    let connection_id_raw = block
        .params
        .get("connection_id")
        .or_else(|| block.params.get("connection"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if connection_id_raw.is_empty() {
        app.set_status(
            StatusKind::Error,
            "no connection set on this block (add `connection=<id>` to the fence)",
        );
        return;
    }
    let raw_query = query_override.unwrap_or_else(|| {
        block
            .params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    });
    if raw_query.is_empty() {
        app.set_status(StatusKind::Error, "empty SQL");
        return;
    }
    // Pre-flight resolves env vars + block refs + connection name.
    // These are fast (in-memory + a couple of SQLite reads) so we
    // keep them on the dispatch thread; only the actual query goes
    // async. If any pre-flight step fails the run never spawns —
    // surface the error and bail.
    let env_vars: std::collections::HashMap<String, String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(load_active_env_vars(&app.environments_store))
    })
    .unwrap_or_default();
    let resolved = match app.document() {
        Some(d) => resolve_block_refs(d.segments(), segment_idx, &raw_query, &env_vars),
        None => Ok((raw_query.clone(), Vec::new())),
    };
    let (query, bind_values) = match resolved {
        Ok(qb) => qb,
        Err(msg) => {
            if let Some(doc) = app.tabs.active_document_mut() {
                if let Some(b) = doc.block_at_mut(segment_idx) {
                    b.state = ExecutionState::Error(msg.clone());
                    b.cached_result = None;
                }
            }
            app.set_status(StatusKind::Error, msg);
            return;
        }
    };
    let limit = block
        .params
        .get("limit")
        .and_then(|v| v.as_u64())
        .unwrap_or(100);
    // Per-query timeout opt-in via `timeout=` token in the fence
    // (parser writes `timeout_ms`). `None` → executor falls back to
    // the connection's default timeout, then to 30s if the
    // connection has no override either.
    let timeout_ms = block.params.get("timeout_ms").and_then(|v| v.as_u64());

    let store = app.connections_store.clone();
    let resolved = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(resolve_connection_id(&store, &connection_id_raw))
    });
    let connection_id = match resolved {
        Ok(id) => id,
        Err(msg) => {
            if let Some(doc) = app.tabs.active_document_mut() {
                if let Some(b) = doc.block_at_mut(segment_idx) {
                    b.state = ExecutionState::Error(msg.clone());
                    b.cached_result = None;
                }
            }
            app.set_status(StatusKind::Error, msg);
            return;
        }
    };

    // Read-only gate: when the connection is flagged `is_readonly`,
    // any write statement is blocked outright. There's no confirm
    // path here — the user has to either flip the conn's flag or
    // pick a different connection. Sync lookup via `block_in_place`,
    // matching the rest of `apply_run_block` (already on the
    // dispatch thread; small SQLite read). Skipped for EXPLAIN —
    // the wrapped query is a read by definition.
    if !as_explain {
        let store = app.connections_store.clone();
        let conn_meta = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(store.get(&connection_id))
        });
        let is_readonly_conn = matches!(
            &conn_meta,
            Ok(Some(c)) if c.is_readonly
        );
        if is_readonly_conn && is_writing_query(&raw_query) {
            let msg =
                "connection is read-only — flip the flag or pick a writable connection".to_string();
            if let Some(doc) = app.tabs.active_document_mut() {
                if let Some(b) = doc.block_at_mut(segment_idx) {
                    b.state = ExecutionState::Error(msg.clone());
                    b.cached_result = None;
                }
            }
            app.set_status(StatusKind::Error, msg);
            return;
        }
    }

    // Confirm gate: ANY write (INSERT/UPDATE/DELETE/CREATE/DROP/
    // ALTER/TRUNCATE/GRANT/REVOKE/VACUUM/REPLACE/MERGE). Pops a y/n
    // modal so the user explicitly OKs every mutation — V6 audit
    // decision, prevents accidental writes on r-spam in a DB block.
    // The `reason` differentiates unscoped destructive (UPDATE/DELETE
    // without WHERE) with a stronger message; other writes get a
    // neutral confirm. Skipped when `force_unscoped` is true (the
    // user already said yes from the previous popup) — and EXPLAIN
    // calls always pass force_unscoped, so this branch is also a
    // no-op for the side-channel.
    if !force_unscoped && is_writing_query(&raw_query) {
        let s = strip_leading_sql_comments(&raw_query);
        let kind: String = s
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .to_ascii_uppercase();
        let reason = if is_unscoped_destructive(&raw_query) {
            format!("{kind} without WHERE will affect every row")
        } else {
            format!("{kind} mutates the database — confirm before running")
        };
        app.modal = Some(crate::modal::Modal::DbConfirmRun(
            crate::app::DbConfirmRunState {
                segment_idx,
                reason,
            },
        ));
        app.vim.mode = crate::vim::mode::Mode::Modal;
        return;
    }

    // Cache check — only for read queries; mutations always
    // re-execute. Pulls the active pane's file path (cache is
    // per-file) and computes the same hash recipe the desktop uses
    // (`raw_query` + only the env vars referenced in the body, then
    // `compute_block_hash`). Hit → set state to `Cached`, paint the
    // ⛁ summary, skip the spawn entirely. Miss → keep `cache_key`
    // so `handle_db_block_result` writes on success.
    //
    // EXPLAIN bypasses the cache entirely: the output is dialect-
    // specific, small, and stat-sensitive (the same EXPLAIN can
    // produce different plans across runs as the planner re-costs).
    // Caching it would either pollute the main query's cache slot
    // or need a separate slot, neither of which is worth shipping.
    // Override changes the target server — same SQL against staging
    // vs prod must NOT share a cache slot. Bypass while active; the
    // original hash resolves normally once cleared.
    let has_override = app.session_overrides.is_active(&connection_id);
    let cache_key: Option<(String, String)> = if as_explain || has_override {
        None
    } else {
        let file_path: Option<String> = app
            .active_pane()
            .and_then(|p| p.document_path.as_ref())
            .map(|p| p.to_string_lossy().to_string());
        if is_cacheable_query(&raw_query) {
            file_path.as_deref().map(|fp| {
                let hash = compute_db_cache_hash(&raw_query, Some(&connection_id), &env_vars);
                (fp.to_string(), hash)
            })
        } else {
            None
        }
    };
    if let Some((fp, hash)) = cache_key.as_ref() {
        let app_pool = app.pool_manager.app_pool().clone();
        let fp_owned = fp.clone();
        let hash_owned = hash.clone();
        let cached =
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(
                    httui_core::block_results::get_block_result(&app_pool, &fp_owned, &hash_owned),
                )
            })
            .ok()
            .flatten();
        if let Some(row) = cached {
            if row.status == "success" {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&row.response) {
                    let summary = db_summary_from_value(Some(&value), row.elapsed_ms as u64);
                    if let Some(doc) = app.tabs.active_document_mut() {
                        if let Some(b) = doc.block_at_mut(segment_idx) {
                            b.state = ExecutionState::Cached;
                            b.cached_result = Some(value);
                        }
                    }
                    app.set_status(StatusKind::Info, format!("⛁ cached · {summary}"));
                    // No AppEvent for cache hits; advance the chain ourselves.
                    crate::commands::refs::on_block_complete(app, segment_idx, true);
                    return;
                }
            }
        }
    }

    // Flip state to `Running` so the renderer paints the spinner /
    // yellow border. Skipped for EXPLAIN — the original query's
    // output stays visible while the plan loads in the background;
    // the status bar carries the "explaining…" affordance instead.
    if !as_explain {
        if let Some(doc) = app.tabs.active_document_mut() {
            if let Some(b) = doc.block_at_mut(segment_idx) {
                b.state = ExecutionState::Running;
            }
        }
    } else {
        app.set_status(StatusKind::Info, "explaining…");
    }

    let token = CancellationToken::new();
    let kind = if as_explain {
        crate::app::RunningKind::Explain
    } else {
        crate::app::RunningKind::Run
    };
    spawn_db_query(
        app,
        segment_idx,
        kind,
        token,
        connection_id,
        query,
        bind_values,
        limit,
        0,
        timeout_ms,
        cache_key,
    );
}

/// Common spawn path for both initial runs and load-more pages.
/// Captures the executor params, fires `tokio::spawn`, stores the
/// cancel handle on `App.running_query`, and arranges for the
/// completion `AppEvent::DbBlockResult` to land back in the main
/// loop. Caller is responsible for setting the block's state to
/// `ExecutionState::Running` before calling — this function only
/// owns the async dispatch.
#[allow(clippy::too_many_arguments)]
pub fn spawn_db_query(
    app: &mut App,
    segment_idx: usize,
    kind: crate::app::RunningKind,
    token: CancellationToken,
    connection_id: String,
    query: String,
    bind_values: Vec<serde_json::Value>,
    limit: u64,
    offset: u64,
    timeout_ms: Option<u64>,
    cache_key: Option<(String, String)>,
) {
    let Some(sender) = app.event_sender.clone() else {
        app.set_status(
            StatusKind::Error,
            "internal: no event sender wired (spawn aborted)",
        );
        return;
    };
    let executor = httui_core::executor::db::DbExecutor::new(app.pool_manager.clone());
    let session_override = app.session_overrides.get(&connection_id).cloned();
    let params = build_db_executor_params(
        &connection_id,
        &query,
        &bind_values,
        offset,
        limit,
        timeout_ms,
        session_override.as_ref(),
    );
    let token_for_task = token.clone();
    let kind_for_task = kind;
    tokio::spawn(async move {
        let outcome = executor
            .execute_with_cancel(params, token_for_task)
            .await
            .map_err(|e| format!("{e}"));
        let result_kind = match kind_for_task {
            crate::app::RunningKind::Run => crate::event::DbBlockResultKind::Run,
            crate::app::RunningKind::LoadMore => crate::event::DbBlockResultKind::LoadMore,
            crate::app::RunningKind::Explain => crate::event::DbBlockResultKind::Explain,
        };
        let _ = sender.send(crate::event::AppEvent::DbBlockResult {
            segment_idx,
            kind: result_kind,
            outcome,
        });
    });
    app.running_query = Some(crate::app::RunningQuery {
        segment_idx,
        cancel: token,
        started_at: std::time::Instant::now(),
        kind,
        cache_key,
        bytes_received: 0,
    });
    // Anchor for `gr` (rerun). Only Run / Explain set this — LoadMore
    // is a transparent pagination follow-up, not a fresh user dispatch,
    // so we'd otherwise pin the anchor to a load-more idx that's a
    // no-op on rerun.
    if !matches!(kind, crate::app::RunningKind::LoadMore) {
        app.record_run_anchor(segment_idx);
    }
}
