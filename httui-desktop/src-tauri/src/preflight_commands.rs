//! Tauri command wrapping `httui_core::preflight`.
//!
//! Context wiring: `connection` from `ConnectionsStore`, `env_var` keys from
//! `EnvironmentsStore`, `branch` from `git rev-parse`, `file_exists`/`command`
//! resolved against FS + PATH.
//!
//! `keychain` was retired — legacy YAML falls through to `PreflightItem::Unknown`
//! and renders as a skip pill (non-breaking).

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sqlx::SqlitePool;
use tauri::State;

use httui_core::frontmatter::split_frontmatter;
use httui_core::git::current_branch;
use httui_core::preflight::{
    evaluate_preflight_with_io, parse_preflight, CheckResult, EvaluationContext, PreflightItem,
};
use serde::Serialize;

use crate::commands::vault_stores::VaultStoreRegistry;

#[derive(Debug, Clone, Serialize)]
pub struct EvaluatedPreflightItem {
    /// `connection` / `env_var` / `branch` / `file_exists` /
    /// `command` / `unknown` — matches the Rust enum tag for per-kind
    /// UI hints (suggestion text, click-to-fix).
    pub kind: String,
    /// Short text rendered inside the pill (the item's name / path /
    /// command — whichever makes most sense at a glance).
    pub label: String,
    pub result: CheckResult,
}

/// Pure helper: given a `file_path` (vault-relative or absolute), a
/// `vault_path`, and the resolved environment context, produce one
/// `EvaluatedPreflightItem` per declared check. Returns an empty Vec
/// when the file can't be read, the frontmatter is missing, or there
/// is no `preflight:` section — the pill row treats "nothing to
/// check" the same as "no chips to render".
///
/// Extracted from the Tauri command so the tests can exercise the
/// evaluator paths without standing up a Tauri State container. The
/// async command wraps this by fetching the context from the active
/// vault's stores + git.
pub fn evaluate_preflight_for_paths(
    file_path: &str,
    vault_path: &str,
    branch: Option<&str>,
    active_env_vars: &HashSet<String>,
    connections: &HashSet<String>,
) -> Vec<EvaluatedPreflightItem> {
    let abs_path: PathBuf = if Path::new(file_path).is_absolute() {
        PathBuf::from(file_path)
    } else {
        PathBuf::from(vault_path).join(file_path)
    };
    let content = match fs::read_to_string(&abs_path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let raw_yaml = match split_frontmatter(&content) {
        Some(split) => split.raw_yaml,
        None => return Vec::new(),
    };
    let items = parse_preflight(&raw_yaml);
    if items.is_empty() {
        return Vec::new();
    }
    let ctx = EvaluationContext {
        branch,
        active_env_vars,
        connections,
    };
    let vault_root = PathBuf::from(vault_path);
    let results = evaluate_preflight_with_io(&items, &ctx, &vault_root);
    items
        .iter()
        .zip(results)
        .map(|(item, result)| EvaluatedPreflightItem {
            kind: kind_tag(item),
            label: label_for(item),
            result,
        })
        .collect()
}

/// Tauri entry point. Fetches the active vault's context (connections,
/// env-var keys, current branch) and delegates to
/// `evaluate_preflight_for_paths`. Individual context fetches that
/// error out fall back to empty sets — the eval still runs for the
/// FS / process checks; the affected kinds surface as Fail.
#[tauri::command]
pub async fn evaluate_preflight_cmd(
    file_path: String,
    vault_path: String,
    pool: State<'_, SqlitePool>,
    registry: State<'_, Arc<VaultStoreRegistry>>,
) -> Result<Vec<EvaluatedPreflightItem>, String> {
    let (connections, env_keys) = match registry.for_active_vault(&pool).await {
        Ok(stores) => {
            let conns: HashSet<String> = stores
                .connections
                .list_public()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|c| c.name)
                .collect();
            let env_keys: HashSet<String> = match stores.environments.active_env().await {
                Ok(Some(env_name)) => stores
                    .environments
                    .list_vars(&env_name)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|v| v.key)
                    .collect(),
                _ => HashSet::new(),
            };
            (conns, env_keys)
        }
        Err(_) => (HashSet::new(), HashSet::new()),
    };
    let branch = current_branch(Path::new(&vault_path));

    Ok(evaluate_preflight_for_paths(
        &file_path,
        &vault_path,
        branch.as_deref(),
        &env_keys,
        &connections,
    ))
}

