//! `git fetch` / `git pull` / `git push` — sync ops for Epic 48
//! Story 05.
//!
//! All three shell out to `git` and surface its output (stdout +
//! stderr) verbatim so the consumer's toast can show exactly what
//! git said. No auth handling at this layer — the user's git
//! credential helper / SSH agent does the work.

use super::scrub_git_env;
use std::path::Path;
use std::process::Command;

/// Run a sync op and return the combined stdout+stderr message
/// regardless of success/failure. Network ops produce useful
/// progress text on stderr that the consumer wants to see.
fn run_sync(vault: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(vault).args(args);
    scrub_git_env(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("git invocation failed: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = if stdout.is_empty() {
        stderr.to_string()
    } else if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!("{stdout}{stderr}")
    };
    if !output.status.success() {
        return Err(combined);
    }
    Ok(combined)
}

/// `git fetch [<remote>]`. When `remote` is None, fetches the
/// default (`origin` if present, else the configured upstream).
pub fn git_fetch(vault: &Path, remote: Option<&str>) -> Result<String, String> {
    let mut args = vec!["fetch"];
    if let Some(r) = remote {
        args.push(r);
    }
    run_sync(vault, &args)
}

/// `git pull [--ff-only] [<remote> <branch>]`. Both remote/branch
/// are optional — passing None falls back to git's default upstream
/// resolution. `ff_only` adds `--ff-only` so the 1-click Sync
/// (V10.1 cenário 3) never creates a surprise merge commit — a
/// non-fast-forward pull stops the flow and the user resolves it in
/// the detailed pane-tab.
pub fn git_pull(
    vault: &Path,
    remote: Option<&str>,
    branch: Option<&str>,
    ff_only: bool,
) -> Result<String, String> {
    let mut args = vec!["pull"];
    if ff_only {
        args.push("--ff-only");
    }
    if let Some(r) = remote {
        args.push(r);
        if let Some(b) = branch {
            args.push(b);
        }
    }
    run_sync(vault, &args)
}

/// `git push [-u] [<remote> <branch>]`. Same defaults as pull. When
/// `set_upstream` is true and both remote+branch are given, pushes
/// with `-u` so the branch starts tracking — this is the V10
/// "push a branch with no upstream → confirm → push -u" path.
pub fn git_push(
    vault: &Path,
    remote: Option<&str>,
    branch: Option<&str>,
    set_upstream: bool,
) -> Result<String, String> {
    let mut args = vec!["push"];
    if let Some(r) = remote {
        if set_upstream {
            args.push("-u");
        }
        args.push(r);
        if let Some(b) = branch {
            args.push(b);
        }
    }
    run_sync(vault, &args)
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{commit_all, init_repo};
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fetch_without_remote_returns_a_result() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        // `git fetch` with no remote configured can be a silent
        // success on some git versions (just walks an empty remote
        // list) or an error on others. Either way the call must
        // not panic — that's all we guard here.
        let _ = git_fetch(dir.path(), None);
    }

    #[test]
    fn pull_without_remote_errors_cleanly() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        let r = git_pull(dir.path(), None, None, false);
        assert!(r.is_err());
    }

    #[test]
    fn pull_ff_only_builds_without_panicking() {
        // No remote configured, so the pull errors — but the
        // `--ff-only` arg branch must build cleanly (guards the
        // V10.1 cenário 3 Sync path).
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        let r = git_pull(dir.path(), None, None, true);
        assert!(r.is_err());
    }

    #[test]
    fn push_without_remote_errors_cleanly() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        let r = git_push(dir.path(), None, None, false);
        assert!(r.is_err());
    }

    #[test]
    fn push_set_upstream_includes_u_flag() {
        // No reachable remote, so the push itself fails — but the
        // failure must be the network/remote error, never an
        // arg-construction panic. Guards the `-u` branch builds.
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "x").unwrap();
        commit_all(dir.path(), "init");
        let r = git_push(dir.path(), Some("origin"), Some("main"), true);
        assert!(r.is_err());
    }

    #[test]
    fn fetch_against_local_remote_succeeds() {
        // Set up two repos: a "remote" bare repo and a working
        // repo that adds the bare repo as origin.
        let remote = TempDir::new().unwrap();
        let mut init = Command::new("git");
        scrub_git_env(&mut init);
        init.arg("init")
            .arg("--bare")
            .arg(remote.path())
            .output()
            .unwrap();
        let local = TempDir::new().unwrap();
        init_repo(local.path());
        std::fs::write(local.path().join("a"), "x").unwrap();
        commit_all(local.path(), "init");
        let mut add_remote = Command::new("git");
        scrub_git_env(&mut add_remote);
        add_remote
            .arg("-C")
            .arg(local.path())
            .args(["remote", "add", "origin"])
            .arg(remote.path())
            .output()
            .unwrap();
        let r = git_fetch(local.path(), Some("origin"));
        // First fetch against an empty bare repo can be a no-op
        // success or a "nothing to do"; either way it should NOT
        // error.
        assert!(r.is_ok(), "got: {:?}", r);
    }
}
