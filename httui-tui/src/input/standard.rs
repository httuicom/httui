//! Standard (non-modal) key decoder. Conventional editor model: arrow
//! keys move the cursor, printable chars insert, `Ctrl+S` saves. No
//! modes — every key resolves to an `Action` directly, the way a
//! plain text editor behaves.
//!
//! Pure function by design (`KeyEvent -> Option<Action>`): no `App`,
//! no side effects, trivially unit-testable. Unmatched keys return
//! `None` so the router can ignore them (V1 keeps the surface
//! deliberately small — clipboard / selection land in a later fase).
//!
//! Introduced by tui-V1 / fase 2 p4.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;
use crate::input::types::Motion;

/// Translate one key in Standard mode. Returns `None` for keys the
/// standard profile doesn't bind (the router treats that as a no-op).
// Wired by the Standard branch of `crate::input::route::route` in
// fase 2 p5 (next commit). Decoder + tests land first so the table is
// reviewed in isolation; `allow(dead_code)` is dropped the moment the
// router calls it.
#[allow(dead_code)]
pub fn resolve(key: KeyEvent) -> Option<Action> {
    let KeyEvent {
        code, modifiers, ..
    } = key;

    Some(match (modifiers, code) {
        // Cursor movement — count is always 1; the standard profile
        // has no count prefix (that's a vim concept).
        (_, KeyCode::Up) => Action::Motion(Motion::Up, 1),
        (_, KeyCode::Down) => Action::Motion(Motion::Down, 1),
        (_, KeyCode::Left) => Action::Motion(Motion::Left, 1),
        (_, KeyCode::Right) => Action::Motion(Motion::Right, 1),
        (_, KeyCode::Home) => Action::Motion(Motion::LineStart, 1),
        (_, KeyCode::End) => Action::Motion(Motion::LineEnd, 1),
        (_, KeyCode::PageDown) => Action::Motion(Motion::HalfPageDown, 1),
        (_, KeyCode::PageUp) => Action::Motion(Motion::HalfPageUp, 1),

        // `Ctrl+S` — universal save, same as vim's `:w` / `<C-s>`.
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => Action::WriteFile,

        // Text editing.
        (_, KeyCode::Enter) => Action::InsertNewline,
        (_, KeyCode::Backspace) => Action::DeleteBackward,
        (_, KeyCode::Delete) => Action::DeleteForward,
        // Any printable char without CONTROL inserts literally. The
        // CONTROL guard keeps `Ctrl+<char>` chords (e.g. the `Ctrl+S`
        // arm above, future clipboard binds) from being typed as text.
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => Action::InsertChar(c),

        // Everything else is unbound in Standard mode.
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn arrows_map_to_unit_motions() {
        assert_eq!(resolve(k(KeyCode::Up)), Some(Action::Motion(Motion::Up, 1)));
        assert_eq!(
            resolve(k(KeyCode::Down)),
            Some(Action::Motion(Motion::Down, 1))
        );
        assert_eq!(
            resolve(k(KeyCode::Left)),
            Some(Action::Motion(Motion::Left, 1))
        );
        assert_eq!(
            resolve(k(KeyCode::Right)),
            Some(Action::Motion(Motion::Right, 1))
        );
    }

    #[test]
    fn home_end_map_to_line_edges() {
        assert_eq!(
            resolve(k(KeyCode::Home)),
            Some(Action::Motion(Motion::LineStart, 1))
        );
        assert_eq!(
            resolve(k(KeyCode::End)),
            Some(Action::Motion(Motion::LineEnd, 1))
        );
    }

    #[test]
    fn page_keys_map_to_half_page_motions() {
        assert_eq!(
            resolve(k(KeyCode::PageDown)),
            Some(Action::Motion(Motion::HalfPageDown, 1))
        );
        assert_eq!(
            resolve(k(KeyCode::PageUp)),
            Some(Action::Motion(Motion::HalfPageUp, 1))
        );
    }

    #[test]
    fn printable_char_without_control_inserts() {
        assert_eq!(
            resolve(k(KeyCode::Char('a'))),
            Some(Action::InsertChar('a'))
        );
        assert_eq!(
            resolve(k(KeyCode::Char(' '))),
            Some(Action::InsertChar(' '))
        );
        // SHIFT (capital letter) still inserts — only CONTROL is the
        // guard.
        assert_eq!(
            resolve(KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::SHIFT)),
            Some(Action::InsertChar('Z'))
        );
    }

    #[test]
    fn enter_backspace_delete_edit_text() {
        assert_eq!(resolve(k(KeyCode::Enter)), Some(Action::InsertNewline));
        assert_eq!(resolve(k(KeyCode::Backspace)), Some(Action::DeleteBackward));
        assert_eq!(resolve(k(KeyCode::Delete)), Some(Action::DeleteForward));
    }

    #[test]
    fn ctrl_s_saves() {
        assert_eq!(resolve(ctrl(KeyCode::Char('s'))), Some(Action::WriteFile));
    }

    #[test]
    fn ctrl_char_is_not_typed_as_text() {
        // A `Ctrl+<char>` that isn't bound (e.g. Ctrl+a) must NOT fall
        // into the InsertChar arm — it's unbound, not typed.
        assert_eq!(resolve(ctrl(KeyCode::Char('a'))), None);
    }

    #[test]
    fn unbound_keys_return_none() {
        assert_eq!(resolve(k(KeyCode::Esc)), None);
        assert_eq!(resolve(k(KeyCode::Tab)), None);
        assert_eq!(resolve(k(KeyCode::F(1))), None);
        assert_eq!(resolve(k(KeyCode::Insert)), None);
    }
}
