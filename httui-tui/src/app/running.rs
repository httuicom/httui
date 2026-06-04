//! In-flight async query bookkeeping.
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-running) — pure code move, no behavior change. Re-exported from
//! `app/mod.rs` so `crate::app::{RunningQuery, RunningKind}` keep
//! resolving. Structural-only (no `impl`/`fn`) → coverage auto-pass.

use std::time::Instant;

use tokio_util::sync::CancellationToken;

/// In-flight async DB execution. Stores the cancel handle so a
/// `Ctrl-C` can abort the running future, plus enough context to
/// fold the result into the right block when the spawned task
/// reports back via `AppEvent::DbBlockResult`. The "unused" fields
/// (`segment_idx`, `started_at`, `kind`) are read by the renderer
/// (spinner placement / elapsed display) — `#[allow(dead_code)]`
/// keeps the warning quiet until that lands.
#[allow(dead_code)]
pub struct RunningQuery {
    pub segment_idx: usize,
    pub cancel: CancellationToken,
    pub started_at: Instant,
    pub kind: RunningKind,
    /// Cache key for save-on-success: `(file_path, hash)`. Populated
    /// by `apply_run_block` only for cacheable queries (SELECT-ish)
    /// and only when the active pane has a file path. `None` for
    /// mutations and load-more pages — they never write to the cache.
    pub cache_key: Option<(String, String)>,
    /// Cumulative response bytes received so far (HTTP only). Driven
    /// by `AppEvent::HttpBlockChunk` events emitted from the
    /// streaming executor; the status bar reads this to paint a
    /// download progress hint while a request is in flight.
    pub bytes_received: u64,
    /// Pre-spawn snapshot used to persist the HTTP response cache on
    /// success. Carried on the running query (not re-derived in the
    /// completion handler) because the user may have moved focus to
    /// another pane between dispatch and completion — `app.document()`
    /// at completion time could point at a different block.
    pub http_cache_meta: Option<HttpRunCacheMeta>,
}

/// Snapshot of everything needed to write a `block_results` row for an
/// HTTP run that's in flight: where to key it, the alias to attach,
/// and the canonical inputs that produce the hash.
#[derive(Clone)]
pub struct HttpRunCacheMeta {
    pub file_path: String,
    pub alias: Option<String>,
    pub method: String,
    pub url: String,
    pub params: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum RunningKind {
    /// Initial run (`r` keypress) — replaces the block's
    /// `cached_result` on completion.
    Run,
    /// Pagination triggered by the result-table prefetch — appends
    /// rows to the existing `cached_result`.
    LoadMore,
    /// `<C-x>` (EXPLAIN) — runs the query wrapped in the dialect's
    /// EXPLAIN keyword. Lands in `cached_result["plan"]` so the
    /// original query's output stays intact; auto-switches the
    /// result tab to `Plan` so the user sees the new plan.
    Explain,
}
