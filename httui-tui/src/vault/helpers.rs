//! Pure validation + core-call helpers shared by the App-coupled
//! vault apply functions (`input/apply/pickers.rs`) and the bootstrap
//! empty-state (`crate::empty_state`).
//!
//! These functions never touch `App` state, never mutate the SQLite
//! pool, and never render anything — they validate raw user input and
//! delegate to `httui_core` (`create_new_vault`, `git_clone`,
//! `scaffold::is_vault`). The caller wraps them with status messages,
//! `switch_vault`, or `set_active_vault` as appropriate.

use std::path::{Path, PathBuf};

use super::expand_tilde;
use crate::app::{VaultOpenEntry, VaultOpenEntryKind};

/// Validate the Create form inputs and scaffold a fresh vault. On
/// success returns the absolute path of the new vault root. On
/// failure returns a user-friendly message the caller surfaces inline
/// on the form's `error` field.
pub fn submit_create(parent_raw: &str, name_raw: &str) -> Result<PathBuf, String> {
    let name = name_raw.trim();
    if name.is_empty() {
        return Err("name is required".into());
    }
    let parent_expanded = expand_tilde(parent_raw.trim());
    let parent_path = PathBuf::from(&parent_expanded);
    if !parent_path.is_dir() {
        return Err(format!("parent must be a directory: {parent_expanded}"));
    }
    let outcome = httui_core::vault_config::create::create_new_vault(&parent_path, name)
        .map_err(|e| format!("create vault: {e}"))?;
    Ok(outcome.destination)
}

/// Validate the Clone form inputs and `git_clone` into `<parent>/<repo>`.
/// On success returns the absolute path of the cloned vault root.
pub fn submit_clone(url_raw: &str, parent_raw: &str) -> Result<PathBuf, String> {
    let url = url_raw.trim();
    if url.is_empty() {
        return Err("URL is required".into());
    }
    let parent_expanded = expand_tilde(parent_raw.trim());
    let parent_path = PathBuf::from(&parent_expanded);
    if !parent_path.is_dir() {
        return Err(format!("parent must be a directory: {parent_expanded}"));
    }
    let outcome = httui_core::git::clone::git_clone(url, Some(&parent_path))
        .map_err(|e| format!("clone: {e}"))?;
    Ok(outcome.destination)
}

/// Read the dir as a `[.., dirs..]` listing. Hidden files (starting
/// with `.`) are skipped because the picker is for choosing workspaces,
/// not browsing dotfiles. Dirs flagged by `scaffold::is_vault` are
/// marked so Enter activates them; plain dirs descend.
pub fn read_dir_entries(path: &Path) -> Result<Vec<VaultOpenEntry>, String> {
    let read = std::fs::read_dir(path).map_err(|e| format!("read dir {}: {e}", path.display()))?;
    let mut dirs: Vec<VaultOpenEntry> = Vec::new();
    for entry in read.flatten() {
        let ftype = match entry.file_type() {
            Ok(t) => t,
            Err(_) => continue,
        };
        if !ftype.is_dir() {
            continue;
        }
        let name = match entry.file_name().into_string() {
            Ok(s) => s,
            Err(_) => continue,
        };
        if name.starts_with('.') {
            continue;
        }
        let kind = if httui_core::vault_config::scaffold::is_vault(&entry.path()) {
            VaultOpenEntryKind::Vault
        } else {
            VaultOpenEntryKind::Directory
        };
        dirs.push(VaultOpenEntry { name, kind });
    }
    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = Vec::with_capacity(dirs.len() + 1);
    if path.parent().is_some() {
        out.push(VaultOpenEntry {
            name: "..".to_string(),
            kind: VaultOpenEntryKind::Parent,
        });
    }
    out.extend(dirs);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn submit_create_rejects_empty_name() {
        let parent = TempDir::new().unwrap();
        let err = submit_create(&parent.path().display().to_string(), "   ").unwrap_err();
        assert!(err.contains("name is required"), "got: {err}");
    }

    #[test]
    fn submit_create_rejects_non_dir_parent() {
        let err = submit_create("/definitely/not/here", "v").unwrap_err();
        assert!(err.contains("parent must be a directory"), "got: {err}");
    }

    #[test]
    fn submit_create_expands_tilde_in_parent() {
        let home = TempDir::new().unwrap();
        std::env::set_var("HOME", home.path());
        let dest = submit_create("~/", "new-vault").expect("create should succeed");
        assert!(dest.starts_with(home.path()), "got: {}", dest.display());
        assert!(dest.ends_with("new-vault"), "got: {}", dest.display());
    }

    #[test]
    fn submit_clone_rejects_empty_url() {
        let parent = TempDir::new().unwrap();
        let err = submit_clone("   ", &parent.path().display().to_string()).unwrap_err();
        assert!(err.contains("URL is required"), "got: {err}");
    }

    #[test]
    fn submit_clone_rejects_non_dir_parent() {
        let err = submit_clone("https://example.com/x.git", "/definitely/not/here").unwrap_err();
        assert!(err.contains("parent must be a directory"), "got: {err}");
    }

    #[test]
    fn read_dir_entries_returns_dirs_sorted_with_parent_marker() {
        let root = TempDir::new().unwrap();
        std::fs::create_dir(root.path().join("z-dir")).unwrap();
        std::fs::create_dir(root.path().join("a-dir")).unwrap();
        std::fs::create_dir(root.path().join(".hidden")).unwrap();
        std::fs::write(root.path().join("file.md"), "").unwrap();

        let entries = read_dir_entries(root.path()).unwrap();
        assert_eq!(entries[0].name, "..");
        assert_eq!(entries[0].kind, VaultOpenEntryKind::Parent);
        let dir_names: Vec<&str> = entries[1..].iter().map(|e| e.name.as_str()).collect();
        assert_eq!(dir_names, vec!["a-dir", "z-dir"]);
    }

    #[test]
    fn read_dir_entries_flags_vault_dirs() {
        let root = TempDir::new().unwrap();
        let vault_dir = root.path().join("my-vault");
        std::fs::create_dir(&vault_dir).unwrap();
        // scaffold marks a dir as a vault — easier than mimicking the
        // probe; use the real fn so we stay aligned with whatever
        // `is_vault` currently checks.
        httui_core::vault_config::scaffold::scaffold_new_vault(&vault_dir).unwrap();

        let entries = read_dir_entries(root.path()).unwrap();
        let entry = entries
            .iter()
            .find(|e| e.name == "my-vault")
            .expect("vault dir listed");
        assert_eq!(entry.kind, VaultOpenEntryKind::Vault);
    }
}
