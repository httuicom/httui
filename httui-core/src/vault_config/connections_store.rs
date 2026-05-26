//! File-backed connections store.
//!
//! Source of truth is `<vault_root>/connections.toml`. This module owns
//! reading, validating, mutating, and atomically writing it; secret
//! values flow through the OS keychain (ADR 0002), never to disk.
//!
//! The pool manager (`db::connections::PoolManager`) keeps using the
//! legacy `db::connections::Connection` struct; this module ships a
//! converter so the rest of the runtime is unchanged.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use tokio::sync::RwLock;

use std::str::FromStr;

use crate::db::connections::Connection as LegacyConnection;
use crate::db::driver::DbDriver;
use crate::db::keychain::delete_secret;

use super::atomic::{read_toml, write_toml};
use super::connection_views::{
    carry_password_ref, database_name_of, description_of, driver_string_for, existing_readonly,
    host_of, port_of, ssl_mode_of, to_legacy, to_public, username_of, ConnectionPublic,
};
use super::connections::{
    CommonFields, Connection, ConnectionsFile, MysqlConfig, PostgresConfig, SqliteConfig,
};
use super::layout::CONNECTIONS_FILE;
use super::secret_resolver::ensure_keychain_ref;
use super::validate::validate_connections_file;
use super::Version;

