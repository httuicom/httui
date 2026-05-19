//! Standard (non-modal) key decoder. Conventional editor model: arrow
//! keys move the cursor, printable chars insert, `Ctrl+S` saves. No
//! modes — every key resolves to an `Action` directly, the way a
//! plain text editor behaves.
//!
//! Pure function by design (`KeyEvent -> Option<Action>`): no `App`,
//! no side effects, trivially unit-testable. Unmatched keys return
//! `None` so the router can ignore them.
//!
//! Since tui-V01 / fase 6 p2-p3 the leaf chords (Ctrl+C/X/V/S/Z/Y,
//! arrows, Home/End/PageUp/Down, Shift-selection variants,
//! Ctrl+Shift+X EXPLAIN, Ctrl+Shift+Z redo, Enter/Backspace/Delete)
//! live in the data-driven [`crate::input::map`] table — `resolve`
//! delegates to [`crate::input::map::lookup_standard`] first and falls
//! through to the parametric `InsertChar(c)` arm only for unbound
//! printable chars. That single fall-through is the only chord-to-
//! action mapping NOT in the map: it's parametric in `c`, not a leaf
//! binding.
//!
//! Introduced by tui-V1 / fase 2 p4; refactored data-driven by
//! tui-V01 / fase 6 p2-p3.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;

/// Translate one key in Standard mode. Returns `None` for keys the
/// standard profile doesn't bind (the router treats that as a no-op).
/// Wired by the Standard branch of `crate::input::route::route`.
pub fn resolve(key: KeyEvent) -> Option<Action> {
    // Leaf chord-to-action bindings live in the inspectable keymap
    // table (Cenário 3 — single source). The cross-profile toggle
    // (`Ctrl+Shift+M`) is intercepted by `route::route` BEFORE this
    // function ever runs, but it also appears in the map so V9's UI
    // can surface it; `lookup_standard` happens to find it too — no
    // harm done, because if we ever reach here with the toggle chord
    // (route lost the race) returning `Action::ToggleEditorMode`
    // would still be the right semantic answer.
    if let Some(action) = crate::input::map::lookup_standard(key) {
        return Some(action);
    }

    // The only chord-to-action binding NOT in the data table: any
    // printable char without CONTROL inserts literally. The CONTROL
    // guard keeps `Ctrl+<char>` chords (Ctrl+S, Ctrl+C, …) from
    // being typed as text — those are bound in the table and would
    // have returned above.
    if let KeyEvent {
        code: KeyCode::Char(c),
        modifiers,
        ..
    } = key
    {
        if !modifiers.contains(KeyModifiers::CONTROL) {
            return Some(Action::InsertChar(c));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    // Fase 6 p3 moved the chord-to-Action table to `input::map`, so
    // `Motion` is no longer imported at module scope (production now
    // only deals in `Action`). Tests still construct `Motion`-bearing
    // `Action`s via the `Action::Motion(...)` arms — pull the type in
    // here so the existing test surface keeps working unchanged.
    use crate::input::types::Motion;

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

    #[test]
    fn ctrl_shift_x_decodes_to_explain_block() {
        // tui-V1 / fase 5 p5: EXPLAIN moves from Ctrl+X (vim) to
        // Ctrl+Shift+X in Standard, because Ctrl+X is Cut here. Matched
        // BEFORE the bare Ctrl+X Cut arm. Lowercase and SHIFT-folded
        // uppercase both resolve (terminals differ).
        assert_eq!(
            resolve(ctrl_shift(KeyCode::Char('x'))),
            Some(Action::ExplainBlock)
        );
        assert_eq!(
            resolve(ctrl_shift(KeyCode::Char('X'))),
            Some(Action::ExplainBlock)
        );
    }

    #[test]
    fn ctrl_x_is_still_cut_no_regression() {
        // Non-regression: the bare Ctrl+X without SHIFT must still be
        // Cut — the new Ctrl+Shift+X arm above must not swallow it.
        assert_eq!(resolve(ctrl(KeyCode::Char('x'))), Some(Action::Cut));
    }
}
