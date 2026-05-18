//! One-shot migration from MVP SQLite-backed storage to the v1 file
//! layout (ADR 0001). Reads from a live `notes.db` pool, writes
//! `connections.toml`, `envs/<name>.toml` and the `[ui]` section of
//! `user.toml`, and optionally backs up the database first.
//!
//! Scope: connections + environments + variables + the seven
//! `app_config` UI prefs (`theme`, `auto_save_ms`,
//! `editor_font_size`, `default_fetch_size`, `history_retention`,
//! `vim_enabled`, `sidebar_open`). Session-state keys (`vaults`,
//! `active_vault`, `pane_layout`, `active_pane_id`, `active_file`,
//! `scroll_positions`) **stay in SQLite** per audit-001.
//!
//! The migration is **idempotent**: rerunning on an already-populated
//! vault is safe — duplicate-name failures from the underlying stores
//! are folded into a "skipped" counter rather than aborting. Re-
//! running prefs migration overwrites the `[ui]` section with the
//! latest SQLite values; `user.toml` is per-machine, this is the
//! correct behaviour for first-run-after-schema-bump.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;

use super::connections_store::CreateConnectionInput;
use super::environments_store::SetVarInput;
use super::layout::WORKSPACE_DIR;
use super::user::UiPrefs;
use super::user_store::UserStore;
use super::{ConnectionsStore, EnvironmentsStore};
use crate::config;
use crate::db::{connections, environments};

/// Legacy MVP database filename. Detected at the vault root by
/// [`detect_migration_candidate`]; not part of the v1 layout
/// contract (which is why this constant lives in the migration
/// module rather than `layout.rs`).
pub const LEGACY_DB_FILE: &str = "notes.db";

/// What the empty-state banner needs to know about a vault to decide
/// whether to surface the MVP-to-v1 migration prompt.
///
/// `should_prompt()` is `true` iff a legacy `notes.db` is present
/// and the v1 `.httui/` layout has *not* been initialised. The
/// frontend gates the banner on this AND the
/// `mvp_migration_dismissed` user pref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationCandidate {
    pub has_legacy_db: bool,
    pub has_v1_layout: bool,
}

impl MigrationCandidate {
    pub fn should_prompt(&self) -> bool {
        self.has_legacy_db && !self.has_v1_layout
    }
}

/// Inspect `vault_path` and report what's there. Pure aside from the
/// two filesystem stat calls; no side effects.
pub fn detect_migration_candidate(vault_path: &Path) -> MigrationCandidate {
    MigrationCandidate {
        has_legacy_db: vault_path.join(LEGACY_DB_FILE).is_file(),
        has_v1_layout: vault_path.join(WORKSPACE_DIR).is_dir(),
    }
}

/// Per-call options. Build via [`MigrationOptions::default`] and
/// override what you need.
#[derive(Debug, Clone)]
pub struct MigrationOptions {
    /// Don't write anything; only walk the SQLite tables and report
    /// what would be migrated.
    pub dry_run: bool,
    /// Copy `notes.db` to `notes.db.pre-v1-backup` before any write.
    /// No-op when the file is missing or `dry_run` is true.
    pub backup: bool,
    /// Path of the per-machine `user.toml`. Currently only used to
    /// satisfy `EnvironmentsStore::new`; the prefs migration that
    /// would actually touch this file is deferred to.
    pub user_config_path: PathBuf,
}

impl MigrationOptions {
    pub fn new(user_config_path: PathBuf) -> Self {
        Self {
            dry_run: false,
            backup: true,
            user_config_path,
        }
    }
}

/// Summary returned from [`run_migration`]. Counts reflect what was
/// actually written (or what would be, on a dry run).
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct MigrationReport {
    pub vault_path: String,
    pub backup_path: Option<String>,
    pub connections_migrated: usize,
    pub connections_skipped: usize,
    pub environments_migrated: usize,
    pub environments_skipped: usize,
    pub variables_migrated: usize,
    pub variables_skipped: usize,
    /// Number of `app_config` prefs keys that landed in
    /// `user.toml [ui]`. Each found+parsed key counts once.
    pub prefs_migrated: usize,
    pub dry_run: bool,
    /// Free-form notes about deferred work / dual-storage warning.
    pub notes: Vec<String>,
}

