pub mod types;

use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use self::types::DbResponse;
use super::{BlockResult, Executor, ExecutorError};
use crate::db::connections::PoolManager;

#[derive(Debug, Deserialize)]
struct DbParams {
    connection_id: String,
    query: String,
    #[serde(default)]
    bind_values: Vec<serde_json::Value>,
    #[serde(default)]
    offset: u32,
    #[serde(default = "default_fetch_size")]
    fetch_size: u32,
    timeout_ms: Option<u64>,
    /// `explain=true` info-string token (Epic 53 Story 01). When set,
    /// the user's SQL is prefixed with the driver's `EXPLAIN ANALYZE …
    /// FORMAT JSON` form via `crate::explain::prefix_explain_sql`, the
    /// query runs once, and the JSON plan is extracted from the result
    /// into `DbResponse.plan`. Single-statement queries only;
    /// drivers without EXPLAIN support (SQLite, BigQuery, Snowflake)
    /// surface `ExplainError::Unsupported` verbatim to the consumer.
    #[serde(default)]
    explain: bool,
    /// Session-scoped host override (V11 cenário 2). In-memory on the
    /// frontend; passed per run. When set (with/without
    /// `session_port_override`) the run uses an override-keyed pool —
    /// the persisted connection record is never touched.
    #[serde(default)]
    session_host_override: Option<String>,
    /// Session-scoped port override (V11 cenário 2).
    #[serde(default)]
    session_port_override: Option<i64>,
}

fn default_fetch_size() -> u32 {
    80
}

const MAX_FETCH_SIZE: u32 = 1000;
const MAX_OFFSET: u32 = 1_000_000;

pub struct DbExecutor {
    conn_manager: Arc<PoolManager>,
}

impl DbExecutor {
    pub fn new(conn_manager: Arc<PoolManager>) -> Self {
        Self { conn_manager }
    }

