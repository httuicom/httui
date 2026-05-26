//! Branch-picker modal handlers — opened from the git panel
//! (`Ctrl+B`). Public surface still lives in [`super::git_panel`].

use crate::app::{App, StatusKind};
use crate::git::GitBranchPickerState;
use crate::modal::Modal;
use crate::vim::mode::Mode;

pub(super) fn open(app: &mut App) {
    let vault = app.vault_path.clone();
    match httui_core::git::status::git_branch_list(&vault) {
        Ok(branches) if !branches.is_empty() => {
            app.modal = Some(Modal::GitBranchPicker(GitBranchPickerState::new(branches)));
            app.vim.mode = Mode::Modal;
        }
        Ok(_) => {
            // Empty list = repo has no commits yet (no
            // `refs/heads/<branch>` to enumerate). Surface the fix.
            app.set_status(
                StatusKind::Info,
                "no branches yet — make a commit first".to_string(),
            );
        }
        Err(msg) => {
            app.set_status(
                StatusKind::Error,
                format!("git branch: {}", msg.lines().next().unwrap_or("")),
            );
        }
    }
}

pub(super) fn close(app: &mut App) {
    if matches!(app.modal, Some(Modal::GitBranchPicker(_))) {
        app.modal = None;
        app.vim.mode = Mode::Git;
    }
}

pub(super) fn move_cursor(app: &mut App, delta: i32) {
    if let Some(Modal::GitBranchPicker(state)) = app.modal.as_mut() {
        state.move_cursor(delta);
    }
}

pub(super) fn confirm(app: &mut App) {
    let target = match app.modal.as_ref() {
        Some(Modal::GitBranchPicker(s)) => s.branches.get(s.selected).map(|b| b.name.clone()),
        _ => None,
    };
    let Some(branch) = target else {
        return;
    };
    let vault = app.vault_path.clone();
    let short = strip_remote_prefix(&branch);
    match httui_core::git::checkout::git_checkout(&vault, &short) {
        Ok(()) => {
            app.modal = None;
            app.vim.mode = Mode::Git;
            crate::commands::git::refresh_git_status(app);
            app.set_status(StatusKind::Info, format!("Switched to {short}"));
        }
        Err(msg) => {
            if let Some(Modal::GitBranchPicker(state)) = app.modal.as_mut() {
                state.error = Some(msg.lines().next().unwrap_or("").to_string());
            }
        }
    }
}

fn strip_remote_prefix(name: &str) -> String {
    if let Some((remote, rest)) = name.split_once('/') {
        if ["origin", "upstream"].contains(&remote) {
            return rest.to_string();
        }
    }
    name.to_string()
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

    fn seed_branch(vault: &std::path::Path, name: &str) {
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault)
            .args(["branch", name])
            .output()
            .unwrap();
    }

    fn seed_commit(vault: &std::path::Path, file: &str, body: &str, msg: &str) {
        std::fs::write(vault.join(file), body).unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault)
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault)
            .args(["commit", "-m", msg])
            .output()
            .unwrap();
    }

    #[test]
    fn strip_remote_prefix_strips_origin_and_upstream() {
        assert_eq!(strip_remote_prefix("origin/main"), "main");
        assert_eq!(strip_remote_prefix("upstream/feat"), "feat");
        assert_eq!(strip_remote_prefix("local-branch"), "local-branch");
        assert_eq!(strip_remote_prefix("fork/branch"), "fork/branch");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_without_commits_emits_actionable_status() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        open(&mut app);
        let msg = app.status_message.as_ref().expect("status set");
        assert!(msg.text.contains("no branches"));
        assert!(app.modal.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_on_non_git_vault_emits_error_status() {
        let (mut app, _d, _v) = build_app().await;
        open(&mut app);
        assert!(app.modal.is_none());
        let msg = app.status_message.as_ref().expect("status set");
        assert!(msg.text.starts_with("git branch:"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_and_move_cursor_handle_modal_state() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        seed_commit(vault.path(), "a.md", "x\n", "seed");
        seed_branch(vault.path(), "feature");
        open(&mut app);
        assert!(matches!(app.modal, Some(Modal::GitBranchPicker(_))));
        move_cursor(&mut app, 1);
        close(&mut app);
        assert!(app.modal.is_none());
        assert_eq!(app.vim.mode, Mode::Git);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_without_modal_is_noop() {
        let (mut app, _d, _v) = build_app().await;
        confirm(&mut app);
        assert!(app.modal.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_surfaces_checkout_error_inline() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        seed_commit(vault.path(), "a.md", "x\n", "seed");
        open(&mut app);
        // Inject a bogus branch so the checkout step fails.
        if let Some(Modal::GitBranchPicker(s)) = app.modal.as_mut() {
            s.branches.push(httui_core::git::status::BranchInfo {
                name: "../bad-name".into(),
                current: false,
                remote: false,
            });
            s.selected = s.branches.len() - 1;
        }
        confirm(&mut app);
        match app.modal.as_ref() {
            Some(Modal::GitBranchPicker(s)) => {
                assert!(s.error.is_some(), "error surfaced inline");
            }
            other => panic!("expected picker, got {other:?}"),
        }
    }
}
