//! File-backed environments store.
//!
//! Source of truth is `<vault_root>/envs/<name>.toml`. Each environment
//! lives in its own file with `[vars]` (literals OK) and `[secrets]`
//! (must be `{{...}}` references — see ADR 0001 / 0002). Active env
//! tracking lives in the per-machine `user.toml` so the same shared
//! vault can have different active envs on different machines.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use serde::Serialize;
use tokio::sync::RwLock;

use crate::db::keychain::{delete_secret, env_var_key, get_secret, store_secret};

use super::atomic::{read_toml, write_toml};
use super::envs::{EnvFile, EnvMeta};
use super::layout::ENVS_DIR;
use super::secret_resolver::{ensure_keychain_ref, resolve_value};
use super::user::UserFile;
use super::validate::{validate_env_file, Severity};
use super::Version;

// --- DTOs --------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentPublic {
    pub name: String,
    pub description: Option<String>,
    pub read_only: bool,
    pub require_confirm: bool,
    pub color: Option<String>,
    pub var_count: usize,
    pub secret_count: usize,
    /// Canvas §6 `[meta].temporary`. Drives the
    /// `temporary` chip in the Environments page.
    pub temporary: bool,
    /// Canvas §6 `[meta].connections_used` allowlist.
    /// Empty list means "all connections".
    pub connections_used: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvVariablePublic {
    pub key: String,
    /// Plaintext value when from `[vars]`; empty string when from
    /// `[secrets]` (caller resolves on demand).
    pub value: String,
    pub is_secret: bool,
}

#[derive(Debug, Clone)]
pub struct SetVarInput {
    pub env_name: String,
    pub key: String,
    /// Raw value. For secret vars it's stored in the keychain and the
    /// TOML keeps only a `{{keychain:...}}` reference.
    ///
    /// Empty + `is_secret=true` is a sentinel: "preserve the existing
    /// keychain entry" (same rule as connection passwords). It errors on
    /// create because there's nothing to preserve.
    pub value: String,
    pub is_secret: bool,
}

// --- Cache -------------------------------------------------------------------

/// Cache entry for one env file. Tracks both base and `.local`
/// override mtimes per ADR 0004.
#[derive(Debug, Clone)]
struct CachedEnv {
    base_mtime: Option<SystemTime>,
    local_mtime: Option<SystemTime>,
    file: EnvFile,
}

/// File-backed CRUD over `envs/*.toml` plus active-env tracking in
/// `user.toml`.
pub struct EnvironmentsStore {
    vault_root: PathBuf,
    user_config_path: PathBuf,
    /// Per-env cache. Key is the env name (== filename stem).
    cache: RwLock<BTreeMap<String, CachedEnv>>,
}

