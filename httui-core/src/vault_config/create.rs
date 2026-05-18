//! `create_new_vault`.
//!
//! Composes the "Create vault" empty-state flow:
//!
//! 1. Validate `parent` (exists + is a directory) and `name`
//!    (non-empty after trim, no path separators, doesn't start with
//!    `.`).
//! 2. Resolve the leaf as `<parent>/<name>`. If it already exists
//!    and is non-empty, refuse with a friendly error (mirrors clone's
//!    pre-flight semantics).
//! 3. Create the directory if needed.
//! 4. Shell out to `git init -q -- <leaf>` so the vault is tracked
//!    from the first commit. Auth-free (local op).
//! 5. Delegate to [`scaffold_new_vault`] so we share the same default
//!    files the welcome screen produces today.
//!
//! Failure at any step bubbles up as a structured `Err(String)` and
//! the caller's UI surfaces it inline.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::scaffold::{scaffold_new_vault, ScaffoldReport};
use crate::git::scrub_git_env;

/// Outcome of a successful create — the absolute path of the new
/// vault plus the scaffold report so the UI can show what files
/// were written.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CreateOutcome {
    pub destination: PathBuf,
    pub scaffold: ScaffoldReport,
}

/// Validate, mkdir, `git init`, scaffold. See module docs.
pub fn create_new_vault(parent: &Path, name: &str) -> Result<CreateOutcome, String> {
    if !parent.exists() {
        return Err(format!("pasta pai '{}' não existe", parent.display()));
    }
    if !parent.is_dir() {
        return Err(format!("'{}' não é uma pasta", parent.display()));
    }

    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("informe um nome para o vault".into());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err("nome não pode conter '/' nem '\\'".into());
    }
    if trimmed.starts_with('.') {
        return Err("nome não pode começar com '.'".into());
    }

    let dest = parent.join(trimmed);

    if dest.exists() {
        let is_empty = dest
            .read_dir()
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if !is_empty {
            return Err(format!("'{}' já existe e não está vazio", dest.display()));
        }
    } else {
        std::fs::create_dir(&dest)
            .map_err(|e| format!("não consegui criar '{}': {e}", dest.display()))?;
    }

    let mut cmd = Command::new("git");
    cmd.arg("init").arg("-q").arg(&dest);
    scrub_git_env(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("git invocation failed: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(if stderr.trim().is_empty() {
            format!(
                "git init exited with code {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            stderr.trim().to_string()
        });
    }

    let scaffold = scaffold_new_vault(&dest).map_err(|e| format!("scaffold vault: {e}"))?;

    Ok(CreateOutcome {
        destination: dest,
        scaffold,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parent_does_not_exist_returns_friendly_error() {
        let parent = TempDir::new().unwrap();
        let missing = parent.path().join("nope");
        let err = create_new_vault(&missing, "v1").unwrap_err();
        assert!(err.contains("pasta pai"), "got: {err}");
    }

    #[test]
    fn parent_is_a_file_returns_friendly_error() {
        let parent = TempDir::new().unwrap();
        let file = parent.path().join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        let err = create_new_vault(&file, "v1").unwrap_err();
        assert!(err.contains("não é uma pasta"), "got: {err}");
    }

    #[test]
    fn empty_name_returns_friendly_error() {
        let parent = TempDir::new().unwrap();
        let err = create_new_vault(parent.path(), "  ").unwrap_err();
        assert!(err.contains("nome"), "got: {err}");
    }

    #[test]
    fn name_with_slash_rejected() {
        let parent = TempDir::new().unwrap();
        let err = create_new_vault(parent.path(), "foo/bar").unwrap_err();
        assert!(err.contains("'/'"), "got: {err}");
    }

    #[test]
    fn name_with_backslash_rejected() {
        let parent = TempDir::new().unwrap();
        let err = create_new_vault(parent.path(), "foo\\bar").unwrap_err();
        assert!(err.contains("'\\\\'") || err.contains("\\"), "got: {err}");
    }

    #[test]
    fn name_starting_with_dot_rejected() {
        let parent = TempDir::new().unwrap();
        let err = create_new_vault(parent.path(), ".hidden").unwrap_err();
        assert!(err.contains("'.'"), "got: {err}");
    }

    #[test]
    fn collision_with_non_empty_dir_rejected() {
        let parent = TempDir::new().unwrap();
        let leaf = parent.path().join("v1");
        std::fs::create_dir(&leaf).unwrap();
        std::fs::write(leaf.join("blocker"), b"x").unwrap();
        let err = create_new_vault(parent.path(), "v1").unwrap_err();
        assert!(err.contains("já existe e não está vazio"), "got: {err}");
    }

    #[test]
    fn happy_path_creates_dir_inits_git_and_scaffolds() {
        let parent = TempDir::new().unwrap();
        let outcome = create_new_vault(parent.path(), "  meu-vault  ").unwrap();
        // Trim applied — leaf is "meu-vault", not "  meu-vault  ".
        assert_eq!(outcome.destination, parent.path().join("meu-vault"));
        assert!(outcome.destination.exists());
        assert!(outcome.destination.join(".git").is_dir(), ".git missing");
        assert!(
            outcome.destination.join("connections.toml").is_file(),
            "scaffold should have written connections.toml",
        );
        assert!(
            outcome.destination.join(".httui").is_dir(),
            "scaffold should have created .httui/",
        );
        assert!(
            outcome
                .scaffold
                .created
                .iter()
                .any(|f| f == "connections.toml"),
            "scaffold report should list created files: {:?}",
            outcome.scaffold.created,
        );
    }

    #[test]
    fn happy_path_works_when_leaf_dir_pre_exists_and_is_empty() {
        let parent = TempDir::new().unwrap();
        let leaf = parent.path().join("preexisting");
        std::fs::create_dir(&leaf).unwrap();
        let outcome = create_new_vault(parent.path(), "preexisting").unwrap();
        assert_eq!(outcome.destination, leaf);
        assert!(leaf.join(".git").is_dir());
    }
}
