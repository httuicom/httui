//! Visual / VisualLine mode key decoder. Mechanically moved out of
//! `vim/parser.rs` (tui-v2 vertical 1, fase 1 p3c) with no logic change.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;
use crate::input::parser::try_motion;
use crate::input::types::{build_textobject, Motion, Operator};
use crate::vim::mode::Mode;
use crate::vim::state::VimState;

/// Translate one key in Visual or VisualLine mode. Reuses the normal-
/// mode motion vocabulary (h/l/j/k/w/b/e/0/^/$/gg/G/Ctrl+D/Ctrl+U) and
/// adds visual-only verbs: `d`/`x` delete the selection, `c`/`s`
/// change it, `y` yanks it, `o` swaps the anchor and the moving end,
/// and `Esc` (or a second `v`/`V`) leaves visual.
pub fn parse_visual(state: &mut VimState, key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;

    if code == KeyCode::Esc {
        state.reset_pending();
        return Action::ExitVisual;
    }

    // Resolve a pending text-object trigger from a previous `a` /
    // `i` keystroke (e.g. `va{` arriving here on the `{`). Build the
    // object via the shared resolver and snap the selection to its
    // range. Anything other than a recognised target char silently
    // cancels — same forgiving behaviour as `parse_normal`.
    if let Some(inner) = state.pending_textobj_inner {
        state.pending_textobj_inner = None;
        let target = match code {
            KeyCode::Char(c) => c,
            _ => return Action::Noop,
        };
        let around = !inner;
        if let Some(textobj) = build_textobject(around, target) {
            return Action::VisualSelectTextObject(textobj);
        }
        return Action::Noop;
    }

    // `v` toggles charwise visual off; `V` toggles linewise off. The
    // other letter swaps mode (handled in dispatch — emits a re-enter).
    let plain_letter =
        !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER);
    if plain_letter {
        if state.mode == Mode::Visual && code == KeyCode::Char('v') {
            return Action::ExitVisual;
        }
        if state.mode == Mode::VisualLine && code == KeyCode::Char('V') {
            return Action::ExitVisual;
        }
    }

    // Digit prefixes for motion counts.
    if let KeyCode::Char(c) = code {
        if c.is_ascii_digit() {
            let d = c.to_digit(10).unwrap() as usize;
            if d == 0 && state.pending_count.is_none() {
                let count = state.take_count();
                return Action::Motion(Motion::LineStart, count.max(1));
            }
            state.push_digit(d);
            return Action::Noop;
        }
    }

    // `gg` resolution.
    if state.pending_g {
        state.pending_g = false;
        if let KeyCode::Char('g') = code {
            let count = state.take_count();
            let motion = if count > 1 {
                Motion::GotoLine(count)
            } else {
                Motion::DocStart
            };
            return Action::Motion(motion, 1);
        }
        return Action::Noop;
    }

    // Visual-only verbs. Operators take priority over motion lookup
    // for `d`/`c`/`y`/`x`/`s` so they don't get parsed as letters.
    if plain_letter {
        match code {
            KeyCode::Char('d') | KeyCode::Char('x') => {
                return Action::VisualOperator(Operator::Delete);
            }
            KeyCode::Char('c') | KeyCode::Char('s') => {
                return Action::VisualOperator(Operator::Change);
            }
            KeyCode::Char('y') => {
                return Action::VisualOperator(Operator::Yank);
            }
            KeyCode::Char('o') => return Action::VisualSwap,
            // `a` / `i` start a text-object trigram. The next
            // keystroke is the target char (`{`, `"`, `w`, …); the
            // resolver at the top of `parse_visual` consumes it and
            // emits `VisualSelectTextObject`. Same state field
            // (`pending_textobj_inner`) as `parse_normal`'s chord.
            KeyCode::Char('a') => {
                state.pending_textobj_inner = Some(false);
                return Action::Noop;
            }
            KeyCode::Char('i') => {
                state.pending_textobj_inner = Some(true);
                return Action::Noop;
            }
            _ => {}
        }
    }

    // `gg` / `G` entry.
    if plain_letter && code == KeyCode::Char('g') {
        let count = state.take_count();
        state.pending_count = if count > 1 { Some(count) } else { None };
        state.pending_g = true;
        return Action::Noop;
    }
    if plain_letter && code == KeyCode::Char('G') {
        let count = state.take_count();
        return if count > 1 {
            Action::Motion(Motion::GotoLine(count), 1)
        } else {
            Action::Motion(Motion::DocEnd, 1)
        };
    }

    // Plain motions extend the selection.
    if let Some(m) = try_motion(key) {
        let count = state.take_count();
        return Action::Motion(m, count.max(1));
    }

    Action::Noop
}
