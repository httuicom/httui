//! Postgres `EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON)` parser.
//!
//! Postgres returns an array of one element (the top-level Plan):
//!
//! ```json
//! [{
//!   "Plan": {
//!     "Node Type": "Limit",
//!     "Startup Cost": 0.42,
//!     "Total Cost": 18.7,
//!     "Plan Rows": 50,
//!     "Plans": [
//!       { "Node Type": "Sort", ... }
//!     ]
//!   }
//! }]
//! ```
//!
//! We pull the documented fields, derive `target` from a
//! per-node-kind heuristic ("Index Cond" / "Sort Key" / relation
//! name etc.), compute `pct` as `total_cost / root_total_cost *
//! 100`, and apply heuristics for `warn`:
//!
//! - Seq Scan with > 10000 estimated rows
//! - Any node with > 50% cost share
//! - Sort with `Sort Method = "external merge"` (work_mem exceeded)

use serde_json::Value;

use super::PlanNode;

/// Sequential-scan row threshold: above this, the heuristic flags
/// the node as a warning (full table scan on a large table).
pub const SEQ_SCAN_WARN_ROWS: u64 = 10_000;

/// Cost-share threshold: any node consuming more than this fraction
/// of the query's total cost is flagged.
pub const COST_SHARE_WARN: u8 = 50;

/// Parse the JSON array Postgres returns from `EXPLAIN (FORMAT
/// JSON)`. Returns `None` if the JSON shape doesn't match what
/// Postgres documents — the consumer can fall back to "EXPLAIN
/// unavailable" copy.
pub fn parse_postgres_explain(json: &Value) -> Option<PlanNode> {
    let plans = json.as_array()?;
    let first = plans.first()?;
    let root_plan = first.get("Plan")?;
    let root_total_cost = root_plan
        .get("Total Cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    Some(parse_node(root_plan, root_total_cost, true))
}

fn parse_node(plan: &Value, root_total_cost: f64, is_root: bool) -> PlanNode {
    let op = plan
        .get("Node Type")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let total_cost = plan
        .get("Total Cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let startup_cost = plan
        .get("Startup Cost")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let rows = plan.get("Plan Rows").and_then(|v| v.as_u64()).unwrap_or(0);
    let pct = if root_total_cost > 0.0 {
        ((total_cost / root_total_cost) * 100.0).round().min(100.0) as u8
    } else {
        0
    };
    let target = derive_target(plan);
    let cost = format!("{:.2}..{:.2}", startup_cost, total_cost);
    let warn = compute_warn(&op, rows, pct, plan, is_root);
    let children = plan
        .get("Plans")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|child| parse_node(child, root_total_cost, false))
                .collect()
        })
        .unwrap_or_default();

    PlanNode {
        op,
        target,
        cost,
        rows,
        pct,
        warn,
        children,
    }
}

fn derive_target(plan: &Value) -> String {
    let candidates = [
        "Sort Key",
        "Index Cond",
        "Hash Cond",
        "Filter",
        "Index Name",
        "Relation Name",
    ];
    for key in candidates {
        if let Some(v) = plan.get(key) {
            if let Some(s) = v.as_str() {
                if !s.trim().is_empty() {
                    return s.to_string();
                }
            }
            if let Some(arr) = v.as_array() {
                let joined = arr
                    .iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                if !joined.is_empty() {
                    return joined;
                }
            }
        }
    }
    String::new()
}

