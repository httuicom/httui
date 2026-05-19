//! Insert-mode key decoder. Mechanically moved out of `vim/parser.rs`
//! (tui-v2 vertical 1, fase 1 p3d) with no logic change.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;

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
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => Action::InsertChar(c),
        _ => Action::Noop,
    }
}