#[derive(Debug, Clone)]
pub struct CreateConnectionInput {
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    /// Raw password from the UI. Always stored in the keychain; the
    /// TOML file holds only a `{{keychain:...}}` reference.
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub is_readonly: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateConnectionInput {
    pub driver: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    /// `Some(empty)` clears; `Some(value)` rewrites; `None` keeps.
    pub password: Option<String>,
    pub ssl_mode: Option<String>,
    pub is_readonly: Option<bool>,
    pub description: Option<String>,
}

/// Cached parse + the on-disk mtimes (base and `.local` override)
/// that produced it. Per ADR 0004, either side changing invalidates.
#[derive(Debug, Clone)]
struct Cached {
    base_mtime: Option<SystemTime>,
    local_mtime: Option<SystemTime>,
    file: ConnectionsFile,
}

/// File-backed CRUD over `connections.toml`.
pub struct ConnectionsStore {
    vault_root: PathBuf,
    cache: RwLock<Option<Cached>>,
}

impl ConnectionsStore {
    pub fn new(vault_root: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            vault_root: vault_root.into(),
            cache: RwLock::new(None),
        })
    }

    fn path(&self) -> PathBuf {
        self.vault_root.join(CONNECTIONS_FILE)
    }

    fn local_path(&self) -> PathBuf {
        super::merge::local_override_path(&self.path()).unwrap_or_else(|| self.path())
    }

    /// Returns a parsed `ConnectionsFile` with `connections.local.toml`
    /// merged in (ADR 0004). Cache hits when both base and override
    /// mtimes are unchanged.
    async fn load(&self) -> Result<ConnectionsFile, String> {
        let path = self.path();
        let local = self.local_path();
        let base_mtime = super::merge::mtime_or_none(&path);
        let local_mtime = super::merge::mtime_or_none(&local);

        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.base_mtime == base_mtime && cached.local_mtime == local_mtime {
                    return Ok(cached.file.clone());
                }
            }
        }

        let file = if path.exists() || local.exists() {
            let (parsed, _local) = super::merge::load_with_local::<ConnectionsFile>(&path)?;
            parsed
        } else {
            ConnectionsFile {
                version: Version::V1,
                connections: BTreeMap::new(),
            }
        };

        let mut cache = self.cache.write().await;
        *cache = Some(Cached {
            base_mtime,
            local_mtime,
            file: file.clone(),
        });
        Ok(file)
    }

    /// Validates and persists `file`. Refuses to write when the
    /// validator returns hard errors.
    async fn persist(&self, file: ConnectionsFile) -> Result<(), String> {
        let report = validate_connections_file(&file);
        if report.has_errors() {
            let summary = report
                .issues
                .iter()
                .filter(|i| i.severity == super::validate::Severity::Error)
                .map(|i| format!("- {i}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(format!(
                "connections.toml refuses to save: validator found errors:\n{summary}"
            ));
        }
        let path = self.path();
        // ADR 0004: writes always target the base file.
        write_toml(&path, &file).map_err(|e| format!("write connections.toml: {e}"))?;

        // Invalidate; next `load()` re-reads + re-merges from disk.
        // We can't repopulate inline because the cached entry must be
        // the *merged* view, and `file` here is base-only.
        let mut cache = self.cache.write().await;
        *cache = None;
        Ok(())
    }

    /// Force the next read to hit disk after external file changes.
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }

    /// Read **just** the base file (no `.local` merge). Mutating
    /// paths use this so writes don't promote local overrides into
    /// the committed base.
    async fn load_base_only(&self) -> Result<ConnectionsFile, String> {
        let path = self.path();
        if !path.exists() {
            return Ok(ConnectionsFile {
                version: Version::V1,
                connections: BTreeMap::new(),
            });
        }
        read_toml::<ConnectionsFile>(&path).map_err(|e| format!("read connections.toml: {e}"))
    }

    pub async fn list_public(&self) -> Result<Vec<ConnectionPublic>, String> {
        let file = self.load().await?;
        Ok(file
            .connections
            .iter()
            .map(|(name, c)| to_public(name, c))
            .collect())
    }

    pub async fn get(&self, name: &str) -> Result<Option<ConnectionPublic>, String> {
        let file = self.load().await?;
        Ok(file.connections.get(name).map(|c| to_public(name, c)))
    }

    /// Returns the legacy `db::connections::Connection` shape that the
    /// pool manager already understands. Resolves password references
    /// against the keychain so the returned struct is usable for actual
    /// DB connection.
    pub async fn get_legacy(&self, name: &str) -> Result<Option<LegacyConnection>, String> {
        let file = self.load().await?;
        let Some(conn) = file.connections.get(name) else {
            return Ok(None);
        };
        Ok(Some(to_legacy(name, conn)?))
    }

    pub async fn create(&self, input: CreateConnectionInput) -> Result<ConnectionPublic, String> {
        // Existence check uses the merged view so "create x" fails
        // when x already exists in either base or local.
        let merged = self.load().await?;
        if merged.connections.contains_key(&input.name) {
            return Err(format!("connection '{}' already exists", input.name));
        }

        let conn = build_connection_from_input(
            &input.name,
            &input.driver,
            input.host.as_deref(),
            input.port,
            input.database_name.as_deref(),
            input.username.as_deref(),
            input.password.as_deref(),
            input.ssl_mode.as_deref(),
            input.is_readonly.unwrap_or(false),
            input.description.as_deref(),
        )?;

        let mut base = self.load_base_only().await?;
        base.connections.insert(input.name.clone(), conn.clone());
        self.persist(base).await?;
        Ok(to_public(&input.name, &conn))
    }

    pub async fn update(
        &self,
        name: &str,
        input: UpdateConnectionInput,
    ) -> Result<ConnectionPublic, String> {
        let mut file = self.load_base_only().await?;
        let existing = file
            .connections
            .get(name)
            .cloned()
            .ok_or_else(|| format!("connection '{name}' not found"))?;

        let driver_now = driver_string_for(&existing);
        let driver = input.driver.as_deref().unwrap_or(driver_now);
        let host = input.host.or_else(|| host_of(&existing));
        let port = input.port.or_else(|| port_of(&existing));
        let database_name = input.database_name.or_else(|| database_name_of(&existing));
        let username = input.username.or_else(|| username_of(&existing));
        let ssl_mode = input.ssl_mode.or_else(|| ssl_mode_of(&existing));
        let is_readonly = input.is_readonly.unwrap_or(existing_readonly(&existing));
        let description = input.description.or_else(|| description_of(&existing));

        // Password handling:
        //   None        → keep existing
        //   Some("")    → clear (delete keychain entry, drop ref)
        //   Some(raw)   → write fresh keychain entry, write fresh ref
        let password_to_pass = match input.password {
            None => carry_password_ref(&existing),
            Some(empty) if empty.is_empty() => {
                let _ = delete_secret(&conn_password_keychain_key(name));
                None
            }
            Some(new_pw) => Some(new_pw),
        };

        let conn = build_connection_from_input(
            name,
            driver,
            host.as_deref(),
            port,
            database_name.as_deref(),
            username.as_deref(),
            password_to_pass.as_deref(),
            ssl_mode.as_deref(),
            is_readonly,
            description.as_deref(),
        )?;

        file.connections.insert(name.to_string(), conn.clone());
        self.persist(file).await?;
        Ok(to_public(name, &conn))
    }

    pub async fn delete(&self, name: &str) -> Result<(), String> {
        let mut file = self.load_base_only().await?;
        if file.connections.remove(name).is_none() {
            return Err(format!("connection '{name}' not found"));
        };
        self.persist(file).await?;
        let _ = delete_secret(&conn_password_keychain_key(name));
        Ok(())
    }
}

