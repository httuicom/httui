//! File-backed user config store.
//!
//! Source of truth is `<config_dir>/httui/user.toml` where
//! `<config_dir>` follows the XDG Base Directory spec on Linux
//! (`$XDG_CONFIG_HOME` first, then `$HOME/.config`) and the OS-native
//! config dir elsewhere (`~/Library/Application Support` on macOS,
//! `%APPDATA%` on Windows).
//!
//! Holds per-machine prefs only — visual settings, keybindings, secrets
//! backend choice, MCP server config, and active-environment tracking.
//! Workspace-collab defaults live in [`WorkspaceStore`](super::workspace_store::WorkspaceStore).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use tokio::sync::RwLock;

use super::atomic::{read_toml, write_toml};
use super::tui_view::TuiViewState;
use super::user::{McpConfig, SecretsBackend, UiPrefs, UserFile};
use super::Version;

const CONFIG_SUBDIR: &str = "httui";
const USER_FILE: &str = "user.toml";

#[derive(Debug, Clone)]
struct Cached {
    mtime: Option<SystemTime>,
    file: UserFile,
}

/// File-backed read/write over `~/.config/httui/user.toml`.
pub struct UserStore {
    path: PathBuf,
    cache: RwLock<Option<Cached>>,
}

impl UserStore {
    /// Build a store anchored at the OS-appropriate config path.
    /// Returns an error if no config directory can be resolved (extremely
    /// rare — only on a totally broken environment with no `$HOME` and
    /// no `$XDG_CONFIG_HOME`).
    pub fn from_default_path() -> Result<Arc<Self>, String> {
        let path = default_user_config_path()?;
        Ok(Self::with_path(path))
    }

    /// Build a store with an explicit path. Used by tests and by
    /// callers that need to override the location (e.g. portable
    /// installs).
    pub fn with_path(path: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            path: path.into(),
            cache: RwLock::new(None),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn current_mtime(&self) -> Option<SystemTime> {
        std::fs::metadata(&self.path)
            .ok()
            .and_then(|m| m.modified().ok())
    }

    /// Returns the parsed file, using the cache when on-disk mtime is
    /// unchanged. Returns a default-valued file when missing.
    pub async fn load(&self) -> Result<UserFile, String> {
        let disk_mtime = self.current_mtime();

        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.mtime == disk_mtime {
                    return Ok(cached.file.clone());
                }
            }
        }

        let file = if self.path.exists() {
            read_toml::<UserFile>(&self.path)
                .map_err(|e| format!("read {}: {e}", self.path.display()))?
        } else {
            UserFile::default()
        };

        let mut cache = self.cache.write().await;
        *cache = Some(Cached {
            mtime: disk_mtime,
            file: file.clone(),
        });
        Ok(file)
    }

    async fn persist(&self, mut file: UserFile) -> Result<(), String> {
        // Force the on-disk version stamp; downstream readers rely on
        // it being explicit.
        file.version = Version::V1;
        write_toml(&self.path, &file).map_err(|e| format!("write {}: {e}", self.path.display()))?;

        let mut cache = self.cache.write().await;
        *cache = Some(Cached {
            mtime: self.current_mtime(),
            file,
        });
        Ok(())
    }

    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }

    pub async fn ui(&self) -> Result<UiPrefs, String> {
        Ok(self.load().await?.ui)
    }

    pub async fn set_ui(&self, ui: UiPrefs) -> Result<(), String> {
        let mut file = self.load().await?;
        file.ui = ui;
        self.persist(file).await
    }

    pub async fn secrets(&self) -> Result<SecretsBackend, String> {
        Ok(self.load().await?.secrets)
    }

    pub async fn set_secrets(&self, secrets: SecretsBackend) -> Result<(), String> {
        let mut file = self.load().await?;
        file.secrets = secrets;
        self.persist(file).await
    }

    pub async fn mcp(&self) -> Result<McpConfig, String> {
        Ok(self.load().await?.mcp)
    }

    pub async fn set_mcp(&self, mcp: McpConfig) -> Result<(), String> {
        let mut file = self.load().await?;
        file.mcp = mcp;
        self.persist(file).await
    }

    /// Replace the entire `UserFile`. Use sparingly — most callers
    /// should mutate one section via the typed setters above so the
    /// other sections aren't accidentally clobbered.
    pub async fn replace(&self, file: UserFile) -> Result<(), String> {
        self.persist(file).await
    }

    /// Read the TUI view snapshot recorded for `vault_path`. Returns
    /// `None` when this vault has no entry yet (first run on this
    /// machine, or the user has never opened BLOCKS view here).
    /// `vault_path` is matched verbatim — callers must canonicalise
    /// before calling (same key the setter writes with).
    pub async fn tui_view_state(&self, vault_path: &str) -> Result<Option<TuiViewState>, String> {
        Ok(self.load().await?.tui_view_state.get(vault_path).cloned())
    }

    /// Write the TUI view snapshot for `vault_path`, replacing any
    /// previous entry. Read-modify-write through `persist` so unrelated
    /// sections (ui prefs, secrets, active_envs) stay intact.
    pub async fn set_tui_view_state(
        &self,
        vault_path: &str,
        snapshot: TuiViewState,
    ) -> Result<(), String> {
        if vault_path.trim().is_empty() {
            return Err("vault_path must not be empty".to_string());
        }
        let mut file = self.load().await?;
        file.tui_view_state
            .insert(vault_path.to_string(), snapshot);
        self.persist(file).await
    }

    /// Drop the TUI view entry for `vault_path`. No-op when the
    /// entry is already missing.
    pub async fn clear_tui_view_state(&self, vault_path: &str) -> Result<(), String> {
        let mut file = self.load().await?;
        if file.tui_view_state.remove(vault_path).is_none() {
            return Ok(());
        }
        self.persist(file).await
    }

    /// Ensure the file exists on disk. Idempotent.
    pub async fn ensure_exists(&self) -> Result<(), String> {
        if self.path.exists() {
            return Ok(());
        }
        let file = self.load().await?;
        self.persist(file).await
    }
}

