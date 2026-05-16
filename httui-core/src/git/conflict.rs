//! Merge-conflict stage extraction for the V10 3-way resolver.
//!
//! A conflicted path has up to three index stages:
//!   - `:1:<path>` — the merge base (common ancestor). Absent for
//!     add/add conflicts.
//!   - `:2:<path>` — our side (HEAD).
//!   - `:3:<path>` — their side (incoming).
//!
//! `git show :N:<path>` prints the blob at that stage. A missing
//! stage exits non-zero; we treat that as an empty pane rather than
//! a hard error so the resolver still opens for add/add conflicts.

use std::path::Path;

use serde::Serialize;

use super::run_git;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConflictVersions {
    /// Stage 1 — common ancestor. Empty when there's no merge base.
    pub base: String,
    /// Stage 2 — our side (HEAD).
    pub ours: String,
    /// Stage 3 — their side (incoming).
    pub theirs: String,
}

/// Read the three merge stages of a conflicted `path`. Missing
/// stages collapse to an empty string. Returns `Err` only when the
/// path has no conflict stages at all (not unmerged / bad path) so
/// the UI can distinguish "nothing to resolve" from "one side was
/// an add".
pub fn git_conflict_versions(
    vault: &Path,
    path: &str,
) -> Result<ConflictVersions, String> {
    let stage = |n: u8| -> String {
        let spec = format!(":{n}:{path}");
        run_git(vault, &["show", &spec]).unwrap_or_default()
    };
    let base = stage(1);
    let ours = stage(2);
    let theirs = stage(3);
    if base.is_empty() && ours.is_empty() && theirs.is_empty() {
        return Err(format!(
            "'{path}' has no conflict stages — not an unmerged path"
        ));
    }
    Ok(ConflictVersions { base, ours, theirs })
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{commit_all, init_repo};
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn git(path: &std::path::Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Build a real merge conflict on `f.txt` and return its dir.
    fn conflicted_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        init_repo(p);
        std::fs::write(p.join("f.txt"), "base\n").unwrap();
        commit_all(p, "base");
        git(p, &["checkout", "-b", "feature"]);
        std::fs::write(p.join("f.txt"), "theirs side\n").unwrap();
        commit_all(p, "theirs");
        git(p, &["checkout", "main"]);
        std::fs::write(p.join("f.txt"), "ours side\n").unwrap();
        commit_all(p, "ours");
        // Merge feature into main — conflicts on f.txt.
        let _ = Command::new("git")
            .arg("-C")
            .arg(p)
            .args(["merge", "feature"])
            .output()
            .unwrap();
        dir
    }

    #[test]
    fn extracts_all_three_stages() {
        let dir = conflicted_repo();
        let v = git_conflict_versions(dir.path(), "f.txt").unwrap();
        assert_eq!(v.base, "base\n");
        assert_eq!(v.ours, "ours side\n");
        assert_eq!(v.theirs, "theirs side\n");
    }

    #[test]
    fn errors_when_path_is_not_unmerged() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("clean.txt"), "x\n").unwrap();
        commit_all(dir.path(), "clean");
        let err =
            git_conflict_versions(dir.path(), "clean.txt").unwrap_err();
        assert!(err.contains("no conflict stages"), "got: {err}");
    }
}
