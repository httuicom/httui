//! Keymap helper primitives — single-key → Motion/Operator/FindKind
//! decoders shared by the per-mode parsers. Mechanically moved out of
//! `vim/parser.rs` (tui-v2 vertical 1, fase 1 p2) with no logic change.
//!
//! Per-mode decoders live in sibling submodules (fase 1 p3).

pub mod lineedit;
pub mod normal;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::types::{Motion, Operator};
use crate::vim::state::FindKind;

/// Try to interpret a single keystroke as a [`Motion`]. Returns `None`
/// when the key is not a motion (e.g. `i`, `:`). The two state-bearing
/// motions (`0`, `gg`/`G` with count) are handled by the caller because
/// they need access to `VimState`.
pub(crate) fn try_motion(key: KeyEvent) -> Option<Motion> {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    // Letter-keyed motions (`h`, `l`, `w`, `e`, …) must NOT match when
    // a control modifier is pressed; otherwise `Ctrl+E` would shadow
    // the file-tree toggle, `Ctrl+H` the move-left, etc. The two
    // CTRL-bearing motions (`Ctrl+D`/`Ctrl+U`) match before falling
    // into the unmodified branch.
    let plain =
        !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
    Some(match (modifiers, code) {
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => Motion::HalfPageDown,
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => Motion::HalfPageUp,
        (_, KeyCode::Left) => Motion::Left,
        (_, KeyCode::Right) => Motion::Right,
        (_, KeyCode::End) => Motion::LineEnd,
        (_, KeyCode::Home) => Motion::LineStart,
        (_, KeyCode::Down) => Motion::Down,
        (_, KeyCode::Up) => Motion::Up,
        _ if plain => match code {
            KeyCode::Char('h') => Motion::Left,
            KeyCode::Char('l') => Motion::Right,
            KeyCode::Char('^') => Motion::FirstNonBlank,
            KeyCode::Char('$') => Motion::LineEnd,
            KeyCode::Char('j') => Motion::Down,
            KeyCode::Char('k') => Motion::Up,
            KeyCode::Char('w') => Motion::WordForward,
            KeyCode::Char('b') => Motion::WordBackward,
            KeyCode::Char('e') => Motion::WordEnd,
            _ => return None,
        },
        _ => return None,
    })
}

pub(crate) fn doubled_for(op: Operator, code: KeyCode) -> bool {
    matches!(
        (op, code),
        (Operator::Delete, KeyCode::Char('d'))
            | (Operator::Change, KeyCode::Char('c'))
            | (Operator::Yank, KeyCode::Char('y'))
    )
}

pub(crate) fn key_to_operator(modifiers: KeyModifiers, code: KeyCode) -> Option<Operator> {
    // Operators are unmodified lowercase keys. `Ctrl+D` is HalfPageDown,
    // `Ctrl+C` is the emergency quit — both must NOT be picked up as
    // `d` or `c` operator entries.
    if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) {
        return None;
    }
    match code {
        KeyCode::Char('d') => Some(Operator::Delete),
        KeyCode::Char('c') => Some(Operator::Change),
        KeyCode::Char('y') => Some(Operator::Yank),
        _ => None,
    }
}

pub(crate) fn key_to_find_kind(modifiers: KeyModifiers, code: KeyCode) -> Option<FindKind> {
    if modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER) {
        return None;
    }
    match code {
        KeyCode::Char('f') => Some(FindKind::F),
        KeyCode::Char('F') => Some(FindKind::FBack),
        KeyCode::Char('t') => Some(FindKind::T),
        KeyCode::Char('T') => Some(FindKind::TBack),
        _ => None,
    }
}

pub(crate) fn find_kind_to_motion(kind: FindKind, target: char) -> Motion {
    match kind {
        FindKind::F => Motion::FindForward(target),
        FindKind::FBack => Motion::FindBackward(target),
        FindKind::T => Motion::TillForward(target),
        FindKind::TBack => Motion::TillBackward(target),
    }
}