impl EnvironmentsStore {
    pub fn new(vault_root: impl Into<PathBuf>, user_config_path: impl Into<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            vault_root: vault_root.into(),
            user_config_path: user_config_path.into(),
            cache: RwLock::new(BTreeMap::new()),
        })
    }

    fn envs_dir(&self) -> PathBuf {
        self.vault_root.join(ENVS_DIR)
    }

    fn env_file_path(&self, name: &str) -> PathBuf {
        self.envs_dir().join(format!("{name}.toml"))
    }

    fn local_path_for(path: &Path) -> PathBuf {
        super::merge::local_override_path(path).unwrap_or_else(|| path.to_path_buf())
    }

    /// Load `<env>.toml` with `<env>.local.toml` overrides merged in
    /// (ADR 0004). Returns `Ok(None)` when **neither** file exists
    /// (env not created). Cache hits when both base and override
    /// mtimes are unchanged.
    async fn load_env(&self, name: &str) -> Result<Option<EnvFile>, String> {
        let path = self.env_file_path(name);
        let local = Self::local_path_for(&path);

        if !path.exists() && !local.exists() {
            return Ok(None);
        }
        let base_mtime = super::merge::mtime_or_none(&path);
        let local_mtime = super::merge::mtime_or_none(&local);

        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(name) {
                if cached.base_mtime == base_mtime && cached.local_mtime == local_mtime {
                    return Ok(Some(cached.file.clone()));
                }
            }
        }

        let (file, _local) = super::merge::load_with_local::<EnvFile>(&path)?;

        let mut cache = self.cache.write().await;
        cache.insert(
            name.to_string(),
            CachedEnv {
                base_mtime,
                local_mtime,
                file: file.clone(),
            },
        );
        Ok(Some(file))
    }

    /// Validate, write atomically, refresh cache.
    async fn persist_env(&self, name: &str, file: EnvFile) -> Result<(), String> {
        let report = validate_env_file(&file);
        if report.has_errors() {
            let summary = report
                .issues
                .iter()
                .filter(|i| i.severity == Severity::Error)
                .map(|i| format!("- {i}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(format!(
                "envs/{name}.toml refuses to save: validator found errors:\n{summary}"
            ));
        }
        let path = self.env_file_path(name);
        // ADR 0004: writes always target the base file.
        write_toml(&path, &file).map_err(|e| format!("write {}: {e}", path.display()))?;

        // Drop the cache entry; next `load_env(name)` re-reads + re-merges.
        let mut cache = self.cache.write().await;
        cache.remove(name);
        Ok(())
    }

    pub async fn invalidate_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }

    /// Read a single env's base file (no `.local` merge). Mutating
    /// paths use this so writes don't promote local overrides into
    /// the committed base. See audit-003. Returns `None` when the
    /// base file doesn't exist, even if a sibling `.local` does.
    async fn load_env_base_only(&self, name: &str) -> Result<Option<EnvFile>, String> {
        let path = self.env_file_path(name);
        if !path.exists() {
            return Ok(None);
        }
        super::atomic::read_toml::<EnvFile>(&path)
            .map(Some)
            .map_err(|e| format!("read {}: {e}", path.display()))
    }

    // --- env-level CRUD -------------------------------------------------

    pub async fn list_envs(&self) -> Result<Vec<EnvironmentPublic>, String> {
        let dir = self.envs_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let entries =
            std::fs::read_dir(&dir).map_err(|e| format!("read dir {}: {e}", dir.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            // Skip *.local.toml (overrides — handled separately, ADR 0004)
            // and any non-.toml file.
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            if name.ends_with(".local") {
                continue;
            }
            if let Some(env) = self.load_env(name).await? {
                out.push(env_to_public(name, &env));
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub async fn get_env(&self, name: &str) -> Result<Option<EnvironmentPublic>, String> {
        Ok(self.load_env(name).await?.map(|f| env_to_public(name, &f)))
    }

    pub async fn create_env(&self, name: &str) -> Result<EnvironmentPublic, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("environment name is required".to_string());
        }
        if !is_valid_env_name(name) {
            return Err(format!(
                "invalid environment name '{name}' — use letters, digits, '-', '_'"
            ));
        }
        if self.env_file_path(name).exists() {
            return Err(format!("environment '{name}' already exists"));
        }
        let file = EnvFile {
            version: Version::V1,
            vars: BTreeMap::new(),
            secrets: BTreeMap::new(),
            meta: EnvMeta::default(),
        };
        self.persist_env(name, file.clone()).await?;
        Ok(env_to_public(name, &file))
    }

    pub async fn delete_env(&self, name: &str) -> Result<(), String> {
        let path = self.env_file_path(name);
        if !path.exists() {
            return Err(format!("environment '{name}' not found"));
        }
        // Best-effort keychain cleanup for every secret in this env.
        if let Some(file) = self.load_env(name).await? {
            for key in file.secrets.keys() {
                let _ = delete_secret(&env_var_key(name, key));
            }
        }
        std::fs::remove_file(&path).map_err(|e| format!("delete {}: {e}", path.display()))?;
        // Drop the local override file too so a stale `<name>.local.toml`
        // doesn't resurrect the env on next load.
        let local = Self::local_path_for(&path);
        if local != path && local.exists() {
            let _ = std::fs::remove_file(&local);
        }
        let mut cache = self.cache.write().await;
        cache.remove(name);
        Ok(())
    }

    /// Rename `old_name` → `new_name`. Migrates the keychain entries
    /// for every secret (so users don't have to re-enter values) and
    /// preserves `<name>.local.toml` overrides under the new key. If
    /// the renamed env was the active one, the active pointer is
    /// updated atomically.
    pub async fn rename_env(&self, old_name: &str, new_name: &str) -> Result<(), String> {
        let new = new_name.trim();
        if new.is_empty() {
            return Err("environment name is required".to_string());
        }
        if !is_valid_env_name(new) {
            return Err(format!(
                "invalid environment name '{new}' — use letters, digits, '-', '_'"
            ));
        }
        if new == old_name {
            return Ok(());
        }
        let old_path = self.env_file_path(old_name);
        if !old_path.exists() {
            return Err(format!("environment '{old_name}' not found"));
        }
        let new_path = self.env_file_path(new);
        if new_path.exists() {
            return Err(format!("environment '{new}' already exists"));
        }
        // Migrate keychain entries first (most fragile step). Best-
        // effort: a missing/failing entry doesn't abort the rename
        // because the file move below is the user-visible side effect.
        if let Some(file) = self.load_env(old_name).await? {
            for key in file.secrets.keys() {
                let kc_old = env_var_key(old_name, key);
                let kc_new = env_var_key(new, key);
                if let Ok(Some(value)) = get_secret(&kc_old) {
                    let _ = store_secret(&kc_new, &value);
                    let _ = delete_secret(&kc_old);
                }
            }
        }
        std::fs::rename(&old_path, &new_path).map_err(|e| {
            format!(
                "rename {} → {}: {e}",
                old_path.display(),
                new_path.display()
            )
        })?;
        let old_local = Self::local_path_for(&old_path);
        let new_local = Self::local_path_for(&new_path);
        if old_local != old_path && old_local.exists() {
            std::fs::rename(&old_local, &new_local).map_err(|e| {
                format!(
                    "rename {} → {}: {e}",
                    old_local.display(),
                    new_local.display()
                )
            })?;
        }
        {
            let mut cache = self.cache.write().await;
            cache.remove(old_name);
            cache.remove(new);
        }
        if let Ok(Some(active)) = self.active_env().await {
            if active == old_name {
                self.set_active_env(Some(new)).await?;
            }
        }
        Ok(())
    }

    // --- variable-level CRUD --------------------------------------------

    pub async fn list_vars(&self, env_name: &str) -> Result<Vec<EnvVariablePublic>, String> {
        let Some(file) = self.load_env(env_name).await? else {
            return Err(format!("environment '{env_name}' not found"));
        };
        let mut out = Vec::new();
        for (key, value) in &file.vars {
            out.push(EnvVariablePublic {
                key: key.clone(),
                value: value.clone(),
                is_secret: false,
            });
        }
        for key in file.secrets.keys() {
            out.push(EnvVariablePublic {
                key: key.clone(),
                value: String::new(),
                is_secret: true,
            });
        }
        out.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(out)
    }

    /// Same shape as `list_vars`, but secrets are pre-resolved through
    /// the keychain. Use this for execution paths (HTTP/DB block ref
    /// resolution) — never for UI listing, where the value must stay
    /// masked.
    ///
    /// Best-effort per key: a missing or broken keychain entry leaves
    /// that secret with an empty value instead of failing the whole
    /// env, so the surrounding plain vars still resolve.
    pub async fn list_vars_resolved(
        &self,
        env_name: &str,
    ) -> Result<Vec<EnvVariablePublic>, String> {
        let Some(file) = self.load_env(env_name).await? else {
            return Err(format!("environment '{env_name}' not found"));
        };
        let mut out = Vec::new();
        for (key, value) in &file.vars {
            out.push(EnvVariablePublic {
                key: key.clone(),
                value: value.clone(),
                is_secret: false,
            });
        }
        for (key, reference) in &file.secrets {
            let value = resolve_value(reference).ok().flatten().unwrap_or_default();
            out.push(EnvVariablePublic {
                key: key.clone(),
                value,
                is_secret: true,
            });
        }
        out.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(out)
    }

    /// Resolve a var for actual execution. Secrets pass through the
    /// keychain. Plain `[vars]` come back verbatim.
    pub async fn resolve_var(&self, env_name: &str, key: &str) -> Result<Option<String>, String> {
        let Some(file) = self.load_env(env_name).await? else {
            return Ok(None);
        };
        if let Some(v) = file.vars.get(key) {
            return Ok(Some(v.clone()));
        }
        if let Some(reference) = file.secrets.get(key) {
            return resolve_value(reference).map_err(|e| format!("resolving secret '{key}': {e}"));
        }
        Ok(None)
    }

    pub async fn set_var(&self, input: SetVarInput) -> Result<EnvVariablePublic, String> {
        let SetVarInput {
            env_name,
            key,
            value,
            is_secret,
        } = input;
        let key = key.trim().to_string();
        if key.is_empty() {
            return Err("variable key is required".to_string());
        }
        // Mutate base only (audit-003). Existence check uses the
        // merged view above (load_env) implicitly: a local-only env
        // has no base file, so the load_env_base_only error fires.
        let mut file = self
            .load_env_base_only(&env_name)
            .await?
            .ok_or_else(|| format!("environment '{env_name}' not found"))?;

        if is_secret {
            // Empty value means "keep the existing keychain entry" — same rule
            // as connection passwords. The UI never exposes the secret in
            // edit forms, so any edit that only touches key/is_secret/etc.
            // submits value="". Treating that as a wipe would silently
            // destroy the secret on save.
            if value.is_empty() {
                if let Some(existing_ref) = file.secrets.get(&key).cloned() {
                    file.vars.remove(&key);
                    file.secrets.insert(key.clone(), existing_ref);
                } else {
                    return Err("value is required for new secret".to_string());
                }
            } else {
                // Move plaintext to keychain; write only the reference into
                // the TOML.
                let kc_key = env_var_key(&env_name, &key);
                let reference = ensure_keychain_ref(&kc_key, &value)?;
                // If a same-named non-secret existed, remove it.
                file.vars.remove(&key);
                file.secrets.insert(key.clone(), reference);
            }
        } else {
            // If a same-named secret existed, drop the keychain entry.
            if file.secrets.remove(&key).is_some() {
                let _ = delete_secret(&env_var_key(&env_name, &key));
            }
            file.vars.insert(key.clone(), value);
        }

        self.persist_env(&env_name, file).await?;
        Ok(EnvVariablePublic {
            key,
            value: String::new(),
            is_secret,
        })
    }

    pub async fn delete_var(&self, env_name: &str, key: &str) -> Result<(), String> {
        // Mutate base only (audit-003).
        let mut file = self
            .load_env_base_only(env_name)
            .await?
            .ok_or_else(|| format!("environment '{env_name}' not found"))?;
        let removed_secret = file.secrets.remove(key).is_some();
        let removed_var = file.vars.remove(key).is_some();
        if !removed_secret && !removed_var {
            return Err(format!(
                "variable '{key}' not found in environment '{env_name}'"
            ));
        }
        if removed_secret {
            let _ = delete_secret(&env_var_key(env_name, key));
        }
        self.persist_env(env_name, file).await?;
        Ok(())
    }

    // --- active-env tracking (per-machine) ------------------------------

    fn read_user_file(&self) -> Result<UserFile, String> {
        if !self.user_config_path.exists() {
            return Ok(UserFile::default());
        }
        read_toml::<UserFile>(&self.user_config_path)
            .map_err(|e| format!("read {}: {e}", self.user_config_path.display()))
    }

    fn write_user_file(&self, f: &UserFile) -> Result<(), String> {
        write_toml(&self.user_config_path, f)
            .map_err(|e| format!("write {}: {e}", self.user_config_path.display()))
    }

    fn vault_key(&self) -> String {
        // Canonical key for the active_envs map: absolute vault path.
        self.vault_root
            .canonicalize()
            .unwrap_or_else(|_| self.vault_root.clone())
            .to_string_lossy()
            .into_owned()
    }

    pub async fn active_env(&self) -> Result<Option<String>, String> {
        let user = self.read_user_file()?;
        Ok(user.active_envs.get(&self.vault_key()).cloned())
    }

    pub async fn set_active_env(&self, name: Option<&str>) -> Result<(), String> {
        let mut user = self.read_user_file()?;
        let key = self.vault_key();
        match name {
            Some(n) if !n.trim().is_empty() => {
                if !self.env_file_path(n).exists() {
                    return Err(format!("environment '{n}' not found"));
                }
                user.active_envs.insert(key, n.to_string());
            }
            _ => {
                user.active_envs.remove(&key);
            }
        }
        self.write_user_file(&user)
    }
}

// --- conversion helpers --------------------------------------------------

fn env_to_public(name: &str, file: &EnvFile) -> EnvironmentPublic {
    EnvironmentPublic {
        name: name.to_string(),
        description: file.meta.description.clone(),
        read_only: file.meta.read_only,
        require_confirm: file.meta.require_confirm,
        color: file.meta.color.clone(),
        var_count: file.vars.len(),
        secret_count: file.secrets.len(),
        temporary: file.meta.temporary,
        connections_used: file.meta.connections_used.clone(),
    }
}

fn is_valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
// Tests serialize keychain access via KEYCHAIN_TEST_LOCK; the std Mutex
// guard is intentionally held across awaits to keep concurrent test
// runs deterministic. The lock is contention-free in practice (each
// test holds it for milliseconds).
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use crate::db::keychain::KEYCHAIN_TEST_LOCK;
    use tempfile::TempDir;

    fn fresh_store() -> (Arc<EnvironmentsStore>, TempDir) {
        let tmp = TempDir::new().unwrap();
        let user_path = tmp.path().join("user.toml");
        let store = EnvironmentsStore::new(tmp.path(), user_path);
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
        assert!(store.list_envs().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn create_env_writes_file() {
        let (store, t) = fresh_store();
        let env = store.create_env("staging").await.unwrap();
        assert_eq!(env.name, "staging");
        assert_eq!(env.var_count, 0);
        assert!(t.path().join("envs/staging.toml").exists());

        let listed = store.list_envs().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "staging");
    }

    #[tokio::test]
    async fn create_env_rejects_duplicate() {
        let (store, _t) = fresh_store();
        let name = unique_name("dup");
        store.create_env(&name).await.unwrap();
        let err = store.create_env(&name).await.unwrap_err();
        assert!(err.contains("already exists"));
    }

    #[tokio::test]
    async fn create_env_rejects_invalid_name() {
        let (store, _t) = fresh_store();
        assert!(store.create_env("").await.is_err());
        assert!(store.create_env("   ").await.is_err());
        assert!(store.create_env("has spaces").await.is_err());
        assert!(store.create_env("../escape").await.is_err());
    }

    #[tokio::test]
    async fn set_plain_var_round_trip() {
        let (store, _t) = fresh_store();
        store.create_env("staging").await.unwrap();
        store
            .set_var(SetVarInput {
                env_name: "staging".into(),
                key: "BASE_URL".into(),
                value: "https://api.example.com".into(),
                is_secret: false,
            })
            .await
            .unwrap();

        let vars = store.list_vars("staging").await.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].key, "BASE_URL");
        assert!(!vars[0].is_secret);
        assert_eq!(vars[0].value, "https://api.example.com");
    }

    #[tokio::test]
    async fn set_secret_var_keeps_value_off_disk() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, t) = fresh_store();
        let env_name = unique_name("sec-env");
        store.create_env(&env_name).await.unwrap();
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "ADMIN_TOKEN".into(),
                value: "super-secret-value".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        let raw = std::fs::read_to_string(t.path().join(format!("envs/{env_name}.toml"))).unwrap();
        assert!(!raw.contains("super-secret-value"));
        assert!(raw.contains(&format!("{{{{keychain:env:{env_name}:ADMIN_TOKEN}}}}")));

        let vars = store.list_vars(&env_name).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert!(vars[0].is_secret);
        assert_eq!(vars[0].value, ""); // masked

        // Cleanup
        let _ = delete_secret(&env_var_key(&env_name, "ADMIN_TOKEN"));
    }

    #[tokio::test]
    async fn editing_secret_with_empty_value_preserves_keychain() {
        // Regression: V4 hotfix 2026-05-23. list_vars masks secrets as
        // value="", so any edit that only touches key/is_secret submits
        // value="" — previously that wiped the keychain entry on save.
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("edit-empty");
        store.create_env(&env_name).await.unwrap();

        // Seed a real secret.
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "API_TOKEN".into(),
                value: "real-secret-42".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        // Simulate the UI re-saving with an empty value (form was opened
        // for edit; list_vars never gave the real value to the form).
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "API_TOKEN".into(),
                value: String::new(),
                is_secret: true,
            })
            .await
            .unwrap();

        // resolve_var must still return the original value.
        let resolved = store
            .resolve_var(&env_name, "API_TOKEN")
            .await
            .unwrap()
            .expect("secret must still resolve");
        assert_eq!(resolved, "real-secret-42");

        // Cleanup
        let _ = delete_secret(&env_var_key(&env_name, "API_TOKEN"));
    }

    #[tokio::test]
    async fn creating_secret_with_empty_value_errors() {
        // Companion to the preserve test: empty + is_secret on a brand-new
        // key has nothing to preserve, so it must error instead of
        // silently writing an empty-string secret.
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("create-empty");
        store.create_env(&env_name).await.unwrap();

        let err = store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "BRAND_NEW".into(),
                value: String::new(),
                is_secret: true,
            })
            .await
            .unwrap_err();
        assert!(err.contains("value is required"));

        // And the env file must not have a `BRAND_NEW` entry anywhere.
        let vars = store.list_vars(&env_name).await.unwrap();
        assert!(vars.iter().all(|v| v.key != "BRAND_NEW"));
    }

    #[tokio::test]
    async fn list_vars_resolved_returns_real_secret_value() {
        // Regression: list_vars masks secrets with value="", so any
        // caller that fed `list_vars` into a ref-resolution map (HTTP/DB
        // block dispatch) was silently sending the empty string for
        // {{SECRET_KEY}}. list_vars_resolved must hit the keychain.
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("resolved");
        store.create_env(&env_name).await.unwrap();

        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "TOKEN".into(),
                value: "real-42".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        let vars = store.list_vars_resolved(&env_name).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].key, "TOKEN");
        assert!(vars[0].is_secret);
        assert_eq!(vars[0].value, "real-42");

        let _ = delete_secret(&env_var_key(&env_name, "TOKEN"));
    }

    #[tokio::test]
    async fn list_vars_resolved_mixes_plain_and_secret() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("mixed");
        store.create_env(&env_name).await.unwrap();

        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "BASE_URL".into(),
                value: "https://api.example.com".into(),
                is_secret: false,
            })
            .await
            .unwrap();
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "TOKEN".into(),
                value: "secret-abc".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        let vars = store.list_vars_resolved(&env_name).await.unwrap();
        let by_key: std::collections::HashMap<_, _> =
            vars.into_iter().map(|v| (v.key, (v.value, v.is_secret))).collect();
        assert_eq!(
            by_key["BASE_URL"],
            ("https://api.example.com".into(), false)
        );
        assert_eq!(by_key["TOKEN"], ("secret-abc".into(), true));

        let _ = delete_secret(&env_var_key(&env_name, "TOKEN"));
    }

    #[tokio::test]
    async fn list_vars_resolved_keeps_plain_when_secret_missing() {
        // A keychain entry can go missing (machine swap, manual purge,
        // backend hiccup). One broken secret must not blank-out every
        // other variable in the env.
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("missing");
        store.create_env(&env_name).await.unwrap();

        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "BASE_URL".into(),
                value: "https://api.example.com".into(),
                is_secret: false,
            })
            .await
            .unwrap();
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "TOKEN".into(),
                value: "stored".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        // Yank the keychain entry behind the reference; the TOML still
        // has the {{keychain:…}} ref.
        let _ = delete_secret(&env_var_key(&env_name, "TOKEN"));

        let vars = store.list_vars_resolved(&env_name).await.unwrap();
        let by_key: std::collections::HashMap<_, _> =
            vars.into_iter().map(|v| (v.key, v.value)).collect();
        assert_eq!(by_key["BASE_URL"], "https://api.example.com");
        assert_eq!(by_key["TOKEN"], ""); // best-effort fallback
    }

    #[tokio::test]
    async fn editing_secret_with_new_value_overwrites_keychain() {
        // Make sure the "empty preserves" path doesn't block legitimate
        // rotations: re-saving with a non-empty value still rewrites the
        // keychain entry to the new secret.
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("rotate");
        store.create_env(&env_name).await.unwrap();

        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "ROT".into(),
                value: "v1".into(),
                is_secret: true,
            })
            .await
            .unwrap();
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "ROT".into(),
                value: "v2".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        let resolved = store
            .resolve_var(&env_name, "ROT")
            .await
            .unwrap()
            .expect("rotated secret must resolve");
        assert_eq!(resolved, "v2");

        let _ = delete_secret(&env_var_key(&env_name, "ROT"));
    }

    #[tokio::test]
    async fn switching_var_kind_clears_old_storage() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("switch");
        store.create_env(&env_name).await.unwrap();

        // Start as plain var
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "TOKEN".into(),
                value: "plain".into(),
                is_secret: false,
            })
            .await
            .unwrap();

        // Promote to secret — old plain entry must be gone from [vars]
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "TOKEN".into(),
                value: "secret-now".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        let vars = store.list_vars(&env_name).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert!(vars[0].is_secret);

        // Demote back to plain — keychain entry must be deleted
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "TOKEN".into(),
                value: "back-to-plain".into(),
                is_secret: false,
            })
            .await
            .unwrap();
        let vars = store.list_vars(&env_name).await.unwrap();
        assert!(!vars[0].is_secret);
    }

    #[tokio::test]
    async fn delete_var_removes_from_correct_section() {
        let (store, _t) = fresh_store();
        let env_name = unique_name("delvar");
        store.create_env(&env_name).await.unwrap();
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "PUBLIC".into(),
                value: "x".into(),
                is_secret: false,
            })
            .await
            .unwrap();

        store.delete_var(&env_name, "PUBLIC").await.unwrap();
        assert!(store.list_vars(&env_name).await.unwrap().is_empty());

        let err = store.delete_var(&env_name, "NOPE").await.unwrap_err();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn delete_env_removes_file_and_clears_cache() {
        let (store, t) = fresh_store();
        let env_name = unique_name("delenv");
        store.create_env(&env_name).await.unwrap();
        let path = t.path().join(format!("envs/{env_name}.toml"));
        assert!(path.exists());

        store.delete_env(&env_name).await.unwrap();
        assert!(!path.exists());
        assert!(store.get_env(&env_name).await.unwrap().is_none());

        let err = store.delete_env(&env_name).await.unwrap_err();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn rename_env_moves_file_and_preserves_vars() {
        let (store, t) = fresh_store();
        let old_name = unique_name("renold");
        let new_name = unique_name("rennew");
        store.create_env(&old_name).await.unwrap();
        store
            .set_var(SetVarInput {
                env_name: old_name.clone(),
                key: "API_BASE".into(),
                value: "x".into(),
                is_secret: false,
            })
            .await
            .unwrap();

        store.rename_env(&old_name, &new_name).await.unwrap();

        assert!(!t.path().join(format!("envs/{old_name}.toml")).exists());
        assert!(t.path().join(format!("envs/{new_name}.toml")).exists());

        let vars = store.list_vars(&new_name).await.unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].key, "API_BASE");
        assert_eq!(vars[0].value, "x");
    }

    #[tokio::test]
    async fn rename_env_rejects_existing_target_and_invalid_name() {
        let (store, _t) = fresh_store();
        let a = unique_name("rena");
        let b = unique_name("renb");
        store.create_env(&a).await.unwrap();
        store.create_env(&b).await.unwrap();

        let err = store.rename_env(&a, &b).await.unwrap_err();
        assert!(err.contains("already exists"));

        let err = store.rename_env(&a, "has spaces").await.unwrap_err();
        assert!(err.contains("invalid environment name"));

        let err = store.rename_env(&a, "").await.unwrap_err();
        assert!(err.contains("required"));
    }

    #[tokio::test]
    async fn rename_env_updates_active_pointer_when_renaming_active() {
        let (store, _t) = fresh_store();
        let old_name = unique_name("renactold");
        let new_name = unique_name("renactnew");
        store.create_env(&old_name).await.unwrap();
        store.set_active_env(Some(&old_name)).await.unwrap();

        store.rename_env(&old_name, &new_name).await.unwrap();

        assert_eq!(
            store.active_env().await.unwrap().as_deref(),
            Some(new_name.as_str())
        );
    }

    #[tokio::test]
    async fn rename_env_no_op_when_same_name() {
        let (store, _t) = fresh_store();
        let name = unique_name("renoop");
        store.create_env(&name).await.unwrap();
        store.rename_env(&name, &name).await.unwrap();
        assert!(store.get_env(&name).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn active_env_round_trip() {
        let (store, _t) = fresh_store();
        store.create_env("staging").await.unwrap();
        assert!(store.active_env().await.unwrap().is_none());

        store.set_active_env(Some("staging")).await.unwrap();
        assert_eq!(
            store.active_env().await.unwrap().as_deref(),
            Some("staging")
        );

        store.set_active_env(None).await.unwrap();
        assert!(store.active_env().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn set_active_rejects_unknown_env() {
        let (store, _t) = fresh_store();
        let err = store
            .set_active_env(Some("does-not-exist"))
            .await
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn list_envs_skips_local_overrides() {
        let (store, t) = fresh_store();
        store.create_env("staging").await.unwrap();
        // Manually drop a staging.local.toml — it should not appear in list.
        std::fs::write(
            t.path().join("envs/staging.local.toml"),
            r#"version = "1"
[vars]
BASE_URL = "http://localhost"
"#,
        )
        .unwrap();
        let envs = store.list_envs().await.unwrap();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].name, "staging");
    }

    #[tokio::test]
    async fn resolve_var_returns_plain_value() {
        let (store, _t) = fresh_store();
        store.create_env("staging").await.unwrap();
        store
            .set_var(SetVarInput {
                env_name: "staging".into(),
                key: "HOST".into(),
                value: "api.example.com".into(),
                is_secret: false,
            })
            .await
            .unwrap();
        let resolved = store.resolve_var("staging", "HOST").await.unwrap();
        assert_eq!(resolved.as_deref(), Some("api.example.com"));

        // Missing key
        assert!(store
            .resolve_var("staging", "MISSING")
            .await
            .unwrap()
            .is_none());
        // Missing env
        assert!(store.resolve_var("nope", "X").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn local_override_merges_into_env() {
        let (store, t) = fresh_store();
        store.create_env("staging").await.unwrap();
        store
            .set_var(SetVarInput {
                env_name: "staging".into(),
                key: "BASE_URL".into(),
                value: "https://api.staging.example.com".into(),
                is_secret: false,
            })
            .await
            .unwrap();
        store
            .set_var(SetVarInput {
                env_name: "staging".into(),
                key: "TENANT".into(),
                value: "tnt_8f2a91".into(),
                is_secret: false,
            })
            .await
            .unwrap();

        // User drops a `.local.toml` overriding BASE_URL only.
        let local = t.path().join("envs/staging.local.toml");
        std::fs::write(
            &local,
            "version = \"1\"\n[vars]\nBASE_URL = \"http://localhost:8080\"\n",
        )
        .unwrap();
        store.invalidate_cache().await;

        // BASE_URL is overridden, TENANT survives from the base.
        let resolved = store.resolve_var("staging", "BASE_URL").await.unwrap();
        assert_eq!(resolved.as_deref(), Some("http://localhost:8080"));
        let tenant = store.resolve_var("staging", "TENANT").await.unwrap();
        assert_eq!(tenant.as_deref(), Some("tnt_8f2a91"));
    }

    #[tokio::test]
    async fn local_only_env_with_no_base_loads() {
        // Edge: user dropped `staging.local.toml` without a committed
        // `staging.toml` yet. ADR 0004 says merge still works.
        let (store, t) = fresh_store();
        std::fs::create_dir_all(t.path().join("envs")).unwrap();
        std::fs::write(
            t.path().join("envs/staging.local.toml"),
            "version = \"1\"\n[vars]\nA = \"1\"\n",
        )
        .unwrap();
        let resolved = store.resolve_var("staging", "A").await.unwrap();
        assert_eq!(resolved.as_deref(), Some("1"));
    }

    #[tokio::test]
    async fn resolve_var_secret_round_trips_through_keychain() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("resolve-sec");
        store.create_env(&env_name).await.unwrap();
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "ADMIN_TOKEN".into(),
                value: "real-secret-value".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        let resolved = store.resolve_var(&env_name, "ADMIN_TOKEN").await.unwrap();
        assert_eq!(resolved.as_deref(), Some("real-secret-value"));

        // Cleanup
        let _ = delete_secret(&env_var_key(&env_name, "ADMIN_TOKEN"));
    }

    #[tokio::test]
    async fn resolve_var_secret_errors_when_keychain_entry_missing() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("missing-sec");
        store.create_env(&env_name).await.unwrap();
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "ORPHAN".into(),
                value: "stored-then-deleted".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        // Delete the keychain entry behind the reference; the TOML
        // still has the {{keychain:...}} reference.
        let _ = delete_secret(&env_var_key(&env_name, "ORPHAN"));

        let err = store
            .resolve_var(&env_name, "ORPHAN")
            .await
            .expect_err("must surface missing-keychain as error");
        assert!(err.contains("ORPHAN"), "error should name the var: {err}");
    }

    #[tokio::test]
    async fn delete_var_secret_clears_keychain_entry() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("delsec");
        store.create_env(&env_name).await.unwrap();
        store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "TOKEN".into(),
                value: "kept-in-keychain".into(),
                is_secret: true,
            })
            .await
            .unwrap();

        store.delete_var(&env_name, "TOKEN").await.unwrap();

        // After delete, the var is gone AND the keychain entry is gone.
        assert!(store.list_vars(&env_name).await.unwrap().is_empty());
        assert!(
            crate::db::keychain::get_secret(&env_var_key(&env_name, "TOKEN"))
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn delete_env_clears_keychain_for_every_secret() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let (store, _t) = fresh_store();
        let env_name = unique_name("delenv-sec");
        store.create_env(&env_name).await.unwrap();
        for key in ["TOKEN_A", "TOKEN_B"] {
            store
                .set_var(SetVarInput {
                    env_name: env_name.clone(),
                    key: key.into(),
                    value: format!("value-of-{key}"),
                    is_secret: true,
                })
                .await
                .unwrap();
        }

        store.delete_env(&env_name).await.unwrap();

        // Every keychain entry from the deleted env is gone.
        for key in ["TOKEN_A", "TOKEN_B"] {
            assert!(
                crate::db::keychain::get_secret(&env_var_key(&env_name, key))
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[tokio::test]
    async fn set_var_empty_key_errors() {
        let (store, _t) = fresh_store();
        let env_name = unique_name("empty-key");
        store.create_env(&env_name).await.unwrap();
        let err = store
            .set_var(SetVarInput {
                env_name: env_name.clone(),
                key: "   ".into(),
                value: "x".into(),
                is_secret: false,
            })
            .await
            .unwrap_err();
        assert!(err.contains("variable key"));
    }

    #[tokio::test]
    async fn set_var_in_missing_env_errors() {
        let (store, _t) = fresh_store();
        let err = store
            .set_var(SetVarInput {
                env_name: "no-such-env".into(),
                key: "X".into(),
                value: "y".into(),
                is_secret: false,
            })
            .await
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn set_active_env_empty_string_clears() {
        let (store, _t) = fresh_store();
        store.create_env("staging").await.unwrap();
        store.set_active_env(Some("staging")).await.unwrap();
        assert_eq!(
            store.active_env().await.unwrap().as_deref(),
            Some("staging")
        );
        // Empty / whitespace-only string clears active.
        store.set_active_env(Some("   ")).await.unwrap();
        assert!(store.active_env().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_envs_skips_dot_local_files() {
        let (store, t) = fresh_store();
        std::fs::create_dir_all(t.path().join("envs")).unwrap();
        std::fs::write(
            t.path().join("envs/staging.toml"),
            "version = \"1\"\n[vars]\nA = \"1\"\n",
        )
        .unwrap();
        std::fs::write(
            t.path().join("envs/staging.local.toml"),
            "version = \"1\"\n[vars]\nA = \"local\"\n",
        )
        .unwrap();
        let envs = store.list_envs().await.unwrap();
        assert_eq!(
            envs.len(),
            1,
            "list must not surface .local as separate env"
        );
        assert_eq!(envs[0].name, "staging");
    }
}
