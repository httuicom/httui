// coverage:exclude file — Tauri command shells delegating to
// `httui_core::block_results`, `block_history`, `block_settings`,
// `block_examples`, and `executor::*`. Same shape and rationale as
// `commands/{connections,environments,files,schema,settings}.rs`
// (audit-016 / 018). Substantive logic lives in those crate modules
// at >80% coverage; the shells thread Tauri state and call through.

//! Block-related Tauri commands — generic dispatch
//! (`execute_block`), result cache, run history, per-block settings,
//! pinned response examples, and the server-side block-hash
//! computation.
//!
//! The streamed/cancel-aware HTTP and DB execution paths live in
//! `executions.rs` — this module is for the legacy non-streamed
//! `execute_block` shell plus the persistence side (history /
//! settings / examples) that the UI hits via Tauri IPC.

use std::sync::Arc;

use sqlx::sqlite::SqlitePool;
use tauri::State;

use httui_core::block_examples::{self, BlockExample};
use httui_core::block_history::{
    self, summarize_last_run, HistoryEntry, InsertEntry, LastRunSummary,
};
use httui_core::block_results::{self, CachedBlockResult};
use httui_core::block_settings::{self, BlockSettings};
use httui_core::db::environments::get_active_environment_id;
use httui_core::executor::{
    self, BlockRequest, BlockResult, Executor, ExecutorError, ExecutorRegistry,
};

// --- Executor wrapper newtypes ------------------------------------------

/// Newtype wrapper letting the registry hold `DbExecutor` via `Arc` so
/// the same instance can also back the streamed/cancel-aware Tauri
/// command in `executions.rs`.
pub struct SharedDbExecutor(pub Arc<executor::db::DbExecutor>);

#[async_trait::async_trait]
impl Executor for SharedDbExecutor {
    fn block_type(&self) -> &str {
        self.0.block_type()
    }

    async fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        self.0.validate(params).await
    }

    async fn execute(
        &self,
        params: serde_json::Value,
    ) -> Result<BlockResult, ExecutorError> {
        self.0.execute(params).await
    }
}

/// Same pattern as `SharedDbExecutor` for the HTTP executor. The
/// streamed command pulls the `Arc<HttpExecutor>` from Tauri state;
/// the legacy `execute_block` path continues through the registry via
/// this wrapper.
pub struct SharedHttpExecutor(pub Arc<executor::http::HttpExecutor>);

#[async_trait::async_trait]
impl Executor for SharedHttpExecutor {
    fn block_type(&self) -> &str {
        self.0.block_type()
    }

    async fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        self.0.validate(params).await
    }

    async fn execute(
        &self,
        params: serde_json::Value,
    ) -> Result<BlockResult, ExecutorError> {
        self.0.execute(params).await
    }
}

// --- Generic dispatch ----------------------------------------------------

/// Generic dispatch: route a `BlockRequest` to the executor registered
/// under `block_type`. Used by the legacy non-streamed path; streamed
/// HTTP/DB execution lives in `executions.rs`.
#[tauri::command]
pub async fn execute_block(
    registry: State<'_, ExecutorRegistry>,
    block_type: String,
    params: serde_json::Value,
) -> Result<BlockResult, String> {
    let req = BlockRequest { block_type, params };
    registry.execute(req).await.map_err(|e| e.to_string())
}

// --- Block result cache --------------------------------------------------

/// Look up a previously cached `BlockResult` by `(file_path, block_hash)`.
/// Returns `None` if no cached row matches.
#[tauri::command]
pub async fn get_block_result(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_hash: String,
) -> Result<Option<CachedBlockResult>, String> {
    block_results::get_block_result(&pool, &file_path, &block_hash)
        .await
        .map_err(|e| e.to_string())
}

/// Persist the terminal outcome of a block execution into the cache so
/// the next run with the same content + env context can short-circuit.
#[tauri::command]
pub async fn save_block_result(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_hash: String,
    status: String,
    response: String,
    elapsed_ms: i64,
    total_rows: Option<i64>,
) -> Result<(), String> {
    block_results::save_block_result(
        &pool,
        &file_path,
        &block_hash,
        &status,
        &response,
        elapsed_ms,
        total_rows,
    )
    .await
    .map_err(|e| e.to_string())
}

// --- Block run history (Story 24.6) -------------------------------------

/// Return the trim-capped run history (metadata only — no bodies)
/// for `(file_path, block_alias)`.
#[tauri::command]
pub async fn list_block_history(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_alias: String,
) -> Result<Vec<HistoryEntry>, String> {
    block_history::list_history(&pool, &file_path, &block_alias)
        .await
        .map_err(|e| e.to_string())
}

