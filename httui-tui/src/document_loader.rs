//! Pick + load the initial markdown file when the TUI opens.
//!
//! Heuristic ordering, applied to a vault directory:
//! 1. `README.md` at the root (case-insensitive).
//! 2. `index.md` at the root (case-insensitive).
//! 3. The first `.md` returned by `httui_core::fs::list_workspace`,
//!    which already filters heavy/hidden directories and sorts
//!    deterministically.
//!
//! Returns `Ok(None)` when the vault has no markdown files yet — the
//! caller renders an empty-state screen and lets the user create one
//! when lands.

use std::path::{Path, PathBuf};

use httui_core::fs::FileEntry;

use crate::buffer::Document;
use crate::error::{TuiError, TuiResult};

pub fn pick_initial_file(vault: &Path) -> Option<PathBuf> {
    let entries = httui_core::fs::list_workspace(&vault.to_string_lossy()).ok()?;

    if let Some(p) = find_top_level(&entries, "readme.md") {
        return Some(PathBuf::from(p));
    }
    if let Some(p) = find_top_level(&entries, "index.md") {
        return Some(PathBuf::from(p));
    }
    first_markdown(&entries).map(PathBuf::from)
}

fn find_top_level(entries: &[FileEntry], target_lowercase: &str) -> Option<String> {
    entries
        .iter()
        .find(|e| !e.is_dir && e.name.to_lowercase() == target_lowercase)
        .map(|e| e.path.clone())
}

fn first_markdown(entries: &[FileEntry]) -> Option<String> {
    for e in entries {
        if !e.is_dir && e.name.ends_with(".md") {
            return Some(e.path.clone());
        }
        if e.is_dir {
            if let Some(children) = e.children.as_deref() {
                if let Some(found) = first_markdown(children) {
                    return Some(found);
                }
            }
        }
    }
    None
}

pub fn load_document(vault: &Path, file: &Path) -> TuiResult<Document> {
    let raw = httui_core::fs::read_note(&vault.to_string_lossy(), &file.to_string_lossy())
        .map_err(|e| TuiError::Config(format!("read {}: {e}", file.display())))?;
    Document::from_markdown(&raw)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn touch(dir: &Path, rel: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, "# hi\n").unwrap();
    }

    #[test]
    fn picks_readme_first() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "z-other.md");
        touch(v.path(), "README.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert_eq!(p, PathBuf::from("README.md"));
    }

    #[test]
    fn falls_back_to_index() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "a.md");
        touch(v.path(), "index.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert_eq!(p, PathBuf::from("index.md"));
    }

    #[test]
    fn falls_back_to_first_alphabetical() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "zebra.md");
        touch(v.path(), "alpha.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert_eq!(p, PathBuf::from("alpha.md"));
    }

    #[test]
    fn descends_into_subdir_when_root_empty() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "docs/notes.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert!(p.to_string_lossy().contains("notes.md"));
    }

    #[test]
    fn returns_none_for_empty_vault() {
        let v = TempDir::new().unwrap();
        assert!(pick_initial_file(v.path()).is_none());
    }

    #[test]
    fn case_insensitive_match() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "readme.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert_eq!(p, PathBuf::from("readme.md"));
    }

    #[test]
    fn load_document_parses_file() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "n.md");
        let doc = load_document(v.path(), Path::new("n.md")).unwrap();
        assert!(doc.segment_count() >= 1);
    }
}
