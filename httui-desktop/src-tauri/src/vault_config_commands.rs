//! Tauri commands for the file-backed vault config stores.
//!
//! Epic 09 ships these as **foundation only** — the desktop frontend
//! still reads/writes through the legacy `app_config` SQLite path
//! until epic 19 (settings split) cuts over. Wiring the commands now
//! lets epic 19 swap the frontend in a single, low-risk PR.
//!
//! Stores are constructed per call: `WorkspaceStore::new(vault_path)`
//! for the vault-anchored workspace file, `UserStore::from_default_path()`
//! for the per-machine user file. The mtime cache inside each store
//! buys nothing across one-shot calls, but the surface stays simple
//! and correct; the long-lived `Arc<Store>` cache pattern arrives with
//! the cutover in epic 19.

use httui_core::secrets::Keychain;
use httui_core::vault_config::create::{create_new_vault, CreateOutcome};
use httui_core::vault_config::gitignore::{ensure_local_overrides_in_gitignore, GitignoreOutcome};
use httui_core::vault_config::migration::{
    detect_migration_candidate, run_migration, MigrationCandidate, MigrationOptions,
    MigrationReport,
};
use httui_core::vault_config::missing_secrets::{scan_missing_secrets, MissingRef};
use httui_core::vault_config::scaffold::{is_vault, scaffold_new_vault, ScaffoldReport};
use httui_core::vault_config::user::UserFile;
use httui_core::vault_config::user_store::default_user_config_path;
use httui_core::vault_config::workspace::{FileSettings, WorkspaceDefaults};
use httui_core::vault_config::{UserStore, WorkspaceStore};
use sqlx::sqlite::SqlitePool;
use std::path::PathBuf;

#[tauri::command]
pub async fn get_workspace_config(vault_path: String) -> Result<WorkspaceDefaults, String> {
    let store = WorkspaceStore::new(vault_path);
    store.defaults().await
}

#[tauri::command]
pub async fn set_workspace_config(
    vault_path: String,
    defaults: WorkspaceDefaults,
) -> Result<(), String> {
    let store = WorkspaceStore::new(vault_path);
    store.set_defaults(defaults).await
}

/// Read the per-file settings entry for `file_path` (vault-relative).
/// Returns `FileSettings::default()` when no entry exists. Carry-over
/// from Epic 39 Story 03 — feeds the editor toolbar's auto-capture
/// toggle.
#[tauri::command]
pub async fn get_file_settings(
    vault_path: String,
    file_path: String,
) -> Result<FileSettings, String> {
    let store = WorkspaceStore::new(vault_path);
    store.file_settings(&file_path).await
}

/// Toggle `auto_capture` for `file_path`. Writes through to
/// `workspace.toml`'s base (never `.local.toml`) and prunes default-
/// valued entries so the file stays minimal.
#[tauri::command]
pub async fn set_file_auto_capture(
    vault_path: String,
    file_path: String,
    auto_capture: bool,
) -> Result<(), String> {
    let store = WorkspaceStore::new(vault_path);
    store.set_file_auto_capture(&file_path, auto_capture).await
}

/// Toggle `docheader_compact` for `file_path`. Same prune semantics
/// as auto_capture. Powers Epic 50 Story 06.
#[tauri::command]
pub async fn set_file_docheader_compact(
    vault_path: String,
    file_path: String,
    compact: bool,
) -> Result<(), String> {
    let store = WorkspaceStore::new(vault_path);
    store
        .set_file_docheader_compact(&file_path, compact)
        .await
}

#[tauri::command]
pub async fn get_user_config() -> Result<UserFile, String> {
    let store = UserStore::from_default_path()?;
    store.load().await
}

#[tauri::command]
pub async fn set_user_config(file: UserFile) -> Result<(), String> {
    let store = UserStore::from_default_path()?;
    store.replace(file).await
}

/// Ensure the vault's `.gitignore` carries the canonical block of
/// `*.local.toml` patterns (ADR 0004). Idempotent. Used by the
/// "Open / Clone / Create vault" flow (epic 17) and surfaced as a
/// "fix it" button in the UI when an already-cloned vault is detected
/// without our entries.
#[tauri::command]
pub async fn ensure_vault_gitignore(vault_path: String) -> Result<GitignoreOutcome, String> {
    let path = PathBuf::from(vault_path);
    ensure_local_overrides_in_gitignore(&path).map_err(|e| format!("ensure gitignore: {e}"))
}

/// Heuristic check: does `vault_path` look like a httui vault?
/// Used by the Open-vault flow before activating it (Epic 17).
#[tauri::command]
pub async fn check_is_vault(vault_path: String) -> Result<bool, String> {
    Ok(is_vault(&PathBuf::from(vault_path)))
}

/// Scaffold a brand-new vault under `vault_path`. Idempotent —
/// safe to run on an existing folder. Returns the list of files
/// the call actually wrote (Epic 17 / Story 04).
#[tauri::command]
pub async fn scaffold_vault(vault_path: String) -> Result<ScaffoldReport, String> {
    let path = PathBuf::from(vault_path);
    scaffold_new_vault(&path).map_err(|e| format!("scaffold vault: {e}"))
}