fn compute_warn(op: &str, rows: u64, pct: u8, plan: &Value, is_root: bool) -> bool {
    if op == "Seq Scan" && rows > SEQ_SCAN_WARN_ROWS {
        return true;
    }
    // Suppress the cost-share heuristic on the root: a single-node
    // plan is always 100% of itself, which carries no signal.
    if !is_root && pct > COST_SHARE_WARN {
        return true;
    }
    if op.starts_with("Sort") {
        if let Some(method) = plan.get("Sort Method").and_then(|v| v.as_str()) {
            if method == "external merge" || method.contains("external") {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Option<PlanNode> {
        let v: Value = serde_json::from_str(s).unwrap();
        parse_postgres_explain(&v)
    }

    #[test]
    fn parse_returns_none_for_empty_array() {
        assert!(parse("[]").is_none());
    }

    #[test]
    fn parse_returns_none_when_no_plan_key() {
        assert!(parse("[{}]").is_none());
    }

    #[test]
    fn parse_returns_none_for_non_array_root() {
        assert!(parse("{}").is_none());
    }

    #[test]
    fn parse_minimal_single_node() {
        let json = r#"[{"Plan":{"Node Type":"Seq Scan","Total Cost":10.0,"Plan Rows":100}}]"#;
        let n = parse(json).unwrap();
        assert_eq!(n.op, "Seq Scan");
        assert_eq!(n.rows, 100);
        assert_eq!(n.pct, 100); // root is always 100% of itself
        assert!(!n.warn); // 100 rows is below threshold
    }

    #[test]
    fn parse_handles_missing_node_type_gracefully() {
        let json = r#"[{"Plan":{"Total Cost":1.0}}]"#;
        let n = parse(json).unwrap();
        assert_eq!(n.op, "Unknown");
    }

    #[test]
    fn parse_extracts_children_from_plans_array() {
        let json = r#"[{"Plan":{
            "Node Type":"Limit",
            "Total Cost":20.0,
            "Plan Rows":50,
            "Plans":[
                {"Node Type":"Sort","Total Cost":18.0,"Plan Rows":1000},
                {"Node Type":"Hash","Total Cost":2.0,"Plan Rows":10}
            ]
        }}]"#;
        let n = parse(json).unwrap();
        assert_eq!(n.op, "Limit");
        assert_eq!(n.children.len(), 2);
        assert_eq!(n.children[0].op, "Sort");
        assert_eq!(n.children[1].op, "Hash");
        // Child cost share is computed against the root, not the
        // parent — Sort is 18/20 = 90%.
        assert_eq!(n.children[0].pct, 90);
    }

    #[test]
    fn parse_flags_seq_scan_over_threshold_as_warn() {
        let json = format!(
            r#"[{{"Plan":{{"Node Type":"Seq Scan","Total Cost":1.0,"Plan Rows":{}}}}}]"#,
            SEQ_SCAN_WARN_ROWS + 1,
        );
        let n = parse(&json).unwrap();
        assert!(n.warn);
    }

    #[test]
    fn parse_does_not_flag_seq_scan_below_threshold() {
        let json = r#"[{"Plan":{"Node Type":"Seq Scan","Total Cost":1.0,"Plan Rows":100}}]"#;
        let n = parse(json).unwrap();
        // The root suppresses the cost-share heuristic so a single
        // small Seq Scan plan is clean.
        assert!(!n.warn);
        // Direct check: at child level, 0% pct + 100 rows => no warn.
        assert!(!super::compute_warn(
            "Seq Scan",
            100,
            0,
            &Value::Null,
            false
        ));
    }

    #[test]
    fn parse_flags_high_cost_share_as_warn() {
        // Child consumes 60% of total cost.
        let json = r#"[{"Plan":{
            "Node Type":"Limit",
            "Total Cost":100.0,
            "Plan Rows":10,
            "Plans":[
                {"Node Type":"Hash Anti Join","Total Cost":60.0,"Plan Rows":50},
                {"Node Type":"Seq Scan","Total Cost":40.0,"Plan Rows":5}
            ]
        }}]"#;
        let n = parse(json).unwrap();
        assert!(!n.children[1].warn); // 40% cost share, no other heuristic
        assert!(n.children[0].warn); // 60% cost share — over threshold
    }

    #[test]
    fn parse_flags_external_merge_sort_as_warn() {
        let json = r#"[{"Plan":{
            "Node Type":"Sort",
            "Total Cost":1.0,
            "Plan Rows":100,
            "Sort Method":"external merge"
        }}]"#;
        let n = parse(json).unwrap();
        assert!(n.warn);
    }

    #[test]
    fn target_prefers_sort_key_over_relation() {
        let json = r#"[{"Plan":{
            "Node Type":"Sort",
            "Total Cost":1.0,
            "Plan Rows":1,
            "Sort Key":["r.created_at DESC"],
            "Relation Name":"routes"
        }}]"#;
        let n = parse(json).unwrap();
        assert_eq!(n.target, "r.created_at DESC");
    }

    #[test]
    fn target_falls_back_to_relation_name() {
        let json = r#"[{"Plan":{
            "Node Type":"Seq Scan",
            "Total Cost":1.0,
            "Plan Rows":1,
            "Relation Name":"routes"
        }}]"#;
        let n = parse(json).unwrap();
        assert_eq!(n.target, "routes");
    }

    #[test]
    fn target_handles_array_sort_keys_with_join() {
        let json = r#"[{"Plan":{
            "Node Type":"Sort",
            "Total Cost":1.0,
            "Plan Rows":1,
            "Sort Key":["a", "b"]
        }}]"#;
        let n = parse(json).unwrap();
        assert_eq!(n.target, "a, b");
    }

    #[test]
    fn cost_string_is_two_decimal_places() {
        let json = r#"[{"Plan":{"Node Type":"Limit","Startup Cost":0.4,"Total Cost":18.7,"Plan Rows":1}}]"#;
        let n = parse(json).unwrap();
        assert_eq!(n.cost, "0.40..18.70");
    }

    #[test]
    fn deeply_nested_tree_keeps_depth_and_count() {
        let json = r#"[{"Plan":{
            "Node Type":"a","Total Cost":1.0,"Plan Rows":1,
            "Plans":[{
                "Node Type":"b","Total Cost":1.0,"Plan Rows":1,
                "Plans":[{
                    "Node Type":"c","Total Cost":1.0,"Plan Rows":1,
                    "Plans":[{
                        "Node Type":"d","Total Cost":1.0,"Plan Rows":1,
                        "Plans":[{
                            "Node Type":"e","Total Cost":1.0,"Plan Rows":1,
                            "Plans":[{"Node Type":"f","Total Cost":1.0,"Plan Rows":1}]
                        }]
                    }]
                }]
            }]
        }}]"#;
        let n = parse(json).unwrap();
        assert_eq!(n.depth(), 6);
        assert_eq!(n.count(), 6);
    }
}
