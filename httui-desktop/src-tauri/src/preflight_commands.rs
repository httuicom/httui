//! Tauri command wrapping `httui_core::preflight`. Powers V6 cenário 9
//! — the inline DocHeader's pill row reads from this command on file
//! open and after each save.
//!
//! MVP scope: the evaluator is fed FS + process resolution (`FileExists`
//! / `Command`) but the connection / env-var / keychain sets are empty.
//! Those checks therefore Fail with a clear reason ("connection `X`
//! not found"). A follow-up enriches the context with the live vault
//! state (active env, declared connections, keychain enumeration) so
//! the same pills surface for those check kinds too.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use httui_core::frontmatter::split_frontmatter;
use httui_core::preflight::{
    evaluate_preflight_with_io, parse_preflight, CheckResult, EvaluationContext, PreflightItem,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct EvaluatedPreflightItem {
    /// `connection` / `env_var` / `branch` / `keychain` / `file_exists`
    /// / `command` / `unknown` — matches the Rust enum tag for
    /// per-kind UI hints (suggestion text, click-to-fix).
    pub kind: String,
    /// Short text rendered inside the pill (the item's name / path /
    /// command — whichever makes most sense at a glance).
    pub label: String,
    pub result: CheckResult,
}

/// Read the frontmatter of `file_path`, parse the `preflight:` block,
/// and evaluate every item against host-side state. `vault_path` is
/// the absolute path of the open vault — relative `FileExists` paths
/// resolve against it.
///
/// Returns an empty vector when:
/// - the file doesn't exist or can't be read,
/// - the file has no frontmatter fence,
/// - the frontmatter has no `preflight:` block.
///
/// Errors propagate through the `Result` only for unrecoverable
/// situations (currently none — the function is best-effort).
#[tauri::command]
pub async fn evaluate_preflight_cmd(
    file_path: String,
    vault_path: String,
) -> Result<Vec<EvaluatedPreflightItem>, String> {
    // Resolve `file_path` relative to `vault_path` so the call matches
    // the read_note / write_note convention (the React layer passes
    // vault-relative paths). Absolute paths fall through unchanged
    // because `PathBuf::join` replaces the base when given an absolute
    // operand — keeps the existing absolute-path tests green.
    let abs_path: PathBuf = if Path::new(&file_path).is_absolute() {
        PathBuf::from(&file_path)
    } else {
        PathBuf::from(&vault_path).join(&file_path)
    };
    let content = match fs::read_to_string(&abs_path) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };
    let raw_yaml = match split_frontmatter(&content) {
        Some(split) => split.raw_yaml,
        None => return Ok(Vec::new()),
    };
    let items = parse_preflight(&raw_yaml);
    if items.is_empty() {
        return Ok(Vec::new());
    }

    // MVP: empty context for non-FS/proc checks. The IO evaluator
    // overrides FileExists + Command with real FS / PATH lookups; the
    // pure path bails on the others without that context, returning
    // Fail / Skip with a helpful reason.
    let env_vars: HashSet<String> = HashSet::new();
    let connections: HashSet<String> = HashSet::new();
    let keychain_keys: HashSet<String> = HashSet::new();
    let ctx = EvaluationContext {
        branch: None,
        active_env_vars: &env_vars,
        connections: &connections,
        keychain_keys: &keychain_keys,
    };

    let vault_root = PathBuf::from(vault_path);
    let results = evaluate_preflight_with_io(&items, &ctx, &vault_root);

    let out: Vec<EvaluatedPreflightItem> = items
        .iter()
        .zip(results)
        .map(|(item, result)| EvaluatedPreflightItem {
            kind: kind_tag(item),
            label: label_for(item),
            result,
        })
        .collect();
    Ok(out)
}

fn kind_tag(item: &PreflightItem) -> String {
    match item {
        PreflightItem::Connection { .. } => "connection",
        PreflightItem::EnvVar { .. } => "env_var",
        PreflightItem::Branch { .. } => "branch",
        PreflightItem::Keychain { .. } => "keychain",
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
        PreflightItem::Keychain { name } => name.clone(),
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

    #[tokio::test]
    async fn returns_empty_when_file_missing() {
        let dir = tempdir().unwrap();
        let r = evaluate_preflight_cmd(
            dir.path().join("missing.md").to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
        )
        .await
        .unwrap();
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn returns_empty_when_no_frontmatter() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(&f, "# just body\nno fence here\n").unwrap();
        let r = evaluate_preflight_cmd(
            f.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
        )
        .await
        .unwrap();
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn returns_empty_when_no_preflight_section() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(
            &f,
            "---\ntitle: x\ntags: [foo]\n---\nbody\n",
        )
        .unwrap();
        let r = evaluate_preflight_cmd(
            f.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
        )
        .await
        .unwrap();
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn evaluates_file_exists_against_vault_root() {
        let dir = tempdir().unwrap();
        // Create the target file under the vault.
        fs::write(dir.path().join("schema.sql"), "").unwrap();
        let f = dir.path().join("note.md");
        fs::write(
            &f,
            "---\npreflight:\n  - file_exists: schema.sql\n  - file_exists: missing.sql\n---\nbody\n",
        )
        .unwrap();
        let r = evaluate_preflight_cmd(
            f.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
        )
        .await
        .unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].kind, "file_exists");
        assert_eq!(r[0].label, "schema.sql");
        assert_eq!(r[0].result, CheckResult::Pass);
        assert_eq!(r[1].kind, "file_exists");
        assert!(matches!(r[1].result, CheckResult::Fail { .. }));
    }

    #[tokio::test]
    async fn missing_command_fails_with_path_reason() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(
            &f,
            "---\npreflight:\n  - command: definitely-not-a-real-binary-zxcvb\n---\n",
        )
        .unwrap();
        let r = evaluate_preflight_cmd(
            f.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
        )
        .await
        .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, "command");
        assert_eq!(r[0].label, "definitely-not-a-real-binary-zxcvb");
        assert!(matches!(r[0].result, CheckResult::Fail { .. }));
    }

    #[tokio::test]
    async fn unknown_check_kinds_route_to_skip() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(
            &f,
            "---\npreflight:\n  - future_kind: anything\n---\n",
        )
        .unwrap();
        let r = evaluate_preflight_cmd(
            f.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
        )
        .await
        .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].kind, "unknown");
        assert!(matches!(r[0].result, CheckResult::Skip { .. }));
    }

    #[tokio::test]
    async fn connection_fails_with_empty_context() {
        // The MVP empty-context contract: declared connection / env_var
        // / keychain checks Fail until the follow-up wires the vault
        // state. The fail reason names the missing kind so the user
        // can identify the gap.
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(
            &f,
            "---\npreflight:\n  - connection: payments-db\n  - env_var: API_TOKEN\n---\n",
        )
        .unwrap();
        let r = evaluate_preflight_cmd(
            f.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
        )
        .await
        .unwrap();
        assert_eq!(r.len(), 2);
        assert!(matches!(r[0].result, CheckResult::Fail { .. }));
        assert!(matches!(r[1].result, CheckResult::Fail { .. }));
    }

    #[tokio::test]
    async fn label_for_command_uses_first_token() {
        let dir = tempdir().unwrap();
        let f = dir.path().join("note.md");
        fs::write(
            &f,
            "---\npreflight:\n  - command: psql --version\n---\n",
        )
        .unwrap();
        let r = evaluate_preflight_cmd(
            f.to_string_lossy().into_owned(),
            dir.path().to_string_lossy().into_owned(),
        )
        .await
        .unwrap();
        assert_eq!(r[0].label, "psql");
    }
}
