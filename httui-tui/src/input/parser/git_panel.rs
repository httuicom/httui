//! Per-key parser for [`Mode::Git`](crate::vim::mode::Mode::Git).
//! Input flows into the commit-message [`LineEdit`]; the surrounding
//! actions (`Esc`, `Ctrl+G`, `Enter`) close the panel or trigger the
//! commit.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;

pub fn parse_git_panel(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        // Escape / Ctrl+C close the panel.
        (_, KeyCode::Esc) => Action::GitPanelCancel,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::GitPanelCancel,
        // Ctrl+G also acts as a toggle while focused — symmetrical
        // with the open chord.
        (KeyModifiers::CONTROL, KeyCode::Char('g')) => Action::GitPanelToggle,
        // Ctrl+B opens the branch picker.
        (KeyModifiers::CONTROL, KeyCode::Char('b')) => Action::OpenGitBranchPicker,
        // Ctrl+L opens the full-screen git log page.
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => Action::OpenGitLogPage,
        // Ctrl+Enter chains commit → pull → push (the 1-click Sync).
        // Plain Enter just commits — same as the desktop's
        // `GitCommitForm` Submit vs `GitSyncBar` Sync split.
        (mods, KeyCode::Enter) if mods.contains(KeyModifiers::CONTROL) => Action::GitPanelSync,
        (_, KeyCode::Enter) => Action::GitPanelCommit,
        // Line editing.
        (_, KeyCode::Backspace) => Action::GitPanelBackspace,
        (_, KeyCode::Delete) => Action::GitPanelDelete,
        (_, KeyCode::Left) => Action::GitPanelCursorLeft,
        (_, KeyCode::Right) => Action::GitPanelCursorRight,
        (_, KeyCode::Home) => Action::GitPanelCursorHome,
        (_, KeyCode::End) => Action::GitPanelCursorEnd,
        // Plain character input (no modifier other than Shift).
        (mods, KeyCode::Char(c))
            if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) =>
        {
            Action::GitPanelChar(c)
        }
        _ => Action::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn esc_cancels() {
        assert_eq!(parse_git_panel(key(KeyCode::Esc)), Action::GitPanelCancel);
    }

    #[test]
    fn ctrl_c_cancels() {
        assert_eq!(
            parse_git_panel(key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::GitPanelCancel,
        );
    }

    #[test]
    fn ctrl_g_toggles() {
        assert_eq!(
            parse_git_panel(key_mod(KeyCode::Char('g'), KeyModifiers::CONTROL)),
            Action::GitPanelToggle,
        );
    }

    #[test]
    fn enter_submits_commit() {
        assert_eq!(parse_git_panel(key(KeyCode::Enter)), Action::GitPanelCommit);
    }

    #[test]
    fn ctrl_enter_submits_sync() {
        assert_eq!(
            parse_git_panel(key_mod(KeyCode::Enter, KeyModifiers::CONTROL)),
            Action::GitPanelSync,
        );
    }

    #[test]
    fn line_editing_keys_map_to_their_actions() {
        assert_eq!(
            parse_git_panel(key(KeyCode::Backspace)),
            Action::GitPanelBackspace
        );
        assert_eq!(parse_git_panel(key(KeyCode::Delete)), Action::GitPanelDelete);
        assert_eq!(parse_git_panel(key(KeyCode::Left)), Action::GitPanelCursorLeft);
        assert_eq!(
            parse_git_panel(key(KeyCode::Right)),
            Action::GitPanelCursorRight
        );
        assert_eq!(parse_git_panel(key(KeyCode::Home)), Action::GitPanelCursorHome);
        assert_eq!(parse_git_panel(key(KeyCode::End)), Action::GitPanelCursorEnd);
    }

    #[test]
    fn plain_char_returns_char_action() {
        assert_eq!(
            parse_git_panel(key(KeyCode::Char('h'))),
            Action::GitPanelChar('h'),
        );
        assert_eq!(
            parse_git_panel(key_mod(KeyCode::Char('A'), KeyModifiers::SHIFT)),
            Action::GitPanelChar('A'),
        );
    }

    #[test]
    fn control_modified_letters_other_than_known_are_noop() {
        assert_eq!(
            parse_git_panel(key_mod(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            Action::Noop,
        );
    }

    #[test]
    fn unknown_codes_are_noop() {
        assert_eq!(parse_git_panel(key(KeyCode::F(5))), Action::Noop);
    }
}
