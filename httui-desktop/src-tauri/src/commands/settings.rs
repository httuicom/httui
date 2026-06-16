// coverage:exclude file — Tauri command shells with no testable logic without a Tauri runtime.

//! App-config Tauri commands — get / set on the `app_config` SQLite table.

use sqlx::sqlite::SqlitePool;
use tauri::State;

use httui_core::config;
use httui_core::db::feature_usage::{self, FeatureUsage};

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

/// Features the local usage dashboard records. Bounding the set keeps a
/// frontend typo from spawning junk rows in the aggregate table.
const KNOWN_FEATURES: &[&str] = &["http_block_run", "db_block_run"];

/// Increment today's local counter for `feature`. Recording is gated by the
/// frontend opt-in, so this trusts the caller; unknown feature names are
/// ignored rather than erroring — telemetry must never break a block run.
#[tauri::command]
pub async fn record_feature_usage(
    pool: State<'_, SqlitePool>,
    feature: String,
) -> Result<(), String> {
    if !KNOWN_FEATURES.contains(&feature.as_str()) {
        return Ok(());
    }
    feature_usage::record_feature_usage(&pool, &feature).await
}

/// Daily per-feature counts in the inclusive `[from, to]` range
/// (ISO `YYYY-MM-DD`). Powers the usage dashboard.
#[tauri::command]
pub async fn get_feature_usage(
    pool: State<'_, SqlitePool>,
    from: String,
    to: String,
) -> Result<Vec<FeatureUsage>, String> {
    feature_usage::get_feature_usage_by_date_range(&pool, &from, &to).await
}

/// Wipe all locally recorded usage. Backs the dashboard's reset control.
#[tauri::command]
pub async fn clear_feature_usage(pool: State<'_, SqlitePool>) -> Result<(), String> {
    feature_usage::clear_feature_usage(&pool).await
}