/// Build a `Connection` from input. If `password` is provided AND it
/// isn't already a `{{...}}` reference, it gets stored in the keychain
/// and the variant ends up with the matching reference string.
#[allow(clippy::too_many_arguments)]
fn build_connection_from_input(
    name: &str,
    driver: &str,
    host: Option<&str>,
    port: Option<u16>,
    database_name: Option<&str>,
    username: Option<&str>,
    password: Option<&str>,
    ssl_mode: Option<&str>,
    is_readonly: bool,
    description: Option<&str>,
) -> Result<Connection, String> {
    let common = CommonFields {
        description: description.map(String::from),
        read_only: is_readonly,
    };

    // SQL drivers go through the DbDriver enum; non-DB Connection
    // variants (Mongo, Http, …) and unknown values fall through to
    // the reject branches below.
    match DbDriver::from_str(driver) {
        Ok(DbDriver::Postgres) => {
            let host = require(host, "host", driver)?;
            let database = require(database_name, "database_name", driver)?;
            let user = require(username, "username", driver)?;
            let password_ref = ensure_password_ref(name, password)?;
            Ok(Connection::Postgres(PostgresConfig {
                host: host.to_string(),
                port: port.unwrap_or(5432),
                database: database.to_string(),
                user: user.to_string(),
                password: password_ref,
                ssl_mode: ssl_mode.map(String::from),
                common,
            }))
        }
        Ok(DbDriver::Mysql) => {
            let host = require(host, "host", driver)?;
            let database = require(database_name, "database_name", driver)?;
            let user = require(username, "username", driver)?;
            let password_ref = ensure_password_ref(name, password)?;
            Ok(Connection::Mysql(MysqlConfig {
                host: host.to_string(),
                port: port.unwrap_or(3306),
                database: database.to_string(),
                user: user.to_string(),
                password: password_ref,
                common,
            }))
        }
        Ok(DbDriver::Sqlite) => {
            let path = require(database_name, "database_name (sqlite path)", driver)?;
            Ok(Connection::Sqlite(SqliteConfig {
                path: path.to_string(),
                common,
            }))
        }
        Err(_) => match driver {
            // Variants below are reachable from a hand-edited TOML but
            // the CRUD UI in v1 only creates the three DB types above.
            // Reject create/update for the others until the UI catches up.
            "mongo" | "http" | "ws" | "grpc" | "graphql" | "bigquery" | "shell" => {
                Err(reject_unimplemented(driver))
            }
            other => Err(format!("unsupported driver: {other}")),
        },
    }
}

/// Convert `password` input to a `{{keychain:...}}` reference.
///
/// Routes through [`secret_resolver::ensure_keychain_ref`] so this
/// store and `EnvironmentsStore` share the same idempotent flow:
/// existing references pass through verbatim, raw values land in the
/// keychain and come back as the canonical reference.
fn ensure_password_ref(name: &str, password: Option<&str>) -> Result<String, String> {
    let Some(raw) = password else {
        return Ok(String::new());
    };
    if raw.is_empty() {
        return Ok(String::new());
    }
    let key = conn_password_keychain_key(name);
    ensure_keychain_ref(&key, raw)
}

/// Keychain key for a connection's password. Mirrors the structure of
/// the reference syntax, joined with `:`.
pub fn conn_password_keychain_key(name: &str) -> String {
    format!("conn:{name}:password")
}

fn require<'a>(value: Option<&'a str>, field: &str, driver: &str) -> Result<&'a str, String> {
    match value {
        Some(v) if !v.is_empty() => Ok(v),
        _ => Err(format!("{field} is required for {driver}")),
    }
}

