//! DB block run-history snapshot + insert helper.

use crate::app::App;
use crate::buffer::Segment;

/// Snapshot of the bits we need to write a `block_run_history` row
/// for a DB block run. Captured up-front (before the result handler
/// mutates state) so the Ok and Err arms can share a single insert
/// path without re-walking the segment list.
#[derive(Clone)]
pub struct DbHistoryMeta {
    pub file_path: String,
    pub block_alias: String,
    /// Stored in the `method` column as `db:<driver>` (e.g.
    /// `db:postgres`). Mirrors how the HTTP path uses `method` as
    /// the request "kind" — same column, different namespace.
    pub method: String,
    /// SQL preview (~200 chars, single-line). Goes in the
    /// `url_canonical` column whose semantic role is "what was
    /// run" — for HTTP that's URL+query; for DB it's the SQL.
    pub url_canonical: String,
    pub request_size: i64,
}

pub fn snapshot_db_history_meta(app: &App, segment_idx: usize) -> Option<DbHistoryMeta> {
    let tab = app.tabs.tabs.get(app.tabs.active())?;
    let file_path = tab.active_leaf().document_path.as_ref()?;
    let file_path = file_path.to_string_lossy().to_string();
    let doc = app.document()?;
    let block = match doc.segments().get(segment_idx)? {
        Segment::Block(b) => b,
        _ => return None,
    };
    if !block.is_db() {
        return None;
    }
    let alias = block
        .alias
        .as_deref()
        .filter(|s| !s.is_empty())?
        .to_string();
    // `block_type` is `db-postgres`, `db-mysql`, `db-sqlite`. Strip
    // the `db-` prefix for the namespaced method label so the
    // history modal shows a clean `postgres` / `mysql` / `sqlite`
    // chip instead of repeating `db-` everywhere.
    let driver = block
        .block_type
        .strip_prefix("db-")
        .unwrap_or(&block.block_type);
    let method = format!("db:{driver}");

    let query = block
        .params
        .get("query")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let url_canonical = preview_sql(&query);
    let request_size = query.len() as i64;
    Some(DbHistoryMeta {
        file_path,
        block_alias: alias,
        method,
        url_canonical,
        request_size,
    })
}

/// Collapse newlines to spaces and trim to ~200 chars so the
/// history modal's row stays readable. Suffix `…` when truncated.
pub(crate) fn preview_sql(sql: &str) -> String {
    const MAX: usize = 200;
    let collapsed = sql
        .replace(['\r', '\n', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.chars().count() > MAX {
        let truncated: String = collapsed.chars().take(MAX).collect();
        format!("{truncated}…")
    } else {
        collapsed
    }
}

/// Derive the `(status, response_size)` columns from a successful
/// DB response. Status borrows the column for "result kind size":
/// `rows.len()` for a SELECT, `rows_affected` for a mutation,
/// `None` for an error result. Response size is the serialized
/// JSON length — coarse but useful for spotting regressions.
pub fn derive_db_history_stats(
    response: &httui_core::executor::db::types::DbResponse,
) -> (Option<i64>, Option<i64>) {
    use httui_core::executor::db::types::DbResult;
    let status = match response.results.first() {
        Some(DbResult::Select { rows, .. }) => Some(rows.len() as i64),
        Some(DbResult::Mutation { rows_affected }) => Some(*rows_affected as i64),
        _ => None,
    };
    let response_size = serde_json::to_string(response).ok().map(|s| s.len() as i64);
    (status, response_size)
}

/// Spawn the SQLite insert in the background (mirrors the HTTP
/// path). Failures land in the tracing log — history is best-
/// effort and never blocks the user.
pub fn record_db_history_async(
    pool: sqlx::SqlitePool,
    meta: DbHistoryMeta,
    elapsed_ms: Option<i64>,
    status: Option<i64>,
    response_size: Option<i64>,
    outcome: &'static str,
) {
    let entry = httui_core::block_history::InsertEntry {
        file_path: meta.file_path,
        block_alias: meta.block_alias,
        method: meta.method,
        url_canonical: meta.url_canonical,
        status,
        request_size: Some(meta.request_size),
        response_size,
        elapsed_ms,
        outcome: outcome.to_string(),
        plan: None,
    };
    tokio::spawn(async move {
        if let Err(e) = httui_core::block_history::insert_history_entry(&pool, entry).await {
            tracing::warn!("db block history insert failed: {e}");
        }
    });
}
