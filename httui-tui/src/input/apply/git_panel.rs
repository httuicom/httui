//! Git side-panel action handlers. Owns the toggle, commit-message
//! editing, and the commit submission flow. Backend wiring lives in
//! [`crate::commands::git`].

use crate::app::{App, StatusKind};
use crate::git::template::commit_template;
use crate::input::action::Action;
use crate::vim::mode::Mode;

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

    #[test]
    fn short_truncates_with_ellipsis_when_over_limit() {
        assert_eq!(short("hi", 10), "hi");
        assert_eq!(short("abcdefghij", 5), "abcde…");
        // Whitespace is trimmed before measuring.
        assert_eq!(short("  hi  ", 10), "hi");
    }
}
