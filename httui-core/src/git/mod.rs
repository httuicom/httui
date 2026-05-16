//! Thin wrapper around the system `git` CLI for the in-app git
//! panel (Epic 20). Functions all accept a `vault_path: &Path` and
//! shell out to `git -C <vault>`. No `git2-rs` dependency — keeps
//! the build slim and the surface easy to swap to libgit2 later if
//! we need richer diff data.
//!
//! Each call returns a typed result; non-zero exit codes from `git`
//! become structured `Err(stderr)` values rather than panics, so the
//! UI can surface them.
//!
//! Network ops (`pull` / `push`) are deliberately omitted from this
//! foundation commit — they need progress reporting + auth flows
//! that will land alongside the panel UI.

pub mod checkout;
pub mod clone;
pub mod conflict;
pub mod log;
pub mod remote;
pub mod remote_host;
pub mod staging;
pub mod status;
pub mod sync;

pub use checkout::{git_checkout, git_checkout_b, git_checkout_conflict_path, ConflictSide};
pub use conflict::{git_conflict_versions, ConflictVersions};
pub use clone::{git_clone, CloneOutcome};
pub use log::{git_first_commit_author, git_log, CommitInfo};
pub use remote::{git_remote_list, Remote};
pub use remote_host::{parse_remote_url, ParsedRemote, RemoteHost};
pub use staging::{git_commit, stage_path, unstage_path};
pub use status::{git_branch_list, git_diff, git_status, BranchInfo, GitStatus};
pub use sync::{git_fetch, git_pull, git_push};

use std::path::Path;
use std::process::{Command, Output};

/// Run `git -C <vault> <args...>` and capture stdout. Errors carry
/// stderr verbatim so the UI can show what `git` actually said.
///
/// Defensively clears the per-invocation `GIT_*` env vars that a
/// parent `git` process (e.g. when this binary runs inside a git
/// hook) injects to point children at the parent's index/work-tree
/// — those would override `-C` and silently target the wrong repo.
/// True when `git check-ignore --quiet -- <path>` reports the path
/// is ignored (exit 0). Returns `false` for every other outcome:
/// path not ignored (exit 1), path outside a git repo (exit 128),
/// `git` not installed, IO failure, etc. Powers Epic 54 Story 04
/// task 2 — the auto-discovery scanner uses this to skip e.g.
/// `node_modules/<sub>/.env` without baking the noisy-dir list
/// any deeper.
///
/// Best-effort: never returns `Err`. The caller should treat the
/// `false` return as "we couldn't confirm it's ignored", not "it's
/// definitely tracked".
pub fn is_path_gitignored<P: AsRef<Path>>(vault: P, path: &str) -> bool {
    let mut cmd = Command::new("git");
    cmd.arg("-C")
        .arg(vault.as_ref())
        .args(["check-ignore", "--quiet", "--", path]);
    scrub_git_env(&mut cmd);
    matches!(cmd.status(), Ok(s) if s.code() == Some(0))
}