    /// Cancel-aware execution returning the typed response directly.
    ///
    /// The `cancel` token is observed with `tokio::select!`. When it fires,
    /// the in-flight driver future is dropped and the caller gets a
    /// cancellation error. Note that `sqlx` does not currently propagate
    /// cancellation to the server for all drivers — for Postgres it works
    /// well, for MySQL/SQLite a running query may still run to completion
    /// on the server side while we release the pooled connection.
    pub async fn execute_with_cancel(
        &self,
        params: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<DbResponse, ExecutorError> {
        let p: DbParams = serde_json::from_value(params)
            .map_err(|e| ExecutorError(format!("Invalid params: {e}")))?;

        let ov = if p.session_host_override.is_some() || p.session_port_override.is_some() {
            Some(crate::db::connections::HostPortOverride {
                host: p.session_host_override.clone(),
                port: p.session_port_override,
            })
        } else {
            None
        };
        let pool = self
            .conn_manager
            .get_pool_with_override(&p.connection_id, ov.as_ref())
            .await
            .map_err(ExecutorError)?;

        // Resolve timeout: explicit per-query > connection default > 30s fallback
        let effective_timeout_ms = if let Some(t) = p.timeout_ms {
            t
        } else {
            self.conn_manager
                .get_query_timeout(&p.connection_id)
                .await
                .unwrap_or(30_000)
        };

        // EXPLAIN swap (Epic 53 Story 01). When `explain=true`, replace
        // the user's SQL with the driver-prefixed form so the executor
        // returns the plan instead of the regular result set. Reject
        // multi-statement up-front — `EXPLAIN ANALYZE` over a script
        // doesn't compose with the per-statement scanner.
        let working_query = if p.explain {
            let parts = crate::db::connections::split_statements(&p.query);
            if parts.len() != 1 {
                return Err(ExecutorError(
                    "EXPLAIN requires a single statement".to_string(),
                ));
            }
            crate::explain::prefix_explain_sql(pool.driver(), &p.query)
                .map_err(|e| ExecutorError(e.to_string()))?
        } else {
            p.query.clone()
        };

        // Split the query on SQL-aware `;` boundaries. Single-statement
        // queries produce a 1-element vec — same as before.
        let statements = crate::db::connections::split_statements(&working_query);
        if statements.is_empty() {
            return Err(ExecutorError("query is empty".to_string()));
        }

        // Bind values are a flat array across the whole query; slice per
        // statement by placeholder count so each statement binds its own.
        let mut bind_cursor = 0usize;
        let mut per_statement_binds: Vec<Vec<serde_json::Value>> =
            Vec::with_capacity(statements.len());
        for stmt in &statements {
            let n = crate::db::connections::count_placeholders(stmt);
            let end = bind_cursor.saturating_add(n).min(p.bind_values.len());
            per_statement_binds.push(p.bind_values[bind_cursor..end].to_vec());
            bind_cursor = end;
        }

        // Apply the timeout to the WHOLE multi-statement run; the select!
        // branch below is single-shot against the full future.
        let start = Instant::now();
        let timeout = std::time::Duration::from_millis(effective_timeout_ms);

        let pool_ref = pool.clone();
        let offset = p.offset;
        let fetch_size = p.fetch_size;
        let stmts = statements.clone();
        let binds = per_statement_binds.clone();
        let run = async move {
            let mut results: Vec<crate::executor::db::types::DbResult> = Vec::new();
            for (i, stmt) in stmts.iter().enumerate() {
                let binds_i = &binds[i];
                match pool_ref
                    .execute_query(stmt, binds_i, offset, fetch_size)
                    .await
                {
                    Ok(r) => {
                        if r.is_select {
                            let col_names: Vec<&str> =
                                r.columns.iter().map(|c| c.name.as_str()).collect();
                            let rows: Vec<serde_json::Value> = r
                                .rows
                                .iter()
                                .map(|row| {
                                    let obj: serde_json::Map<String, serde_json::Value> = col_names
                                        .iter()
                                        .zip(row.iter())
                                        .map(|(name, val)| (name.to_string(), val.clone()))
                                        .collect();
                                    serde_json::Value::Object(obj)
                                })
                                .collect();
                            results.push(crate::executor::db::types::DbResult::Select {
                                columns: r.columns,
                                rows,
                                has_more: r.has_more,
                            });
                        } else {
                            results.push(crate::executor::db::types::DbResult::Mutation {
                                rows_affected: r.rows_affected.unwrap_or(0),
                            });
                        }
                    }
                    Err(mut info) => {
                        // Error inside a statement becomes a DbResult::Error in
                        // this position; subsequent statements still run so users
                        // can see what's right even when one piece is wrong.
                        // Resolve Postgres byte-position → (line, column) now
                        // that we know which statement text applies.
                        crate::db::connections::enrich_error_with_query(&mut info, stmt);
                        results.push(crate::executor::db::types::DbResult::Error {
                            message: info.message,
                            line: info.location.line,
                            column: info.location.column,
                        });
                    }
                }
            }
            Ok::<_, String>(results)
        };
        let timed = tokio::time::timeout(timeout, run);

        let results = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                Err(ExecutorError("Query cancelled".to_string()))
            }
            res = timed => {
                match res {
                    Err(_) => Err(ExecutorError(format!(
                        "Query timed out after {effective_timeout_ms}ms"
                    ))),
                    Ok(Err(e)) => Err(ExecutorError(e)),
                    Ok(Ok(r)) => Ok(r),
                }
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // T30: Audit log — log both success and failure
        let truncated_query: String = p.query.chars().take(500).collect();
        let status = if results.is_ok() { "success" } else { "error" };
        let _ = sqlx::query(
            "INSERT INTO query_log (connection_id, query, status, duration_ms) VALUES (?, ?, ?, ?)",
        )
        .bind(&p.connection_id)
        .bind(&truncated_query)
        .bind(status)
        .bind(duration_ms as i64)
        .execute(self.conn_manager.app_pool())
        .await;

        let results = results?;

        // EXPLAIN extraction (Epic 53 Story 01). The helper short-
        // circuits when `explain=false`; when true, it pulls the JSON
        // plan from the single-row result the driver returned and
        // applies the 200 KB cap. Calling unconditionally keeps the
        // executor body uniform — the gating lives inside the helper
        // where it's reachable from unit tests on both branches.
        let plan = compute_plan(p.explain, &results);

        Ok(DbResponse {
            results,
            messages: Vec::new(),
            plan,
            stats: crate::executor::db::types::DbStats {
                elapsed_ms: duration_ms,
                rows_streamed: None,
            },
        })
    }
}

/// Top-level helper called from the executor body. Returns `None`
/// unless the user opted in via `explain=true`; otherwise delegates
/// to `extract_plan_from_results`. Wrapping the gate in its own fn
/// keeps the both branches reachable from unit tests without needing
/// a real Postgres/MySQL pool.
fn compute_plan(
    explain: bool,
    results: &[crate::executor::db::types::DbResult],
) -> Option<serde_json::Value> {
    if !explain {
        return None;
    }
    extract_plan_from_results(results)
}

/// Extract the EXPLAIN plan from a result vec. Postgres `EXPLAIN
/// (FORMAT JSON)` returns a single-row, single-column result whose
/// value is a JSON `[{"Plan": …}]` array. MySQL `EXPLAIN FORMAT=JSON`
/// returns the JSON as text in `EXPLAIN`-named column. We pull row 0,
/// take the first column value, parse it if it came back as a string,
/// and apply the 200 KB cap. When the body overflows the cap, the
/// truncated form is stored as `serde_json::Value::String` so the
/// consumer can still display a notice (and detects the truncation
/// via the type shape).
fn extract_plan_from_results(
    results: &[crate::executor::db::types::DbResult],
) -> Option<serde_json::Value> {
    use crate::executor::db::types::DbResult;
    let first = results.first()?;
    let DbResult::Select { rows, .. } = first else {
        return None;
    };
    let row_obj = rows.first()?.as_object()?;
    let raw_value = row_obj.values().next()?;

    // Normalize to a JSON-ish Value. Postgres returns the plan already
    // as a JSON array (sqlx maps the json column → Value); MySQL hands
    // it back as a JSON string.
    let parsed: serde_json::Value = match raw_value {
        serde_json::Value::String(s) => {
            serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.clone()))
        }
        other => other.clone(),
    };

    // Apply the 200 KB cap on the serialized form. When over, store the
    // truncated text as a string Value so the consumer can detect the
    // truncation by `typeof` shape.
    let serialized = parsed.to_string();
    let (capped, truncated) = crate::explain::cap_explain_body(&serialized);
    if truncated {
        Some(serde_json::Value::String(capped))
    } else {
        Some(parsed)
    }
}

