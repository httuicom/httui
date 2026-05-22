//! Vault scaffolding + validation (foundation).
//!
//! Two pure operations live here:
//!
//! - [`is_vault`] — heuristic check that decides whether a folder
//!   "looks like a httui vault". Used by the Open-vault flow before
//!   activating it; also used by the migration script before
//!   touching anything.
//! - [`scaffold_new_vault`] — writes the empty default structure for
//!   a brand-new vault: `runbooks/`, `connections.toml`,
//!   `envs/local.toml`, `.httui/workspace.toml`, `.gitignore` with
//!   the local-overrides block (ADR 0004).
//!
//! Frontend wiring (welcome screen, git clone progress, modal
//! forms) lives in upcoming UI work.

use std::path::Path;

use super::atomic::write_atomic;
use super::gitignore::ensure_local_overrides_in_gitignore;
use super::layout::{CONNECTIONS_FILE, ENVS_DIR, WORKSPACE_DIR, WORKSPACE_FILE};

/// Heuristic: is this folder a httui vault? A folder counts as a
/// vault when **any** of these signals are present:
///
/// - It contains a `.httui/` directory.
/// - It contains a `runbooks/` directory.
/// - It contains at least one `*.md` file at the top level.
/// - It contains a `connections.toml` or `envs/` at the top level.
///
/// Empty or unrecognised folders return `false` so the Open-vault
/// flow can prompt the user to scaffold instead.
pub fn is_vault(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    if path.join(WORKSPACE_DIR).is_dir() {
        return true;
    }
    if path.join("runbooks").is_dir() {
        return true;
    }
    if path.join(CONNECTIONS_FILE).is_file() {
        return true;
    }
    if path.join(ENVS_DIR).is_dir() {
        return true;
    }
    // Top-level markdown counts.
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("md") {
                return true;
            }
        }
    }
    false
}

/// Defaults written by [`scaffold_new_vault`]. Held in one place so
/// the welcome-screen UI and the create-modal preview can reference
/// the same constants.
pub const DEFAULT_CONNECTIONS_TOML: &str = "version = \"1\"\n\n# Connections live here. Each `[connections.<name>]` block is a\n# database (postgres/mysql/sqlite/mongo) or HTTP target. Passwords\n# are stored as `{{keychain:...}}` references — never plaintext.\n";

pub const DEFAULT_LOCAL_ENV_TOML: &str = "version = \"1\"\n\n[meta]\ndescription = \"Local-development overrides\"\n\n[vars]\n\n[secrets]\n";

pub const DEFAULT_WORKSPACE_TOML: &str = "version = \"1\"\n\n# Workspace defaults — committed alongside the vault. Visual prefs\n# (theme, font) live in user.toml and stay per-machine.\n[defaults]\n";

/// Result describing what the scaffold did. Useful for the UI's
/// "vault created" toast.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScaffoldReport {
    pub vault_path: String,
    /// Vault-relative paths the scaffold wrote.
    pub created: Vec<String>,
    /// True when the folder already had vault markers and the
    /// scaffold left them alone (idempotent run).
    pub already_a_vault: bool,
}