/// Resolve the on-disk path for `user.toml`, applying XDG rules on
/// Linux and OS-native conventions elsewhere.
///
/// Resolution order:
/// 1. `$XDG_CONFIG_HOME/httui/user.toml` if set and non-empty
/// 2. The OS-native config dir (`dirs::config_dir`):
///    - Linux: `$HOME/.config/httui/user.toml`
///    - macOS: `$HOME/Library/Application Support/httui/user.toml`
///    - Windows: `%APPDATA%\httui\user.toml`
/// 3. Error if neither resolves (no `$HOME`, no `$XDG_CONFIG_HOME`).
pub fn default_user_config_path() -> Result<PathBuf, String> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Ok(PathBuf::from(xdg).join(CONFIG_SUBDIR).join(USER_FILE));
        }
    }
    let base = dirs::config_dir().ok_or_else(|| {
        "could not resolve user config directory (no XDG_CONFIG_HOME, no HOME)".to_string()
    })?;
    Ok(base.join(CONFIG_SUBDIR).join(USER_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store_in(dir: &TempDir) -> Arc<UserStore> {
        UserStore::with_path(dir.path().join("httui").join("user.toml"))
    }

    #[tokio::test]
    async fn load_returns_default_when_missing() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        let f = s.load().await.unwrap();
        assert_eq!(f.ui.theme, "system");
        assert_eq!(f.secrets.backend, "auto");
    }

    #[tokio::test]
    async fn set_ui_creates_file_and_round_trips() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        let ui = UiPrefs {
            theme: "dark".into(),
            font_size: 13,
            ..UiPrefs::default()
        };
        s.set_ui(ui).await.unwrap();
        assert!(s.path().exists());
        let raw = std::fs::read_to_string(s.path()).unwrap();
        assert!(raw.contains("theme = \"dark\""));
        assert!(raw.contains("version = \"1\""));
        let loaded = s.ui().await.unwrap();
        assert_eq!(loaded.theme, "dark");
        assert_eq!(loaded.font_size, 13);
    }

    #[tokio::test]
    async fn set_secrets_keeps_other_sections() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        let ui = UiPrefs {
            theme: "dark".into(),
            ..UiPrefs::default()
        };
        s.set_ui(ui).await.unwrap();

        let secrets = SecretsBackend {
            backend: "1password".into(),
            ..SecretsBackend::default()
        };
        s.set_secrets(secrets).await.unwrap();

        // UI must survive a secrets-only mutation.
        let f = s.load().await.unwrap();
        assert_eq!(f.ui.theme, "dark");
        assert_eq!(f.secrets.backend, "1password");
    }

    #[tokio::test]
    async fn set_mcp_round_trips() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        let mut mcp = McpConfig::default();
        mcp.servers
            .insert("notes".to_string(), toml::Value::String("on".into()));
        // Mutating a single map field after construction reads cleaner
        // than struct-update with a literal map.
        s.set_mcp(mcp).await.unwrap();
        let loaded = s.mcp().await.unwrap();
        assert_eq!(loaded.servers.len(), 1);
        assert_eq!(
            loaded.servers.get("notes"),
            Some(&toml::Value::String("on".into()))
        );
    }

    #[tokio::test]
    async fn replace_overwrites_everything() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        s.set_ui(UiPrefs {
            theme: "dark".into(),
            ..UiPrefs::default()
        })
        .await
        .unwrap();
        let mut fresh = UserFile::default();
        fresh.ui.theme = "light".into();
        s.replace(fresh).await.unwrap();
        assert_eq!(s.ui().await.unwrap().theme, "light");
    }

    #[tokio::test]
    async fn ensure_exists_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        assert!(!s.path().exists());
        s.ensure_exists().await.unwrap();
        assert!(s.path().exists());
        let m1 = std::fs::metadata(s.path()).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        s.ensure_exists().await.unwrap();
        let m2 = std::fs::metadata(s.path()).unwrap().modified().unwrap();
        assert_eq!(m1, m2);
    }

    #[tokio::test]
    async fn load_picks_up_external_edit_via_mtime() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        s.ensure_exists().await.unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(
            s.path(),
            "version = \"1\"\n[ui]\ntheme = \"dark\"\nfont_family = \"Iosevka\"\nfont_size = 12\ndensity = \"compact\"\n",
        )
        .unwrap();
        let f = s.load().await.unwrap();
        assert_eq!(f.ui.theme, "dark");
        assert_eq!(f.ui.font_size, 12);
    }

    #[tokio::test]
    async fn invalidate_cache_forces_reread() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        s.ensure_exists().await.unwrap();
        let _ = s.load().await.unwrap();
        std::fs::write(
            s.path(),
            "version = \"1\"\n[ui]\ntheme = \"high-contrast\"\nfont_family = \"x\"\nfont_size = 20\ndensity = \"comfortable\"\n",
        )
        .unwrap();
        s.invalidate_cache().await;
        let f = s.load().await.unwrap();
        assert_eq!(f.ui.theme, "high-contrast");
    }

    #[tokio::test]
    async fn read_invalid_toml_returns_error() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        std::fs::create_dir_all(s.path().parent().unwrap()).unwrap();
        std::fs::write(s.path(), "this = = invalid").unwrap();
        let err = s.load().await.unwrap_err();
        assert!(err.contains("read"), "got {err}");
    }

    fn with_env<F: FnOnce()>(key: &str, value: Option<&str>, f: F) {
        // SAFETY: tests using std::env::set_var must not run in parallel
        // with other env-mutating tests. `cargo test` runs tests in the
        // same process; we serialize via this module's mutex.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let prev = std::env::var(key).ok();
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn xdg_config_home_takes_precedence() {
        with_env("XDG_CONFIG_HOME", Some("/custom/xdg"), || {
            let p = default_user_config_path().unwrap();
            assert_eq!(p, PathBuf::from("/custom/xdg/httui/user.toml"));
        });
    }

    #[test]
    fn empty_xdg_config_home_falls_back() {
        with_env("XDG_CONFIG_HOME", Some(""), || {
            let p = default_user_config_path().unwrap();
            // Whatever dirs::config_dir returns, the suffix is ours.
            assert!(p.ends_with("httui/user.toml"), "got {}", p.display());
        });
    }

    #[test]
    fn fallback_uses_os_native_when_xdg_unset() {
        with_env("XDG_CONFIG_HOME", None, || {
            let p = default_user_config_path().unwrap();
            assert!(p.ends_with("httui/user.toml"), "got {}", p.display());
        });
    }

    #[tokio::test]
    async fn tui_view_state_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        assert!(s.tui_view_state("/some/vault").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn set_tui_view_state_round_trips_per_vault() {
        use super::super::tui_view::{
            BlocksWorkspaceSnapshot, PaneLeafSnapshot, PaneSnapshot, TuiViewState,
        };
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        let snap = TuiViewState {
            last_view: "blocks".into(),
            sidebar_open: true,
            blocks: Some(BlocksWorkspaceSnapshot {
                expanded_files: vec!["api.md".into()],
                cursor: None,
                root: PaneSnapshot::Leaf(PaneLeafSnapshot {
                    file: Some("api.md".into()),
                    ..PaneLeafSnapshot::default()
                }),
                focused: vec![],
            }),
        };
        s.set_tui_view_state("/vault/a", snap.clone()).await.unwrap();
        let back = s.tui_view_state("/vault/a").await.unwrap().unwrap();
        assert_eq!(back, snap);
        // Another vault returns None — entries are keyed.
        assert!(s.tui_view_state("/vault/b").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn set_tui_view_state_keeps_other_sections() {
        use super::super::tui_view::TuiViewState;
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        s.set_ui(UiPrefs {
            theme: "dark".into(),
            ..UiPrefs::default()
        })
        .await
        .unwrap();
        s.set_tui_view_state(
            "/vault/a",
            TuiViewState {
                last_view: "doc".into(),
                sidebar_open: false,
                blocks: None,
            },
        )
        .await
        .unwrap();
        let f = s.load().await.unwrap();
        assert_eq!(f.ui.theme, "dark");
        assert_eq!(f.tui_view_state.len(), 1);
    }

    #[tokio::test]
    async fn set_tui_view_state_rejects_empty_vault_path() {
        use super::super::tui_view::TuiViewState;
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        let err = s
            .set_tui_view_state("   ", TuiViewState::default())
            .await
            .unwrap_err();
        assert!(err.contains("must not be empty"), "got: {err}");
    }

    #[tokio::test]
    async fn clear_tui_view_state_removes_entry() {
        use super::super::tui_view::TuiViewState;
        let dir = TempDir::new().unwrap();
        let s = store_in(&dir);
        s.set_tui_view_state(
            "/vault/a",
            TuiViewState {
                last_view: "doc".into(),
                sidebar_open: false,
                blocks: None,
            },
        )
        .await
        .unwrap();
        s.clear_tui_view_state("/vault/a").await.unwrap();
        assert!(s.tui_view_state("/vault/a").await.unwrap().is_none());
        // Clearing a missing entry is a silent no-op.
        s.clear_tui_view_state("/vault/never-set").await.unwrap();
    }
}
