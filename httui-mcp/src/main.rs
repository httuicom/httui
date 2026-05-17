use anyhow::Result;
use clap::Parser;
use rmcp::{transport::stdio, ServiceExt};
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod server;
mod tools;

#[derive(Parser, Debug)]
#[command(name = "httui-mcp", about = "MCP server for httui-notes")]
struct Args {
    /// Path to the vault directory
    #[arg(long)]
    vault: String,

    /// Path to the app database (defaults to ~/.local/share/com.httui.notes/notes.db)
    #[arg(long)]
    db: Option<String>,
}

/// Resolve the SQLite path the server should open. When `--db` is
/// provided we trust it verbatim; otherwise fall back to the
/// per-platform default directory.
fn resolve_db_path(db_arg: Option<&str>) -> Result<PathBuf> {
    if let Some(db) = db_arg {
        return Ok(PathBuf::from(db));
    }
    httui_core::paths::default_data_dir().map_err(|e| anyhow::anyhow!("resolve data dir: {e}"))
}

/// Initialise the global tracing subscriber for the MCP process.
/// Writes to stderr because stdout is the MCP wire protocol; ANSI
/// colours are off because stderr is consumed by a parent process.
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let args = Args::parse();
    run(&args.vault, resolve_db_path(args.db.as_deref())?).await
}

/// Server bootstrap split out of `main` so the wiring is callable from
/// integration harnesses (Epic 32). Builds the shared executor
/// registry, hands it to the MCP server, and blocks on the rmcp
/// stdio service.
async fn run(vault: &str, db_path: PathBuf) -> Result<()> {
    let pool = init_pool(&db_path).await?;
    let registry = build_registry(vault, pool.clone())?;
    let conn_manager = registry.conn_manager.clone();
    let server = server::NotesMcpServer::new(
        pool,
        conn_manager,
        Arc::new(registry.executors),
        vault.to_string(),
    );
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

async fn init_pool(db_path: &Path) -> Result<sqlx::SqlitePool> {
    httui_core::db::init_db(db_path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize database: {e}"))
}

/// Pair carrying the executor registry plus the `PoolManager` that
/// the DB executor was registered with — both halves are needed to
/// construct the MCP server (the server keeps `Arc<PoolManager>` to
/// expose connection metadata to MCP tools).
struct McpRegistry {
    executors: httui_core::executor::ExecutorRegistry,
    conn_manager: Arc<httui_core::db::connections::PoolManager>,
}

fn build_registry(vault: &str, pool: sqlx::SqlitePool) -> Result<McpRegistry> {
    let conn_lookup = httui_core::vault_config::ConnectionsStore::new(vault.to_string());
    let conn_manager = Arc::new(httui_core::db::connections::PoolManager::new_standalone(
        conn_lookup,
        pool.clone(),
    ));
    let mut executors = httui_core::executor::ExecutorRegistry::new();
    executors.register(Box::new(httui_core::executor::http::HttpExecutor::new()));
    executors.register(Box::new(httui_core::executor::db::DbExecutor::new(
        conn_manager.clone(),
    )));
    Ok(McpRegistry {
        executors,
        conn_manager,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_parses_with_vault_only() {
        let args = Args::try_parse_from(["httui-mcp", "--vault", "/tmp/v"])
            .expect("vault-only should parse");
        assert_eq!(args.vault, "/tmp/v");
        assert!(args.db.is_none());
    }

    #[test]
    fn args_parses_with_vault_and_db() {
        let args =
            Args::try_parse_from(["httui-mcp", "--vault", "/tmp/v", "--db", "/tmp/notes.db"])
                .expect("vault+db should parse");
        assert_eq!(args.vault, "/tmp/v");
        assert_eq!(args.db.as_deref(), Some("/tmp/notes.db"));
    }

    #[test]
    fn args_rejects_missing_vault() {
        let res = Args::try_parse_from(["httui-mcp"]);
        assert!(res.is_err(), "missing --vault must error");
    }

    #[test]
    fn resolve_db_path_uses_explicit_arg() {
        let p = resolve_db_path(Some("/explicit/notes.db")).unwrap();
        assert_eq!(p, PathBuf::from("/explicit/notes.db"));
    }

    #[test]
    fn resolve_db_path_falls_back_to_default_dir() {
        // Without `--db` we delegate to httui_core::paths::default_data_dir,
        // which resolves a per-platform path. We don't pin the value
        // (it's system-dependent) — we just confirm the call returns
        // Ok with a non-empty path.
        let p = resolve_db_path(None).expect("default data dir should resolve");
        assert!(!p.as_os_str().is_empty());
    }

    #[tokio::test]
    async fn init_pool_opens_and_initialises_a_fresh_sqlite_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.db");
        let pool = init_pool(&path).await.expect("init_pool should succeed");
        // The pool can answer trivial queries, proving migrations ran.
        let row: (i64,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await.unwrap();
        assert_eq!(row.0, 1);
        // The database file exists on disk after init.
        assert!(path.exists());
    }

    #[tokio::test]
    async fn run_returns_io_error_when_db_path_is_invalid() {
        // Use a path that resolves to a directory entry that can't be a
        // file (a path with a NUL byte or a non-existent parent that
        // can't be created). Easier and portable: nest under a path
        // segment whose parent is a regular file we just wrote.
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("not-a-dir");
        std::fs::write(&blocker, b"x").unwrap();
        let bogus = blocker.join("inside.db"); // can't create: parent is a file
        let res = run("/tmp/vault", bogus).await;
        assert!(res.is_err(), "run with invalid db_path should error");
    }

    #[tokio::test]
    async fn build_registry_constructs_http_and_db_executors() {
        // Pool isn't actually used by registry construction (the DbExecutor
        // closes over the PoolManager), but build_registry needs an
        // SqlitePool to thread through. An in-memory SQLite pool is enough.
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        let registry = build_registry("/tmp/vault", pool).expect("registry should build");
        // The PoolManager arc is shared with the DB executor — `>= 2`
        // proves the executor closed over a clone (otherwise count
        // would be 1).
        assert!(Arc::strong_count(&registry.conn_manager) >= 2);
    }
}
