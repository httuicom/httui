//! Line-edit prompt key decoders — the generic `parse_lineedit_prompt`
//! plus the per-prompt wrappers (cmdline / search / tree-prompt /
//! fence-edit / tree / quick-open). Mechanically moved out of
//! `vim/parser.rs` (tui-v2 vertical 1, fase 1 p3b) with no logic change.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;
use crate::input::types::LineEditAction;

/// Generic LineEdit prompt key decoder. Each prompt mode maps the
/// abstract action set to its concrete `Action` variant.
pub(crate) fn parse_lineedit_prompt<F>(key: KeyEvent, mut emit: F) -> Action
where
    F: FnMut(LineEditAction) -> Action,
{
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => emit(LineEditAction::Cancel),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => emit(LineEditAction::Cancel),
        (_, KeyCode::Enter) => emit(LineEditAction::Execute),
        (_, KeyCode::Backspace) => emit(LineEditAction::Backspace),
        (_, KeyCode::Delete) => emit(LineEditAction::Delete),
        (_, KeyCode::Left) => emit(LineEditAction::CursorLeft),
        (_, KeyCode::Right) => emit(LineEditAction::CursorRight),
        (_, KeyCode::Home) => emit(LineEditAction::CursorHome),
        (_, KeyCode::End) => emit(LineEditAction::CursorEnd),
        (KeyModifiers::CONTROL, KeyCode::Char('a')) => emit(LineEditAction::CursorHome),
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => emit(LineEditAction::CursorEnd),
        (KeyModifiers::CONTROL, KeyCode::Char('b')) => emit(LineEditAction::CursorLeft),
        (KeyModifiers::CONTROL, KeyCode::Char('f')) => emit(LineEditAction::CursorRight),
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => emit(LineEditAction::Delete),
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            emit(LineEditAction::Char(c))
        }
        _ => Action::Noop,
    }
}

/// Translate one key in command-line mode (the `:` prompt).
pub fn parse_cmdline(key: KeyEvent) -> Action {
    parse_lineedit_prompt(key, |op| match op {
        LineEditAction::Cancel => Action::CmdlineCancel,
        LineEditAction::Execute => Action::CmdlineExecute,
        LineEditAction::Char(c) => Action::CmdlineChar(c),
        LineEditAction::Backspace => Action::CmdlineBackspace,
        LineEditAction::Delete => Action::CmdlineDelete,
        LineEditAction::CursorLeft => Action::CmdlineCursorLeft,
        LineEditAction::CursorRight => Action::CmdlineCursorRight,
        LineEditAction::CursorHome => Action::CmdlineCursorHome,
        LineEditAction::CursorEnd => Action::CmdlineCursorEnd,
    })
}

/// Translate one key in search mode (the `/` or `?` prompt).
pub fn parse_search(key: KeyEvent) -> Action {
    parse_lineedit_prompt(key, |op| match op {
        LineEditAction::Cancel => Action::SearchCancel,
        LineEditAction::Execute => Action::SearchExecute,
        LineEditAction::Char(c) => Action::SearchChar(c),
        LineEditAction::Backspace => Action::SearchBackspace,
        LineEditAction::Delete => Action::SearchDelete,
        LineEditAction::CursorLeft => Action::SearchCursorLeft,
        LineEditAction::CursorRight => Action::SearchCursorRight,
        LineEditAction::CursorHome => Action::SearchCursorHome,
        LineEditAction::CursorEnd => Action::SearchCursorEnd,
    })
}

/// Translate one key inside the in-tree prompt (`a`/`r`/`d` shortcuts).
/// Mirrors `parse_cmdline` shape but emits tree-prompt-specific actions.
/// Supports cursor navigation: arrows, Home/End, Delete, plus the
/// emacs-style Ctrl-A/E/B/F/D shortcuts most TUI prompts honor.
pub fn parse_tree_prompt(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::TreePromptCancel,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::TreePromptCancel,
        (_, KeyCode::Enter) => Action::TreePromptExecute,
        (_, KeyCode::Backspace) => Action::TreePromptBackspace,
        (_, KeyCode::Delete) => Action::TreePromptDelete,
        (_, KeyCode::Left) => Action::TreePromptCursorLeft,
        (_, KeyCode::Right) => Action::TreePromptCursorRight,
        (_, KeyCode::Home) => Action::TreePromptCursorHome,
        (_, KeyCode::End) => Action::TreePromptCursorEnd,
        (KeyModifiers::CONTROL, KeyCode::Char('a')) => Action::TreePromptCursorHome,
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => Action::TreePromptCursorEnd,
        (KeyModifiers::CONTROL, KeyCode::Char('b')) => Action::TreePromptCursorLeft,
        (KeyModifiers::CONTROL, KeyCode::Char('f')) => Action::TreePromptCursorRight,
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => Action::TreePromptDelete,
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            Action::TreePromptChar(c)
        }
        _ => Action::Noop,
    }
}