fn reject_unimplemented(driver: &str) -> String {
    format!(
        "creating/updating `{driver}` connections from the app UI is not supported yet; \
         hand-edit connections.toml if you need this variant"
    )
}

#[cfg(test)]
// Tests serialize keychain access via KEYCHAIN_TEST_LOCK; the std Mutex
// guard is intentionally held across awaits to keep concurrent test
// runs deterministic. The lock is contention-free in practice (each
// test holds it for milliseconds).
mod tests {
    // Inner attribute: clippy 1.95 stopped propagating the outer-attribute
    // form to bodies of `#[tokio::test]` functions, so move the allow
    // inside the module so it applies to every test function below.
    #![allow(clippy::await_holding_lock)]

    use super::*;
    use crate::db::keychain::KEYCHAIN_TEST_LOCK;
    use tempfile::TempDir;

    fn fresh_store() -> (Arc<ConnectionsStore>, TempDir) {
        let tmp = TempDir::new().unwrap();
        let store = ConnectionsStore::new(tmp.path());
        (store, tmp)
    }

    fn unique_name(prefix: &str) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        format!(
            "{prefix}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        )
    }

    #[tokio::test]
    async fn list_on_empty_vault_returns_empty() {
        let (store, _t) = fresh_store();
        assert!(store.list_public().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn create_postgres_writes_file_and_keychain() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, t) = fresh_store();
        let name = unique_name("pg-create");
        let pub_ = store
            .create(CreateConnectionInput {
                name: name.clone(),
                driver: "postgres".into(),
                host: Some("localhost".into()),
                port: Some(5432),
                database_name: Some("test".into()),
                username: Some("u".into()),
                password: Some("hunter2".into()),
                ssl_mode: Some("require".into()),
                is_readonly: Some(false),
                description: None,
            })
            .await
            .expect("create");
        assert_eq!(pub_.name, name);
        assert_eq!(pub_.driver, "postgres");
        assert!(pub_.has_password);

        // File on disk has only a reference, never the raw password.
        let raw = std::fs::read_to_string(t.path().join("connections.toml")).unwrap();
        assert!(!raw.contains("hunter2"));
        assert!(raw.contains(&format!("{{{{keychain:conn:{name}:password}}}}")));

        // Cleanup keychain entry to keep the test environment clean.
        let _ = delete_secret(&conn_password_keychain_key(&name));
    }

    #[tokio::test]
    async fn create_rejects_duplicate_name() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let name = unique_name("dup");

