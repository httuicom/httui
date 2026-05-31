//! Insert-mode key decoder. Mechanically moved out of `vim/parser.rs`
//! (tui-v2 vertical 1, fase 1 p3d) with no logic change.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;
use crate::input::types::Motion;

/// Translate one key in Insert mode.
pub fn parse_insert(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;

    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::ExitInsert,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::ExitInsert,
        // `<C-s>` saves without leaving insert — typing flow stays
        // intact, the file just hits disk. Mirrors the normal-mode
        // bind in `parse_normal`.
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => Action::WriteFile,
        (_, KeyCode::Enter) => Action::InsertNewline,
        (_, KeyCode::Backspace) => Action::DeleteBackward,
        (_, KeyCode::Delete) => Action::DeleteForward,
        // Arrow keys move the cursor mid-INSERT — without this, navigating
        // inside the buffer (and accepting completion popup items) requires
        // dropping to NORMAL first, which is broken-feeling in form fields
        // (header value, URL, etc.) where users expect a regular text input.
        (_, KeyCode::Left) => Action::Motion(Motion::Left, 1),
        (_, KeyCode::Right) => Action::Motion(Motion::Right, 1),
        (_, KeyCode::Up) => Action::Motion(Motion::Up, 1),
        (_, KeyCode::Down) => Action::Motion(Motion::Down, 1),
        (_, KeyCode::Home) => Action::Motion(Motion::LineStart, 1),
        (_, KeyCode::End) => Action::Motion(Motion::LineEnd, 1),
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => Action::InsertChar(c),
        _ => Action::Noop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn arrow_keys_move_cursor_in_insert() {
        assert_eq!(parse_insert(k(KeyCode::Left)), Action::Motion(Motion::Left, 1));
        assert_eq!(
            parse_insert(k(KeyCode::Right)),
            Action::Motion(Motion::Right, 1)
        );
        assert_eq!(parse_insert(k(KeyCode::Up)), Action::Motion(Motion::Up, 1));
        assert_eq!(
            parse_insert(k(KeyCode::Down)),
            Action::Motion(Motion::Down, 1)
        );
        assert_eq!(
            parse_insert(k(KeyCode::Home)),
            Action::Motion(Motion::LineStart, 1)
        );
        assert_eq!(
            parse_insert(k(KeyCode::End)),
            Action::Motion(Motion::LineEnd, 1)
        );
    }

    #[test]
    fn esc_and_ctrl_c_leave_insert() {
        assert_eq!(parse_insert(k(KeyCode::Esc)), Action::ExitInsert);
        assert_eq!(
            parse_insert(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::ExitInsert
        );
    }
}
