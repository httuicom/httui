//! Response shape for the `db-*` block type.
//!
//! Supports multi-statement / multi-result-set queries. The executor emits
//! `results` with one entry per statement; consumers index via `results[i]`.
//! Ref-resolution keeps `{{alias.response.col}}` working against `results[0].rows[0]`.

use serde::{Deserialize, Serialize};

use crate::db::connections::ColumnInfo;

/// Top-level response wrapper for a db-* block execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbResponse {
    pub results: Vec<DbResult>,
    #[serde(default)]
    pub messages: Vec<DbMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<serde_json::Value>,
    pub stats: DbStats,
}

/// One result set produced by a single statement within a db block.
///
/// Serialized with a `kind` discriminator so the TS union is a clean
/// `{ kind: "select" | "mutation" | "error", ... }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DbResult {
    Select {
        columns: Vec<ColumnInfo>,
        rows: Vec<serde_json::Value>,
        has_more: bool,
    },
    Mutation {
        rows_affected: u64,
    },
    Error {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        line: Option<u32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        column: Option<u32>,
    },
}

/// A backend-emitted message (NOTICE, WARNING, RAISE).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbMessage {
    pub severity: DbMessageSeverity,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DbMessageSeverity {
    Notice,
    Warning,
    Error,
}

/// Per-execution timing and counter stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbStats {
    pub elapsed_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows_streamed: Option<u64>,
}

/// Streaming chunk emitted to a `tauri::Channel<DbChunk>` during execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DbChunk {
    /// Terminal chunk containing the full response. Consumer should close
    /// the subscription after receiving this.
    Complete(DbResponse),
    /// Terminal chunk indicating the execution failed before completing.
    Error { message: String },
    /// Terminal chunk indicating the execution was cancelled.
    Cancelled,
}

impl DbResponse {
    /// Build a single-select-result response (the common case today).
    pub fn single_select(
        columns: Vec<ColumnInfo>,
        rows: Vec<serde_json::Value>,
        has_more: bool,
        elapsed_ms: u64,
    ) -> Self {
        Self {
            results: vec![DbResult::Select {
                columns,
                rows,
                has_more,
            }],
            messages: Vec::new(),
            plan: None,
            stats: DbStats {
                elapsed_ms,
                rows_streamed: None,
            },
        }
    }

    /// Build a single-mutation-result response.
    pub fn single_mutation(rows_affected: u64, elapsed_ms: u64) -> Self {
        Self {
            results: vec![DbResult::Mutation { rows_affected }],
            messages: Vec::new(),
            plan: None,
            stats: DbStats {
                elapsed_ms,
                rows_streamed: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_select_with_kind_tag() {
        let resp = DbResponse::single_select(
            vec![ColumnInfo {
                name: "id".into(),
                type_name: "int".into(),
            }],
            vec![serde_json::json!({"id": 1})],
            false,
            12,
        );
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["results"][0]["kind"], "select");
        assert_eq!(json["results"][0]["columns"][0]["name"], "id");
        // ColumnInfo.type_name is renamed to `type` on the wire for TS ergonomics.
        assert_eq!(json["results"][0]["columns"][0]["type"], "int");
        assert!(json["results"][0]["columns"][0].get("type_name").is_none());
        assert_eq!(json["results"][0]["rows"][0]["id"], 1);
        assert_eq!(json["results"][0]["has_more"], false);
        assert_eq!(json["stats"]["elapsed_ms"], 12);
        assert!(json["plan"].is_null() || json.get("plan").is_none());
    }

    #[test]
    fn serializes_mutation_with_kind_tag() {
        let resp = DbResponse::single_mutation(3, 5);
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["results"][0]["kind"], "mutation");
        assert_eq!(json["results"][0]["rows_affected"], 3);
    }

    #[test]
    fn deserializes_error_variant() {
        let json = serde_json::json!({
            "results": [{
                "kind": "error",
                "message": "relation \"foo\" does not exist",
                "line": 2,
                "column": 6,
            }],
            "messages": [],
            "stats": { "elapsed_ms": 8 }
        });
        let resp: DbResponse = serde_json::from_value(json).unwrap();
        match &resp.results[0] {
            DbResult::Error {
                message,
                line,
                column,
            } => {
                assert!(message.contains("relation"));
                assert_eq!(*line, Some(2));
                assert_eq!(*column, Some(6));
            }
            _ => panic!("expected Error variant"),
        }
    }

    #[test]
    fn plan_omitted_when_absent_on_serialize() {
        let resp = DbResponse::single_mutation(1, 1);
        let s = serde_json::to_string(&resp).unwrap();
        assert!(!s.contains("\"plan\""));
    }

    #[test]
    fn db_chunk_complete_serializes_inline() {
        let chunk = DbChunk::Complete(DbResponse::single_mutation(2, 4));
        let json = serde_json::to_value(&chunk).unwrap();
        assert_eq!(json["kind"], "complete");
        assert_eq!(json["results"][0]["kind"], "mutation");
        assert_eq!(json["results"][0]["rows_affected"], 2);
        assert_eq!(json["stats"]["elapsed_ms"], 4);
    }

    #[test]
    fn db_chunk_error_and_cancelled_serialize() {
        let err = DbChunk::Error {
            message: "boom".into(),
        };
        let err_json = serde_json::to_value(&err).unwrap();
        assert_eq!(err_json["kind"], "error");
        assert_eq!(err_json["message"], "boom");

        let cancelled = DbChunk::Cancelled;
        let cancelled_json = serde_json::to_value(&cancelled).unwrap();
        assert_eq!(cancelled_json["kind"], "cancelled");
    }

    #[test]
    fn messages_severity_roundtrip() {
        let resp = DbResponse {
            results: Vec::new(),
            messages: vec![
                DbMessage {
                    severity: DbMessageSeverity::Notice,
                    text: "hello".into(),
                    code: Some("00000".into()),
                },
                DbMessage {
                    severity: DbMessageSeverity::Warning,
                    text: "slow".into(),
                    code: None,
                },
            ],
            plan: None,
            stats: DbStats {
                elapsed_ms: 0,
                rows_streamed: None,
            },
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains("\"severity\":\"notice\""));
        assert!(s.contains("\"severity\":\"warning\""));
        let parsed: DbResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.messages.len(), 2);
    }
}