/// Walk the vault for `{{keychain:...}}` references and report
/// which ones are missing from the local OS keychain (Epic 18 /
/// Story 01). Used by the first-run flow to batch-prompt the user
/// instead of stamping a prompt on every block execution.
#[tauri::command]
pub async fn list_missing_secrets(vault_path: String) -> Result<Vec<MissingRef>, String> {
    let path = PathBuf::from(vault_path);
    scan_missing_secrets(&path, &Keychain)
}

/// Create a brand-new vault at `<parent>/<name>` — V1 vertical 1,
/// cenário 3. Composes mkdir + `git init` + scaffold so the user
/// gets a versionable, ready-to-edit vault in one step. The leaf
/// name is the user's input; the backend rejects empty/path-traversal
/// inputs and refuses to overwrite an existing non-empty folder.
#[tauri::command]
pub async fn create_vault_cmd(
    parent_path: String,
    name: String,
) -> Result<CreateOutcome, String> {
    let parent = PathBuf::from(parent_path);
    create_new_vault(&parent, &name)
}

/// Probe `vault_path` to decide whether to surface the MVP→v1
/// migration banner (Epic 41 Story 07 carry slice 2). Returns a
/// `MigrationCandidate { has_legacy_db, has_v1_layout }` —
/// `should_prompt()` on the result is `true` iff the legacy
/// `notes.db` is present and the v1 `.httui/` layout is not
/// initialised. Frontend gates the banner on this AND the
/// `mvp_migration_dismissed` user pref.
#[tauri::command]
pub async fn detect_vault_migration(
    vault_path: String,
) -> Result<MigrationCandidate, String> {
    Ok(detect_migration_candidate(&PathBuf::from(vault_path)))
}

/// Migrate the MVP SQLite-backed vault to the v1 file layout (Epic
/// 12 / audit-005). Migrates `connections` and `environments` +
/// `env_variables`. Prefs migration is part of Epic 19's settings
/// split.
///
/// The Tauri command is the only entry point; there is no CLI flag
/// in v1. Set `dry_run = true` to preview without writing.
#[tauri::command]
pub async fn migrate_vault_to_v1(
    pool: tauri::State<'_, SqlitePool>,
    vault_path: String,
    dry_run: bool,
) -> Result<MigrationReport, String> {
    let user_path = default_user_config_path()?;
    let opts = MigrationOptions {
        dry_run,
        backup: true,
        user_config_path: user_path,
    };
    let path = PathBuf::from(vault_path);
    run_migration(&pool, &path, &opts).await
}