fn kind_tag(item: &PreflightItem) -> String {
    match item {
        PreflightItem::Connection { .. } => "connection",
        PreflightItem::EnvVar { .. } => "env_var",
        PreflightItem::Branch { .. } => "branch",
        PreflightItem::FileExists { .. } => "file_exists",
        PreflightItem::Command { .. } => "command",
        PreflightItem::Unknown { .. } => "unknown",
    }
    .to_string()
}

fn label_for(item: &PreflightItem) -> String {
    match item {
        PreflightItem::Connection { name } => name.clone(),
        PreflightItem::EnvVar { name } => name.clone(),
        PreflightItem::Branch { name } => name.clone(),
        PreflightItem::FileExists { path } => path.clone(),
        PreflightItem::Command { command } => {
            // First whitespace-separated token — that's the binary the
            // evaluator actually checked for.
            command
                .split_whitespace()
                .next()
                .unwrap_or(command)
                .to_string()
        }
        PreflightItem::Unknown { key, .. } => key.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn empty_ctx() -> (HashSet<String>, HashSet<String>) {
        (HashSet::new(), HashSet::new())
    }

    #[test]
    fn returns_empty_when_file_missing() {
        let dir = tempdir().unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &dir.path().join("missing.md").to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert!(r.is_empty());
    }

    #[test]
    fn returns_empty_when_no_frontmatter() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "# just body\nno fence here\n").unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert!(r.is_empty());
    }

    #[test]
    fn returns_empty_when_no_preflight_section() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "---\ntitle: x\ntags: [foo]\n---\nbody\n").unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert!(r.is_empty());
    }

    #[test]
    fn evaluates_file_exists_against_vault_root() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("schema.sql"), "").unwrap();
        let f = dir.path().join("note.md");
        fs::write(
            &f,
            "---\npreflight:\n  - file_exists: schema.sql\n  - file_exists: missing.sql\n---\nbody\n",
        )
        .unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].kind, "file_exists");
        assert_eq!(r[0].label, "schema.sql");
        assert_eq!(r[0].result, CheckResult::Pass);
        assert_eq!(r[1].kind, "file_exists");
        assert!(matches!(r[1].result, CheckResult::Fail { .. }));
    }

    #[test]
    fn missing_command_fails_with_path_reason() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(
            &f,
            "---\npreflight:\n  - command: definitely-not-a-real-binary-zxcvb\n---\n",
        )
        .unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, "command");
        assert_eq!(r[0].label, "definitely-not-a-real-binary-zxcvb");
        assert!(matches!(r[0].result, CheckResult::Fail { .. }));
    }

    #[test]
    fn unknown_check_kinds_route_to_skip() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "---\npreflight:\n  - future_kind: anything\n---\n").unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, "unknown");
        assert!(matches!(r[0].result, CheckResult::Skip { .. }));
    }

    #[test]
    fn connection_fails_when_name_not_declared() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "---\npreflight:\n  - connection: payments-db\n---\n").unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert_eq!(r.len(), 1);
        assert!(matches!(r[0].result, CheckResult::Fail { .. }));
    }

    #[test]
    fn connection_passes_when_name_in_context() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "---\npreflight:\n  - connection: payments-db\n---\n").unwrap();
        let mut conns = HashSet::new();
        conns.insert("payments-db".to_string());
        let envs = HashSet::new();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, "connection");
        assert_eq!(r[0].result, CheckResult::Pass);
    }

    #[test]
    fn env_var_passes_when_key_in_active_env() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "---\npreflight:\n  - env_var: API_TOKEN\n---\n").unwrap();
        let mut envs = HashSet::new();
        envs.insert("API_TOKEN".to_string());
        let conns = HashSet::new();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, "env_var");
        assert_eq!(r[0].result, CheckResult::Pass);
    }

    #[test]
    fn branch_passes_when_current_matches() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "---\npreflight:\n  - branch: main\n---\n").unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            Some("main"),
            &envs,
            &conns,
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, "branch");
        assert_eq!(r[0].result, CheckResult::Pass);
    }

    #[test]
    fn branch_fails_when_current_differs() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "---\npreflight:\n  - branch: main\n---\n").unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            Some("feature-x"),
            &envs,
            &conns,
        );
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, "branch");
        assert!(matches!(r[0].result, CheckResult::Fail { .. }));
    }

    #[test]
    fn label_for_command_uses_first_token() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "---\npreflight:\n  - command: psql --version\n---\n").unwrap();
        let (envs, conns) = empty_ctx();
        let r = evaluate_preflight_for_paths(
            &f.to_string_lossy(),
            &dir.path().to_string_lossy(),
            None,
            &envs,
            &conns,
        );
        assert_eq!(r[0].label, "psql");
    }
}
