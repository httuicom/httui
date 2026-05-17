//! MongoDB `db.collection.explain("executionStats")` parser.
//!
//! Mongo's plan JSON nests stages through `inputStage` (single child)
//! or `inputStages[]` (multiple children — e.g. `OR` / `SHARD_MERGE`):
//!
//! ```json
//! {
//!   "queryPlanner": {
//!     "winningPlan": {
//!       "stage": "FETCH",
//!       "inputStage": {
//!         "stage": "IXSCAN",
//!         "indexName": "name_1",
//!         "keyPattern": { "name": 1 }
//!       }
//!     }
//!   },
//!   "executionStats": {
//!     "totalDocsExamined": 100,
//!     "executionTimeMillis": 5
//!   }
//! }
//! ```
//!
//! We translate `winningPlan` into the same `PlanNode` shape the
//! Postgres + MySQL parsers produce. Mongo doesn't expose per-stage
//! cost numbers, so `cost` stays empty and `pct` is 0 (or 100 for
//! root) — the UI's cost-bar reads 0 cleanly. Per-stage `nReturned`
//! from `executionStats` carries when present; otherwise rows is 0.
//! `target` is derived from per-stage fields (`indexName` for
//! IXSCAN, `keyPattern` rendered for index lookups).

use serde_json::Value;

use super::PlanNode;

/// Document threshold above which `COLLSCAN` triggers a warn.
/// Mirrors the Postgres `Seq Scan` heuristic — full collection
/// scan over a large dataset is the equivalent shape.
pub const COLLSCAN_WARN_DOCS: u64 = 10_000;

pub fn parse_mongo_explain(json: &Value) -> Option<PlanNode> {
    let winning = json
        .get("queryPlanner")
        .and_then(|v| v.get("winningPlan"))?;
    Some(parse_stage(winning, true))
}

fn parse_stage(stage: &Value, is_root: bool) -> PlanNode {
    let op = stage
        .get("stage")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let target = derive_target(stage);
    let rows = stage
        .get("nReturned")
        .and_then(|v| v.as_u64())
        .or_else(|| stage.get("docsExamined").and_then(|v| v.as_u64()))
        .unwrap_or(0);

    let mut children = Vec::new();
    if let Some(input) = stage.get("inputStage") {
        children.push(parse_stage(input, false));
    }
    if let Some(arr) = stage.get("inputStages").and_then(|v| v.as_array()) {
        for child in arr {
            children.push(parse_stage(child, false));
        }
    }

    let warn = compute_warn(&op, &rows, stage);
    PlanNode {
        op,
        target,
        cost: String::new(),
        rows,
        pct: if is_root { 100 } else { 0 },
        warn,
        children,
    }
}

fn derive_target(stage: &Value) -> String {
    if let Some(name) = stage.get("indexName").and_then(|v| v.as_str()) {
        if !name.is_empty() {
            return name.to_string();
        }
    }
    if let Some(pattern) = stage.get("keyPattern") {
        let rendered = render_key_pattern(pattern);
        if !rendered.is_empty() {
            return rendered;
        }
    }
    if let Some(direction) = stage.get("direction").and_then(|v| v.as_str()) {
        return format!("direction={direction}");
    }
    String::new()
}

fn render_key_pattern(pattern: &Value) -> String {
    let map = match pattern.as_object() {
        Some(m) => m,
        None => return String::new(),
    };
    let parts: Vec<String> = map
        .iter()
        .map(|(k, v)| {
            let dir = v.as_i64().unwrap_or(0);
            format!("{k}: {dir}")
        })
        .collect();
    if parts.is_empty() {
        String::new()
    } else {
        format!("{{ {} }}", parts.join(", "))
    }
}

