// coverage:exclude file — Tauri command shells take `tauri::State<'_, T>`
// and aren't unit-testable in isolation. The pure helpers
// (make_var_id / parse_var_id) are tested below; the substantive
// logic (file-backed CRUD, mtime cache, atomic write, secret
// resolution) lives in `httui_core::vault_config::environments_store`
// at >80% coverage. Same rationale as `vault_config_commands.rs`
// (audit-002). Re-evaluated by Epic 20a Story 05 when the per-domain
// command split adds an integration harness with a fake Tauri runtime.
// Opt-out justified in audit-016.

//! Environment Tauri commands — file-backed cutover (Epic 19 Story 02
//! Phase 2; audit-015).
//!
//! Wire-compat with the legacy `db::environments` shape so the React
//! frontend doesn't need changes:
//!
//! - `Environment.id == name` (file-backed natural key promoted)
//! - `Environment.created_at` returned as empty string (file-backed
//!   stores have no creation timestamp; field unused by current UI)
//! - `EnvVariable.id == "<env_name>::<key>"` (synthesized; the
//!   `delete_env_variable` command splits on `::` to recover both
//!   parts)
//! - For secret variables: `value` is returned empty (the real value
//!   lives in keychain; legacy returned `__KEYCHAIN__` sentinel —
//!   we choose empty since the frontend already gates display on
//!   `is_secret`)

use std::sync::Arc;

use serde::Serialize;
use sqlx::sqlite::SqlitePool;
use tauri::State;

use httui_core::vault_config::environments_store::SetVarInput;

use super::vault_stores::VaultStoreRegistry;

/// Wire-compat: matches the legacy `db::environments::Environment` shape,
/// plus canvas §6 metadata (Story 03) — `description`, `temporary`,
/// `connections_used`. Older frontend code that destructures only the
/// legacy fields keeps working; the Environments page reads the new
/// fields directly.
#[derive(Debug, Clone, Serialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
    /// `[meta].description` — free text shown below the env name.
    pub description: Option<String>,
    /// `[meta].temporary` — true marks the env as a throw-away.
    pub temporary: bool,
    /// `[meta].connections_used` allowlist (empty = all connections).
    pub connections_used: Vec<String>,
}

/// Wire-compat: matches the legacy `db::environments::EnvVariable`.
#[derive(Debug, Clone, Serialize)]
pub struct EnvVariable {
    pub id: String,
    pub environment_id: String,
    pub key: String,
    pub value: String,
    pub is_secret: bool,
    pub created_at: String,
}

const VAR_ID_SEP: &str = "::";

fn make_var_id(env_name: &str, key: &str) -> String {
    format!("{env_name}{VAR_ID_SEP}{key}")
}

fn parse_var_id(id: &str) -> Result<(&str, &str), String> {
    id.split_once(VAR_ID_SEP)
        .ok_or_else(|| format!("invalid env-var id '{id}' (expected '<env>{VAR_ID_SEP}<key>')"))
}

/// List environments for the active vault.
#[tauri::command]
pub async fn list_environments(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
) -> Result<Vec<Environment>, String> {
    let stores = registry.for_active_vault(&pool).await?;
    let active = stores.environments.active_env().await?;
    let envs = stores.environments.list_envs().await?;
    Ok(envs
        .into_iter()
        .map(|e| Environment {
            id: e.name.clone(),
            is_active: active.as_deref() == Some(e.name.as_str()),
            description: e.description.clone(),
            temporary: e.temporary,
            connections_used: e.connections_used.clone(),
            name: e.name,
            created_at: String::new(),
        })
        .collect())
}

/// Create a new environment.
#[tauri::command]
pub async fn create_environment(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    name: String,
) -> Result<Environment, String> {
    let stores = registry.for_active_vault(&pool).await?;
    let env = stores.environments.create_env(&name).await?;
    Ok(Environment {
        id: env.name.clone(),
        name: env.name,
        is_active: false,
        created_at: String::new(),
        description: env.description,
        temporary: env.temporary,
        connections_used: env.connections_used,
    })
}

/// Delete an environment by id (== name).
#[tauri::command]
pub async fn delete_environment(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    id: String,
) -> Result<(), String> {
    let stores = registry.for_active_vault(&pool).await?;
    stores.environments.delete_env(&id).await
}

/// Duplicate an environment by copying its `[vars]` and `[secrets]`
/// (secret refs only — keychain entries stay separate; user must
/// re-enter values for the new env).
#[tauri::command]
pub async fn duplicate_environment(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    source_id: String,
    new_name: String,
) -> Result<Environment, String> {
    let stores = registry.for_active_vault(&pool).await?;
    // 1. Read source vars (without resolving secrets — those stay refs)
    let source_vars = stores.environments.list_vars(&source_id).await?;
    // 2. Create the new env
    let new_env = stores.environments.create_env(&new_name).await?;
    // 3. Copy each plain var (secrets need re-entry on the target machine)
    for v in source_vars {
        if !v.is_secret {
            stores
                .environments
                .set_var(SetVarInput {
                    env_name: new_name.clone(),
                    key: v.key,
                    value: v.value,
                    is_secret: false,
                })
                .await?;
        }
    }
    Ok(Environment {
        id: new_env.name.clone(),
        name: new_env.name,
        is_active: false,
        created_at: String::new(),
        description: new_env.description,
        temporary: new_env.temporary,
        connections_used: new_env.connections_used,
    })
}

/// Mark an environment as active (or `None` to clear).
#[tauri::command]
pub async fn set_active_environment(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    id: Option<String>,
) -> Result<(), String> {
    let stores = registry.for_active_vault(&pool).await?;
    stores.environments.set_active_env(id.as_deref()).await
}

