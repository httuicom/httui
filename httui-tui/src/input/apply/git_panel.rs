//! Git side-panel action handlers. Owns the toggle, commit-message
//! editing, and the commit submission flow. Backend wiring lives in
//! [`crate::commands::git`].

use crate::app::{App, StatusKind};
use crate::commands::git::{SyncOutcome, SyncStage};
use crate::git::template::commit_template;
use crate::git::GitSetUpstreamConfirmState;
use crate::input::action::Action;
use crate::modal::Modal;
use crate::vim::mode::Mode;

use super::{git_branch_picker, git_log_page};

pub(crate) fn apply_git_panel(app: &mut App, action: Action) {
    match action {
        Action::GitPanelToggle => {
            let now_visible = app.git_panel.toggle_visible();
            if now_visible {
                crate::commands::git::refresh_git_status(app);
                app.vim.mode = Mode::Git;
            } else {
                close_panel(app);
            }
        }
        Action::GitPanelCancel => {
            app.git_panel.visible = false;
            close_panel(app);
        }
        Action::GitPanelChar(c) => {
            app.git_panel.commit_error = None;
            app.git_panel.commit_message.insert_char(c);
        }
        Action::GitPanelBackspace => {
            app.git_panel.commit_error = None;
            let _ = app.git_panel.commit_message.delete_before();
        }
        Action::GitPanelDelete => {
            app.git_panel.commit_error = None;
            let _ = app.git_panel.commit_message.delete_after();
        }
        Action::GitPanelCursorLeft => app.git_panel.commit_message.move_left(),
        Action::GitPanelCursorRight => app.git_panel.commit_message.move_right(),
        Action::GitPanelCursorHome => app.git_panel.commit_message.move_home(),
        Action::GitPanelCursorEnd => app.git_panel.commit_message.move_end(),
        Action::GitPanelCommit => submit_commit(app),
        Action::GitPanelSync => submit_sync(app),
        Action::GitConfirmSetUpstream => confirm_set_upstream(app),
        Action::GitCancelSetUpstream => cancel_set_upstream(app),
        Action::OpenGitBranchPicker => git_branch_picker::open(app),
        Action::CloseGitBranchPicker => git_branch_picker::close(app),
        Action::MoveGitBranchPickerCursor(delta) => git_branch_picker::move_cursor(app, delta),
        Action::ConfirmGitBranchPicker => git_branch_picker::confirm(app),
        Action::OpenGitLogPage => git_log_page::open(app),
        Action::CloseGitLogPage => git_log_page::close(app),
        Action::MoveGitLogPageCursor(delta) => git_log_page::move_cursor(app, delta),
        Action::ScrollGitLogDiff(delta) => git_log_page::scroll_diff(app, delta),
        _ => unreachable!("apply_git_panel: variante fora do grupo"),
    }
}

fn close_panel(app: &mut App) {
    if app.vim.mode == Mode::Git {
        app.vim.enter_normal();
    }
}

/// Run the commit flow: empty draft → template prefill; non-empty →
/// stage every change and run `git commit`. Success clears the
/// draft; failure parks an error message on the panel so the
/// renderer can surface it.
fn submit_commit(app: &mut App) {
    let raw = app.git_panel.commit_message.as_str().trim().to_string();
    let effective = if raw.is_empty() {
        app.git_panel
            .status
            .as_ref()
            .map(commit_template)
            .unwrap_or_default()
    } else {
        raw
    };
    if effective.is_empty() {
        app.git_panel.commit_error = Some("nothing to commit".to_string());
        return;
    }
    match crate::commands::git::commit_changes(app, &effective) {
        Ok(()) => {
            app.git_panel.commit_message =
                crate::vim::lineedit::LineEdit::from_str(String::new());
            app.git_panel.commit_error = None;
            app.set_status(
                StatusKind::Info,
                format!("Committed: {}", short(&effective, 50)),
            );
        }
        Err(msg) => {
            app.git_panel.commit_error = Some(msg);
        }
    }
}

fn submit_sync(app: &mut App) {
    let raw = app.git_panel.commit_message.as_str().trim().to_string();
    let message = if raw.is_empty() {
        app.git_panel.status.as_ref().map(commit_template)
    } else {
        Some(raw)
    };
    let has_changes = app
        .git_panel
        .status
        .as_ref()
        .map(|s| !s.changed.is_empty())
        .unwrap_or(false);
    // Only pass a commit message when there's something to commit;
    // a clean tree skips the commit step but still runs pull+push.
    let commit_message: Option<String> = if has_changes { message } else { None };
    let outcome = crate::commands::git::sync_changes(app, commit_message.as_deref());
    handle_sync_outcome(app, outcome);
}

