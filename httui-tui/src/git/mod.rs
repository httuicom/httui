//! Git panel state — sidebar mounted to the right of the editor.
//!
//! Mirrors `GitSidePanel` from the desktop (V10.1) functionally, not
//! visually. `httui_core::git` owns every git invocation; this module
//! only holds the panel's UI state (visibility, status snapshot,
//! commit-form draft, list selection) so the renderer is a pure
//! projection.

use httui_core::git::log::CommitInfo;
use httui_core::git::status::{DiffMetrics, GitStatus};

use crate::vim::lineedit::LineEdit;

pub mod template;

/// Number of recent commits surfaced inline in the panel's HISTORY
/// section. The desktop's `GitSidePanel` keeps this short (3-5);
/// "View all" jumps to the full-screen log page (`Ctrl+L`).
pub const HISTORY_PREVIEW_COUNT: usize = 3;

/// State carried by the set-upstream confirm modal. The user is
/// asked whether to push the current branch with `-u <remote>`.
#[derive(Debug, Clone)]
pub struct GitSetUpstreamConfirmState {
    pub remote: String,
    pub branch: String,
}

/// State of the branch picker modal (opened by `Ctrl+B` while the
/// git panel is focused). Lists every local branch + remote-tracking
/// branch; Enter checks out the highlighted entry.
#[derive(Debug, Clone)]
pub struct GitBranchPickerState {
    pub branches: Vec<httui_core::git::status::BranchInfo>,
    pub selected: usize,
    /// Last error from the checkout attempt, kept around so the
    /// renderer can show it inline (the picker stays open).
    pub error: Option<String>,
}

/// One of the three versions a conflicted file can be resolved to.
/// `Base` writes the merge-ancestor blob; `Ours` keeps HEAD; `Theirs`
/// takes the incoming side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictVersion {
    Base,
    Ours,
    Theirs,
}

/// State of the full-screen 3-way conflict resolver. Opened from
/// the git panel when there are unmerged files in the working tree.
#[derive(Debug)]
pub struct GitConflictResolverState {
    pub files: Vec<String>,
    pub selected_file: usize,
    /// Cached three-stage versions for `files[selected_file]`.
    /// Refreshed when the file cursor moves.
    pub versions: Option<httui_core::git::conflict::ConflictVersions>,
    pub error: Option<String>,
}

impl GitConflictResolverState {
    pub fn new(files: Vec<String>) -> Self {
        Self {
            files,
            selected_file: 0,
            versions: None,
            error: None,
        }
    }

    pub fn move_file_cursor(&mut self, delta: i32) {
        if self.files.is_empty() {
            return;
        }
        let len = self.files.len() as i32;
        let next = (self.selected_file as i32 + delta).rem_euclid(len);
        self.selected_file = next as usize;
        self.versions = None;
        self.error = None;
    }

    pub fn current_file(&self) -> Option<&str> {
        self.files.get(self.selected_file).map(|s| s.as_str())
    }
}

/// State for the full-screen git log page (opened by `Ctrl+L` while
/// the git panel is focused). Lists the last N commits on the left;
/// the diff for the selected commit fills the right pane.
#[derive(Debug)]
pub struct GitLogPageState {
    pub commits: Vec<httui_core::git::log::CommitInfo>,
    pub selected: usize,
    /// Cached diff body for `commits[selected]`. Refreshed when the
    /// cursor moves; `None` while loading.
    pub diff: Option<String>,
    pub error: Option<String>,
    /// Vertical scroll offset into the diff. Up/Down inside the diff
    /// pane scrolls without changing the selected commit.
    pub diff_scroll: u16,
}

impl GitLogPageState {
    pub fn new(commits: Vec<httui_core::git::log::CommitInfo>) -> Self {
        Self {
            commits,
            selected: 0,
            diff: None,
            error: None,
            diff_scroll: 0,
        }
    }

    pub fn move_cursor(&mut self, delta: i32) {
        if self.commits.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.commits.len() as i32;
        let next = (self.selected as i32 + delta).rem_euclid(len);
        self.selected = next as usize;
        self.diff = None;
        self.diff_scroll = 0;
    }
}

