//! Git panel state. `httui_core::git` owns every git invocation;
//! this module only holds the panel's UI state so the renderer is a
//! pure projection.

use httui_core::git::log::CommitInfo;
use httui_core::git::status::{DiffMetrics, GitStatus};

use crate::vim::lineedit::LineEdit;

pub mod template;

/// Recent commits shown inline in the HISTORY section. Full log
/// lives behind `Ctrl+L` (`Modal::GitLogPage`).
pub const HISTORY_PREVIEW_COUNT: usize = 3;

/// Set-upstream confirm modal — asks whether to push with
/// `-u <remote>`.
#[derive(Debug, Clone)]
pub struct GitSetUpstreamConfirmState {
    pub remote: String,
    pub branch: String,
}

/// Branch picker modal state.
#[derive(Debug, Clone)]
pub struct GitBranchPickerState {
    pub branches: Vec<httui_core::git::status::BranchInfo>,
    pub selected: usize,
    /// Last checkout error — keeps the picker open and rendered inline.
    pub error: Option<String>,
}

/// `Base` writes the merge-ancestor blob; `Ours` keeps HEAD;
/// `Theirs` takes the incoming side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictVersion {
    Base,
    Ours,
    Theirs,
}

/// Full-screen 3-way conflict resolver state.
#[derive(Debug)]
pub struct GitConflictResolverState {
    pub files: Vec<String>,
    pub selected_file: usize,
    /// Cached three-stage versions for the current file. Reset on
    /// cursor move.
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

/// Full-screen git log page state — commit list + diff for the
/// selected commit.
#[derive(Debug)]
pub struct GitLogPageState {
    pub commits: Vec<httui_core::git::log::CommitInfo>,
    pub selected: usize,
    /// Cached diff for the selected commit. `None` while loading.
    pub diff: Option<String>,
    pub error: Option<String>,
    /// Vertical scroll into the diff pane; independent of `selected`.
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
    pub visible: bool,
    /// `None` until the first refresh, or when the vault isn't a git
    /// repo (`git_status` returns `Err`).
    pub status: Option<GitStatus>,
    /// Diff against `HEAD`. Tracked changes only — untracked files
    /// have no baseline and never contribute to +/- counts.
    pub metrics: DiffMetrics,
    pub status_error: Option<String>,
    /// File-list cursor into [`GitStatus::changed`]. Clamped after
    /// every refresh.
    pub selected: usize,
    /// Commit draft. Empty buffer + submit triggers template prefill
    /// (see [`template::commit_template`]).
    pub commit_message: LineEdit,
    /// Cleared on the next edit keystroke so progress is visible.
    pub commit_error: Option<String>,
    pub recent_commits: Vec<CommitInfo>,
    /// `git commit --amend` toggle. Reset to `false` after every
    /// successful commit.
    pub amend: bool,
}

impl GitPanel {
    /// Returns the new visibility state.
    pub fn toggle_visible(&mut self) -> bool {
        self.visible = !self.visible;
        self.visible
    }
}

#[cfg(test)]
pub(crate) mod test_helpers {
    //! Mirror of `httui_core::git::test_helpers` (that module is
    //! `pub(crate)` so it can't cross crates). Used by panel /
    //! commands / apply tests to share repo-init boilerplate.

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
