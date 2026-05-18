//! `git add <path>` / `git reset HEAD <path>` / `git commit` —
//! stage / unstage / commit operations.

use std::path::Path;

use super::run_git;

/// `git add <path>` — stages a single file. Path is vault-relative.
pub fn stage_path(vault: &Path, path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("path is empty".into());
    }
    run_git(vault, &["add", "--", path])?;
    Ok(())
}

/// `git reset HEAD -- <path>` — unstages a file (drops the staged
/// version, keeps the working-tree edits).
pub fn unstage_path(vault: &Path, path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("path is empty".into());
    }
    run_git(vault, &["reset", "HEAD", "--", path])?;
    Ok(())
}

/// `git commit -m <message>` (with `--amend --no-edit` when amend is
/// true). Empty message is rejected up front.
pub fn git_commit(vault: &Path, message: &str, amend: bool) -> Result<(), String> {
    if !amend && message.trim().is_empty() {
        return Err("commit message is empty".into());
    }
    let args: Vec<&str> = if amend {
        vec!["commit", "--amend", "--no-edit"]
    } else {
        vec!["commit", "-m", message]
    };
    run_git(vault, &args)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::scrub_git_env;
    use super::super::test_helpers::{commit_all, init_repo};
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn git_at(p: &Path, args: &[&str]) -> std::process::Output {
        let mut cmd = Command::new("git");
        scrub_git_env(&mut cmd);
        cmd.arg("-C").arg(p).args(args).output().unwrap()
    }

    fn rev_parse_head(p: &Path) -> String {
        let r = git_at(p, &["rev-parse", "HEAD"]);
        String::from_utf8_lossy(&r.stdout).trim().to_string()
    }

    fn status_porcelain(p: &Path) -> String {
        let r = git_at(p, &["status", "--porcelain"]);
        String::from_utf8_lossy(&r.stdout).to_string()
    }

    #[test]
    fn stage_path_marks_file_as_added() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("new"), "x").unwrap();
        stage_path(dir.path(), "new").unwrap();
        let s = status_porcelain(dir.path());
        assert!(s.contains("A  new"), "got: {s}");
    }

    #[test]
    fn unstage_path_drops_staged_version() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("a"), "y").unwrap();
        stage_path(dir.path(), "a").unwrap();
        let staged = status_porcelain(dir.path());
        assert!(staged.starts_with("M "), "got: {staged}");
        unstage_path(dir.path(), "a").unwrap();
        let unstaged = status_porcelain(dir.path());
        assert!(unstaged.starts_with(" M"), "got: {unstaged}");
    }

    #[test]
    fn git_commit_creates_a_new_commit() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        let before = rev_parse_head(dir.path());
        std::fs::write(dir.path().join("b"), "y").unwrap();
        stage_path(dir.path(), "b").unwrap();
        git_commit(dir.path(), "second commit", false).unwrap();
        let after = rev_parse_head(dir.path());
        assert_ne!(before, after);
    }

    #[test]
    fn git_commit_amend_keeps_message_via_no_edit() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "first");
        std::fs::write(dir.path().join("a"), "y").unwrap();
        stage_path(dir.path(), "a").unwrap();
        git_commit(dir.path(), "", true).unwrap();
        let r = git_at(dir.path(), &["log", "-1", "--pretty=%s"]);
        assert_eq!(String::from_utf8_lossy(&r.stdout).trim(), "first");
    }

    #[test]
    fn stage_path_rejects_empty_path() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        let err = stage_path(dir.path(), "  ").unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn unstage_path_rejects_empty_path() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        let err = unstage_path(dir.path(), "").unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn git_commit_rejects_empty_message_when_not_amending() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        let err = git_commit(dir.path(), "   ", false).unwrap_err();
        assert!(err.contains("empty"));
    }
}