fn handle_sync_outcome(app: &mut App, outcome: SyncOutcome) {
    match outcome {
        SyncOutcome::Done(msg) => {
            app.git_panel.commit_message =
                crate::vim::lineedit::LineEdit::from_str(String::new());
            app.git_panel.commit_error = None;
            app.set_status(StatusKind::Info, format!("Sync: {msg}"));
        }
        SyncOutcome::NeedsUpstream { remote, branch } => {
            app.modal = Some(Modal::GitSetUpstreamConfirm(GitSetUpstreamConfirmState {
                remote,
                branch,
            }));
            app.vim.mode = Mode::Modal;
        }
        SyncOutcome::Failed { stage, message } => {
            let label = match stage {
                SyncStage::Commit => "commit",
                SyncStage::Pull => "pull",
                SyncStage::Push => "push",
            };
            let line = message.lines().next().unwrap_or("").to_string();
            app.git_panel.commit_error = Some(format!("{label}: {line}"));
        }
    }
}

fn confirm_set_upstream(app: &mut App) {
    let (remote, branch) = match app.modal.as_ref() {
        Some(Modal::GitSetUpstreamConfirm(s)) => (s.remote.clone(), s.branch.clone()),
        _ => return,
    };
    app.modal = None;
    app.vim.mode = Mode::Git;
    match crate::commands::git::push_with_set_upstream(app, &remote, &branch) {
        Ok(()) => app.set_status(
            StatusKind::Info,
            format!("Pushed (-u {remote}/{branch})"),
        ),
        Err(message) => {
            app.git_panel.commit_error =
                Some(format!("push -u: {}", message.lines().next().unwrap_or("")));
        }
    }
}

fn cancel_set_upstream(app: &mut App) {
    if matches!(app.modal, Some(Modal::GitSetUpstreamConfirm(_))) {
        app.modal = None;
        app.vim.mode = Mode::Git;
    }
}