#[cfg(test)]
mod tests {
    //! Tauri commands deliberately stay thin — they construct the store,
    //! delegate, and return. The substantive logic (cache, normalize,
    //! atomic write, XDG resolution) is covered exhaustively in
    //! `httui_core::vault_config::{workspace_store, user_store}` tests.
    //!
    //! These tests cover only what the wrappers themselves add: that
    //! the right store is constructed and that the round-trip through
    //! the wrapper preserves data.

    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn temp_xdg() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_path_buf();
        (dir, path)
    }

    #[tokio::test]
    async fn workspace_round_trip_via_commands() {
        let (_dir, vault) = temp_xdg();
        let vault_str = vault.to_string_lossy().into_owned();

        let initial = get_workspace_config(vault_str.clone()).await.unwrap();
        assert!(initial.environment.is_none());

        set_workspace_config(
            vault_str.clone(),
            WorkspaceDefaults {
                environment: Some("staging".into()),
                git_remote: Some("origin".into()),
                git_branch: Some("main".into()),
            },
        )
        .await
        .unwrap();

        let after = get_workspace_config(vault_str).await.unwrap();
        assert_eq!(after.environment.as_deref(), Some("staging"));
        assert_eq!(after.git_remote.as_deref(), Some("origin"));
        assert_eq!(after.git_branch.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn ensure_vault_gitignore_creates_then_idempotent() {
        let (_dir, vault) = temp_xdg();
        let vault_str = vault.to_string_lossy().into_owned();
        let first = ensure_vault_gitignore(vault_str.clone()).await.unwrap();
        assert_eq!(first, GitignoreOutcome::Created);
        let second = ensure_vault_gitignore(vault_str).await.unwrap();
        assert_eq!(second, GitignoreOutcome::AlreadyPresent);
        assert!(vault.join(".gitignore").exists());
    }

    #[tokio::test]
    async fn detect_vault_migration_returns_neither_for_fresh_folder() {
        let (_dir, vault) = temp_xdg();
        let vault_str = vault.to_string_lossy().into_owned();
        let r = detect_vault_migration(vault_str).await.unwrap();
        assert!(!r.has_legacy_db);
        assert!(!r.has_v1_layout);
        assert!(!r.should_prompt());
    }

    #[tokio::test]
    async fn detect_vault_migration_prompts_when_legacy_db_present() {
        let (_dir, vault) = temp_xdg();
        std::fs::write(vault.join("notes.db"), b"").unwrap();
        let vault_str = vault.to_string_lossy().into_owned();
        let r = detect_vault_migration(vault_str).await.unwrap();
        assert!(r.has_legacy_db);
        assert!(!r.has_v1_layout);
        assert!(r.should_prompt());
    }

    #[tokio::test]
    async fn detect_vault_migration_does_not_prompt_after_v1_layout_initialized() {
        let (_dir, vault) = temp_xdg();
        std::fs::write(vault.join("notes.db"), b"").unwrap();
        std::fs::create_dir(vault.join(".httui")).unwrap();
        let vault_str = vault.to_string_lossy().into_owned();
        let r = detect_vault_migration(vault_str).await.unwrap();
        assert!(r.has_legacy_db);
        assert!(r.has_v1_layout);
        assert!(!r.should_prompt());
    }

    #[tokio::test]
    async fn check_is_vault_round_trip() {
        let (_dir, folder) = temp_xdg();
        let folder_str = folder.to_string_lossy().into_owned();
        // Empty folder isn't a vault.
        assert!(!check_is_vault(folder_str.clone()).await.unwrap());
        // Scaffold turns it into one.
        let report = scaffold_vault(folder_str.clone()).await.unwrap();
        assert!(!report.already_a_vault);
        assert!(check_is_vault(folder_str).await.unwrap());
    }

    #[tokio::test]
    async fn scaffold_vault_is_idempotent_via_command() {
        let (_dir, folder) = temp_xdg();
        let folder_str = folder.to_string_lossy().into_owned();
        scaffold_vault(folder_str.clone()).await.unwrap();
        let r2 = scaffold_vault(folder_str).await.unwrap();
        assert!(r2.already_a_vault);
        assert!(r2.created.is_empty());
    }

    #[tokio::test]
    async fn create_vault_cmd_round_trip() {
        let (_dir, parent) = temp_xdg();
        let parent_str = parent.to_string_lossy().into_owned();
        let outcome = create_vault_cmd(parent_str, "novo-vault".into())
            .await
            .unwrap();
        assert_eq!(outcome.destination, parent.join("novo-vault"));
        assert!(outcome.destination.join(".git").is_dir());
        assert!(outcome.destination.join("connections.toml").is_file());
    }

    #[tokio::test]
    async fn create_vault_cmd_rejects_path_traversal_in_name() {
        let (_dir, parent) = temp_xdg();
        let parent_str = parent.to_string_lossy().into_owned();
        let err = create_vault_cmd(parent_str, "../evil".into())
            .await
            .unwrap_err();
        assert!(err.contains("'.'") || err.contains("'/'"), "got: {err}");
    }

    #[tokio::test]
    async fn file_settings_round_trip_via_commands() {
        let (_dir, vault) = temp_xdg();
        let vault_str = vault.to_string_lossy().into_owned();

        let initial = get_file_settings(vault_str.clone(), "rollout.md".into())
            .await
            .unwrap();
        assert!(!initial.auto_capture);

        set_file_auto_capture(vault_str.clone(), "rollout.md".into(), true)
            .await
            .unwrap();

        let after = get_file_settings(vault_str, "rollout.md".into())
            .await
            .unwrap();
        assert!(after.auto_capture);
    }

    #[tokio::test]
    async fn set_file_auto_capture_validates_path() {
        let (_dir, vault) = temp_xdg();
        let vault_str = vault.to_string_lossy().into_owned();
        let err = set_file_auto_capture(vault_str, "  ".into(), true)
            .await
            .unwrap_err();
        assert!(err.contains("must not be empty"), "got: {err}");
    }

    #[tokio::test]
    async fn workspace_get_returns_defaults_when_missing() {
        let (_dir, vault) = temp_xdg();
        let vault_str = vault.to_string_lossy().into_owned();
        let d = get_workspace_config(vault_str).await.unwrap();
        assert!(d.environment.is_none());
        assert!(d.git_remote.is_none());
        assert!(d.git_branch.is_none());
    }

    // `user_config` tests need to redirect `default_user_config_path`
    // to a tempdir. The function reads `XDG_CONFIG_HOME` / OS-native
    // dirs, so the test mutates `XDG_CONFIG_HOME` under a serial
    // mutex (mirrors the pattern in `user_store::tests`).

    static USER_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_xdg<F: FnOnce()>(value: &str, f: F) {
        let _guard = USER_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var("XDG_CONFIG_HOME").ok();
        std::env::set_var("XDG_CONFIG_HOME", value);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        match prev {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    #[test]
    fn user_round_trip_via_commands() {
        let dir = TempDir::new().unwrap();
        let xdg = dir.path().to_string_lossy().into_owned();

        with_xdg(&xdg, || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                // Empty file → defaults.
                let initial = get_user_config().await.unwrap();
                assert_eq!(initial.ui.theme, "system");

                // Replace with a tweaked file and read it back.
                let mut tweaked = initial.clone();
                tweaked.ui.theme = "dark".into();
                tweaked.ui.font_size = 13;
                set_user_config(tweaked).await.unwrap();

                let after = get_user_config().await.unwrap();
                assert_eq!(after.ui.theme, "dark");
                assert_eq!(after.ui.font_size, 13);
            });
        });
    }
}