/// Run the migration. See module docs for the contract.
pub async fn run_migration(
    pool: &SqlitePool,
    vault_path: &Path,
    opts: &MigrationOptions,
) -> Result<MigrationReport, String> {
    let mut report = MigrationReport {
        vault_path: vault_path.display().to_string(),
        dry_run: opts.dry_run,
        ..Default::default()
    };

    // Backup first — small file, cheap, undoable.
    if opts.backup && !opts.dry_run {
        let db_path = vault_path.join("notes.db");
        if db_path.exists() {
            let backup = vault_path.join("notes.db.pre-v1-backup");
            std::fs::copy(&db_path, &backup).map_err(|e| format!("backup notes.db: {e}"))?;
            report.backup_path = Some(backup.display().to_string());
        }
    }

    migrate_connections(pool, vault_path, opts.dry_run, &mut report).await?;
    migrate_environments(
        pool,
        vault_path,
        &opts.user_config_path,
        opts.dry_run,
        &mut report,
    )
    .await?;
    migrate_prefs(pool, &opts.user_config_path, opts.dry_run, &mut report).await?;

    if opts.dry_run {
        report
            .notes
            .push("Dry run — nothing written. Re-run with dry_run=false to apply.".to_string());
    } else {
        report.notes.push(
            "Connections + envs + prefs migrated. The legacy SQLite `connections` / `environments` / `env_variables` tables are still read by the running app until the frontend cutover lands; the seven prefs keys in `app_config` can be dropped now."
                .to_string(),
        );
    }

    Ok(report)
}

/// Migrate the seven UI prefs keys from `app_config` into the
/// per-machine `user.toml [ui]` section. See audit-005 for the
/// scope decision and the original audit
/// for the per-key
/// classification.
async fn migrate_prefs(
    pool: &SqlitePool,
    user_config_path: &Path,
    dry_run: bool,
    report: &mut MigrationReport,
) -> Result<(), String> {
    // Read each app_config row; missing rows just leave the
    // corresponding UiPrefs field at its default.
    let mut prefs = UiPrefs::default();
    let mut count: usize = 0;

    if let Some(theme_json) = get_pref(pool, "theme").await? {
        if apply_theme_json(&theme_json, &mut prefs) {
            count += 1;
        }
    }
    if let Some(ms) = get_pref(pool, "auto_save_ms").await? {
        if let Ok(n) = ms.parse::<u32>() {
            prefs.auto_save_ms = n;
            count += 1;
        }
    }
    if let Some(s) = get_pref(pool, "editor_font_size").await? {
        if let Ok(n) = s.parse::<u16>() {
            prefs.font_size = n;
            count += 1;
        }
    }
    if let Some(s) = get_pref(pool, "default_fetch_size").await? {
        if let Ok(n) = s.parse::<u32>() {
            prefs.default_fetch_size = n;
            count += 1;
        }
    }
    if let Some(s) = get_pref(pool, "history_retention").await? {
        if let Ok(n) = s.parse::<u32>() {
            prefs.history_retention = n;
            count += 1;
        }
    }
    if let Some(s) = get_pref(pool, "vim_enabled").await? {
        prefs.vim_enabled = s == "true";
        count += 1;
    }
    if let Some(s) = get_pref(pool, "sidebar_open").await? {
        prefs.sidebar_open = s == "true";
        count += 1;
    }

    report.prefs_migrated = count;
    if dry_run || count == 0 {
        return Ok(());
    }

    // Write back via UserStore so the atomic-write + version stamp
    // contract from epic 09 applies.
    let store = UserStore::with_path(user_config_path);
    store.set_ui(prefs).await?;
    Ok(())
}

async fn get_pref(pool: &SqlitePool, key: &str) -> Result<Option<String>, String> {
    config::get_config(pool, key)
        .await
        .map_err(|e| format!("read app_config[{key}]: {e}"))
}

