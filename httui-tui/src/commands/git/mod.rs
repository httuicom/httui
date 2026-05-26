//! Git operations consumed by the side panel and the status bar.
//! Every git invocation lives in `httui_core::git`; this module owns
//! the wiring (sync calls, error propagation onto the panel state, /
//! eventually status-bar refresh cadence).

use crate::app::App;
use httui_core::git::staging::{git_commit, stage_path};
use httui_core::git::status::DiffMetrics;
use httui_core::git::sync::{git_pull, git_push};
use httui_core::git::git_remote_list;

/// Refresh the panel's `git status` snapshot plus diff metrics.
/// Errors (not a git repo, `git` missing) are surfaced through
/// `status_error` so the renderer can show a friendly message; both
/// `status` and `metrics` are reset in that case so stale data never
/// bleeds across vaults. Shortstat is best-effort: a failure there is
/// silently downgraded to zero counts (the status snapshot is the
/// load-bearing piece — UI never gates on metrics).
pub fn refresh_git_status(app: &mut App) {
    let vault = app.vault_path.clone();
    match httui_core::git::git_status(&vault) {
        Ok(status) => {
            let max = status.changed.len().saturating_sub(1);
            if app.git_panel.selected > max {
                app.git_panel.selected = max;
            }
            app.git_panel.status = Some(status);
            app.git_panel.status_error = None;
            app.git_panel.metrics =
                httui_core::git::git_diff_shortstat(&vault).unwrap_or_default();
            app.git_panel.recent_commits = httui_core::git::log::git_log(
                &vault,
                crate::git::HISTORY_PREVIEW_COUNT,
                None,
            )
            .unwrap_or_default();
        }
        Err(msg) => {
            app.git_panel.status = None;
            app.git_panel.status_error = Some(msg);
            app.git_panel.metrics = DiffMetrics::default();
            app.git_panel.recent_commits.clear();
        }
    }
}

/// Outcome of a Sync round (stage → commit → pull → push). The UI
/// uses the variant to decide what to surface: success message,
/// confirm-set-upstream modal, or an error message keyed to the
/// failing stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    /// All requested steps succeeded.
    Done(String),
    /// `git push` rejected the branch because it has no upstream
    /// configured. The UI opens a confirmation modal; on accept it
    /// calls [`push_with_set_upstream`] with these fields.
    NeedsUpstream { remote: String, branch: String },
    /// A stage failed. Status snapshot is refreshed before returning.
    Failed { stage: SyncStage, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStage {
    Commit,
    Pull,
    Push,
}

/// Stage every changed file from the panel's last snapshot and run
/// `git commit -m <message>` (or `--amend` when `amend` is true).
/// Returns `Ok(())` on success, `Err(msg)` when there's nothing to
/// commit (and not amending — `--amend` works on a clean tree too),
/// no fresh status snapshot, or git rejects the commit. Refreshes
/// the panel's status snapshot regardless of outcome.
pub fn commit_changes(app: &mut App, message: &str, amend: bool) -> Result<(), String> {
    let vault = app.vault_path.clone();
    let Some(status) = app.git_panel.status.as_ref() else {
        return Err("no git status snapshot — refresh first".to_string());
    };
    if status.changed.is_empty() && !amend {
        return Err("nothing to commit".to_string());
    }
    let paths: Vec<String> = status.changed.iter().map(|c| c.path.clone()).collect();
    for path in &paths {
        stage_path(&vault, path)?;
    }
    let result = git_commit(&vault, message, amend);
    refresh_git_status(app);
    result
}

/// Run the 1-click Sync pipeline. When `commit_message` is `Some`
/// and the working tree has changes, the commit step runs first;
/// otherwise it's skipped. Pull is `--ff-only` (never auto-merge).
/// Push without upstream returns [`SyncOutcome::NeedsUpstream`] so
/// the UI can confirm before issuing `git push -u`.
pub fn sync_changes(app: &mut App, commit_message: Option<&str>) -> SyncOutcome {
    let vault = app.vault_path.clone();
    let initial_status = app.git_panel.status.clone();

    if let Some(msg) = commit_message {
        let has_changes = initial_status
            .as_ref()
            .map(|s| !s.changed.is_empty())
            .unwrap_or(false);
        if has_changes {
            if let Err(message) = commit_changes(app, msg, false) {
                return SyncOutcome::Failed {
                    stage: SyncStage::Commit,
                    message,
                };
            }
        }
    }

    // Re-snapshot after the commit so `upstream` / `branch` reflect
    // the post-commit world (the branch shouldn't change, but ahead/
    // behind certainly do).
    refresh_git_status(app);
    let post = app.git_panel.status.clone();
    let upstream = post.as_ref().and_then(|s| s.upstream.clone());
    let branch = post.as_ref().and_then(|s| s.branch.clone());

    if upstream.is_some() {
        if let Err(message) = git_pull(&vault, None, None, true) {
            return SyncOutcome::Failed {
                stage: SyncStage::Pull,
                message,
            };
        }
    }

    match git_push(&vault, None, None, false) {
        Ok(_) => {
            refresh_git_status(app);
            SyncOutcome::Done("synced".to_string())
        }
        Err(message) => {
            if needs_upstream_error(&message) {
                if let Some((remote, branch)) = first_remote_and_branch(&vault, branch.as_deref()) {
                    return SyncOutcome::NeedsUpstream { remote, branch };
                }
            }
            SyncOutcome::Failed {
                stage: SyncStage::Push,
                message,
            }
        }
    }
}

/// Re-run the push step with `-u`, given the remote + branch picked
/// by the confirm modal. Refreshes status afterwards.
pub fn push_with_set_upstream(app: &mut App, remote: &str, branch: &str) -> Result<(), String> {
    let vault = app.vault_path.clone();
    let result = git_push(&vault, Some(remote), Some(branch), true).map(|_| ());
    refresh_git_status(app);
    result
}

fn needs_upstream_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("has no upstream") || lower.contains("no upstream branch")
}

