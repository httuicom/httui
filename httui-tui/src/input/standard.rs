//! Standard (non-modal) key decoder. Conventional editor model: arrow
//! keys move the cursor, printable chars insert, configured chords run
//! commands. No modes — every key resolves to an `Action` directly.
//!
//! Since tui-V03 the chord→`Action` bindings are config-driven: they
//! live in `config.keymap`, resolved into a runtime list by
//! `crate::input::keymap` and threaded in here as the `keymap`
//! argument. `resolve` itself only owns the two non-table rules —
//! the `/` slash trigger and the parametric `InsertChar(c)` fallback.
//!
//! Introduced by tui-V1 / fase 2 p4; data-driven by tui-V01 / fase 6;
//! config-driven by tui-V03.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;
use crate::input::keychord::KeyChord;

/// Translate one key in Standard mode. Returns `None` for keys the
/// standard profile doesn't bind (the router treats that as a no-op).
/// Wired by the Standard branch of `crate::input::route::route`.
///
/// Lookup order:
/// 1. the config-driven `keymap` (`crate::input::keymap::lookup`);
/// 2. `/` without CONTROL → `SlashKey` (opens the block-template
///    picker when the cursor is in prose);
/// 3. any printable char without CONTROL → `InsertChar(c)`.
///
/// The cross-profile editor-mode toggle and the running-query `Esc`
/// cancel are intercepted by `route::route` BEFORE this runs.
pub fn resolve(keymap: &[(KeyChord, Action)], key: KeyEvent) -> Option<Action> {
    if let Some(action) = crate::input::keymap::lookup(keymap, key) {
        return Some(action);
    }

    // tui-V2 vertical 2: `/` in Standard is a context-aware
    // slash-commands trigger. The applier (`input::apply::slash`)
    // inserts the literal `/` and, in prose, opens the picker on top.
    // Matched before the printable-char fallback so it never falls
    // through to plain `InsertChar('/')`.
    if let KeyEvent {
        code: KeyCode::Char('/'),
        modifiers,
        ..
    } = key
    {
        if !modifiers.contains(KeyModifiers::CONTROL) {
            return Some(Action::SlashKey);
        }
    }

    // The only chord-to-action mapping NOT in the keymap table: any
    // printable char without CONTROL inserts literally. The CONTROL
    // guard keeps `Ctrl+<char>` chords from being typed as text —
    // bound ones already returned above; unbound ones stay `None`.
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
    use crate::config::KeymapConfig;
    use crate::input::keymap::resolve_standard_keymap;
    use crate::input::types::Motion;

    /// The default Standard keymap — what every fresh install sees.
    fn km() -> Vec<(KeyChord, Action)> {
        resolve_standard_keymap(&KeymapConfig::default())
    }

    fn k(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn resolve_consults_the_keymap() {
        // Smoke test of the layering: `resolve` delegates table chords
        // to `keymap::lookup`. Exhaustive chord→Action coverage lives
        // in `input::keymap` tests.
        assert_eq!(resolve(&km(), ctrl(KeyCode::Char('c'))), Some(Action::Copy));
        assert_eq!(
            resolve(&km(), k(KeyCode::Up)),
            Some(Action::Motion(Motion::Up, 1)),
        );
        assert_eq!(
            resolve(&km(), KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT)),
            Some(Action::SelectExtend(Motion::Up)),
        );
    }

    #[test]
    fn printable_char_without_control_inserts() {
        assert_eq!(
            resolve(&km(), k(KeyCode::Char('a'))),
            Some(Action::InsertChar('a')),
        );
        assert_eq!(
            resolve(&km(), k(KeyCode::Char(' '))),
            Some(Action::InsertChar(' ')),
        );
        // SHIFT (capital letter) still inserts — only CONTROL guards.
        assert_eq!(
            resolve(&km(), KeyEvent::new(KeyCode::Char('Z'), KeyModifiers::SHIFT)),
            Some(Action::InsertChar('Z')),
        );
    }

    #[test]
    fn ctrl_char_that_is_unbound_is_not_typed_as_text() {
        // `Ctrl+a` isn't in the default keymap — it must resolve to
        // `None`, NOT fall into the `InsertChar` arm.
        assert_eq!(resolve(&km(), ctrl(KeyCode::Char('a'))), None);
    }

    #[test]
    fn unbound_keys_return_none() {
        // Esc → handled specially by `route_standard` (query cancel).
        assert_eq!(resolve(&km(), k(KeyCode::Esc)), None);
        // F11 carries no default Standard binding (the runtime defaults
        // are `Alt+letter`, not F-keys).
        assert_eq!(resolve(&km(), k(KeyCode::F(11))), None);
    }

    #[test]
    fn tab_and_shift_tab_resolve_to_tab_next_and_prev() {
        assert_eq!(resolve(&km(), k(KeyCode::Tab)), Some(Action::TabNext));
        assert_eq!(resolve(&km(), k(KeyCode::BackTab)), Some(Action::TabPrev));
    }

    // ───── tui-V2 vertical 2 / cenário 1 — `/` opens slash picker ─────

    #[test]
    fn slash_decodes_to_slash_key_action() {
        assert_eq!(resolve(&km(), k(KeyCode::Char('/'))), Some(Action::SlashKey));
        assert_ne!(
            resolve(&km(), k(KeyCode::Char('/'))),
            Some(Action::InsertChar('/')),
        );
    }

    #[test]
    fn ctrl_slash_does_not_open_slash_picker() {
        // Ctrl+`/` (some terminals emit it for "toggle comment") must
        // not trigger the picker — returns `None`.
        assert_eq!(resolve(&km(), ctrl(KeyCode::Char('/'))), None);
    }

    #[test]
    fn shift_slash_still_opens_slash_picker() {
        // `/` with bare SHIFT is still a literal `/` keystroke.
        let ev = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::SHIFT);
        assert_eq!(resolve(&km(), ev), Some(Action::SlashKey));
    }
}
