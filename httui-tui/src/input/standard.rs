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
/// Wired by the Standard branch of `crate::input::route::route`.
pub fn resolve(key: KeyEvent) -> Option<Action> {
    let KeyEvent {
        code, modifiers, ..
    } = key;

    // Motions that selection-extend (`Shift`+them) and move
    // (without `Shift`) symmetrically. Keep this list and the page
    // arms below in lockstep.
    let motion = match code {
        KeyCode::Up => Some(Motion::Up),
        KeyCode::Down => Some(Motion::Down),
        KeyCode::Left => Some(Motion::Left),
        KeyCode::Right => Some(Motion::Right),
        KeyCode::Home => Some(Motion::LineStart),
        KeyCode::End => Some(Motion::LineEnd),
        _ => None,
    };
    if let Some(m) = motion {
        // `Shift`+<motion> extends the selection; the bare key just
        // moves (the router collapses any active anchor on a plain
        // Motion). This degrades gracefully when SHIFT is absent —
        // identical behaviour to before fase 3.
        return Some(if modifiers.contains(KeyModifiers::SHIFT) {
            Action::SelectExtend(m)
        } else {
            Action::Motion(m, 1)
        });
    }

    Some(match (modifiers, code) {
        // Page motions — no selection-extend variant in V1 (page
        // keys aren't a selection idiom users expect).
        (_, KeyCode::PageDown) => Action::Motion(Motion::HalfPageDown, 1),
        (_, KeyCode::PageUp) => Action::Motion(Motion::HalfPageUp, 1),

        // Clipboard chords. The CONTROL guard on the InsertChar arm
        // below already keeps these from being typed; here they get
        // their real meaning. `Ctrl+C` is Copy — the running-query
        // cancel moves to `Esc` in fase 3 p3.
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::Copy,
        (KeyModifiers::CONTROL, KeyCode::Char('x')) => Action::Cut,
        (KeyModifiers::CONTROL, KeyCode::Char('v')) => Action::PasteSystem,

        // `Ctrl+S` — universal save, same as vim's `:w` / `<C-s>`.
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => Action::WriteFile,

        // Undo / redo (tui-V1 / fase 4 p1). These sit BEFORE the
        // InsertChar arm (whose guard is `!CONTROL`) so the Ctrl
        // chords never get typed as text. `Ctrl+Shift+Z` is matched
        // first — otherwise the bare `Ctrl+Z` arm (which doesn't look
        // at SHIFT) would swallow the redo chord. `Ctrl+Y` is the
        // Windows-style redo alias.
        (m, KeyCode::Char('z') | KeyCode::Char('Z'))
            if m.contains(KeyModifiers::CONTROL) && m.contains(KeyModifiers::SHIFT) =>
        {
            Action::Redo
        }
        (KeyModifiers::CONTROL, KeyCode::Char('z')) => Action::Undo,
        (KeyModifiers::CONTROL, KeyCode::Char('y')) => Action::Redo,

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

    fn shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    #[test]
    fn shift_arrows_extend_the_selection() {
        // fase 3 p1: Shift+<motion> decodes to SelectExtend(<same
        // Motion>) for every selecting motion.
        assert_eq!(
            resolve(shift(KeyCode::Up)),
            Some(Action::SelectExtend(Motion::Up))
        );
        assert_eq!(
            resolve(shift(KeyCode::Down)),
            Some(Action::SelectExtend(Motion::Down))
        );
        assert_eq!(
            resolve(shift(KeyCode::Left)),
            Some(Action::SelectExtend(Motion::Left))
        );
        assert_eq!(
            resolve(shift(KeyCode::Right)),
            Some(Action::SelectExtend(Motion::Right))
        );
        assert_eq!(
            resolve(shift(KeyCode::Home)),
            Some(Action::SelectExtend(Motion::LineStart))
        );
        assert_eq!(
            resolve(shift(KeyCode::End)),
            Some(Action::SelectExtend(Motion::LineEnd))
        );
    }

    #[test]
    fn bare_arrows_still_plain_motions_after_fase3() {
        // Degrade-gracefully guarantee: dropping SHIFT yields exactly
        // the pre-fase-3 Motion(_, 1) (no SelectExtend leak).
        for (code, m) in [
            (KeyCode::Up, Motion::Up),
            (KeyCode::Down, Motion::Down),
            (KeyCode::Left, Motion::Left),
            (KeyCode::Right, Motion::Right),
            (KeyCode::Home, Motion::LineStart),
            (KeyCode::End, Motion::LineEnd),
        ] {
            assert_eq!(resolve(k(code)), Some(Action::Motion(m, 1)));
        }
    }

    #[test]
    fn ctrl_cxv_decode_to_clipboard_actions() {
        assert_eq!(resolve(ctrl(KeyCode::Char('c'))), Some(Action::Copy));
        assert_eq!(resolve(ctrl(KeyCode::Char('x'))), Some(Action::Cut));
        assert_eq!(resolve(ctrl(KeyCode::Char('v'))), Some(Action::PasteSystem));
    }

    #[test]
    fn esc_still_none_after_clipboard_binds() {
        // fase 3 p3 routes query-cancel onto Esc; the decoder still
        // returns None for Esc (the router owns the cancel path).
        assert_eq!(resolve(k(KeyCode::Esc)), None);
    }

    fn ctrl_shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL | KeyModifiers::SHIFT)
    }

    #[test]
    fn ctrl_z_decodes_to_undo() {
        // tui-V1 / fase 4 p1: Ctrl+Z is undo in Standard mode.
        assert_eq!(resolve(ctrl(KeyCode::Char('z'))), Some(Action::Undo));
    }

    #[test]
    fn ctrl_y_decodes_to_redo() {
        // Windows-style redo alias.
        assert_eq!(resolve(ctrl(KeyCode::Char('y'))), Some(Action::Redo));
    }

    #[test]
    fn ctrl_shift_z_decodes_to_redo() {
        // Ctrl+Shift+Z is the conventional redo chord; matched BEFORE
        // the bare Ctrl+Z arm so the SHIFT variant wins. Both the
        // lowercase and the SHIFT-folded uppercase code resolve.
        assert_eq!(resolve(ctrl_shift(KeyCode::Char('z'))), Some(Action::Redo));
        assert_eq!(resolve(ctrl_shift(KeyCode::Char('Z'))), Some(Action::Redo));
    }

    #[test]
    fn ctrl_z_does_not_fall_into_insert_char() {
        // The undo arm sits before the InsertChar arm, so Ctrl+Z is
        // never typed literally as a 'z'.
        assert_ne!(
            resolve(ctrl(KeyCode::Char('z'))),
            Some(Action::InsertChar('z'))
        );
        assert_eq!(resolve(ctrl(KeyCode::Char('z'))), Some(Action::Undo));
    }

    #[test]
    fn page_keys_have_no_shift_select_variant() {
        // Page keys keep their plain motion even with SHIFT (no
        // SelectExtend) — V1 scope decision.
        assert_eq!(
            resolve(shift(KeyCode::PageDown)),
            Some(Action::Motion(Motion::HalfPageDown, 1))
        );
        assert_eq!(
            resolve(shift(KeyCode::PageUp)),
            Some(Action::Motion(Motion::HalfPageUp, 1))
        );
    }
}
