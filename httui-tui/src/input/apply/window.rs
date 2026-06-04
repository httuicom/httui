// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
//! Window / split pane commands. Mechanically moved out of
//! `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p5a) with no logic
//! change.

use crate::app::{App, StatusKind};
use crate::input::action::Action;
use crate::input::types::WindowCmd;
use crate::pane::{FocusDir, SplitDir};

/// `apply_action` sub-match for the window/split domain. Mechanically
/// split out of the `apply_action` router in `vim/dispatch.rs` (tui-v2
/// vertical 1, fase 1 p6a) — the arm bodies are copied verbatim. The
/// outer router routes only the variants in this group here, so the
/// `unreachable!` is a compile-time-backed invariant.
pub(crate) fn apply_window(app: &mut App, action: Action, _recording: bool) {
    match action {
        Action::Window(cmd) => apply_window_cmd(app, cmd),
        _ => unreachable!("apply_window: variante fora do grupo"),
    }
}

// ───────────── window / split commands ─────────────

pub(crate) fn apply_window_cmd(app: &mut App, cmd: WindowCmd) {
    match cmd {
        WindowCmd::SplitVertical => split_focused(app, SplitDir::Vertical),
        WindowCmd::SplitHorizontal => split_focused(app, SplitDir::Horizontal),
        WindowCmd::FocusLeft => focus_dir(app, FocusDir::Left),
        WindowCmd::FocusRight => focus_dir(app, FocusDir::Right),
        WindowCmd::FocusUp => focus_dir(app, FocusDir::Up),
        WindowCmd::FocusDown => focus_dir(app, FocusDir::Down),
        WindowCmd::Cycle => {
            if let Some(tab) = app.active_tab_mut() {
                tab.cycle_focus();
            }
            app.refresh_viewport_for_cursor();
        }
        WindowCmd::Close => close_focused_pane(app),
        WindowCmd::Equalize => {
            if let Some(tab) = app.active_tab_mut() {
                tab.equalize();
            }
        }
    }
}

pub(crate) fn split_focused(app: &mut App, dir: SplitDir) {
    let Some(tab) = app.active_tab_mut() else {
        return;
    };
    let new_pane = tab.active_leaf().snapshot_clone();
    tab.split(dir, new_pane);
    app.refresh_viewport_for_cursor();
}

pub(crate) fn focus_dir(app: &mut App, dir: FocusDir) {
    use crate::vim::mode::Mode;
    // Sidebar (tree) is treated as a virtual pane glued to the left:
    // `Ctrl+W l` from the tree jumps into the leftmost pane, `Ctrl+W h`
    // from the leftmost pane jumps into the tree (when visible). The
    // reflex is the same as nvim-tree / vim's quickfix windows.
    let in_tree = matches!(app.vim.mode, Mode::Tree | Mode::TreePrompt);
    if in_tree {
        match dir {
            FocusDir::Right => {
                app.vim.enter_normal();
                app.refresh_viewport_for_cursor();
            }
            // Sidebar has no neighbour above/below/further-left.
            FocusDir::Left | FocusDir::Up | FocusDir::Down => {}
        }
        return;
    }
    // Ctrl+W h/j/k/l while a BLOCKS-view EDIT buffer is open should
    // commit it before moving the focus — otherwise the cursor stays
    // attached to the old pane's sub-doc, the user can't tell the new
    // pane is focused, and subsequent keystrokes still hit the buffer.
    if app.active_pane().is_some_and(|p| p.block_edit.is_some()) {
        crate::input::apply::blocks_view::commit_edit(app);
    }
    let moved = app
        .active_tab_mut()
        .map(|tab| tab.focus_dir(dir))
        .unwrap_or(false);
    // No pane neighbour in that direction + heading left + sidebar
    // open → cross into the tree.
    if !moved && matches!(dir, FocusDir::Left) && app.tree.visible {
        app.vim.mode = Mode::Tree;
    }
    app.refresh_viewport_for_cursor();
}

/// Close the focused pane. When it's the only pane in the active tab,
/// closes the tab; when there are no tabs left, quits.
pub(crate) fn close_focused_pane(app: &mut App) {
    let leaf_count = app.active_tab().map(|t| t.leaf_count()).unwrap_or(0);
    if leaf_count > 1 {
        if app.document().is_some_and(|d| d.is_dirty()) {
            app.set_status(
                StatusKind::Error,
                "no write since last change (add ! to override)",
            );
            return;
        }
        if let Some(tab) = app.active_tab_mut() {
            tab.close_focused();
        }
        app.refresh_viewport_for_cursor();
        return;
    }
    match app.close_tab(false) {
        Ok(msg) => app.set_status(StatusKind::Info, msg),
        Err(msg) => {
            app.set_status(StatusKind::Error, msg);
            return;
        }
    }
    if app.tabs.is_empty() {
        app.should_quit = true;
    }
}
