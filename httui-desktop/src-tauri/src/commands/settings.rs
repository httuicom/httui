// coverage:exclude file — Tauri command shells delegating to
// `httui_core::config`. Same shape and rationale as
// `commands/{connections,environments,files,schema}.rs`
// (audit-016 / 018). Substantive logic lives in `httui_core::config`.

//! App-config Tauri commands — get / set on the `app_config` SQLite
//! table. The full settings split (per-machine `user.toml` vs
//! workspace `workspace.toml`) is the job; these commands stay
//! pointed at the legacy SQLite-backed `app_config` for v1 boot
//! compatibility.

use sqlx::sqlite::SqlitePool;
use tauri::State;

use httui_core::config;

/// Read a single key from the `app_config` table.
#[tauri::command]
pub async fn get_config(
    pool: State<'_, SqlitePool>,
    key: String,
) -> Result<Option<String>, String> {
    config::get_config(&pool, &key)
        .await
        .map_err(|e| e.to_string())
}

/// Upsert a single key into the `app_config` table.
#[tauri::command]
pub async fn set_config(
    pool: State<'_, SqlitePool>,
    key: String,
    value: String,
) -> Result<(), String> {
    config::set_config(&pool, &key, &value)
        .await
        .map_err(|e| e.to_string())
}