/// Create the default structure for a new vault under `path`. The
/// folder is created if missing. **Idempotent**: if files already
/// exist they are left alone — the report lists only files this call
/// actually wrote.
pub fn scaffold_new_vault(path: &Path) -> std::io::Result<ScaffoldReport> {
    std::fs::create_dir_all(path)?;
    std::fs::create_dir_all(path.join("runbooks"))?;
    std::fs::create_dir_all(path.join(ENVS_DIR))?;
    std::fs::create_dir_all(path.join(WORKSPACE_DIR))?;

    let mut created = Vec::new();

    let connections = path.join(CONNECTIONS_FILE);
    if !connections.exists() {
        write_atomic(&connections, DEFAULT_CONNECTIONS_TOML)?;
        created.push(CONNECTIONS_FILE.to_string());
    }

    let env_local_rel = format!("{ENVS_DIR}/local.toml");
    let env_local = path.join(&env_local_rel);
    if !env_local.exists() {
        write_atomic(&env_local, DEFAULT_LOCAL_ENV_TOML)?;
        created.push(env_local_rel);
    }

    let workspace_rel = format!("{WORKSPACE_DIR}/{WORKSPACE_FILE}");
    let workspace = path.join(&workspace_rel);
    if !workspace.exists() {
        write_atomic(&workspace, DEFAULT_WORKSPACE_TOML)?;
        created.push(workspace_rel);
    }

    // Gitignore: only mark as `created` when we actually wrote.
    let gitignore_outcome = ensure_local_overrides_in_gitignore(path)?;
    use super::gitignore::GitignoreOutcome::*;
    match gitignore_outcome {
        Created => created.push(".gitignore".to_string()),
        Augmented => created.push(".gitignore (augmented)".to_string()),
        AlreadyPresent => {}
    }

    let already_a_vault = created.is_empty();

    Ok(ScaffoldReport {
        vault_path: path.display().to_string(),
        created,
        already_a_vault,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn empty_folder_is_not_a_vault() {
        let dir = TempDir::new().unwrap();
        assert!(!is_vault(dir.path()));
    }

    #[test]
    fn missing_path_is_not_a_vault() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("nope");
        assert!(!is_vault(&p));
    }

    #[test]
    fn folder_with_dot_httui_is_a_vault() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".httui")).unwrap();
        assert!(is_vault(dir.path()));
    }

    #[test]
    fn folder_with_runbooks_is_a_vault() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("runbooks")).unwrap();
        assert!(is_vault(dir.path()));
    }

    #[test]
    fn folder_with_connections_toml_is_a_vault() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("connections.toml"), "version = \"1\"\n").unwrap();
        assert!(is_vault(dir.path()));
    }

    #[test]
    fn folder_with_envs_dir_is_a_vault() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("envs")).unwrap();
        assert!(is_vault(dir.path()));
    }

    #[test]
    fn folder_with_top_level_markdown_is_a_vault() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("README.md"), "# notes").unwrap();
        assert!(is_vault(dir.path()));
    }

    #[test]
    fn folder_with_only_unrelated_files_is_not_a_vault() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("notes.txt"), "x").unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "x").unwrap();
        assert!(!is_vault(dir.path()));
    }

    #[test]
    fn scaffold_creates_full_structure() {
        let dir = TempDir::new().unwrap();
        let vault = dir.path().join("vault");
        let r = scaffold_new_vault(&vault).unwrap();
        assert!(!r.already_a_vault);
        assert!(vault.join("runbooks").is_dir());
        assert!(vault.join("envs").is_dir());
        assert!(vault.join(".httui").is_dir());
        assert!(vault.join("connections.toml").is_file());
        assert!(vault.join("envs/local.toml").is_file());
        assert!(vault.join(".httui/workspace.toml").is_file());
        assert!(vault.join(".gitignore").is_file());
        // Every default file is in the report.
        assert!(r.created.contains(&"connections.toml".to_string()));
        assert!(r.created.contains(&"envs/local.toml".to_string()));
        assert!(r.created.contains(&".httui/workspace.toml".to_string()));
        // After scaffold the folder is recognised as a vault.
        assert!(is_vault(&vault));
    }

    #[test]
    fn scaffold_writes_valid_toml_files() {
        let dir = TempDir::new().unwrap();
        let vault = dir.path().join("vault");
        scaffold_new_vault(&vault).unwrap();
        for name in [
            "connections.toml",
            "envs/local.toml",
            ".httui/workspace.toml",
        ] {
            let raw = std::fs::read_to_string(vault.join(name)).unwrap();
            // Smoke-test: each default file parses as valid TOML.
            toml::from_str::<toml::Value>(&raw)
                .unwrap_or_else(|e| panic!("invalid TOML in {name}: {e}"));
            assert!(
                raw.contains("version = \"1\""),
                "{name} should stamp version"
            );
        }
    }

    #[test]
    fn scaffold_is_idempotent() {
        let dir = TempDir::new().unwrap();
        let vault = dir.path().join("vault");
        scaffold_new_vault(&vault).unwrap();
        let r2 = scaffold_new_vault(&vault).unwrap();
        // Second run: nothing new written.
        assert!(r2.created.is_empty());
        assert!(r2.already_a_vault);
    }

    #[test]
    fn scaffold_preserves_existing_files() {
        let dir = TempDir::new().unwrap();
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();
        std::fs::write(
            vault.join("connections.toml"),
            "version = \"1\"\n[connections.existing]\ntype = \"sqlite\"\nfile = \"x.db\"\n",
        )
        .unwrap();
        scaffold_new_vault(&vault).unwrap();
        let after = std::fs::read_to_string(vault.join("connections.toml")).unwrap();
        assert!(
            after.contains("connections.existing"),
            "scaffold must not overwrite an existing connections.toml"
        );
    }

    #[test]
    fn scaffold_creates_parent_dir_if_missing() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("a/b/vault");
        scaffold_new_vault(&nested).unwrap();
        assert!(nested.is_dir());
        assert!(nested.join(".httui/workspace.toml").is_file());
    }
}