fn short(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    let cut: String = trimmed.chars().take(max).collect();
    format!("{cut}…")
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
    async fn toggle_flips_visibility_and_refreshes_on_open() {
        let (mut app, _d, _v) = build_app().await;
        assert!(!app.git_panel.visible);

        apply_git_panel(&mut app, Action::GitPanelToggle);
        assert!(app.git_panel.visible);
        assert_eq!(app.vim.mode, Mode::Git);

        apply_git_panel(&mut app, Action::GitPanelToggle);
        assert!(!app.git_panel.visible);
        assert_eq!(app.vim.mode, Mode::Normal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_closes_panel_and_returns_to_normal_mode() {
        let (mut app, _d, _v) = build_app().await;
        apply_git_panel(&mut app, Action::GitPanelToggle);
        assert!(app.git_panel.visible);
        apply_git_panel(&mut app, Action::GitPanelCancel);
        assert!(!app.git_panel.visible);
        assert_eq!(app.vim.mode, Mode::Normal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn char_action_inserts_into_commit_draft() {
        let (mut app, _d, _v) = build_app().await;
        apply_git_panel(&mut app, Action::GitPanelChar('h'));
        apply_git_panel(&mut app, Action::GitPanelChar('i'));
        assert_eq!(app.git_panel.commit_message.as_str(), "hi");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn backspace_and_delete_edit_the_draft() {
        let (mut app, _d, _v) = build_app().await;
        apply_git_panel(&mut app, Action::GitPanelChar('a'));
        apply_git_panel(&mut app, Action::GitPanelChar('b'));
        apply_git_panel(&mut app, Action::GitPanelBackspace);
        assert_eq!(app.git_panel.commit_message.as_str(), "a");
        apply_git_panel(&mut app, Action::GitPanelChar('c'));
        apply_git_panel(&mut app, Action::GitPanelCursorLeft);
        apply_git_panel(&mut app, Action::GitPanelDelete);
        assert_eq!(app.git_panel.commit_message.as_str(), "a");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cursor_navigation_actions_move_the_caret() {
        let (mut app, _d, _v) = build_app().await;
        for c in "abc".chars() {
            apply_git_panel(&mut app, Action::GitPanelChar(c));
        }
        apply_git_panel(&mut app, Action::GitPanelCursorHome);
        apply_git_panel(&mut app, Action::GitPanelChar('X'));
        assert_eq!(app.git_panel.commit_message.as_str(), "Xabc");
        apply_git_panel(&mut app, Action::GitPanelCursorEnd);
        apply_git_panel(&mut app, Action::GitPanelChar('!'));
        assert_eq!(app.git_panel.commit_message.as_str(), "Xabc!");
        apply_git_panel(&mut app, Action::GitPanelCursorLeft);
        apply_git_panel(&mut app, Action::GitPanelCursorRight);
        apply_git_panel(&mut app, Action::GitPanelChar('?'));
        assert_eq!(app.git_panel.commit_message.as_str(), "Xabc!?");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_with_no_status_snapshot_errors() {
        let (mut app, _d, _v) = build_app().await;
        // Panel opened in a non-git vault → status is None.
        apply_git_panel(&mut app, Action::GitPanelToggle);
        apply_git_panel(&mut app, Action::GitPanelChar('m'));
        apply_git_panel(&mut app, Action::GitPanelCommit);
        assert!(app.git_panel.commit_error.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_with_real_repo_clears_draft_on_success() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("note.md"), "hi\n").unwrap();
        apply_git_panel(&mut app, Action::GitPanelToggle);
        for c in "hello".chars() {
            apply_git_panel(&mut app, Action::GitPanelChar(c));
        }
        apply_git_panel(&mut app, Action::GitPanelCommit);
        assert!(app.git_panel.commit_error.is_none(), "commit should succeed");
        assert!(app.git_panel.commit_message.as_str().is_empty());
        // After commit, status snapshot is clean.
        let status = app.git_panel.status.as_ref().expect("status snapshot");
        assert!(status.clean);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn empty_draft_uses_template_prefill() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("note.md"), "hi\n").unwrap();
        apply_git_panel(&mut app, Action::GitPanelToggle);
        // No keys typed → commit_message empty → template "Update note".
        apply_git_panel(&mut app, Action::GitPanelCommit);
        assert!(app.git_panel.commit_error.is_none(), "template commit should succeed");
        // git log to verify subject.
        let log = std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["log", "-1", "--pretty=%s"])
            .output()
            .unwrap();
        let subject = String::from_utf8_lossy(&log.stdout).trim().to_string();
        assert_eq!(subject, "Update note");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic(expected = "apply_git_panel: variante fora do grupo")]
    async fn unexpected_variant_panics() {
        let (mut app, _d, _v) = build_app().await;
        apply_git_panel(&mut app, Action::Quit);
    }

    fn init_bare_remote(remote: &TempDir) {
        std::process::Command::new("git")
            .args(["init", "--bare", "--initial-branch=main"])
            .arg(remote.path())
            .output()
            .unwrap();
    }

    fn add_origin(vault_path: &std::path::Path, remote_path: &std::path::Path) {
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault_path)
            .args(["remote", "add", "origin"])
            .arg(remote_path)
            .output()
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_with_no_remote_records_push_failure_on_panel() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        apply_git_panel(&mut app, Action::GitPanelToggle);
        for c in "hello".chars() {
            apply_git_panel(&mut app, Action::GitPanelChar(c));
        }
        apply_git_panel(&mut app, Action::GitPanelSync);
        let err = app
            .git_panel
            .commit_error
            .as_ref()
            .expect("push failure surfaced");
        assert!(err.starts_with("push:"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_with_remote_no_upstream_opens_modal_and_drops_to_modal_mode() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        let remote = TempDir::new().unwrap();
        init_bare_remote(&remote);
        add_origin(vault.path(), remote.path());
        apply_git_panel(&mut app, Action::GitPanelToggle);
        for c in "hello".chars() {
            apply_git_panel(&mut app, Action::GitPanelChar(c));
        }
        apply_git_panel(&mut app, Action::GitPanelSync);
        match app.modal.as_ref() {
            Some(Modal::GitSetUpstreamConfirm(s)) => {
                assert_eq!(s.remote, "origin");
                assert_eq!(s.branch, "main");
            }
            other => panic!("expected modal, got {other:?}"),
        }
        assert_eq!(app.vim.mode, Mode::Modal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_set_upstream_pushes_and_returns_to_git_mode() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        let remote = TempDir::new().unwrap();
        init_bare_remote(&remote);
        add_origin(vault.path(), remote.path());
        apply_git_panel(&mut app, Action::GitPanelToggle);
        apply_git_panel(&mut app, Action::GitPanelChar('h'));
        apply_git_panel(&mut app, Action::GitPanelSync);
        assert!(matches!(app.modal, Some(Modal::GitSetUpstreamConfirm(_))));
        apply_git_panel(&mut app, Action::GitConfirmSetUpstream);
        assert!(app.modal.is_none());
        assert_eq!(app.vim.mode, Mode::Git);
        // Status now has upstream.
        crate::commands::git::refresh_git_status(&mut app);
        assert!(app
            .git_panel
            .status
            .as_ref()
            .and_then(|s| s.upstream.as_deref())
            .is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_set_upstream_closes_modal_and_returns_to_git_mode() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        let remote = TempDir::new().unwrap();
        init_bare_remote(&remote);
        add_origin(vault.path(), remote.path());
        apply_git_panel(&mut app, Action::GitPanelToggle);
        apply_git_panel(&mut app, Action::GitPanelChar('h'));
        apply_git_panel(&mut app, Action::GitPanelSync);
        apply_git_panel(&mut app, Action::GitCancelSetUpstream);
        assert!(app.modal.is_none());
        assert_eq!(app.vim.mode, Mode::Git);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_set_upstream_without_modal_is_noop() {
        let (mut app, _d, _v) = build_app().await;
        apply_git_panel(&mut app, Action::GitConfirmSetUpstream);
        // No modal, no panic, no state change.
        assert!(app.modal.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_clean_tree_runs_push_only() {
        // Clean tree → no commit step; push still attempted (and
        // fails here since there's no remote). What we verify is the
        // commit-error stays unset (no "commit:" prefix).
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        // Commit existing change manually so tree is clean.
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["commit", "-m", "seed"])
            .output()
            .unwrap();
        apply_git_panel(&mut app, Action::GitPanelToggle);
        apply_git_panel(&mut app, Action::GitPanelSync);
        let err = app
            .git_panel
            .commit_error
            .as_ref()
            .expect("push fails — no remote");
        assert!(err.starts_with("push:"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_branch_picker_with_real_repo_seeds_state() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["commit", "-m", "seed"])
            .output()
            .unwrap();
        apply_git_panel(&mut app, Action::GitPanelToggle);
        apply_git_panel(&mut app, Action::OpenGitBranchPicker);
        match app.modal.as_ref() {
            Some(Modal::GitBranchPicker(s)) => {
                assert!(s.branches.iter().any(|b| b.name == "main"));
            }
            other => panic!("expected branch picker, got {other:?}"),
        }
        assert_eq!(app.vim.mode, Mode::Modal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn move_and_close_branch_picker() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["commit", "-m", "seed"])
            .output()
            .unwrap();
        apply_git_panel(&mut app, Action::GitPanelToggle);
        apply_git_panel(&mut app, Action::OpenGitBranchPicker);
        // Single branch — move wraps back to 0.
        apply_git_panel(&mut app, Action::MoveGitBranchPickerCursor(1));
        match app.modal.as_ref() {
            Some(Modal::GitBranchPicker(s)) => assert_eq!(s.selected, 0),
            _ => panic!("expected branch picker"),
        }
        apply_git_panel(&mut app, Action::CloseGitBranchPicker);
        assert!(app.modal.is_none());
        assert_eq!(app.vim.mode, Mode::Git);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_branch_picker_switches_branch() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["commit", "-m", "seed"])
            .output()
            .unwrap();
        // Create a second branch so the picker has something to switch to.
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["branch", "feature"])
            .output()
            .unwrap();
        apply_git_panel(&mut app, Action::GitPanelToggle);
        apply_git_panel(&mut app, Action::OpenGitBranchPicker);
        // Select `feature` (whichever index it's at).
        if let Some(Modal::GitBranchPicker(s)) = app.modal.as_mut() {
            s.selected = s
                .branches
                .iter()
                .position(|b| b.name == "feature")
                .expect("feature branch in list");
        }
        apply_git_panel(&mut app, Action::ConfirmGitBranchPicker);
        assert!(app.modal.is_none(), "modal closes on success");
        assert_eq!(app.vim.mode, Mode::Git);
        // Status refreshed → branch chip is `feature`.
        assert_eq!(
            app.git_panel
                .status
                .as_ref()
                .and_then(|s| s.branch.as_deref()),
            Some("feature"),
        );
    }

    #[test]
    fn short_truncates_with_ellipsis_when_over_limit() {
        assert_eq!(short("hi", 10), "hi");
        assert_eq!(short("abcdefghij", 5), "abcde…");
        // Whitespace is trimmed before measuring.
        assert_eq!(short("  hi  ", 10), "hi");
    }
}