/// MVP frontend serialised theme as a JSON object
/// `{ mode, accent, ... }` per `stores/settings.ts`. We only
/// migrate `mode` into the `theme` string; accent/etc. are
/// per-machine UI niceties that the new `[ui]` section doesn't
/// have room for yet — + will expand the schema if needed.
/// Returns `true` when at least the `mode` field was extracted.
fn apply_theme_json(raw: &str, out: &mut UiPrefs) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
        return false;
    };
    if let Some(mode) = value.get("mode").and_then(|v| v.as_str()) {
        out.theme = mode.to_string();
        return true;
    }
    // Some MVP installs stored the bare string ("dark"). Try that
    // shape too before giving up.
    if let Some(s) = value.as_str() {
        out.theme = s.to_string();
        return true;
    }
    false
}

async fn migrate_connections(
    pool: &SqlitePool,
    vault_path: &Path,
    dry_run: bool,
    report: &mut MigrationReport,
) -> Result<(), String> {
    let conns = connections::list_connections(pool).await?;
    if dry_run {
        report.connections_migrated = conns.len();
        return Ok(());
    }

    let store: Arc<ConnectionsStore> = ConnectionsStore::new(vault_path);
    for c in conns {
        let input = legacy_to_input(&c);
        match store.create(input).await {
            Ok(_) => report.connections_migrated += 1,
            Err(e) if e.contains("already exists") => report.connections_skipped += 1,
            Err(e) => {
                return Err(format!("migrate connection '{}': {e}", c.name));
            }
        }
    }
    Ok(())
}

async fn migrate_environments(
    pool: &SqlitePool,
    vault_path: &Path,
    user_config_path: &Path,
    dry_run: bool,
    report: &mut MigrationReport,
) -> Result<(), String> {
    let envs = environments::list_environments(pool).await?;
    if dry_run {
        report.environments_migrated = envs.len();
        for env in &envs {
            let vars = environments::list_env_variables(pool, &env.id).await?;
            report.variables_migrated += vars.len();
        }
        return Ok(());
    }

    let store: Arc<EnvironmentsStore> = EnvironmentsStore::new(vault_path, user_config_path);
    for env in envs {
        match store.create_env(&env.name).await {
            Ok(_) => report.environments_migrated += 1,
            Err(e) if e.contains("already exists") => {
                report.environments_skipped += 1;
            }
            Err(e) => {
                return Err(format!("migrate env '{}': {e}", env.name));
            }
        }

        let vars = environments::list_env_variables(pool, &env.id).await?;
        for var in vars {
            let input = SetVarInput {
                env_name: env.name.clone(),
                key: var.key.clone(),
                value: var.value,
                is_secret: var.is_secret,
            };
            match store.set_var(input).await {
                Ok(_) => report.variables_migrated += 1,
                Err(e) if e.contains("already exists") => {
                    report.variables_skipped += 1;
                }
                Err(e) => {
                    return Err(format!("migrate var '{}/{}': {e}", env.name, var.key));
                }
            }
        }
    }
    Ok(())
}

fn legacy_to_input(c: &connections::Connection) -> CreateConnectionInput {
    CreateConnectionInput {
        name: c.name.clone(),
        driver: c.driver.clone(),
        host: c.host.clone(),
        port: c.port.and_then(|p| u16::try_from(p).ok()),
        database_name: c.database_name.clone(),
        username: c.username.clone(),
        password: c.password.clone(),
        ssl_mode: c.ssl_mode.clone(),
        is_readonly: Some(c.is_readonly),
        description: None,
    }
}

#[cfg(test)]
mod tests {
    // Tests use `KEYCHAIN_TEST_LOCK` (a `std::sync::Mutex`) and hold
    // it across the migration's `.await` boundaries. The lock is the
    // serialization point for the in-process keychain stub; safe to
    // hold here because `cargo test` runs each test in its own thread
    // and the lock is contention-free in practice.
    #![allow(clippy::await_holding_lock)]

    use super::*;
    use crate::db::init_db;
    use tempfile::TempDir;

    // --- Detection tests -------------------------------------------------

