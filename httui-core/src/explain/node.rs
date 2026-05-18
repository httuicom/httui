//! Driver-agnostic plan-node shape.
//!
//! The fields are tuned for the canvas Workbench mock — `op`
//! (e.g. "Limit"), `target` ("(rows=50)" / "r.created_at DESC"),
//! `cost` ("0.42..18.7"), `rows`, `pct` (share of total cost
//! 0..100), `warn` (heuristic flag), `children`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlanNode {
    /// Operation kind — "Limit", "Sort", "Hash Anti Join", "Seq Scan", "Hash", …
    pub op: String,
    /// Free-text target line shown next to the op label. Driver
    /// dependent; for Postgres this is e.g. `"on routes r"` or
    /// `"r.created_at DESC"`.
    pub target: String,
    /// Cost range as the driver reports it ("0.42..18.7"). Stored
    /// as a string so we don't lose driver-specific formatting.
    pub cost: String,
    /// Estimated/actual row count produced by this node.
    pub rows: u64,
    /// Share of total query cost, 0..100. Computed by the parser
    /// against the root node's total cost.
    pub pct: u8,
    /// Heuristic warn flag — Seq Scan over a large table, hash
    /// anti-join with majority cost share, etc. The parser owns
    /// the heuristics; the UI just colors the bar.
    pub warn: bool,
    pub children: Vec<PlanNode>,
}

impl PlanNode {
    pub fn leaf(op: impl Into<String>) -> Self {
        Self {
            op: op.into(),
            target: String::new(),
            cost: String::new(),
            rows: 0,
            pct: 0,
            warn: false,
            children: Vec::new(),
        }
    }

    /// Recursive total node count (1 + sum of children).
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(|c| c.count()).sum::<usize>()
    }

    /// Maximum depth (root = 1). Useful for the "tree handles 6+
    /// levels without horizontal scroll issues" acceptance test.
    pub fn depth(&self) -> usize {
        1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
    }
}