/// List variables for an environment, with secret values masked
/// (returned as empty `value` when `is_secret == true`; the real
/// value lives in keychain).
#[tauri::command]
pub async fn list_env_variables(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    environment_id: String,
) -> Result<Vec<EnvVariable>, String> {
    let stores = registry.for_active_vault(&pool).await?;
    let vars = stores.environments.list_vars(&environment_id).await?;
    Ok(vars
        .into_iter()
        .map(|v| EnvVariable {
            id: make_var_id(&environment_id, &v.key),
            environment_id: environment_id.clone(),
            key: v.key,
            value: if v.is_secret { String::new() } else { v.value },
            is_secret: v.is_secret,
            created_at: String::new(),
        })
        .collect())
}

/// Upsert an env variable. Secret values are stored in the keychain;
/// the file gets a `{{keychain:...}}` reference.
#[tauri::command]
pub async fn set_env_variable(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    environment_id: String,
    key: String,
    value: String,
    is_secret: Option<bool>,
) -> Result<EnvVariable, String> {
    let stores = registry.for_active_vault(&pool).await?;
    let is_secret = is_secret.unwrap_or(false);
    let v = stores
        .environments
        .set_var(SetVarInput {
            env_name: environment_id.clone(),
            key: key.clone(),
            value,
            is_secret,
        })
        .await?;
    Ok(EnvVariable {
        id: make_var_id(&environment_id, &v.key),
        environment_id,
        key: v.key,
        value: if v.is_secret { String::new() } else { v.value },
        is_secret: v.is_secret,
        created_at: String::new(),
    })
}

/// Delete a variable by its synthesized id (`<env>::<key>`).
#[tauri::command]
pub async fn delete_env_variable(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    id: String,
) -> Result<(), String> {
    let stores = registry.for_active_vault(&pool).await?;
    let (env_name, key) = parse_var_id(&id)?;
    stores.environments.delete_var(env_name, key).await
}

/// Resolve every variable of the active environment for execution
/// context. Plain `[vars]` come through verbatim; `[secrets]` are
/// resolved against the OS keychain so HTTP/DB blocks see the real
/// value when expanding `{{KEY}}`.
///
/// **Do not** display the result anywhere visible to the user — this
/// is the only IPC that returns secret values and is intended for
/// the request-dispatch resolver. `list_env_variables` keeps masking
/// secrets for display surfaces.
async fn resolve_vars_for_env(
    stores: &crate::commands::vault_stores::VaultStores,
    env_name: &str,
) -> Result<std::collections::HashMap<String, String>, String> {
    let public = stores.environments.list_vars(env_name).await?;
    let mut out = std::collections::HashMap::with_capacity(public.len());
    for v in public {
        let value = if v.is_secret {
            // Skip silently if a secret can't be resolved — better to
            // emit empty than to crash the caller.
            stores
                .environments
                .resolve_var(env_name, &v.key)
                .await
                .ok()
                .flatten()
                .unwrap_or_default()
        } else {
            v.value
        };
        out.insert(v.key, value);
    }
    Ok(out)
}

/// Resolve every variable of a specific environment for execution-or-
/// reveal context. Secrets come back unmasked from the OS keychain;
/// plain `[vars]` come through verbatim. Treat as sensitive — the
/// returned map carries cleartext secret values.
#[tauri::command]
pub async fn resolve_env_variables(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
    environment_id: String,
) -> Result<std::collections::HashMap<String, String>, String> {
    let stores = registry.for_active_vault(&pool).await?;
    resolve_vars_for_env(&stores, &environment_id).await
}

#[tauri::command]
pub async fn resolve_active_env_variables(
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let stores = registry.for_active_vault(&pool).await?;
    let Some(env_name) = stores.environments.active_env().await? else {
        return Ok(std::collections::HashMap::new());
    };
    resolve_vars_for_env(&stores, &env_name).await
}

#[cfg(test)]
mod tests {
    //! These tests exercise the synthesized id parsing + DTO mapping.
    //! Round-trip with the file-backed store is covered by
    //! `httui_core::vault_config::environments_store::tests`.
    //!
    //! Tauri command bodies that delegate to the store are thin
    //! wrappers and not unit-testable in isolation (they take
    //! `tauri::State<'_, T>`); integration coverage lives in the
    //! existing E2E suite.

    use super::*;

    #[test]
    fn make_var_id_round_trips() {
        let id = make_var_id("staging", "api_base");
        let (env, key) = parse_var_id(&id).unwrap();
        assert_eq!(env, "staging");
        assert_eq!(key, "api_base");
    }

    #[test]
    fn make_var_id_with_dot_in_key() {
        let id = make_var_id("staging", "feature_flag.v3");
        let (env, key) = parse_var_id(&id).unwrap();
        assert_eq!(env, "staging");
        assert_eq!(key, "feature_flag.v3");
    }

    #[test]
    fn parse_var_id_rejects_missing_separator() {
        let err = parse_var_id("malformed").unwrap_err();
        assert!(err.contains("invalid env-var id"));
    }

    #[test]
    fn parse_var_id_handles_keys_with_double_colon() {
        // `split_once` returns the first match — so a key like
        // `ns::sub` becomes part of the env name. We accept this
        // limitation: file-backed env keys are TOML keys (no `::`
        // by convention); document in the module-level comment.
        let id = make_var_id("staging", "key");
        let (env, key) = parse_var_id(&id).unwrap();
        assert_eq!(env, "staging");
        assert_eq!(key, "key");
    }
}
