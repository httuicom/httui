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

    #[test]
    fn strip_remote_prefix_strips_origin_and_upstream() {
        assert_eq!(strip_remote_prefix("origin/main"), "main");
        assert_eq!(strip_remote_prefix("upstream/feat"), "feat");
        assert_eq!(strip_remote_prefix("local-branch"), "local-branch");
        assert_eq!(strip_remote_prefix("fork/branch"), "fork/branch");
    }
}
