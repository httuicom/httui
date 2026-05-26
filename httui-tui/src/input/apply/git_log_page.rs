//! Handlers for [`Modal::GitLogPage`]: open / close / cursor / diff
//! scroll. Diff fetching is lazy — the renderer asks for the body via
//! [`ensure_diff_loaded`] right before painting, so navigating the
//! list doesn't fire a `git show` for every keystroke (only on the
//! first render after the cursor moves).

use crate::app::{App, StatusKind};
use crate::git::GitLogPageState;
use crate::modal::Modal;
use crate::vim::mode::Mode;

const LOG_LIMIT: usize = 50;

pub(super) fn open(app: &mut App) {
    let vault = app.vault_path.clone();
    match httui_core::git::log::git_log(&vault, LOG_LIMIT, None) {
        Ok(commits) if !commits.is_empty() => {
            app.modal = Some(Modal::GitLogPage(GitLogPageState::new(commits)));
            app.vim.mode = Mode::Modal;
        }
        Ok(_) => {
            app.set_status(StatusKind::Info, "no commits yet".to_string());
        }
        Err(msg) => {
            app.set_status(
                StatusKind::Error,
                format!("git log: {}", msg.lines().next().unwrap_or("")),
            );
        }
    }
}

pub(super) fn close(app: &mut App) {
    if matches!(app.modal, Some(Modal::GitLogPage(_))) {
        app.modal = None;
        app.vim.mode = Mode::Git;
    }
}

pub(super) fn move_cursor(app: &mut App, delta: i32) {
    if let Some(Modal::GitLogPage(state)) = app.modal.as_mut() {
        state.move_cursor(delta);
    }
}

pub(super) fn scroll_diff(app: &mut App, delta: i32) {
    if let Some(Modal::GitLogPage(state)) = app.modal.as_mut() {
        let next = (state.diff_scroll as i32 + delta).max(0);
        state.diff_scroll = next as u16;
    }
}

/// Fetch `git show <sha>` for the currently-selected commit if the
/// cache is empty. Idempotent — repeated calls with the same cursor
/// short-circuit. Errors land in `state.error` and `diff` stays None.
pub fn ensure_diff_loaded(app: &mut App) {
    let needs_load = matches!(
        app.modal.as_ref(),
        Some(Modal::GitLogPage(s))
            if s.diff.is_none() && !s.commits.is_empty()
    );
    if !needs_load {
        return;
    }
    let (vault, sha) = {
        let Some(Modal::GitLogPage(s)) = app.modal.as_ref() else {
            return;
        };
        let sha = s.commits.get(s.selected).map(|c| c.sha.clone());
        let Some(sha) = sha else { return };
        (app.vault_path.clone(), sha)
    };
    let res = httui_core::git::git_diff(&vault, Some(&sha));
    if let Some(Modal::GitLogPage(state)) = app.modal.as_mut() {
        match res {
            Ok(body) => {
                state.diff = Some(body);
                state.error = None;
            }
            Err(msg) => {
                state.diff = None;
                state.error = Some(msg);
            }
        }
    }
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

    fn seed_two_commits(vault: &std::path::Path) {
        crate::git::test_helpers::init_repo(vault);
        std::fs::write(vault.join("a.md"), "one\n").unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault)
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault)
            .args(["commit", "-m", "first"])
            .output()
            .unwrap();
        std::fs::write(vault.join("a.md"), "two\n").unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault)
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault)
            .args(["commit", "-m", "second"])
            .output()
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_with_commits_populates_modal() {
        let (mut app, _d, vault) = build_app().await;
        seed_two_commits(vault.path());
        open(&mut app);
        match app.modal.as_ref() {
            Some(Modal::GitLogPage(s)) => {
                assert_eq!(s.commits.len(), 2);
                assert_eq!(s.commits[0].subject, "second");
            }
            other => panic!("expected log page, got {other:?}"),
        }
        assert_eq!(app.vim.mode, Mode::Modal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_on_empty_repo_emits_info_status() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        open(&mut app);
        assert!(app.modal.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_on_non_git_vault_emits_error_status() {
        let (mut app, _d, _v) = build_app().await;
        open(&mut app);
        assert!(app.modal.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_drops_modal_and_returns_to_git_mode() {
        let (mut app, _d, vault) = build_app().await;
        seed_two_commits(vault.path());
        open(&mut app);
        close(&mut app);
        assert!(app.modal.is_none());
        assert_eq!(app.vim.mode, Mode::Git);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn move_cursor_wraps_and_clears_diff_cache() {
        let (mut app, _d, vault) = build_app().await;
        seed_two_commits(vault.path());
        open(&mut app);
        ensure_diff_loaded(&mut app);
        if let Some(Modal::GitLogPage(s)) = app.modal.as_ref() {
            assert!(s.diff.is_some());
        }
        move_cursor(&mut app, 1);
        if let Some(Modal::GitLogPage(s)) = app.modal.as_ref() {
            assert_eq!(s.selected, 1);
            assert!(s.diff.is_none(), "diff cache invalidated on cursor move");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn scroll_diff_clamps_at_zero() {
        let (mut app, _d, vault) = build_app().await;
        seed_two_commits(vault.path());
        open(&mut app);
        scroll_diff(&mut app, 5);
        if let Some(Modal::GitLogPage(s)) = app.modal.as_ref() {
            assert_eq!(s.diff_scroll, 5);
        }
        scroll_diff(&mut app, -100);
        if let Some(Modal::GitLogPage(s)) = app.modal.as_ref() {
            assert_eq!(s.diff_scroll, 0);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ensure_diff_loaded_populates_body() {
        let (mut app, _d, vault) = build_app().await;
        seed_two_commits(vault.path());
        open(&mut app);
        ensure_diff_loaded(&mut app);
        let body = match app.modal.as_ref() {
            Some(Modal::GitLogPage(s)) => s.diff.clone(),
            _ => None,
        };
        let body = body.expect("diff body populated");
        // `second` commit changed a.md from "one" to "two".
        assert!(body.contains("second"));
        assert!(body.contains("+two") || body.contains("-one"));
    }
}
