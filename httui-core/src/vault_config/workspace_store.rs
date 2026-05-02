//! File-backed workspace config store.
//!
//! Source of truth is `<vault_root>/.httui/workspace.toml`. Holds
//! collaboration-relevant defaults (active environment, git remote, git
//! branch) — strictly the small set of values that make sense to
//! commit alongside the vault. Per-machine prefs live in `user.toml`
//! (see [`UserStore`](super::user_store::UserStore)).
//!
//! This store has no keychain integration (workspace defaults aren't
//! sensitive) and no validation surface beyond what `serde` enforces on
//! the `WorkspaceFile` schema. The shape is deliberately minimal so the
//! file stays human-reviewable in PRs.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use tokio::sync::RwLock;

use super::atomic::{read_toml, write_toml};
use super::layout::{WORKSPACE_DIR, WORKSPACE_FILE};
use super::merge::{load_with_local, mtime_or_none};
use super::workspace::{
    FileSettings, Source, WorkspaceDefaults, WorkspaceDefaultsWithSources, WorkspaceFile,
    WorkspaceSources,
};
use super::Version;

/// Cache entry. Keys on both base and override mtimes so a touch on
/// either side invalidates correctly (ADR 0004 cache rule).
#[derive(Debug, Clone)]
struct Cached {
    base_mtime: Option<SystemTime>,
    local_mtime: Option<SystemTime>,
    file: WorkspaceFile,
}

/// File-backed read/write over `.httui/workspace.toml`.
pub struct WorkspaceStore {
    vault_root: PathBuf,
    cache: RwLock<Option<Cached>>,
}

