//! MySQL `EXPLAIN FORMAT=JSON` parser.
//!
//! MySQL's plan JSON differs from Postgres's array-of-Plan shape;
//! the tree is rooted at `query_block` and uses `nested_loop` for
//! joins + nested `query_block` for subqueries:
//!
//! ```json
//! {
//!   "query_block": {
//!     "select_id": 1,
//!     "cost_info": { "query_cost": "1.20" },
//!     "nested_loop": [
//!       { "table": {
//!           "table_name": "users",
//!           "access_type": "ALL",
//!           "rows_examined_per_scan": 1000,
//!           "cost_info": { "read_cost": "1.0", "eval_cost": "0.2" }
//!       }},
//!       { "table": { ... } }
//!     ]
//!   }
//! }
//! ```
//!
//! We translate to the same `PlanNode` shape the Postgres parser
//! produces so the React `<ExplainPlan>` consumer doesn't fan out
//! per backend. `pct` is computed against the root query cost; the
//! warn heuristic flags `access_type = "ALL"` (full table scan)
//! over the row threshold + non-root cost share over 50%.

use serde_json::Value;

use super::PlanNode;

/// Row threshold above which `access_type=ALL` triggers a warn.
/// Mirrors the Postgres `Seq Scan` heuristic in spirit.
pub const FULL_SCAN_WARN_ROWS: u64 = 10_000;

/// Cost-share threshold; mirrors `postgres::COST_SHARE_WARN`.
pub const COST_SHARE_WARN: u8 = 50;

pub fn parse_mysql_explain(json: &Value) -> Option<PlanNode> {
    let query_block = json.get("query_block")?;
    let root_cost = root_query_cost(query_block);
    Some(parse_query_block(query_block, root_cost, true))
}

fn parse_query_block(qb: &Value, root_cost: f64, is_root: bool) -> PlanNode {
    let select_id = qb
        .get("select_id")
        .and_then(|v| v.as_u64())
        .map(|n| n.to_string())
        .unwrap_or_default();
    let cost = qb
        .get("cost_info")
        .and_then(|c| c.get("query_cost"))
        .and_then(|v| {
            v.as_str()
                .map(|s| s.to_string())
                .or_else(|| v.as_f64().map(|f| format!("{f:.2}")))
        })
        .unwrap_or_default();
    let total_cost = parse_cost_str(&cost);
    let pct = pct_of(total_cost, root_cost);
    let target = if select_id.is_empty() {
        String::new()
    } else {
        format!("select_id={select_id}")
    };
    let mut children = Vec::new();
    if let Some(nl) = qb.get("nested_loop").and_then(|v| v.as_array()) {
        for item in nl {
            children.push(parse_nested_item(item, root_cost));
        }
    }
    if let Some(table) = qb.get("table") {
        children.push(parse_table(table, root_cost));
    }
    if let Some(sub) = qb.get("nested") {
        // Some shapes use `nested` for subqueries.
        if let Some(arr) = sub.as_array() {
            for item in arr {
                if let Some(qb2) = item.get("query_block") {
                    children.push(parse_query_block(qb2, root_cost, false));
                }
            }
        }
    }
    let warn = !is_root && pct > COST_SHARE_WARN;
    PlanNode {
        op: "Query Block".to_string(),
        target,
        cost,
        rows: 0,
        pct,
        warn,
        children,
    }
}

fn parse_nested_item(item: &Value, root_cost: f64) -> PlanNode {
    if let Some(table) = item.get("table") {
        return parse_table(table, root_cost);
    }
    if let Some(qb) = item.get("query_block") {
        return parse_query_block(qb, root_cost, false);
    }
    PlanNode::leaf("Unknown")
}