fn compute_warn(op: &str, rows: &u64, _stage: &Value) -> bool {
    op == "COLLSCAN" && *rows > COLLSCAN_WARN_DOCS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Option<PlanNode> {
        let v: Value = serde_json::from_str(s).unwrap();
        parse_mongo_explain(&v)
    }

    #[test]
    fn parse_returns_none_when_no_winning_plan() {
        assert!(parse("{}").is_none());
        assert!(parse(r#"{"queryPlanner":{}}"#).is_none());
    }

    #[test]
    fn parse_minimal_collscan() {
        let json = r#"{
            "queryPlanner": {
                "winningPlan": {
                    "stage": "COLLSCAN",
                    "direction": "forward"
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.op, "COLLSCAN");
        assert_eq!(n.target, "direction=forward");
        assert_eq!(n.rows, 0);
        assert_eq!(n.pct, 100);
    }

    #[test]
    fn parse_fetch_with_ixscan_input_stage() {
        let json = r#"{
            "queryPlanner": {
                "winningPlan": {
                    "stage": "FETCH",
                    "inputStage": {
                        "stage": "IXSCAN",
                        "indexName": "name_1"
                    }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.op, "FETCH");
        assert_eq!(n.children.len(), 1);
        assert_eq!(n.children[0].op, "IXSCAN");
        assert_eq!(n.children[0].target, "name_1");
    }

    #[test]
    fn parse_handles_multiple_input_stages_via_input_stages() {
        // OR-style queries fan out via inputStages.
        let json = r#"{
            "queryPlanner": {
                "winningPlan": {
                    "stage": "OR",
                    "inputStages": [
                        { "stage": "IXSCAN", "indexName": "a_1" },
                        { "stage": "IXSCAN", "indexName": "b_1" }
                    ]
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.op, "OR");
        assert_eq!(n.children.len(), 2);
        assert_eq!(n.children[0].target, "a_1");
        assert_eq!(n.children[1].target, "b_1");
    }

    #[test]
    fn parse_target_falls_back_to_key_pattern_when_no_index_name() {
        let json = r#"{
            "queryPlanner": {
                "winningPlan": {
                    "stage": "IXSCAN",
                    "keyPattern": { "name": 1, "age": -1 }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert!(n.target.contains("name: 1"));
        assert!(n.target.contains("age: -1"));
    }

    #[test]
    fn parse_collscan_with_high_docs_examined_warns() {
        let json = format!(
            r#"{{
                "queryPlanner": {{
                    "winningPlan": {{
                        "stage": "COLLSCAN",
                        "docsExamined": {}
                    }}
                }}
            }}"#,
            COLLSCAN_WARN_DOCS + 1,
        );
        let n = parse(&json).unwrap();
        assert!(n.warn);
    }

    #[test]
    fn parse_collscan_below_threshold_no_warn() {
        let json = r#"{
            "queryPlanner": {
                "winningPlan": {
                    "stage": "COLLSCAN",
                    "docsExamined": 100
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert!(!n.warn);
    }

    #[test]
    fn parse_ixscan_does_not_warn_even_at_high_rows() {
        // Only COLLSCAN triggers the doc-count warn; IXSCAN is by
        // definition the wanted shape.
        let json = format!(
            r#"{{
                "queryPlanner": {{
                    "winningPlan": {{
                        "stage": "IXSCAN",
                        "indexName": "x",
                        "nReturned": {}
                    }}
                }}
            }}"#,
            COLLSCAN_WARN_DOCS + 1,
        );
        let n = parse(&json).unwrap();
        assert!(!n.warn);
    }

    #[test]
    fn parse_uses_n_returned_when_present_else_docs_examined() {
        let json_n = r#"{"queryPlanner":{"winningPlan":{"stage":"FETCH","nReturned":42}}}"#;
        assert_eq!(parse(json_n).unwrap().rows, 42);
        let json_d = r#"{"queryPlanner":{"winningPlan":{"stage":"FETCH","docsExamined":7}}}"#;
        assert_eq!(parse(json_d).unwrap().rows, 7);
    }

    #[test]
    fn parse_unknown_stage_falls_back_to_unknown_op() {
        let json = r#"{"queryPlanner":{"winningPlan":{}}}"#;
        let n = parse(json).unwrap();
        assert_eq!(n.op, "Unknown");
    }

    #[test]
    fn parse_root_pct_is_100_children_pct_is_0() {
        let json = r#"{
            "queryPlanner": {
                "winningPlan": {
                    "stage": "FETCH",
                    "inputStage": { "stage": "IXSCAN", "indexName": "x" }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.pct, 100);
        assert_eq!(n.children[0].pct, 0);
    }

    #[test]
    fn parse_deep_chain_keeps_depth() {
        let json = r#"{
            "queryPlanner": {
                "winningPlan": {
                    "stage": "PROJECTION",
                    "inputStage": {
                        "stage": "FETCH",
                        "inputStage": {
                            "stage": "IXSCAN",
                            "indexName": "x"
                        }
                    }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.depth(), 3);
    }
}