/// Translate one key inside the inline fence-edit prompt (alias /
/// limit / timeout). Same emacs-style shortcuts as the tree prompt
/// — keeps muscle memory consistent across all TUI prompts.
///
/// Note: `Ctrl-A` here is `CursorHome`, NOT "open alias edit". The
/// "open alias edit" chord (`<C-a>`) only fires in normal mode; once
/// we're inside the prompt, the same chord becomes the standard
/// emacs jump-to-line-start.
pub fn parse_fence_edit(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::FenceEditCancel,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::FenceEditCancel,
        (_, KeyCode::Enter) => Action::FenceEditConfirm,
        (_, KeyCode::Backspace) => Action::FenceEditBackspace,
        (_, KeyCode::Delete) => Action::FenceEditDelete,
        (_, KeyCode::Left) => Action::FenceEditCursorLeft,
        (_, KeyCode::Right) => Action::FenceEditCursorRight,
        (_, KeyCode::Home) => Action::FenceEditCursorHome,
        (_, KeyCode::End) => Action::FenceEditCursorEnd,
        (KeyModifiers::CONTROL, KeyCode::Char('a')) => Action::FenceEditCursorHome,
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => Action::FenceEditCursorEnd,
        (KeyModifiers::CONTROL, KeyCode::Char('b')) => Action::FenceEditCursorLeft,
        (KeyModifiers::CONTROL, KeyCode::Char('f')) => Action::FenceEditCursorRight,
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => Action::FenceEditDelete,
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            Action::FenceEditChar(c)
        }
        _ => Action::Noop,
    }
}

/// Translate one key while the file-tree sidebar is focused. The
/// keymap mirrors vim's netrw / nerdtree:
///
/// - `j`/`k` (or arrows) move the selection
/// - `gg`/`G` jump to first/last entry
/// - `Enter` or `l` opens a file or expands a folder
/// - `h` collapses
/// - `R` refreshes
/// - `Tab` returns focus to the editor (sidebar stays visible)
/// - `Esc` or `Ctrl+E` does the same
pub fn parse_tree(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::FocusSwap,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::FocusSwap,
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => Action::TreeToggle,
        (_, KeyCode::Tab) => Action::FocusSwap,
        (_, KeyCode::Char('j')) | (_, KeyCode::Down) => Action::TreeSelectNext,
        (_, KeyCode::Char('k')) | (_, KeyCode::Up) => Action::TreeSelectPrev,
        (_, KeyCode::Char('G')) => Action::TreeSelectLast,
        (_, KeyCode::Char('g')) => Action::TreeSelectFirst,
        (_, KeyCode::Char('l')) | (_, KeyCode::Right) | (_, KeyCode::Enter) => Action::TreeActivate,
        (_, KeyCode::Char('h')) | (_, KeyCode::Left) => Action::TreeCollapse,
        (_, KeyCode::Char('R')) => Action::TreeRefresh,
        (_, KeyCode::Char('a')) => Action::TreeCreate,
        (_, KeyCode::Char('r')) => Action::TreeRename,
        (_, KeyCode::Char('d')) | (_, KeyCode::Char('D')) => Action::TreeDelete,
        _ => Action::Noop,
    }
}

/// Translate one key inside the quick-open modal. Bindings split across
/// list navigation (Up/Down, Ctrl-P/N/K/J) and the inline LineEdit
/// (Left/Right/Home/End/Delete, Ctrl-A/E/B/F/D).
pub fn parse_quickopen(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    // List-navigation shortcuts win first — they shadow some of the
    // LineEdit bindings (e.g. Ctrl-N stays "next item", not "delete").
    let list_nav = match (modifiers, code) {
        (_, KeyCode::Up) => Some(Action::QuickOpenSelectPrev),
        (_, KeyCode::Down) => Some(Action::QuickOpenSelectNext),
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Some(Action::QuickOpenSelectPrev),
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => Some(Action::QuickOpenSelectNext),
        (KeyModifiers::CONTROL, KeyCode::Char('k')) => Some(Action::QuickOpenSelectPrev),
        (KeyModifiers::CONTROL, KeyCode::Char('j')) => Some(Action::QuickOpenSelectNext),
        _ => None,
    };
    if let Some(action) = list_nav {
        return action;
    }
    parse_lineedit_prompt(key, |op| match op {
        LineEditAction::Cancel => Action::QuickOpenCancel,
        LineEditAction::Execute => Action::QuickOpenExecute,
        LineEditAction::Char(c) => Action::QuickOpenChar(c),
        LineEditAction::Backspace => Action::QuickOpenBackspace,
        LineEditAction::Delete => Action::QuickOpenDelete,
        LineEditAction::CursorLeft => Action::QuickOpenCursorLeft,
        LineEditAction::CursorRight => Action::QuickOpenCursorRight,
        LineEditAction::CursorHome => Action::QuickOpenCursorHome,
        LineEditAction::CursorEnd => Action::QuickOpenCursorEnd,
    })
}