impl GitBranchPickerState {
    pub fn new(branches: Vec<httui_core::git::status::BranchInfo>) -> Self {
        let selected = branches.iter().position(|b| b.current).unwrap_or(0);
        Self {
            branches,
            selected,
            error: None,
        }
    }

    pub fn move_cursor(&mut self, delta: i32) {
        if self.branches.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.branches.len() as i32;
        let next = (self.selected as i32 + delta).rem_euclid(len);
        self.selected = next as usize;
    }
}

#[derive(Debug, Default)]
pub struct GitPanel {
    /// `true` when the side panel is rendered next to the editor.
    /// Toggled by `Ctrl+G` (chord shared by vim + standard profiles).
    pub visible: bool,
    /// Last `git status` snapshot, refreshed on open and after each
    /// commit / stage / sync. `None` until the first refresh (or when
    /// the vault isn't a git repo — `git_status` returns `Err`).
    pub status: Option<GitStatus>,
    /// Diff against `HEAD` aggregated by `git diff --shortstat`.
    /// Tracked changes only — untracked files contribute to
    /// [`status.changed`](GitStatus::changed) but never to +/- counts
    /// because they have no baseline.
    pub metrics: DiffMetrics,
    /// Last error from `git_status`, kept around so the renderer can
    /// surface "not a git repo" / `git` missing without panicking.
    pub status_error: Option<String>,
    /// Index into [`GitStatus::changed`] for the file-list cursor.
    /// Clamped after every refresh.
    pub selected: usize,
    /// Commit-message draft. Empty buffer + submit triggers the
    /// template prefill (see [`template::commit_template`]).
    pub commit_message: LineEdit,
    /// Last error from a failed commit attempt (nothing to commit,
    /// `git commit` rejected by hook, etc.). Cleared on the next
    /// edit keystroke so the user sees they're making progress.
    pub commit_error: Option<String>,
    /// Latest `HISTORY_PREVIEW_COUNT` commits, refreshed alongside
    /// the status snapshot. Surfaced inline in the panel; the full
    /// log lives behind `Ctrl+L` (`Modal::GitLogPage`).
    pub recent_commits: Vec<CommitInfo>,
    /// `git commit --amend` toggle. Reset to `false` after every
    /// successful commit so the next one defaults to a normal
    /// commit. Flipped by `Ctrl+A` while the panel is focused.
    pub amend: bool,
}

impl GitPanel {
    /// Flip [`visible`](Self::visible). Returns the new state so the
    /// caller can decide what else to do (refresh status on open,
    /// hand focus back to the editor on close).
    pub fn toggle_visible(&mut self) -> bool {
        self.visible = !self.visible;
        self.visible
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    //! Local mirror of `httui_core::git::test_helpers` — that module
    //! is `pub(crate)` so it can't be reached from another crate.
    //! Tests across `httui-tui` (panel / commands / apply) share these
    //! helpers to keep repo-init boilerplate out of every test.

    use std::path::Path;
    use std::process::Command;

    fn git() -> Command {
        let mut c = Command::new("git");
        for v in [
            "GIT_DIR",
            "GIT_INDEX_FILE",
            "GIT_WORK_TREE",
            "GIT_AUTHOR_NAME",
            "GIT_AUTHOR_EMAIL",
            "GIT_COMMITTER_NAME",
            "GIT_COMMITTER_EMAIL",
        ] {
            c.env_remove(v);
        }
        c
    }

    pub fn init_repo(path: &Path) {
        let init = git()
            .args(["init", "-b", "main"])
            .arg(path)
            .output()
            .expect("git init");
        assert!(init.status.success(), "git init failed");
        for (k, v) in [
            ("user.email", "test@httui.local"),
            ("user.name", "Test"),
            ("commit.gpgsign", "false"),
        ] {
            let r = git()
                .arg("-C")
                .arg(path)
                .args(["config", k, v])
                .output()
                .expect("git config");
            assert!(r.status.success(), "git config {k} failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_panel_is_hidden_with_no_status() {
        let p = GitPanel::default();
        assert!(!p.visible);
        assert!(p.status.is_none());
        assert!(p.status_error.is_none());
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn toggle_visible_flips_and_returns_new_state() {
        let mut p = GitPanel::default();
        assert!(p.toggle_visible());
        assert!(p.visible);
        assert!(!p.toggle_visible());
        assert!(!p.visible);
    }
}