fn first_remote_and_branch(
    vault: &std::path::Path,
    branch: Option<&str>,
) -> Option<(String, String)> {
    let remotes = git_remote_list(vault).ok()?;
    let remote = remotes
        .iter()
        .find(|r| r.name == "origin")
        .or_else(|| remotes.first())?
        .name
        .clone();
    let branch = branch?.to_string();
    Some((remote, branch))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn build_app() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn non_git_vault_sets_status_error() {
        let (mut app, _d, _v) = build_app().await;
        refresh_git_status(&mut app);
        assert!(app.git_panel.status.is_none());
        let err = app
            .git_panel
            .status_error
            .as_ref()
            .expect("error populated for non-git vault");
        assert!(err.contains("not a git repository") || err.contains("fatal"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn git_repo_clears_previous_error() {
        let (mut app, _d, vault) = build_app().await;
        // Prime with a stale error.
        app.git_panel.status_error = Some("stale".to_string());
        crate::git::test_helpers::init_repo(vault.path());

        refresh_git_status(&mut app);
        assert!(app.git_panel.status_error.is_none());
        assert!(app.git_panel.status.is_some());
    }

    fn init_bare_remote(remote: &TempDir) {
        let mut cmd = std::process::Command::new("git");
        cmd.args(["init", "--bare", "--initial-branch=main"])
            .arg(remote.path())
            .output()
            .unwrap();
    }

    fn add_origin(vault_path: &std::path::Path, remote_path: &std::path::Path) {
        let r = std::process::Command::new("git")
            .arg("-C")
            .arg(vault_path)
            .args(["remote", "add", "origin"])
            .arg(remote_path)
            .output()
            .unwrap();
        assert!(r.status.success(), "remote add failed");
    }

    #[test]
    fn needs_upstream_error_matches_git_phrasings() {
        assert!(needs_upstream_error(
            "fatal: The current branch main has no upstream branch."
        ));
        assert!(needs_upstream_error("has no upstream"));
        assert!(needs_upstream_error("no upstream branch"));
        assert!(!needs_upstream_error(
            "fatal: unable to access 'https://...': network failure"
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn first_remote_and_branch_prefers_origin() {
        let (_app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a"), "x\n").unwrap();
        // Add two remotes; `origin` must win the picker.
        let other = TempDir::new().unwrap();
        let origin = TempDir::new().unwrap();
        init_bare_remote(&other);
        init_bare_remote(&origin);
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["remote", "add", "alt"])
            .arg(other.path())
            .output();
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["remote", "add", "origin"])
            .arg(origin.path())
            .output();
        let picked = first_remote_and_branch(vault.path(), Some("main"));
        assert_eq!(
            picked,
            Some(("origin".to_string(), "main".to_string()))
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn first_remote_and_branch_falls_back_to_only_remote() {
        let (_app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        let alt = TempDir::new().unwrap();
        init_bare_remote(&alt);
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["remote", "add", "alt"])
            .arg(alt.path())
            .output();
        let picked = first_remote_and_branch(vault.path(), Some("main"));
        assert_eq!(picked, Some(("alt".to_string(), "main".to_string())));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn first_remote_and_branch_returns_none_without_branch() {
        let (_app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        let alt = TempDir::new().unwrap();
        init_bare_remote(&alt);
        let _ = std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["remote", "add", "origin"])
            .arg(alt.path())
            .output();
        assert!(first_remote_and_branch(vault.path(), None).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_changes_with_no_remote_reports_failed_push() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        refresh_git_status(&mut app);
        let outcome = sync_changes(&mut app, Some("first"));
        // Commit succeeds, pull is skipped (no upstream), push fails
        // (no remote configured at all → not "needs upstream").
        match outcome {
            SyncOutcome::Failed { stage, .. } => assert_eq!(stage, SyncStage::Push),
            other => panic!("expected push failure, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_changes_with_remote_but_no_upstream_returns_needs_upstream() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        let remote = TempDir::new().unwrap();
        init_bare_remote(&remote);
        add_origin(vault.path(), remote.path());
        refresh_git_status(&mut app);
        let outcome = sync_changes(&mut app, Some("first"));
        match outcome {
            SyncOutcome::NeedsUpstream { remote, branch } => {
                assert_eq!(remote, "origin");
                assert_eq!(branch, "main");
            }
            other => panic!("expected NeedsUpstream, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn push_with_set_upstream_succeeds_after_first_commit() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        let remote = TempDir::new().unwrap();
        init_bare_remote(&remote);
        add_origin(vault.path(), remote.path());
        refresh_git_status(&mut app);
        // First commit happens via sync (push then NeedsUpstream).
        let _ = sync_changes(&mut app, Some("first"));
        let r = push_with_set_upstream(&mut app, "origin", "main");
        assert!(r.is_ok(), "push -u should succeed: {:?}", r);
        // After push -u, status should now have an upstream.
        refresh_git_status(&mut app);
        assert!(app
            .git_panel
            .status
            .as_ref()
            .and_then(|s| s.upstream.as_deref())
            .is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refresh_clamps_selection_when_list_shrinks() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        refresh_git_status(&mut app);
        // 1 untracked file → selected may be 0; force it past the end
        // and ensure refresh clamps.
        app.git_panel.selected = 9;
        refresh_git_status(&mut app);
        let len = app.git_panel.status.as_ref().unwrap().changed.len();
        assert!(app.git_panel.selected <= len.saturating_sub(1));
    }
}