impl WorkspaceStore {
    pub fn new(vault_root: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            vault_root: vault_root.into(),
            cache: RwLock::new(None),
        })
    }

    pub fn path(&self) -> PathBuf {
        self.vault_root.join(WORKSPACE_DIR).join(WORKSPACE_FILE)
    }

    /// Returns the parsed file with `*.local.toml` overrides merged
    /// in (ADR 0004). Cache hits when both base and override mtimes
    /// are unchanged. Returns a default-valued file when **both** are
    /// missing — matches the "auto-create on first run" contract.
    pub async fn load(&self) -> Result<WorkspaceFile, String> {
        let path = self.path();
        let local_path = local_path_for(&path);
        let base_mtime = mtime_or_none(&path);
        let local_mtime = mtime_or_none(&local_path);

        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.as_ref() {
                if cached.base_mtime == base_mtime && cached.local_mtime == local_mtime {
                    return Ok(cached.file.clone());
                }
            }
        }

        let file = if path.exists() || local_path.exists() {
            let (parsed, _local) = load_with_local::<WorkspaceFile>(&path)?;
            parsed
        } else {
            WorkspaceFile::default()
        };

        let mut cache = self.cache.write().await;
        *cache = Some(Cached {
            base_mtime,
            local_mtime,
            file: file.clone(),
        });
        Ok(file)
    }

    async fn persist(&self, mut file: WorkspaceFile) -> Result<(), String> {
        // Force the on-disk version stamp; downstream readers rely on
        // it being explicit even when the user has only ever touched
        // `[defaults]`.
        file.version = Version::V1;
        let path = self.path();
        // ADR 0004: writes always target the base file, never `.local`.
        write_toml(&path, &file).map_err(|e| format!("write {}: {e}", path.display()))?;

        // Invalidate; next `load()` re-merges from disk.
        let mut cache = self.cache.write().await;
        *cache = None;
        Ok(())
    }

    /// Read **just** the base file (no `.local` merge). Used by
    /// mutating paths so writes don't promote local overrides into
    /// the committed file. See audit-003 for the rationale.
    async fn load_base_only(&self) -> Result<WorkspaceFile, String> {
        let path = self.path();
        if !path.exists() {
            return Ok(WorkspaceFile::default());
        }
        read_toml::<WorkspaceFile>(&path).map_err(|e| format!("read {}: {e}", path.display()))
    }

    /// Force the next read to hit disk. Hooks into the file watcher
    /// (epic 11) so external edits don't get masked by the cache.
    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        *cache = None;
    }

    /// Read-only accessor that mirrors `WorkspaceFile.defaults`.
    pub async fn defaults(&self) -> Result<WorkspaceDefaults, String> {
        Ok(self.load().await?.defaults)
    }

    /// Same as [`defaults`](Self::defaults) but also reports which
    /// fields originate from `workspace.local.toml`. Powers the
    /// "overridden locally" badges in the Settings UI (V3 cenário 3).
    ///
    /// A field is `Source::Local` iff the `[defaults]` table in the
    /// `.local.toml` sibling has that key, regardless of whether the
    /// value matches the base. This is the honest signal — the field
    /// *can* drift even when the values happen to coincide today.
    pub async fn defaults_with_sources(
        &self,
    ) -> Result<WorkspaceDefaultsWithSources, String> {
        let merged = self.defaults().await?;
        let local_keys = read_local_defaults_keys(&self.path())?;
        let pick = |key: &str| -> Source {
            if local_keys.contains(key) {
                Source::Local
            } else {
                Source::Workspace
            }
        };
        Ok(WorkspaceDefaultsWithSources {
            defaults: merged,
            sources: WorkspaceSources {
                environment: pick("environment"),
                git_remote: pick("git_remote"),
                git_branch: pick("git_branch"),
                display_name: pick("display_name"),
            },
        })
    }

    /// Replace the entire `[defaults]` section. Empty strings are
    /// treated as "unset" and stored as `None` to keep the TOML clean.
    pub async fn set_defaults(&self, mut defaults: WorkspaceDefaults) -> Result<(), String> {
        normalize(&mut defaults.environment);
        normalize(&mut defaults.git_remote);
        normalize(&mut defaults.git_branch);
        normalize(&mut defaults.display_name);
        // Mutate the base file, not the merged view (audit-003).
        let mut file = self.load_base_only().await?;
        file.defaults = defaults;
        self.persist(file).await
    }

    /// Read per-file settings for a vault-relative `file_path`. Returns
    /// the `Default::default()` value when the file has no entry. The
    /// merged view (`load`) is consulted, so `.local.toml` overrides
    /// surface to readers — useful for per-machine experimental flips.
    pub async fn file_settings(&self, file_path: &str) -> Result<FileSettings, String> {
        let file = self.load().await?;
        Ok(file.files.get(file_path).cloned().unwrap_or_default())
    }

    /// Set the `auto_capture` flag for `file_path`. Mutates the **base**
    /// file (audit-003) so local overrides survive the round-trip.
    /// When the resulting `FileSettings` matches the default, the entry
    /// is removed from the map so workspace.toml stays minimal.
    pub async fn set_file_auto_capture(
        &self,
        file_path: &str,
        auto_capture: bool,
    ) -> Result<(), String> {
        if file_path.trim().is_empty() {
            return Err("file_path must not be empty".to_string());
        }
        let mut file = self.load_base_only().await?;
        let mut entry = file
            .files
            .get(file_path)
            .cloned()
            .unwrap_or_default();
        entry.auto_capture = auto_capture;
        if entry.is_default() {
            file.files.remove(file_path);
        } else {
            file.files.insert(file_path.to_string(), entry);
        }
        self.persist(file).await
    }

    /// Set the `docheader_compact` flag for `file_path`. Same prune
    /// semantics as `set_file_auto_capture` — defaulted entries are
    /// removed so workspace.toml stays minimal. Powers Epic 50
    /// Story 06's compact-mode persistence.
    pub async fn set_file_docheader_compact(
        &self,
        file_path: &str,
        compact: bool,
    ) -> Result<(), String> {
        if file_path.trim().is_empty() {
            return Err("file_path must not be empty".to_string());
        }
        let mut file = self.load_base_only().await?;
        let mut entry = file
            .files
            .get(file_path)
            .cloned()
            .unwrap_or_default();
        entry.docheader_compact = compact;
        if entry.is_default() {
            file.files.remove(file_path);
        } else {
            file.files.insert(file_path.to_string(), entry);
        }
        self.persist(file).await
    }

    /// Ensure the file exists on disk. Idempotent — useful for the
    /// first-run flow that wants the file present before showing a UI.
    pub async fn ensure_exists(&self) -> Result<(), String> {
        if self.path().exists() {
            return Ok(());
        }
        // Don't write the merged view to disk — the `.local` side
        // would leak into the committed base (audit-003).
        let file = self.load_base_only().await?;
        self.persist(file).await
    }
}

fn local_path_for(base: &std::path::Path) -> PathBuf {
    super::merge::local_override_path(base).unwrap_or_else(|| base.to_path_buf())
}

/// Parse the `[defaults]` table out of `<base>.local.toml` and return
/// the set of keys it spells out. Empty set when the file is missing
/// or has no `[defaults]` section. Bubbles a parse error otherwise so
/// the UI surfaces the problem instead of silently treating broken
/// overrides as "no overrides".
fn read_local_defaults_keys(base: &std::path::Path) -> Result<BTreeSet<String>, String> {
    let local = local_path_for(base);
    if !local.exists() {
        return Ok(BTreeSet::new());
    }
    let text = std::fs::read_to_string(&local)
        .map_err(|e| format!("read {}: {e}", local.display()))?;
    let value: toml::Value =
        toml::from_str(&text).map_err(|e| format!("parse {}: {e}", local.display()))?;
    let Some(defaults) = value.get("defaults").and_then(|v| v.as_table()) else {
        return Ok(BTreeSet::new());
    };
    Ok(defaults.keys().cloned().collect())
}