/// Return the most recent N run-history rows for a file across all
/// aliases. Powers the Epic 29 sidebar History tab. Pass `limit <= 0`
/// to fall back to the 50-entry default.
#[tauri::command]
pub async fn list_block_history_for_file(
    pool: State<'_, SqlitePool>,
    file_path: String,
    limit: i64,
) -> Result<Vec<HistoryEntry>, String> {
    block_history::list_history_for_file(&pool, &file_path, limit)
        .await
        .map_err(|e| e.to_string())
}

/// Aggregate the most recent run-all session for a file. Powers
/// Epic 50 Story 03's `<DocHeaderMetaStrip>` Last-run chip — pulls
/// the latest 50 rows + applies `summarize_last_run`'s 5s session
/// window heuristic so the consumer just renders `formatLastRun`.
#[tauri::command]
pub async fn block_history_last_run_summary(
    pool: State<'_, SqlitePool>,
    file_path: String,
) -> Result<LastRunSummary, String> {
    let entries = block_history::list_history_for_file(&pool, &file_path, 50)
        .await
        .map_err(|e| e.to_string())?;
    Ok(summarize_last_run(&entries))
}

/// Append a single run-history row; trim to the retention cap is
/// handled by the underlying `insert_history_entry`.
#[tauri::command]
pub async fn insert_block_history(
    pool: State<'_, SqlitePool>,
    entry: InsertEntry,
) -> Result<(), String> {
    block_history::insert_history_entry(&pool, entry)
        .await
        .map_err(|e| e.to_string())
}

/// Delete every run-history row for `(file_path, block_alias)`.
/// Returns the number of rows removed.
#[tauri::command]
pub async fn purge_block_history(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_alias: String,
) -> Result<u64, String> {
    block_history::purge_history(&pool, &file_path, &block_alias)
        .await
        .map_err(|e| e.to_string())
}

// --- Per-block settings (Onda 1) ----------------------------------------

/// Fetch persistent per-block settings (limit/timeout overrides) for
/// `(file_path, block_alias)`. Returns defaults if no row exists.
#[tauri::command]
pub async fn get_block_settings(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_alias: String,
) -> Result<BlockSettings, String> {
    block_settings::get_settings(&pool, &file_path, &block_alias)
        .await
        .map_err(|e| e.to_string())
}

/// Insert or update the per-block settings row.
#[tauri::command]
pub async fn upsert_block_settings(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_alias: String,
    settings: BlockSettings,
) -> Result<(), String> {
    block_settings::upsert_settings(&pool, &file_path, &block_alias, settings)
        .await
        .map_err(|e| e.to_string())
}

/// Delete per-block settings for `(file_path, block_alias)` — used when
/// the block is removed from the document.
#[tauri::command]
pub async fn purge_block_settings(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_alias: String,
) -> Result<u64, String> {
    block_settings::purge_settings(&pool, &file_path, &block_alias)
        .await
        .map_err(|e| e.to_string())
}

// --- Pinned response examples (Onda 3) ----------------------------------

/// Pin a named response snapshot for a block so the user can revisit
/// it later without re-running.
#[tauri::command]
pub async fn save_block_example(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_alias: String,
    name: String,
    response_json: String,
) -> Result<i64, String> {
    block_examples::save_example(&pool, &file_path, &block_alias, &name, &response_json)
        .await
        .map_err(|e| e.to_string())
}

/// List every pinned example for `(file_path, block_alias)`.
#[tauri::command]
pub async fn list_block_examples(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_alias: String,
) -> Result<Vec<BlockExample>, String> {
    block_examples::list_examples(&pool, &file_path, &block_alias)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a single pinned example by primary key.
#[tauri::command]
pub async fn delete_block_example(pool: State<'_, SqlitePool>, id: i64) -> Result<u64, String> {
    block_examples::delete_example(&pool, id)
        .await
        .map_err(|e| e.to_string())
}

/// Delete every pinned example for `(file_path, block_alias)`.
#[tauri::command]
pub async fn purge_block_examples(
    pool: State<'_, SqlitePool>,
    file_path: String,
    block_alias: String,
) -> Result<u64, String> {
    block_examples::purge_examples_for_block(&pool, &file_path, &block_alias)
        .await
        .map_err(|e| e.to_string())
}

// --- Block hash ----------------------------------------------------------

/// T31/T35: Server-side hash computation including environment +
/// connection context.
#[tauri::command]
pub async fn compute_block_hash(
    pool: State<'_, SqlitePool>,
    content: String,
    connection_id: Option<String>,
) -> Result<String, String> {
    let env_id = get_active_environment_id(&pool).await;
    Ok(block_results::compute_block_hash(
        &content,
        env_id.as_deref(),
        connection_id.as_deref(),
    ))
}
