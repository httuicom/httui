//! Public DTOs for `block_run_history` rows. Co-located here so
//! the queries module (which carries the SQL + tests) doesn't pay
//! coverage tax on derive-generated code paths that aren't directly
//! exercised by line-counting tools.
//!
//! Keep this file small + cohesive. If a struct grows enough to
//! deserve its own validation logic, that lives next to the
//! function that builds it — not here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: i64,
    pub file_path: String,
    pub block_alias: String,
    pub method: String,
    pub url_canonical: String,
    pub status: Option<i64>,
    pub request_size: Option<i64>,
    pub response_size: Option<i64>,
    pub elapsed_ms: Option<i64>,
    pub outcome: String,
    pub ran_at: String,
    /// `EXPLAIN`-flavoured JSON plan when the SQL block carried
    /// `explain=true`. Capped via `crate::explain::cap_explain_body`
    /// (200 KB). `None` for regular runs and any non-SQL block.
    /// (migration 012).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertEntry {
    pub file_path: String,
    pub block_alias: String,
    pub method: String,
    pub url_canonical: String,
    pub status: Option<i64>,
    pub request_size: Option<i64>,
    pub response_size: Option<i64>,
    pub elapsed_ms: Option<i64>,
    pub outcome: String,
    #[serde(default)]
    pub plan: Option<String>,
}