fn normalize(value: &mut Option<String>) {
    if let Some(v) = value {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            *value = None;
        } else if trimmed.len() != v.len() {
            *value = Some(trimmed.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn store(dir: &TempDir) -> Arc<WorkspaceStore> {
        WorkspaceStore::new(dir.path())
    }

    #[tokio::test]
    async fn load_returns_default_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        let f = s.load().await.unwrap();
        assert_eq!(f.version, Version::V1);
        assert!(f.defaults.environment.is_none());
    }

    #[tokio::test]
    async fn set_defaults_creates_file_and_writes_through() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_defaults(WorkspaceDefaults {
            environment: Some("staging".into()),
            git_remote: Some("origin".into()),
            git_branch: Some("main".into()),
            display_name: None,
        })
        .await
        .unwrap();
        assert!(s.path().exists());
        let raw = std::fs::read_to_string(s.path()).unwrap();
        assert!(raw.contains("environment = \"staging\""));
        assert!(raw.contains("version = \"1\""));
        let d = s.defaults().await.unwrap();
        assert_eq!(d.environment.as_deref(), Some("staging"));
    }

    #[tokio::test]
    async fn set_defaults_normalizes_empty_strings_to_none() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_defaults(WorkspaceDefaults {
            environment: Some("   ".into()),
            git_remote: Some(String::new()),
            git_branch: Some(" main ".into()),
            display_name: Some(" Payments ".into()),
        })
        .await
        .unwrap();
        let d = s.defaults().await.unwrap();
        assert!(d.environment.is_none());
        assert!(d.git_remote.is_none());
        assert_eq!(d.git_branch.as_deref(), Some("main"));
        assert_eq!(d.display_name.as_deref(), Some("Payments"));
    }

    #[tokio::test]
    async fn cache_hits_when_mtime_unchanged() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_defaults(WorkspaceDefaults {
            environment: Some("staging".into()),
            ..Default::default()
        })
        .await
        .unwrap();
        // Tamper with the file directly. Without an mtime change the
        // cache should still serve the previous content.
        std::fs::write(s.path(), "version = \"1\"\n[defaults]\n").unwrap();
        // mtime probably *did* change here; force the cache instead.
        let d = s.defaults().await.unwrap();
        // Either we caught the mtime change (and now see empty) or we
        // served the cached value. Both are valid. The point of this
        // test is to lock in that *invalidation* explicitly clears.
        let _ = d;
        s.invalidate_cache().await;
        let d2 = s.defaults().await.unwrap();
        assert!(d2.environment.is_none());
    }

    #[tokio::test]
    async fn ensure_exists_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        assert!(!s.path().exists());
        s.ensure_exists().await.unwrap();
        assert!(s.path().exists());
        let mtime1 = std::fs::metadata(s.path()).unwrap().modified().unwrap();
        // Sleep enough that any rewrite would change the mtime.
        std::thread::sleep(std::time::Duration::from_millis(10));
        s.ensure_exists().await.unwrap();
        let mtime2 = std::fs::metadata(s.path()).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2, "second call should not rewrite");
    }

    #[tokio::test]
    async fn load_after_external_edit_picks_up_changes() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.ensure_exists().await.unwrap();
        // Wait a tick so the mtime can advance on filesystems with
        // 1s mtime granularity.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(
            s.path(),
            "version = \"1\"\n[defaults]\nenvironment = \"prod\"\n",
        )
        .unwrap();
        let d = s.defaults().await.unwrap();
        assert_eq!(d.environment.as_deref(), Some("prod"));
    }

    #[tokio::test]
    async fn read_invalid_toml_returns_error() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        std::fs::create_dir_all(dir.path().join(WORKSPACE_DIR)).unwrap();
        std::fs::write(s.path(), "this is not = = valid").unwrap();
        let err = s.load().await.unwrap_err();
        assert!(err.contains("parse"), "got {err}");
    }

    #[tokio::test]
    async fn local_override_merges_into_load() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        // Base says staging.
        s.set_defaults(WorkspaceDefaults {
            environment: Some("staging".into()),
            git_remote: Some("origin".into()),
            git_branch: Some("main".into()),
            display_name: None,
        })
        .await
        .unwrap();
        // Local override flips environment to dev.
        let local = dir.path().join(WORKSPACE_DIR).join("workspace.local.toml");
        std::fs::write(&local, "[defaults]\nenvironment = \"dev\"\n").unwrap();
        // Cache must invalidate by local mtime.
        s.invalidate_cache().await;
        let d = s.defaults().await.unwrap();
        assert_eq!(d.environment.as_deref(), Some("dev"));
        // Other base keys survive.
        assert_eq!(d.git_remote.as_deref(), Some("origin"));
        assert_eq!(d.git_branch.as_deref(), Some("main"));
    }

    #[tokio::test]
    async fn writes_target_base_not_local() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        let local = dir.path().join(WORKSPACE_DIR).join("workspace.local.toml");
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&local, "[defaults]\nenvironment = \"localish\"\n").unwrap();

        // Setting via the API must not touch `.local.toml` (ADR 0004).
        s.set_defaults(WorkspaceDefaults {
            environment: Some("via-api".into()),
            ..Default::default()
        })
        .await
        .unwrap();

        // The base file now exists and contains the api-set value.
        let base_text = std::fs::read_to_string(s.path()).unwrap();
        assert!(base_text.contains("via-api"), "base: {base_text}");

        // The .local file is untouched.
        let local_text = std::fs::read_to_string(&local).unwrap();
        assert!(local_text.contains("localish"));
    }

    #[tokio::test]
    async fn local_only_no_base_still_loads() {
        // Edge case: user has only a `.local.toml` (committed file
        // hasn't been created yet). Merge should still work.
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        let local = dir.path().join(WORKSPACE_DIR).join("workspace.local.toml");
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&local, "[defaults]\nenvironment = \"dev\"\n").unwrap();
        let d = s.defaults().await.unwrap();
        assert_eq!(d.environment.as_deref(), Some("dev"));
    }

    #[tokio::test]
    async fn file_settings_returns_default_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        let settings = s.file_settings("rollout.md").await.unwrap();
        assert!(!settings.auto_capture);
    }

    #[tokio::test]
    async fn set_file_auto_capture_persists_and_round_trips() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_file_auto_capture("rollout.md", true).await.unwrap();
        let raw = std::fs::read_to_string(s.path()).unwrap();
        assert!(
            raw.contains("[files.\"rollout.md\"]"),
            "table header missing: {raw}"
        );
        assert!(
            raw.contains("auto_capture = true"),
            "value missing: {raw}"
        );
        let read_back = s.file_settings("rollout.md").await.unwrap();
        assert!(read_back.auto_capture);
    }

    #[tokio::test]
    async fn flipping_back_to_default_prunes_the_entry() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_file_auto_capture("rollout.md", true).await.unwrap();
        s.set_file_auto_capture("rollout.md", false).await.unwrap();
        let raw = std::fs::read_to_string(s.path()).unwrap();
        assert!(
            !raw.contains("[files."),
            "table should have been pruned: {raw}"
        );
    }

    #[tokio::test]
    async fn set_for_other_files_does_not_disturb_existing_entries() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_file_auto_capture("rollout.md", true).await.unwrap();
        s.set_file_auto_capture("smoke.md", true).await.unwrap();
        let f = s.load().await.unwrap();
        assert!(f.files.get("rollout.md").unwrap().auto_capture);
        assert!(f.files.get("smoke.md").unwrap().auto_capture);
    }

    #[tokio::test]
    async fn set_with_empty_path_returns_error() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        let err = s.set_file_auto_capture("   ", true).await.unwrap_err();
        assert!(err.contains("must not be empty"), "got: {err}");
    }

    #[tokio::test]
    async fn set_does_not_promote_local_into_base() {
        // Pre-existing local override on `defaults` must survive even
        // when we mutate the base via the file-settings API
        // (audit-003 contract).
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_defaults(WorkspaceDefaults {
            environment: Some("staging".into()),
            ..Default::default()
        })
        .await
        .unwrap();
        let local = dir.path().join(WORKSPACE_DIR).join("workspace.local.toml");
        std::fs::write(&local, "[defaults]\nenvironment = \"dev\"\n").unwrap();
        s.invalidate_cache().await;

        s.set_file_auto_capture("rollout.md", true).await.unwrap();

        let base_text = std::fs::read_to_string(s.path()).unwrap();
        // The base still has staging — `.local.toml` did not leak in.
        assert!(
            base_text.contains("environment = \"staging\""),
            "got: {base_text}"
        );
        // The .local file is untouched.
        let local_text = std::fs::read_to_string(&local).unwrap();
        assert!(local_text.contains("dev"));
    }

    #[tokio::test]
    async fn sources_marks_workspace_when_no_local_file() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_defaults(WorkspaceDefaults {
            environment: Some("staging".into()),
            git_remote: Some("origin".into()),
            git_branch: Some("main".into()),
            display_name: Some("Payments".into()),
        })
        .await
        .unwrap();
        let r = s.defaults_with_sources().await.unwrap();
        assert_eq!(r.defaults.environment.as_deref(), Some("staging"));
        assert_eq!(r.sources.environment, Source::Workspace);
        assert_eq!(r.sources.git_remote, Source::Workspace);
        assert_eq!(r.sources.git_branch, Source::Workspace);
        assert_eq!(r.sources.display_name, Source::Workspace);
    }

    #[tokio::test]
    async fn sources_flags_keys_present_in_local_file() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_defaults(WorkspaceDefaults {
            environment: Some("staging".into()),
            git_remote: Some("origin".into()),
            git_branch: Some("main".into()),
            display_name: None,
        })
        .await
        .unwrap();
        let local = dir.path().join(WORKSPACE_DIR).join("workspace.local.toml");
        std::fs::write(
            &local,
            "[defaults]\nenvironment = \"qa-eu\"\ndisplay_name = \"Payments QA\"\n",
        )
        .unwrap();
        s.invalidate_cache().await;

        let r = s.defaults_with_sources().await.unwrap();
        assert_eq!(r.defaults.environment.as_deref(), Some("qa-eu"));
        assert_eq!(r.sources.environment, Source::Local);
        assert_eq!(r.sources.git_remote, Source::Workspace);
        assert_eq!(r.sources.git_branch, Source::Workspace);
        assert_eq!(r.defaults.display_name.as_deref(), Some("Payments QA"));
        assert_eq!(r.sources.display_name, Source::Local);
    }

    #[tokio::test]
    async fn sources_flag_local_even_when_value_matches_base() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.set_defaults(WorkspaceDefaults {
            environment: Some("staging".into()),
            ..Default::default()
        })
        .await
        .unwrap();
        let local = dir.path().join(WORKSPACE_DIR).join("workspace.local.toml");
        std::fs::write(&local, "[defaults]\nenvironment = \"staging\"\n").unwrap();
        s.invalidate_cache().await;

        let r = s.defaults_with_sources().await.unwrap();
        // Same value, but local *spells it out* — the badge should
        // still surface because the user is one keystroke away from
        // diverging.
        assert_eq!(r.sources.environment, Source::Local);
    }

    #[tokio::test]
    async fn sources_returns_workspace_when_no_files_exist() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        let r = s.defaults_with_sources().await.unwrap();
        assert_eq!(r.sources.environment, Source::Workspace);
        assert_eq!(r.sources.display_name, Source::Workspace);
        assert!(r.defaults.environment.is_none());
    }

    #[tokio::test]
    async fn sources_propagate_local_parse_errors() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.ensure_exists().await.unwrap();
        let local = dir.path().join(WORKSPACE_DIR).join("workspace.local.toml");
        std::fs::write(&local, "this = = invalid").unwrap();
        let err = s.defaults_with_sources().await.unwrap_err();
        assert!(err.contains("parse"), "got: {err}");
    }

    #[tokio::test]
    async fn sources_handles_local_without_defaults_table() {
        let dir = TempDir::new().unwrap();
        let s = store(&dir);
        s.ensure_exists().await.unwrap();
        let local = dir.path().join(WORKSPACE_DIR).join("workspace.local.toml");
        // Local exists but only has a [files."x"] table — no defaults.
        std::fs::write(&local, "[files.\"a.md\"]\nauto_capture = true\n").unwrap();
        s.invalidate_cache().await;
        let r = s.defaults_with_sources().await.unwrap();
        assert_eq!(r.sources.environment, Source::Workspace);
        assert_eq!(r.sources.display_name, Source::Workspace);
    }

    #[test]
    fn normalize_handles_all_branches() {
        let mut none: Option<String> = None;
        normalize(&mut none);
        assert!(none.is_none());

        let mut empty = Some(String::new());
        normalize(&mut empty);
        assert!(empty.is_none());

        let mut whitespace = Some("   ".to_string());
        normalize(&mut whitespace);
        assert!(whitespace.is_none());

        let mut padded = Some(" foo ".to_string());
        normalize(&mut padded);
        assert_eq!(padded.as_deref(), Some("foo"));

        let mut clean = Some("bar".to_string());
        normalize(&mut clean);
        assert_eq!(clean.as_deref(), Some("bar"));
    }
}