    #[test]
    fn detect_returns_neither_for_empty_vault() {
        let tmp = TempDir::new().unwrap();
        let r = detect_migration_candidate(tmp.path());
        assert!(!r.has_legacy_db);
        assert!(!r.has_v1_layout);
        assert!(!r.should_prompt());
    }

    #[test]
    fn detect_flags_legacy_db_when_notes_db_present() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("notes.db"), b"").unwrap();
        let r = detect_migration_candidate(tmp.path());
        assert!(r.has_legacy_db);
        assert!(!r.has_v1_layout);
        assert!(r.should_prompt(), "legacy db without v1 layout = prompt");
    }

    #[test]
    fn detect_flags_v1_layout_when_dot_httui_dir_present() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join(".httui")).unwrap();
        let r = detect_migration_candidate(tmp.path());
        assert!(!r.has_legacy_db);
        assert!(r.has_v1_layout);
        assert!(!r.should_prompt());
    }

    #[test]
    fn detect_does_not_prompt_when_both_present() {
        // Mid-migration / dual-state: vault has both. Don't prompt;
        // the v1 layout already absorbed (or is in the process of
        // absorbing) the legacy data.
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("notes.db"), b"").unwrap();
        std::fs::create_dir(tmp.path().join(".httui")).unwrap();
        let r = detect_migration_candidate(tmp.path());
        assert!(r.has_legacy_db);
        assert!(r.has_v1_layout);
        assert!(!r.should_prompt());
    }

    #[test]
    fn detect_ignores_notes_db_directory() {
        // A directory named `notes.db` should NOT count as the
        // legacy database — `is_file` is the right check.
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir(tmp.path().join("notes.db")).unwrap();
        let r = detect_migration_candidate(tmp.path());
        assert!(!r.has_legacy_db);
    }

    #[test]
    fn detect_ignores_dot_httui_when_it_is_a_file() {
        // Symmetric guard for the v1 layout check.
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join(".httui"), b"").unwrap();
        let r = detect_migration_candidate(tmp.path());
        assert!(!r.has_v1_layout);
    }

    #[test]
    fn detect_handles_nonexistent_vault_path() {
        // Caller might pass a path that hasn't been created yet
        // (e.g. user typed a freshly-picked folder). Don't panic;
        // report neither.
        let tmp = TempDir::new().unwrap();
        let phantom = tmp.path().join("does-not-exist");
        let r = detect_migration_candidate(&phantom);
        assert!(!r.has_legacy_db);
        assert!(!r.has_v1_layout);
    }

    #[test]
    fn legacy_db_file_constant_is_notes_db() {
        assert_eq!(LEGACY_DB_FILE, "notes.db");
    }

    // --- Existing migration tests below ----------------------------------

    /// Setup helper: fresh SQLite + populated with one connection,
    /// one environment, two vars (one secret).
    async fn populated_db(tmp: &TempDir) -> (SqlitePool, String) {
        let pool = init_db(tmp.path()).await.unwrap();

        // One connection
        connections::create_connection(
            &pool,
            connections::CreateConnection {
                name: "pg-staging".into(),
                driver: "postgres".into(),
                host: Some("pg.example.com".into()),
                port: Some(5432),
                database_name: Some("payments".into()),
                username: Some("app".into()),
                password: Some(String::new()),
                ssl_mode: Some("require".into()),
                timeout_ms: None,
                query_timeout_ms: None,
                ttl_seconds: None,
                max_pool_size: None,
                is_readonly: Some(false),
            },
        )
        .await
        .unwrap();

        // One environment + two vars (one plaintext, one secret)
        let env = environments::create_environment(&pool, "staging".into())
            .await
            .unwrap();
        environments::set_env_variable(
            &pool,
            &env.id,
            "BASE_URL".into(),
            "https://api.example.com".into(),
            false,
        )
        .await
        .unwrap();
        let env_id = env.id.clone();
        (pool, env_id)
    }

    #[tokio::test]
    async fn dry_run_does_not_write() {
        let tmp = TempDir::new().unwrap();
        let (pool, _eid) = populated_db(&tmp).await;

        let opts = MigrationOptions {
            dry_run: true,
            backup: false,
            user_config_path: tmp.path().join("user.toml"),
        };
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();
        let report = run_migration(&pool, &vault, &opts).await.unwrap();

        assert!(report.dry_run);
        assert_eq!(report.connections_migrated, 1);
        assert_eq!(report.environments_migrated, 1);
        assert_eq!(report.variables_migrated, 1);
        // Nothing on disk.
        assert!(!vault.join("connections.toml").exists());
        assert!(!vault.join("envs").exists());
    }

    #[tokio::test]
    async fn writes_files_in_normal_run() {
        let _g = crate::db::keychain::KEYCHAIN_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let (pool, _eid) = populated_db(&tmp).await;
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();

        let opts = MigrationOptions {
            dry_run: false,
            backup: false,
            user_config_path: tmp.path().join("user.toml"),
        };
        let report = run_migration(&pool, &vault, &opts).await.unwrap();

        assert_eq!(report.connections_migrated, 1);
        assert_eq!(report.environments_migrated, 1);
        assert_eq!(report.variables_migrated, 1);
        assert!(vault.join("connections.toml").exists());
        assert!(vault.join("envs/staging.toml").exists());
    }

    #[tokio::test]
    async fn backup_copies_db_when_present() {
        let _g = crate::db::keychain::KEYCHAIN_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let (pool, _eid) = populated_db(&tmp).await;
        // The init_db pool created `notes.db` under tmp.path(). Use
        // tmp.path() as the vault root for this test.
        let vault = tmp.path();
        let db_file = vault.join("notes.db");
        assert!(db_file.exists(), "notes.db should exist after init_db");

        let opts = MigrationOptions {
            dry_run: false,
            backup: true,
            user_config_path: tmp.path().join("user.toml"),
        };
        let report = run_migration(&pool, vault, &opts).await.unwrap();
        assert!(report.backup_path.is_some());
        assert!(vault.join("notes.db.pre-v1-backup").exists());
    }

    #[tokio::test]
    async fn rerun_is_idempotent() {
        let _g = crate::db::keychain::KEYCHAIN_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let tmp = TempDir::new().unwrap();
        let (pool, _eid) = populated_db(&tmp).await;
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();

        let opts = MigrationOptions {
            dry_run: false,
            backup: false,
            user_config_path: tmp.path().join("user.toml"),
        };
        run_migration(&pool, &vault, &opts).await.unwrap();
        // Second run: everything already there — counts as skipped.
        let r2 = run_migration(&pool, &vault, &opts).await.unwrap();
        assert_eq!(r2.connections_skipped, 1);
        assert_eq!(r2.environments_skipped, 1);
    }

    #[tokio::test]
    async fn empty_db_yields_zero_counts() {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();

        let opts = MigrationOptions {
            dry_run: false,
            backup: false,
            user_config_path: tmp.path().join("user.toml"),
        };
        let report = run_migration(&pool, &vault, &opts).await.unwrap();
        assert_eq!(report.connections_migrated, 0);
        assert_eq!(report.environments_migrated, 0);
        assert_eq!(report.variables_migrated, 0);
    }

    #[test]
    fn legacy_to_input_truncates_oversized_port() {
        let c = connections::Connection {
            id: "x".into(),
            name: "n".into(),
            driver: "postgres".into(),
            host: None,
            port: Some(70_000), // > u16
            database_name: None,
            username: None,
            password: None,
            ssl_mode: None,
            timeout_ms: 0,
            query_timeout_ms: 0,
            ttl_seconds: 0,
            max_pool_size: 0,
            is_readonly: false,
            last_tested_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let input = legacy_to_input(&c);
        assert!(input.port.is_none());
    }

    // --- prefs migration -----------------------------------------------

    #[tokio::test]
    async fn migrate_prefs_round_trip() {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();

        // Populate the seven keys.
        config::set_config(&pool, "theme", r#"{"mode":"dark","accent":"purple"}"#)
            .await
            .unwrap();
        config::set_config(&pool, "auto_save_ms", "500")
            .await
            .unwrap();
        config::set_config(&pool, "editor_font_size", "13")
            .await
            .unwrap();
        config::set_config(&pool, "default_fetch_size", "200")
            .await
            .unwrap();
        config::set_config(&pool, "history_retention", "25")
            .await
            .unwrap();
        config::set_config(&pool, "vim_enabled", "true")
            .await
            .unwrap();
        config::set_config(&pool, "sidebar_open", "false")
            .await
            .unwrap();

        let user_path = tmp.path().join("user.toml");
        let opts = MigrationOptions {
            dry_run: false,
            backup: false,
            user_config_path: user_path.clone(),
        };
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();

        let report = run_migration(&pool, &vault, &opts).await.unwrap();
        assert_eq!(report.prefs_migrated, 7);

        // Re-read user.toml via UserStore and confirm the values
        // landed.
        let store = UserStore::with_path(&user_path);
        let ui = store.ui().await.unwrap();
        assert_eq!(ui.theme, "dark");
        assert_eq!(ui.auto_save_ms, 500);
        assert_eq!(ui.font_size, 13);
        assert_eq!(ui.default_fetch_size, 200);
        assert_eq!(ui.history_retention, 25);
        assert!(ui.vim_enabled);
        assert!(!ui.sidebar_open);
    }

    #[tokio::test]
    async fn migrate_prefs_dry_run_does_not_write() {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();
        config::set_config(&pool, "auto_save_ms", "500")
            .await
            .unwrap();

        let user_path = tmp.path().join("user.toml");
        let opts = MigrationOptions {
            dry_run: true,
            backup: false,
            user_config_path: user_path.clone(),
        };
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();

        let report = run_migration(&pool, &vault, &opts).await.unwrap();
        assert_eq!(report.prefs_migrated, 1, "counts what would migrate");
        assert!(!user_path.exists(), "dry run never writes");
    }

    #[tokio::test]
    async fn migrate_prefs_zero_when_no_app_config_rows() {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();
        let user_path = tmp.path().join("user.toml");
        let opts = MigrationOptions {
            dry_run: false,
            backup: false,
            user_config_path: user_path.clone(),
        };
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();

        let report = run_migration(&pool, &vault, &opts).await.unwrap();
        assert_eq!(report.prefs_migrated, 0);
        // No prefs to write → user.toml stays absent.
        assert!(!user_path.exists());
    }

    #[tokio::test]
    async fn migrate_prefs_skips_unparseable_values() {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();
        config::set_config(&pool, "auto_save_ms", "not-a-number")
            .await
            .unwrap();
        config::set_config(&pool, "editor_font_size", "13")
            .await
            .unwrap();

        let user_path = tmp.path().join("user.toml");
        let opts = MigrationOptions {
            dry_run: false,
            backup: false,
            user_config_path: user_path.clone(),
        };
        let vault = tmp.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();

        let report = run_migration(&pool, &vault, &opts).await.unwrap();
        // Only the parsable value counts.
        assert_eq!(report.prefs_migrated, 1);
    }

    #[test]
    fn apply_theme_json_handles_object_shape() {
        let mut prefs = UiPrefs::default();
        let ok = apply_theme_json(r#"{"mode":"dark"}"#, &mut prefs);
        assert!(ok);
        assert_eq!(prefs.theme, "dark");
    }

    #[test]
    fn apply_theme_json_handles_bare_string() {
        let mut prefs = UiPrefs::default();
        let ok = apply_theme_json("\"high-contrast\"", &mut prefs);
        assert!(ok);
        assert_eq!(prefs.theme, "high-contrast");
    }

    #[test]
    fn apply_theme_json_rejects_invalid_json() {
        let mut prefs = UiPrefs::default();
        let ok = apply_theme_json("not json", &mut prefs);
        assert!(!ok);
        // Defaults preserved.
        assert_eq!(prefs.theme, "system");
    }

    #[test]
    fn apply_theme_json_rejects_object_without_mode() {
        let mut prefs = UiPrefs::default();
        let ok = apply_theme_json(r#"{"accent":"red"}"#, &mut prefs);
        assert!(!ok);
    }
}