        let mk = |n: String| CreateConnectionInput {
            name: n,
            driver: "sqlite".into(),
            host: None,
            port: None,
            database_name: Some("/tmp/x.sqlite".into()),
            username: None,
            password: None,
            ssl_mode: None,
            is_readonly: None,
            description: None,
        };
        store.create(mk(name.clone())).await.unwrap();
        let err = store.create(mk(name)).await.unwrap_err();
        assert!(err.contains("already exists"));
    }

    #[tokio::test]
    async fn update_changes_only_provided_fields() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let name = unique_name("up");

        store
            .create(CreateConnectionInput {
                name: name.clone(),
                driver: "postgres".into(),
                host: Some("h1".into()),
                port: Some(5432),
                database_name: Some("d".into()),
                username: Some("u".into()),
                password: Some("pw".into()),
                ssl_mode: None,
                is_readonly: Some(false),
                description: None,
            })
            .await
            .unwrap();

        // Update only host
        let updated = store
            .update(
                &name,
                UpdateConnectionInput {
                    host: Some("h2".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.host.as_deref(), Some("h2"));
        assert!(updated.has_password); // password preserved

        let _ = delete_secret(&conn_password_keychain_key(&name));
    }

    #[tokio::test]
    async fn update_password_some_empty_clears_it() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, t) = fresh_store();
        let name = unique_name("clear");

        store
            .create(CreateConnectionInput {
                name: name.clone(),
                driver: "postgres".into(),
                host: Some("h".into()),
                port: Some(5432),
                database_name: Some("d".into()),
                username: Some("u".into()),
                password: Some("pw".into()),
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap();

        store
            .update(
                &name,
                UpdateConnectionInput {
                    password: Some(String::new()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let pub_ = store.get(&name).await.unwrap().unwrap();
        assert!(!pub_.has_password);

        let raw = std::fs::read_to_string(t.path().join("connections.toml")).unwrap();
        assert!(!raw.contains("pw"));
    }

    #[tokio::test]
    async fn delete_removes_entry() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let name = unique_name("del");

        store
            .create(CreateConnectionInput {
                name: name.clone(),
                driver: "sqlite".into(),
                host: None,
                port: None,
                database_name: Some("/tmp/d.sqlite".into()),
                username: None,
                password: None,
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap();
        store.delete(&name).await.unwrap();
        assert!(store.get(&name).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn cache_hits_when_mtime_unchanged() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let name = unique_name("cache");

        store
            .create(CreateConnectionInput {
                name: name.clone(),
                driver: "sqlite".into(),
                host: None,
                port: None,
                database_name: Some("/tmp/c.sqlite".into()),
                username: None,
                password: None,
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap();

        // List twice; second should not re-parse (we can't directly observe
        // that, so just sanity-check the result is consistent).
        let a = store.list_public().await.unwrap();
        let b = store.list_public().await.unwrap();
        assert_eq!(a.len(), b.len());
    }

    #[tokio::test]
    async fn unsupported_driver_for_create_errors() {
        let (store, _t) = fresh_store();
        let err = store
            .create(CreateConnectionInput {
                name: "x".into(),
                driver: "unknown".into(),
                host: None,
                port: None,
                database_name: None,
                username: None,
                password: None,
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap_err();
        assert!(err.contains("unsupported driver"));
    }

    #[tokio::test]
    async fn local_override_merges_into_connection() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (store, t) = fresh_store();
        store
            .create(CreateConnectionInput {
                name: "pg".into(),
                driver: "postgres".into(),
                host: Some("pg.staging.acme.local".into()),
                port: Some(5432),
                database_name: Some("payments".into()),
                username: Some("app".into()),
                password: Some("secret".into()),
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap();

        // Drop a local override pointing at an SSH tunnel.
        let local = t.path().join("connections.local.toml");
        std::fs::write(
            &local,
            "version = \"1\"\n[connections.pg]\ntype = \"postgres\"\nhost = \"127.0.0.1\"\nport = 15432\ndatabase = \"payments\"\nuser = \"app\"\npassword = \"\"\n",
        )
        .unwrap();
        store.invalidate_cache().await;

        let conn = store.get("pg").await.unwrap().unwrap();
        assert_eq!(conn.host.as_deref(), Some("127.0.0.1"));
        assert_eq!(conn.port, Some(15432));
        // `database_name` survives from the base when not overridden.
        assert_eq!(conn.database_name.as_deref(), Some("payments"));
    }

    #[tokio::test]
    async fn writes_target_base_not_local() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (store, t) = fresh_store();
        // Pre-create the base via API.
        store
            .create(CreateConnectionInput {
                name: "pg".into(),
                driver: "postgres".into(),
                host: Some("base-host".into()),
                port: Some(5432),
                database_name: Some("x".into()),
                username: Some("u".into()),
                password: Some(String::new()),
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap();

        // User drops a local override pointing at a tunnel.
        let local = t.path().join("connections.local.toml");
        std::fs::write(
            &local,
            "version = \"1\"\n[connections.pg]\nhost = \"local-host\"\nport = 15432\n",
        )
        .unwrap();
        store.invalidate_cache().await;

        // Update via the API. ADR 0004: writes hit the base file
        // regardless of overrides.
        store
            .update(
                "pg",
                UpdateConnectionInput {
                    host: Some("base-updated".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let base_text = std::fs::read_to_string(t.path().join("connections.toml")).unwrap();
        assert!(base_text.contains("base-updated"), "base: {base_text}");
        // Local file is unchanged.
        let local_text = std::fs::read_to_string(&local).unwrap();
        assert!(local_text.contains("local-host"));
        assert!(!local_text.contains("base-updated"));

        // The merged read still shows the local override on host.
        let merged = store.get("pg").await.unwrap().unwrap();
        assert_eq!(merged.host.as_deref(), Some("local-host"));
        assert_eq!(merged.port, Some(15432));
    }
}
