//! `git checkout` + `git checkout -b` — branch switching.
//! Plus `git_checkout_conflict_path` for accepting one side of a
//! merge conflict.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::run_git;

/// Which side of a merge conflict to keep. Mirrors the
/// `<GitConflictBanner>` Accept-yours / Accept-theirs buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictSide {
    /// Keep the version from the current branch (HEAD).
    Ours,
    /// Keep the version from the branch being merged.
    Theirs,
}

impl ConflictSide {
    fn flag(self) -> &'static str {
        match self {
            ConflictSide::Ours => "--ours",
            ConflictSide::Theirs => "--theirs",
        }
    }
}

/// `git checkout --ours|--theirs <path>` — replace a conflicted
/// working-tree file with one side of the merge. Caller is expected
/// to follow up with `stage_path` to mark the conflict resolved.
///
/// Returns the verbatim git stderr on failure (`path not in
/// conflict`, `path does not exist`, etc.) so the consumer toast
/// can surface the actual reason.
pub fn git_checkout_conflict_path(
    vault: &Path,
    path: &str,
    side: ConflictSide,
) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("path is empty".into());
    }
    run_git(vault, &["checkout", side.flag(), "--", path])?;
    Ok(())
}

/// `git checkout <branch>` — switch to an existing branch. Errors
/// surface verbatim from git so the consumer can show them in a
/// toast (uncommitted changes, branch not found, etc.).
pub fn git_checkout(vault: &Path, branch: &str) -> Result<(), String> {
    if branch.trim().is_empty() {
        return Err("branch name is empty".into());
    }
    run_git(vault, &["checkout", branch])?;
    Ok(())
}

/// `git checkout -b <new>` — create a new branch from the current
/// branch and switch to it.
pub fn git_checkout_b(vault: &Path, new_branch: &str) -> Result<(), String> {
    if new_branch.trim().is_empty() {
        return Err("new branch name is empty".into());
    }
    run_git(vault, &["checkout", "-b", new_branch])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{commit_all, init_repo};
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn checkout_b_creates_and_switches() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        git_checkout_b(dir.path(), "feat/x").unwrap();
        let head = head_branch(dir.path());
        assert_eq!(head, "feat/x");
    }

    #[test]
    fn checkout_switches_back_to_main() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        git_checkout_b(dir.path(), "feat/x").unwrap();
        git_checkout(dir.path(), "main").unwrap();
        let head = head_branch(dir.path());
        assert_eq!(head, "main");
    }

    fn head_branch(p: &std::path::Path) -> String {
        let mut cmd = std::process::Command::new("git");
        super::super::scrub_git_env(&mut cmd);
        let out = cmd
            .arg("-C")
            .arg(p)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn checkout_returns_error_for_unknown_branch() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        let err = git_checkout(dir.path(), "nope").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn checkout_rejects_empty_branch_name() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        let err = git_checkout(dir.path(), "  ").unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn checkout_b_rejects_empty_branch_name() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        let err = git_checkout_b(dir.path(), "").unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn checkout_b_returns_error_when_branch_already_exists() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        git_checkout_b(dir.path(), "feat/x").unwrap();
        // Switch back so we're not on feat/x.
        git_checkout(dir.path(), "main").unwrap();
        // Try to create the same name again.
        let err = git_checkout_b(dir.path(), "feat/x").unwrap_err();
        assert!(!err.is_empty());
    }

    /// Build a deliberate merge conflict on `conflict.txt`. Returns
    /// the temp dir; the working tree is in the conflicted post-merge
    /// state. Both sides have written to the same lines so git can't
    /// auto-merge.
    fn make_merge_conflict() -> TempDir {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("conflict.txt"), "base line\n").unwrap();
        commit_all(dir.path(), "base");
        // Branch off and write THEIRS.
        git_checkout_b(dir.path(), "feat/theirs").unwrap();
        std::fs::write(dir.path().join("conflict.txt"), "from-theirs\n").unwrap();
        commit_all(dir.path(), "theirs change");
        // Back to main and write OURS over the same line.
        git_checkout(dir.path(), "main").unwrap();
        std::fs::write(dir.path().join("conflict.txt"), "from-ours\n").unwrap();
        commit_all(dir.path(), "ours change");
        // Attempt the merge — must fail with conflict.
        let merge = std::process::Command::new("git");
        let mut merge = merge;
        super::super::scrub_git_env(&mut merge);
        let out = merge
            .arg("-C")
            .arg(dir.path())
            .args(["merge", "feat/theirs"])
            .output()
            .unwrap();
        assert!(!out.status.success(), "merge should have conflicted");
        dir
    }

    #[test]
    fn checkout_conflict_ours_keeps_ours_version() {
        let dir = make_merge_conflict();
        git_checkout_conflict_path(dir.path(), "conflict.txt", ConflictSide::Ours).unwrap();
        let body = std::fs::read_to_string(dir.path().join("conflict.txt")).unwrap();
        assert_eq!(body.trim_end(), "from-ours");
    }

    #[test]
    fn checkout_conflict_theirs_keeps_theirs_version() {
        let dir = make_merge_conflict();
        git_checkout_conflict_path(dir.path(), "conflict.txt", ConflictSide::Theirs).unwrap();
        let body = std::fs::read_to_string(dir.path().join("conflict.txt")).unwrap();
        assert_eq!(body.trim_end(), "from-theirs");
    }

    #[test]
    fn checkout_conflict_rejects_empty_path() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        let err = git_checkout_conflict_path(dir.path(), "  ", ConflictSide::Ours).unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn checkout_conflict_errors_when_path_not_tracked() {
        // `git checkout --ours <path>` is silently a no-op for a
        // non-conflicted *tracked* path (it re-checks-out the index
        // entry, no-op). Untracked / unknown paths surface the git
        // error verbatim — the consumer toast wants that string.
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("clean.txt"), "fine\n").unwrap();
        commit_all(dir.path(), "init");
        let err =
            git_checkout_conflict_path(dir.path(), "ghost.txt", ConflictSide::Ours).unwrap_err();
        assert!(!err.is_empty(), "stderr should surface git's reason");
    }

    #[test]
    fn conflict_side_serializes_snake_case() {
        // Sanity check the IPC contract — the Tauri command will
        // accept `"ours"` / `"theirs"` strings.
        let json_ours = serde_json::to_string(&ConflictSide::Ours).unwrap();
        let json_theirs = serde_json::to_string(&ConflictSide::Theirs).unwrap();
        assert_eq!(json_ours, "\"ours\"");
        assert_eq!(json_theirs, "\"theirs\"");
        let parsed: ConflictSide = serde_json::from_str("\"theirs\"").unwrap();
        assert_eq!(parsed, ConflictSide::Theirs);
    }
}
