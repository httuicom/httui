//! Unified `PlanNode` tree for `EXPLAIN ANALYZE` output across
//! drivers. ships the Postgres parser first
//! (most complete; canvas mock targets it). MySQL and MongoDB
//! parsers carry to follow-up slices.
//!
//! The tree is the surface the React `<ExplainPlan>` component
//! consumes; the per-driver parsers are responsible for translating
//! each driver's JSON shape into the same `PlanNode` shape so the
//! UI doesn't fan out per backend.

pub mod mongo;
pub mod mysql;
pub mod node;
pub mod postgres;
pub mod prefix;

pub use mongo::parse_mongo_explain;
pub use mysql::parse_mysql_explain;
pub use node::PlanNode;
pub use postgres::parse_postgres_explain;
pub use prefix::{cap_explain_body, prefix_explain_sql, ExplainError, EXPLAIN_BODY_CAP};