fn parse_table(table: &Value, root_cost: f64) -> PlanNode {
    let access_type = table
        .get("access_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let table_name = table
        .get("table_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let rows = table
        .get("rows_examined_per_scan")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let read_cost = table
        .get("cost_info")
        .and_then(|c| c.get("read_cost"))
        .and_then(|v| v.as_str().map(parse_cost_str).or_else(|| v.as_f64()))
        .unwrap_or(0.0);
    let eval_cost = table
        .get("cost_info")
        .and_then(|c| c.get("eval_cost"))
        .and_then(|v| v.as_str().map(parse_cost_str).or_else(|| v.as_f64()))
        .unwrap_or(0.0);
    let total = read_cost + eval_cost;
    let cost = format!("{:.2}..{:.2}", read_cost, total);
    let pct = pct_of(total, root_cost);
    let op = match access_type {
        "ALL" => "Full Table Scan",
        "ref" => "Ref Scan",
        "range" => "Range Scan",
        "index" => "Index Scan",
        "const" => "Const Lookup",
        "eq_ref" => "Eq Ref Lookup",
        "" => "Table",
        _ => access_type,
    };
    let warn = (access_type == "ALL" && rows > FULL_SCAN_WARN_ROWS) || pct > COST_SHARE_WARN;
    PlanNode {
        op: op.to_string(),
        target: table_name.to_string(),
        cost,
        rows,
        pct,
        warn,
        children: Vec::new(),
    }
}

fn root_query_cost(qb: &Value) -> f64 {
    qb.get("cost_info")
        .and_then(|c| c.get("query_cost"))
        .and_then(|v| v.as_str().map(parse_cost_str).or_else(|| v.as_f64()))
        .unwrap_or(0.0)
}

fn parse_cost_str(s: &str) -> f64 {
    s.parse().unwrap_or(0.0)
}

fn pct_of(value: f64, root: f64) -> u8 {
    if root <= 0.0 {
        return 0;
    }
    ((value / root) * 100.0).round().clamp(0.0, 100.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Option<PlanNode> {
        let v: Value = serde_json::from_str(s).unwrap();
        parse_mysql_explain(&v)
    }

    #[test]
    fn parse_returns_none_when_no_query_block() {
        assert!(parse("{}").is_none());
    }

    #[test]
    fn parse_minimal_single_table() {
        let json = r#"{
            "query_block": {
                "select_id": 1,
                "cost_info": { "query_cost": "1.20" },
                "table": {
                    "table_name": "users",
                    "access_type": "ALL",
                    "rows_examined_per_scan": 100,
                    "cost_info": { "read_cost": "1.0", "eval_cost": "0.2" }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.op, "Query Block");
        assert_eq!(n.target, "select_id=1");
        assert_eq!(n.children.len(), 1);
        let child = &n.children[0];
        assert_eq!(child.op, "Full Table Scan");
        assert_eq!(child.target, "users");
        assert_eq!(child.rows, 100);
    }

    #[test]
    fn parse_nested_loop_produces_multiple_children() {
        let json = r#"{
            "query_block": {
                "select_id": 1,
                "cost_info": { "query_cost": "5.0" },
                "nested_loop": [
                    { "table": {
                        "table_name": "a",
                        "access_type": "ALL",
                        "rows_examined_per_scan": 10,
                        "cost_info": { "read_cost": "2.0", "eval_cost": "0.0" }
                    }},
                    { "table": {
                        "table_name": "b",
                        "access_type": "ref",
                        "rows_examined_per_scan": 1,
                        "cost_info": { "read_cost": "0.5", "eval_cost": "0.0" }
                    }}
                ]
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.children.len(), 2);
        assert_eq!(n.children[0].op, "Full Table Scan");
        assert_eq!(n.children[1].op, "Ref Scan");
    }

    #[test]
    fn parse_full_scan_over_threshold_warns() {
        let json = format!(
            r#"{{
                "query_block": {{
                    "cost_info": {{ "query_cost": "100.0" }},
                    "table": {{
                        "table_name": "big",
                        "access_type": "ALL",
                        "rows_examined_per_scan": {},
                        "cost_info": {{ "read_cost": "1.0", "eval_cost": "0.0" }}
                    }}
                }}
            }}"#,
            FULL_SCAN_WARN_ROWS + 1,
        );
        let n = parse(&json).unwrap();
        assert!(n.children[0].warn);
    }

    #[test]
    fn parse_full_scan_below_threshold_no_warn() {
        let json = r#"{
            "query_block": {
                "cost_info": { "query_cost": "100.0" },
                "table": {
                    "table_name": "small",
                    "access_type": "ALL",
                    "rows_examined_per_scan": 100,
                    "cost_info": { "read_cost": "0.1", "eval_cost": "0.0" }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert!(!n.children[0].warn);
    }

    #[test]
    fn parse_high_cost_share_warns_non_root_node() {
        // Single table accounting for ~80% of total query cost.
        let json = r#"{
            "query_block": {
                "cost_info": { "query_cost": "10.0" },
                "table": {
                    "table_name": "x",
                    "access_type": "ref",
                    "rows_examined_per_scan": 50,
                    "cost_info": { "read_cost": "8.0", "eval_cost": "0.0" }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert!(n.children[0].warn);
    }

    #[test]
    fn parse_translates_each_documented_access_type() {
        for (access, expected_op) in [
            ("ALL", "Full Table Scan"),
            ("ref", "Ref Scan"),
            ("range", "Range Scan"),
            ("index", "Index Scan"),
            ("const", "Const Lookup"),
            ("eq_ref", "Eq Ref Lookup"),
        ] {
            let json = format!(
                r#"{{
                    "query_block": {{
                        "cost_info": {{ "query_cost": "1.0" }},
                        "table": {{
                            "table_name": "t",
                            "access_type": "{access}",
                            "rows_examined_per_scan": 1,
                            "cost_info": {{ "read_cost": "0.0", "eval_cost": "0.0" }}
                        }}
                    }}
                }}"#,
            );
            let n = parse(&json).unwrap();
            assert_eq!(n.children[0].op, expected_op, "for access_type {access}");
        }
    }

    #[test]
    fn parse_unknown_access_type_passes_through() {
        let json = r#"{
            "query_block": {
                "cost_info": { "query_cost": "1.0" },
                "table": {
                    "table_name": "t",
                    "access_type": "fulltext",
                    "rows_examined_per_scan": 1,
                    "cost_info": { "read_cost": "0.0", "eval_cost": "0.0" }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.children[0].op, "fulltext");
    }

    #[test]
    fn parse_handles_subquery_via_nested_array() {
        let json = r#"{
            "query_block": {
                "select_id": 1,
                "cost_info": { "query_cost": "5.0" },
                "nested": [
                    {
                        "query_block": {
                            "select_id": 2,
                            "cost_info": { "query_cost": "2.0" }
                        }
                    }
                ]
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.children.len(), 1);
        assert_eq!(n.children[0].op, "Query Block");
        assert_eq!(n.children[0].target, "select_id=2");
    }

    #[test]
    fn parse_root_node_no_warn_even_with_high_pct() {
        // Root is always 100% of itself; the heuristic suppresses
        // the cost-share check at the root.
        let json = r#"{
            "query_block": {
                "cost_info": { "query_cost": "1.0" },
                "table": {
                    "table_name": "t",
                    "access_type": "ref",
                    "rows_examined_per_scan": 1,
                    "cost_info": { "read_cost": "0.5", "eval_cost": "0.0" }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        assert!(!n.warn);
    }

    #[test]
    fn parse_cost_string_round_trips_two_decimals() {
        let json = r#"{
            "query_block": {
                "cost_info": { "query_cost": "1.0" },
                "table": {
                    "table_name": "t",
                    "access_type": "ref",
                    "rows_examined_per_scan": 1,
                    "cost_info": { "read_cost": "1.234", "eval_cost": "0.567" }
                }
            }
        }"#;
        let n = parse(json).unwrap();
        // read_cost..total
        // read_cost..total format: 1.234 → 1.23, 1.234+0.567 → 1.80
        assert_eq!(n.children[0].cost, "1.23..1.80");
    }

    #[test]
    fn parse_unknown_nested_loop_item_falls_back_to_unknown() {
        let json = r#"{
            "query_block": {
                "cost_info": { "query_cost": "1.0" },
                "nested_loop": [{ "wat": "lol" }]
            }
        }"#;
        let n = parse(json).unwrap();
        assert_eq!(n.children[0].op, "Unknown");
    }
}