#[async_trait]
impl Executor for DbExecutor {
    fn block_type(&self) -> &str {
        "db"
    }

    async fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        let p: DbParams =
            serde_json::from_value(params.clone()).map_err(|e| format!("Invalid params: {e}"))?;

        if p.connection_id.trim().is_empty() {
            return Err("connection_id is required".to_string());
        }
        if p.query.trim().is_empty() {
            return Err("query is required".to_string());
        }
        if p.fetch_size == 0 {
            return Err("fetch_size must be >= 1".to_string());
        }
        if p.fetch_size > MAX_FETCH_SIZE {
            return Err(format!("fetch_size must be <= {MAX_FETCH_SIZE}"));
        }
        if p.offset > MAX_OFFSET {
            return Err(format!("offset must be <= {MAX_OFFSET}"));
        }

        Ok(())
    }

    async fn execute(&self, params: serde_json::Value) -> Result<BlockResult, ExecutorError> {
        // Fresh token that never fires — preserves pre-stage-3 behavior for
        // callers that don't care about cancellation.
        let response = self
            .execute_with_cancel(params, CancellationToken::new())
            .await?;

        let duration_ms = response.stats.elapsed_ms;
        let data = serde_json::to_value(&response)
            .map_err(|e| ExecutorError(format!("Failed to serialize DB response: {e}")))?;

        Ok(BlockResult {
            status: "success".to_string(),
            data,
            duration_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connections::{CreateConnection, PoolManager};
    use sqlx::sqlite::SqlitePool;

    async fn setup_test_env() -> (Arc<PoolManager>, String) {
        // Create the app pool with connections table
        let app_pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            r#"CREATE TABLE connections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                driver TEXT NOT NULL CHECK (driver IN ('postgres', 'mysql', 'sqlite')),
                host TEXT, port INTEGER, database_name TEXT,
                username TEXT, password TEXT,
                ssl_mode TEXT DEFAULT 'disable',
                timeout_ms INTEGER DEFAULT 10000,
                query_timeout_ms INTEGER DEFAULT 30000,
                ttl_seconds INTEGER DEFAULT 300,
                max_pool_size INTEGER DEFAULT 5,
                is_readonly INTEGER NOT NULL DEFAULT 0,
                last_tested_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"#,
        )
        .execute(&app_pool)
        .await
        .unwrap();

        // Create a test SQLite connection pointing to in-memory
        let conn = crate::db::connections::create_connection(
            &app_pool,
            CreateConnection {
                name: "test-sqlite".to_string(),
                driver: "sqlite".to_string(),
                host: None,
                port: None,
                database_name: Some(":memory:".to_string()),
                username: None,
                password: None,
                ssl_mode: None,
                timeout_ms: None,
                query_timeout_ms: None,
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: None,
            },
        )
        .await
        .unwrap();

        let manager = Arc::new(PoolManager::new_standalone(
            crate::db::lookup::SqliteLookup::new(app_pool.clone()),
            app_pool,
        ));
        (manager, conn.id)
    }

    #[tokio::test]
    async fn test_db_executor_validate() {
        let (manager, _) = setup_test_env().await;
        let executor = DbExecutor::new(manager);

        // Missing connection_id
        let result = executor
            .validate(&serde_json::json!({
                "connection_id": "",
                "query": "SELECT 1"
            }))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("connection_id"));

        // Missing query
        let result = executor
            .validate(&serde_json::json!({
                "connection_id": "some-id",
                "query": ""
            }))
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("query"));

        // Valid
        let result = executor
            .validate(&serde_json::json!({
                "connection_id": "some-id",
                "query": "SELECT 1"
            }))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_db_executor_select() {
        let (manager, conn_id) = setup_test_env().await;

        // Create a table in the target database
        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE test (id INTEGER, name TEXT)")
                    .execute(p)
                    .await
                    .unwrap();
                sqlx::query("INSERT INTO test VALUES (1, 'alice'), (2, 'bob')")
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("Expected SQLite pool"),
        }

        let executor = DbExecutor::new(manager);
        let result = executor
            .execute(serde_json::json!({
                "connection_id": conn_id,
                "query": "SELECT * FROM test",
            }))
            .await
            .unwrap();

        assert_eq!(result.status, "success");
        let first = &result.data["results"][0];
        assert_eq!(first["kind"], "select");
        assert_eq!(first["has_more"], false);
        assert_eq!(first["rows"].as_array().unwrap().len(), 2);
        assert_eq!(first["columns"].as_array().unwrap().len(), 2);
        assert!(result.data["stats"]["elapsed_ms"].is_number());
    }

    #[tokio::test]
    async fn test_db_executor_mutation() {
        let (manager, conn_id) = setup_test_env().await;

        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE items (id INTEGER, val TEXT)")
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("Expected SQLite pool"),
        }

        let executor = DbExecutor::new(manager);
        let result = executor
            .execute(serde_json::json!({
                "connection_id": conn_id,
                "query": "INSERT INTO items VALUES (1, 'hello'), (2, 'world')",
            }))
            .await
            .unwrap();

        assert_eq!(result.status, "success");
        let first = &result.data["results"][0];
        assert_eq!(first["kind"], "mutation");
        assert_eq!(first["rows_affected"], 2);
    }

    #[tokio::test]
    async fn test_db_executor_with_bind_params() {
        let (manager, conn_id) = setup_test_env().await;

        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE users (id INTEGER, name TEXT, active INTEGER)")
                    .execute(p)
                    .await
                    .unwrap();
                sqlx::query(
                    "INSERT INTO users VALUES (1, 'alice', 1), (2, 'bob', 0), (3, 'charlie', 1)",
                )
                .execute(p)
                .await
                .unwrap();
            }
            _ => panic!("Expected SQLite pool"),
        }

        let executor = DbExecutor::new(manager);
        let result = executor
            .execute(serde_json::json!({
                "connection_id": conn_id,
                "query": "SELECT * FROM users WHERE active = ?",
                "bind_values": [1],
            }))
            .await
            .unwrap();

        assert_eq!(result.status, "success");
        let first = &result.data["results"][0];
        assert_eq!(first["kind"], "select");
        assert_eq!(first["has_more"], false);
        assert_eq!(first["rows"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_db_executor_progressive_fetch() {
        let (manager, conn_id) = setup_test_env().await;

        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE nums (n INTEGER)")
                    .execute(p)
                    .await
                    .unwrap();
                for i in 1..=15 {
                    sqlx::query("INSERT INTO nums VALUES (?)")
                        .bind(i)
                        .execute(p)
                        .await
                        .unwrap();
                }
            }
            _ => panic!("Expected SQLite pool"),
        }

        let executor = DbExecutor::new(manager);

        // First fetch: offset=0, fetch_size=5 → 5 rows, has_more=true
        let result = executor
            .execute(serde_json::json!({
                "connection_id": conn_id,
                "query": "SELECT * FROM nums",
                "offset": 0,
                "fetch_size": 5,
            }))
            .await
            .unwrap();

        assert_eq!(result.data["results"][0]["has_more"], true);
        assert_eq!(
            result.data["results"][0]["rows"].as_array().unwrap().len(),
            5
        );

        // Second fetch: offset=5, fetch_size=5 → 5 rows, has_more=true
        let result = executor
            .execute(serde_json::json!({
                "connection_id": conn_id,
                "query": "SELECT * FROM nums",
                "offset": 5,
                "fetch_size": 5,
            }))
            .await
            .unwrap();

        assert_eq!(result.data["results"][0]["has_more"], true);
        assert_eq!(
            result.data["results"][0]["rows"].as_array().unwrap().len(),
            5
        );

        // Third fetch: offset=10, fetch_size=5 → 5 rows, has_more=false
        let result = executor
            .execute(serde_json::json!({
                "connection_id": conn_id,
                "query": "SELECT * FROM nums",
                "offset": 10,
                "fetch_size": 5,
            }))
            .await
            .unwrap();

        assert_eq!(result.data["results"][0]["has_more"], false);
        assert_eq!(
            result.data["results"][0]["rows"].as_array().unwrap().len(),
            5
        );

        // Fourth fetch: offset=15, fetch_size=5 → 0 rows, has_more=false
        let result = executor
            .execute(serde_json::json!({
                "connection_id": conn_id,
                "query": "SELECT * FROM nums",
                "offset": 15,
                "fetch_size": 5,
            }))
            .await
            .unwrap();

        assert_eq!(result.data["results"][0]["has_more"], false);
        assert_eq!(
            result.data["results"][0]["rows"].as_array().unwrap().len(),
            0
        );
    }

    // ───── Stage 3: cancel-aware execution ─────

    #[tokio::test]
    async fn test_execute_with_cancel_completes_when_not_cancelled() {
        let (manager, conn_id) = setup_test_env().await;
        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE t (n INTEGER)")
                    .execute(p)
                    .await
                    .unwrap();
                sqlx::query("INSERT INTO t VALUES (1), (2)")
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("expected SQLite pool"),
        }

        let executor = DbExecutor::new(manager);
        let token = CancellationToken::new();
        let resp = executor
            .execute_with_cancel(
                serde_json::json!({
                    "connection_id": conn_id,
                    "query": "SELECT * FROM t",
                }),
                token,
            )
            .await
            .unwrap();

        // Fresh token never fires → execution completes normally.
        assert_eq!(resp.results.len(), 1);
        match &resp.results[0] {
            crate::executor::db::types::DbResult::Select { rows, .. } => {
                assert_eq!(rows.len(), 2);
            }
            other => panic!("expected Select, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_execute_with_cancel_returns_error_when_pre_cancelled() {
        let (manager, conn_id) = setup_test_env().await;
        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE t (n INTEGER)")
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("expected SQLite pool"),
        }

        let executor = DbExecutor::new(manager);
        let token = CancellationToken::new();
        // Cancel before calling — the select! branch with `biased` will
        // observe the cancelled token immediately.
        token.cancel();

        let err = executor
            .execute_with_cancel(
                serde_json::json!({
                    "connection_id": conn_id,
                    "query": "SELECT 1",
                }),
                token,
            )
            .await
            .unwrap_err();

        assert_eq!(err.0, "Query cancelled");
    }

    #[tokio::test]
    async fn test_execute_with_cancel_during_query() {
        let (manager, conn_id) = setup_test_env().await;
        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE t (n INTEGER)")
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("expected SQLite pool"),
        }

        let executor = Arc::new(DbExecutor::new(manager));
        let token = CancellationToken::new();
        let cancel_handle = token.clone();

        // Spawn the executor; fire cancel shortly after.
        let exec_fut = {
            let executor = executor.clone();
            tokio::spawn(async move {
                executor
                    .execute_with_cancel(
                        serde_json::json!({
                            "connection_id": conn_id,
                            "query": "SELECT 1",
                        }),
                        token,
                    )
                    .await
            })
        };

        // Yield then cancel. SQLite in-memory queries are so fast they
        // typically finish first, so accept either outcome — what matters
        // is that no panic and no deadlock occurs, and if cancelled the
        // error message matches.
        tokio::task::yield_now().await;
        cancel_handle.cancel();

        let result = exec_fut.await.expect("task joined");
        match result {
            Ok(resp) => assert_eq!(resp.results.len(), 1),
            Err(e) => assert_eq!(e.0, "Query cancelled"),
        }
    }

    // ───── Stage 6: multi-statement execution ─────

    #[tokio::test]
    async fn test_multi_statement_returns_multiple_results() {
        let (manager, conn_id) = setup_test_env().await;
        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE t (n INTEGER)")
                    .execute(p)
                    .await
                    .unwrap();
                sqlx::query("INSERT INTO t VALUES (1), (2), (3)")
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("expected sqlite"),
        }
        let executor = DbExecutor::new(manager);
        let resp = executor
            .execute_with_cancel(
                serde_json::json!({
                    "connection_id": conn_id,
                    "query": "SELECT count(*) AS n FROM t; INSERT INTO t VALUES (4); SELECT count(*) AS n FROM t",
                }),
                CancellationToken::new(),
            )
            .await
            .unwrap();

        assert_eq!(resp.results.len(), 3);
        // First: SELECT with 3 rows (before insert)
        match &resp.results[0] {
            crate::executor::db::types::DbResult::Select { rows, .. } => {
                assert_eq!(rows[0]["n"], 3);
            }
            other => panic!("expected Select, got {other:?}"),
        }
        // Second: INSERT mutation
        match &resp.results[1] {
            crate::executor::db::types::DbResult::Mutation { rows_affected } => {
                assert_eq!(*rows_affected, 1);
            }
            other => panic!("expected Mutation, got {other:?}"),
        }
        // Third: SELECT after insert
        match &resp.results[2] {
            crate::executor::db::types::DbResult::Select { rows, .. } => {
                assert_eq!(rows[0]["n"], 4);
            }
            other => panic!("expected Select, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_db_executor_statement_error_becomes_error_result() {
        // Exercises the per-statement `Err` arm (error enrich +
        // DbResult::Error) and the "error" audit-log branch.
        let (manager, conn_id) = setup_test_env().await;
        let executor = DbExecutor::new(manager);
        let result = executor
            .execute(serde_json::json!({
                "connection_id": conn_id,
                "query": "SELECT * FROM table_that_does_not_exist",
            }))
            .await
            .unwrap();
        // The command still succeeds; the failure is captured per
        // statement so the user sees which piece is wrong.
        let first = &result.data["results"][0];
        assert_eq!(first["kind"], "error");
        let msg = first["message"].as_str().unwrap().to_lowercase();
        assert!(
            msg.contains("no such table") || msg.contains("does_not_exist"),
            "got: {msg}"
        );
    }

    #[tokio::test]
    async fn test_db_executor_mixed_ok_and_error_statements() {
        let (manager, conn_id) = setup_test_env().await;
        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE ok (n INTEGER)")
                    .execute(p)
                    .await
                    .unwrap();
                sqlx::query("INSERT INTO ok VALUES (1)")
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("expected sqlite"),
        }
        let executor = DbExecutor::new(manager);
        let resp = executor
            .execute_with_cancel(
                serde_json::json!({
                    "connection_id": conn_id,
                    "query": "SELECT n FROM ok; SELECT * FROM missing_tbl; SELECT n FROM ok",
                }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(resp.results.len(), 3);
        // First + third OK, middle is an Error — later statements still run.
        assert!(matches!(
            resp.results[0],
            crate::executor::db::types::DbResult::Select { .. }
        ));
        assert!(matches!(
            resp.results[1],
            crate::executor::db::types::DbResult::Error { .. }
        ));
        assert!(matches!(
            resp.results[2],
            crate::executor::db::types::DbResult::Select { .. }
        ));
    }

    #[tokio::test]
    async fn test_db_executor_explicit_timeout_is_honored() {
        // `timeout_ms` short-circuits the connection-default lookup —
        // covers the `Some(t)` arm of the timeout resolution.
        let (manager, conn_id) = setup_test_env().await;
        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE t (n INTEGER)")
                    .execute(p)
                    .await
                    .unwrap();
                sqlx::query("INSERT INTO t VALUES (1)")
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("expected sqlite"),
        }
        let executor = DbExecutor::new(manager);
        let resp = executor
            .execute_with_cancel(
                serde_json::json!({
                    "connection_id": conn_id,
                    "query": "SELECT n FROM t",
                    "timeout_ms": 30000,
                }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(resp.results.len(), 1);
    }

    // ───── V11 cenário 2: session host:port override ─────

    #[tokio::test]
    async fn test_session_override_runs_against_override_keyed_pool() {
        // SQLite ignores host/port, so the query still succeeds — what
        // we're covering is the `ov = Some(..)` branch + the
        // get_pool_with_override call site in the executor.
        let (manager, conn_id) = setup_test_env().await;
        let executor = DbExecutor::new(manager);
        let resp = executor
            .execute_with_cancel(
                serde_json::json!({
                    "connection_id": conn_id,
                    "query": "SELECT 1 AS n",
                    "session_host_override": "db.staging",
                    "session_port_override": 5599,
                }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        match &resp.results[0] {
            crate::executor::db::types::DbResult::Select { rows, .. } => {
                assert_eq!(rows[0]["n"], 1);
            }
            other => panic!("expected Select, got {other:?}"),
        }
    }

    // ───── Epic 53 Story 01: explain wiring ─────

    #[tokio::test]
    async fn test_explain_on_sqlite_returns_unsupported_error() {
        // SQLite is explicitly unsupported per Epic 53 spec — the
        // prefix builder errors out and the executor surfaces the
        // driver name to the consumer.
        let (manager, conn_id) = setup_test_env().await;
        let executor = DbExecutor::new(manager);
        let err = executor
            .execute_with_cancel(
                serde_json::json!({
                    "connection_id": conn_id,
                    "query": "SELECT 1",
                    "explain": true,
                }),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            err.0.contains("sqlite"),
            "expected sqlite in unsupported message, got: {}",
            err.0
        );
        assert!(
            err.0.to_lowercase().contains("explain"),
            "expected EXPLAIN in unsupported message, got: {}",
            err.0
        );
    }

    #[tokio::test]
    async fn test_explain_rejects_multi_statement_query() {
        let (manager, conn_id) = setup_test_env().await;
        let executor = DbExecutor::new(manager);
        let err = executor
            .execute_with_cancel(
                serde_json::json!({
                    "connection_id": conn_id,
                    "query": "SELECT 1; SELECT 2",
                    "explain": true,
                }),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(
            err.0.contains("single statement"),
            "expected single-statement error, got: {}",
            err.0
        );
    }

    #[tokio::test]
    async fn test_default_run_omits_plan() {
        // Without `explain=true`, `DbResponse.plan` stays None — the
        // shape regression guard for legacy consumers.
        let (manager, conn_id) = setup_test_env().await;
        let pool = manager.get_pool(&conn_id).await.unwrap();
        match pool.as_ref() {
            crate::db::connections::DatabasePool::Sqlite(p) => {
                sqlx::query("CREATE TABLE t (n INTEGER)")
                    .execute(p)
                    .await
                    .unwrap();
                sqlx::query("INSERT INTO t VALUES (1)")
                    .execute(p)
                    .await
                    .unwrap();
            }
            _ => panic!("expected SQLite pool"),
        }
        let executor = DbExecutor::new(manager);
        let resp = executor
            .execute_with_cancel(
                serde_json::json!({
                    "connection_id": conn_id,
                    "query": "SELECT * FROM t",
                }),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(resp.plan.is_none());
    }

    // ───── Pure-helper tests for `extract_plan_from_results` ─────

    #[test]
    fn extract_plan_from_postgres_shape_returns_parsed_value() {
        // Postgres `EXPLAIN (FORMAT JSON)` returns a single row whose
        // single column is the JSON plan as a JSON value (sqlx maps the
        // json column → serde_json::Value).
        let plan_value = serde_json::json!([{
            "Plan": {
                "Node Type": "Seq Scan",
                "Total Cost": 12.5
            }
        }]);
        let row = serde_json::json!({ "QUERY PLAN": plan_value });
        let results = vec![crate::executor::db::types::DbResult::Select {
            columns: vec![crate::db::connections::ColumnInfo {
                name: "QUERY PLAN".into(),
                type_name: "json".into(),
            }],
            rows: vec![row],
            has_more: false,
        }];
        let extracted = extract_plan_from_results(&results).unwrap();
        // Round-trip: same value, untruncated, parsed (Array shape).
        assert!(extracted.is_array());
        assert_eq!(extracted[0]["Plan"]["Node Type"], "Seq Scan");
    }

    #[test]
    fn extract_plan_from_mysql_shape_parses_string() {
        // MySQL hands the JSON back as text, not a json column.
        let plan_text = r#"{"query_block": {"select_id": 1}}"#;
        let row = serde_json::json!({ "EXPLAIN": plan_text });
        let results = vec![crate::executor::db::types::DbResult::Select {
            columns: vec![crate::db::connections::ColumnInfo {
                name: "EXPLAIN".into(),
                type_name: "text".into(),
            }],
            rows: vec![row],
            has_more: false,
        }];
        let extracted = extract_plan_from_results(&results).unwrap();
        assert!(extracted.is_object());
        assert_eq!(extracted["query_block"]["select_id"], 1);
    }

    #[test]
    fn extract_plan_falls_back_to_string_when_unparseable() {
        // Defensive: if the driver hands back a string that isn't valid
        // JSON, store it as-is so the consumer can still display it.
        let row = serde_json::json!({ "QUERY PLAN": "not json at all" });
        let results = vec![crate::executor::db::types::DbResult::Select {
            columns: vec![crate::db::connections::ColumnInfo {
                name: "QUERY PLAN".into(),
                type_name: "text".into(),
            }],
            rows: vec![row],
            has_more: false,
        }];
        let extracted = extract_plan_from_results(&results).unwrap();
        assert_eq!(extracted, serde_json::json!("not json at all"));
    }

    #[test]
    fn extract_plan_caps_oversized_payload() {
        // Build a plan whose serialized form exceeds the 200 KB cap.
        // The truncated form is stored as a string Value so consumers
        // can detect the truncation by `typeof` shape.
        let huge: String = "a".repeat(crate::explain::EXPLAIN_BODY_CAP + 8_000);
        let row = serde_json::json!({ "QUERY PLAN": huge });
        let results = vec![crate::executor::db::types::DbResult::Select {
            columns: vec![crate::db::connections::ColumnInfo {
                name: "QUERY PLAN".into(),
                type_name: "text".into(),
            }],
            rows: vec![row],
            has_more: false,
        }];
        let extracted = extract_plan_from_results(&results).unwrap();
        let serde_json::Value::String(s) = extracted else {
            panic!("expected truncated string fallback");
        };
        assert!(s.len() <= crate::explain::EXPLAIN_BODY_CAP);
        assert!(s.ends_with("[explain payload truncated]"));
    }

    #[test]
    fn extract_plan_returns_none_on_empty_results() {
        let results: Vec<crate::executor::db::types::DbResult> = Vec::new();
        assert!(extract_plan_from_results(&results).is_none());
    }

    #[test]
    fn extract_plan_returns_none_on_mutation_or_error() {
        let results = vec![crate::executor::db::types::DbResult::Mutation { rows_affected: 1 }];
        assert!(extract_plan_from_results(&results).is_none());

        let results = vec![crate::executor::db::types::DbResult::Error {
            message: "boom".into(),
            line: None,
            column: None,
        }];
        assert!(extract_plan_from_results(&results).is_none());
    }

    #[test]
    fn extract_plan_returns_none_on_empty_rows() {
        let results = vec![crate::executor::db::types::DbResult::Select {
            columns: vec![crate::db::connections::ColumnInfo {
                name: "QUERY PLAN".into(),
                type_name: "json".into(),
            }],
            rows: Vec::new(),
            has_more: false,
        }];
        assert!(extract_plan_from_results(&results).is_none());
    }

    // ───── compute_plan gate ─────

    #[test]
    fn compute_plan_short_circuits_when_explain_false() {
        // Even when `results` is plan-shaped, the gate returns None
        // unless the user opted in. Guards a regular SELECT with one
        // row + one TEXT column from being misread as a plan.
        let plan_value = serde_json::json!([{ "Plan": { "Node Type": "Seq Scan" } }]);
        let row = serde_json::json!({ "QUERY PLAN": plan_value });
        let results = vec![crate::executor::db::types::DbResult::Select {
            columns: vec![crate::db::connections::ColumnInfo {
                name: "QUERY PLAN".into(),
                type_name: "json".into(),
            }],
            rows: vec![row],
            has_more: false,
        }];
        assert!(compute_plan(false, &results).is_none());
    }

    #[test]
    fn compute_plan_extracts_when_explain_true() {
        let plan_value = serde_json::json!([{ "Plan": { "Node Type": "Index Scan" } }]);
        let row = serde_json::json!({ "QUERY PLAN": plan_value });
        let results = vec![crate::executor::db::types::DbResult::Select {
            columns: vec![crate::db::connections::ColumnInfo {
                name: "QUERY PLAN".into(),
                type_name: "json".into(),
            }],
            rows: vec![row],
            has_more: false,
        }];
        let extracted = compute_plan(true, &results).unwrap();
        assert_eq!(extracted[0]["Plan"]["Node Type"], "Index Scan");
    }
}