pub(crate) fn run_git<P: AsRef<Path>>(vault: P, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(vault.as_ref()).args(args);
    scrub_git_env(&mut cmd);
    let output: Output = cmd
        .output()
        .map_err(|e| format!("git invocation failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(if stderr.trim().is_empty() {
            format!(
                "git exited with code {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            stderr
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Return the vault's current branch name, or `None` for a detached
/// HEAD / a path that isn't a git repo. Used by the preflight
/// evaluator (V6 cenário 9) to decide whether a `branch: <name>`
/// check passes. Cheap wrapper around `git rev-parse --abbrev-ref
/// HEAD` — much lighter than `git_status` for callers that only
/// need the branch name.
pub fn current_branch(vault: &Path) -> Option<String> {
    let raw = run_git(vault, &["rev-parse", "--abbrev-ref", "HEAD"]).ok()?;
    let trimmed = raw.trim();
    // Detached HEAD prints "HEAD" rather than a branch name.
    if trimmed.is_empty() || trimmed == "HEAD" {
        return None;
    }
    Some(trimmed.to_string())
}

/// Strip the env vars `git` itself sets when running inside a hook
/// or alias. Without this, `Command::new("git")` children inherit
/// `GIT_DIR` (etc.) from the host repo and ignore our `-C <path>`
/// — production-side latent bug, test-side reliable failure mode.
pub(crate) fn scrub_git_env(cmd: &mut Command) {
    for var in [
        // Per-invocation overrides that re-target the repo.
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_WORK_TREE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
        "GIT_NAMESPACE",
        "GIT_PREFIX",
        "GIT_EDITOR",
        // Identity overrides that bypass `[user]` config — set when
        // running inside a `git commit` hook so the child commits
        // would otherwise impersonate the host's author/committer.
        "GIT_AUTHOR_NAME",
        "GIT_AUTHOR_EMAIL",
        "GIT_AUTHOR_DATE",
        "GIT_COMMITTER_NAME",
        "GIT_COMMITTER_EMAIL",
        "GIT_COMMITTER_DATE",
    ] {
        cmd.env_remove(var);
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::scrub_git_env;
    use std::path::Path;
    use std::process::Command;

    fn git() -> Command {
        let mut c = Command::new("git");
        scrub_git_env(&mut c);
        c
    }

    /// Initialise a temporary git repo at `path`, configure
    /// non-interactive identity, return the path. Caller keeps the
    /// `TempDir` alive.
    pub fn init_repo(path: &Path) {
        let init = git().arg("init").arg(path).output().unwrap();
        assert!(init.status.success(), "git init failed");
        for (k, v) in [
            ("user.email", "test@httui.local"),
            ("user.name", "Test"),
            ("commit.gpgsign", "false"),
            ("init.defaultBranch", "main"),
        ] {
            let r = git()
                .arg("-C")
                .arg(path)
                .args(["config", k, v])
                .output()
                .unwrap();
            assert!(r.status.success(), "git config {k} failed");
        }
    }

    /// Stage and commit everything currently in the working tree.
    /// Returns the resulting commit's full SHA.
    pub fn commit_all(path: &Path, message: &str) -> String {
        let add = git()
            .arg("-C")
            .arg(path)
            .args(["add", "-A"])
            .output()
            .unwrap();
        assert!(add.status.success(), "git add failed");
        let cm = git()
            .arg("-C")
            .arg(path)
            .args(["commit", "-m", message])
            .output()
            .unwrap();
        assert!(
            cm.status.success(),
            "git commit failed: {}",
            String::from_utf8_lossy(&cm.stderr)
        );
        let rev = git()
            .arg("-C")
            .arg(path)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(rev.status.success(), "git rev-parse failed");
        String::from_utf8_lossy(&rev.stdout).trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn run_git_returns_error_on_non_repo() {
        let dir = TempDir::new().unwrap();
        let err = run_git(dir.path(), &["status"]).unwrap_err();
        assert!(
            err.contains("not a git repository") || err.contains("fatal"),
            "got: {err}"
        );
    }

    #[test]
    fn run_git_returns_stdout_on_success() {
        let dir = TempDir::new().unwrap();
        test_helpers::init_repo(dir.path());
        let out = run_git(dir.path(), &["rev-parse", "--is-inside-work-tree"]).unwrap();
        assert_eq!(out.trim(), "true");
    }

    #[test]
    fn gitignored_returns_true_when_pattern_matches() {
        let dir = TempDir::new().unwrap();
        test_helpers::init_repo(dir.path());
        std::fs::write(dir.path().join(".gitignore"), "node_modules/\n*.env\n")
            .unwrap();
        // Files don't even need to exist — `git check-ignore` works
        // off the patterns alone.
        assert!(is_path_gitignored(dir.path(), "node_modules/foo/.env"));
        assert!(is_path_gitignored(dir.path(), "secrets.env"));
    }

    #[test]
    fn gitignored_returns_false_when_pattern_does_not_match() {
        let dir = TempDir::new().unwrap();
        test_helpers::init_repo(dir.path());
        std::fs::write(dir.path().join(".gitignore"), "node_modules/\n").unwrap();
        assert!(!is_path_gitignored(dir.path(), "runbooks/auth.md"));
        assert!(!is_path_gitignored(dir.path(), ".env"));
    }

    #[test]
    fn gitignored_returns_false_outside_a_git_repo() {
        let dir = TempDir::new().unwrap();
        // No `git init` — every check returns false (not "Err").
        assert!(!is_path_gitignored(dir.path(), ".env"));
    }

    #[test]
    fn gitignored_returns_false_when_no_gitignore_present() {
        let dir = TempDir::new().unwrap();
        test_helpers::init_repo(dir.path());
        // Empty repo — nothing is ignored by default.
        assert!(!is_path_gitignored(dir.path(), ".env"));
    }
}
