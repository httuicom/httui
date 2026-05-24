//! Keychord parsing — turns a human string like `"f2"`, `"ctrl+e"` or
//! `"shift+up"` into a [`KeyChord`] that can be matched against an
//! incoming `crossterm` key event.
//!
//! Used by config-driven keybindings (`EditorConfig` /
//! `KeymapConfig`) so the default UX doesn't depend on hard-coded
//! chords — see tui-V03, where `Ctrl+Shift+M` proved unreachable on a
//! terminal without the kitty keyboard protocol.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A parsed keychord: a key code plus the Ctrl / Alt / Shift state.
///
/// `Shift` is meaningful only for non-character keys (arrows, F-keys),
/// where terminals report it reliably and it carries intent
/// (`Shift+Up` = extend selection). On a *character* key, `Shift` is
/// already folded into the letter's case and is too unreliable across
/// terminals to gate on — so [`KeyChord::matches`] ignores `Shift` for
/// `Char` codes and compares letters case-insensitively.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChord {
    pub code: KeyCode,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyChord {
    /// `true` when `key` is this chord. Letter codes compare
    /// case-insensitively; `Ctrl` and `Alt` must match exactly;
    /// `Shift` must match exactly for non-`Char` codes and is ignored
    /// for `Char` codes.
    pub fn matches(&self, key: KeyEvent) -> bool {
        let code_ok = match (self.code, key.code) {
            (KeyCode::Char(a), KeyCode::Char(b)) => a.eq_ignore_ascii_case(&b),
            (a, b) => a == b,
        };
        if !code_ok {
            return false;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) != self.ctrl
            || key.modifiers.contains(KeyModifiers::ALT) != self.alt
        {
            return false;
        }
        match self.code {
            // Char + BackTab are inherently shift-bearing: terminals
            // disagree on whether they also set `KeyModifiers::SHIFT`,
            // so we accept either presentation.
            KeyCode::Char(_) | KeyCode::BackTab => true,
            _ => key.modifiers.contains(KeyModifiers::SHIFT) == self.shift,
        }
    }
}

/// Parse a keychord string into a [`KeyChord`]. Returns `None` for
/// empty or malformed input so callers can fall back to a default.
///
/// Grammar: modifier tokens joined to exactly one key token by `+`,
/// e.g. `"ctrl+e"`, `"shift+up"`, `"alt+f4"`, `"f2"`. Case-insensitive,
/// whitespace around tokens is trimmed.
///
/// - Modifiers: `ctrl`/`control`, `alt`/`opt`/`option`, `shift`.
/// - Keys: `f1`..`f12`, a single character, or one of `enter`/
///   `return`, `tab`, `esc`/`escape`, `space`, `backspace`, the arrows
///   `up`/`down`/`left`/`right`, `home`, `end`, `pageup`, `pagedown`,
///   `delete`/`del`, `insert`.
pub fn parse_key_chord(s: &str) -> Option<KeyChord> {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut code: Option<KeyCode> = None;

    for raw in s.split('+') {
        let part = raw.trim().to_ascii_lowercase();
        if part.is_empty() {
            return None;
        }
        match part.as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "opt" | "option" => alt = true,
            "shift" => shift = true,
            other => {
                // A key token — exactly one is allowed per chord.
                if code.is_some() {
                    return None;
                }
                code = Some(parse_key_code(other)?);
            }
        }
    }

    let code = code?;
    // Normalize `shift+tab` → `BackTab` so the chord matches whether
    // the terminal sends `KeyCode::BackTab` (most common) or
    // `KeyCode::Tab` with `KeyModifiers::SHIFT`. `BackTab` is a one-of-
    // a-kind code: by definition it IS Shift+Tab, so we drop the
    // explicit shift bit here and let `matches` treat the modifier as
    // implicit (mirrors how `Char` codes ignore shift).
    let (code, shift) = if shift && matches!(code, KeyCode::Tab) {
        (KeyCode::BackTab, false)
    } else {
        (code, shift)
    };
    Some(KeyChord {
        code,
        ctrl,
        alt,
        shift,
    })
}

/// Parse a single key token (already lowercased, non-empty).
fn parse_key_code(token: &str) -> Option<KeyCode> {
    match token {
        "enter" | "return" => return Some(KeyCode::Enter),
        "tab" => return Some(KeyCode::Tab),
        "esc" | "escape" => return Some(KeyCode::Esc),
        "space" => return Some(KeyCode::Char(' ')),
        "backspace" => return Some(KeyCode::Backspace),
        "delete" | "del" => return Some(KeyCode::Delete),
        "insert" => return Some(KeyCode::Insert),
        "up" => return Some(KeyCode::Up),
        "down" => return Some(KeyCode::Down),
        "left" => return Some(KeyCode::Left),
        "right" => return Some(KeyCode::Right),
        "home" => return Some(KeyCode::Home),
        "end" => return Some(KeyCode::End),
        "pageup" => return Some(KeyCode::PageUp),
        "pagedown" => return Some(KeyCode::PageDown),
        _ => {}
    }
    // F-keys: `f1`..`f12`. `strip_prefix` also matches the bare
    // letter `"f"` — its empty remainder fails to parse and falls
    // through to the single-character branch below.
    if let Some(digits) = token.strip_prefix('f') {
        if let Ok(n) = digits.parse::<u8>() {
            return (1..=12).contains(&n).then_some(KeyCode::F(n));
        }
    }
    // Single character — anything longer is malformed.
    let mut chars = token.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(KeyCode::Char(c)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn chord(code: KeyCode, ctrl: bool, alt: bool, shift: bool) -> KeyChord {
        KeyChord {
            code,
            ctrl,
            alt,
            shift,
        }
    }

    #[test]
    fn parses_plain_f_key() {
        assert_eq!(
            parse_key_chord("f2").unwrap(),
            chord(KeyCode::F(2), false, false, false)
        );
    }

    #[test]
    fn parses_ctrl_letter() {
        assert_eq!(
            parse_key_chord("ctrl+e").unwrap(),
            chord(KeyCode::Char('e'), true, false, false)
        );
    }

    #[test]
    fn parses_modifier_aliases_and_alt() {
        assert!(parse_key_chord("control+e").unwrap().ctrl);
        assert!(parse_key_chord("alt+x").unwrap().alt);
        assert!(parse_key_chord("opt+x").unwrap().alt);
        assert!(parse_key_chord("option+x").unwrap().alt);
    }

    #[test]
    fn parses_shift_token() {
        let c = parse_key_chord("shift+up").unwrap();
        assert_eq!(c, chord(KeyCode::Up, false, false, true));
        let c = parse_key_chord("ctrl+shift+m").unwrap();
        assert_eq!(c, chord(KeyCode::Char('m'), true, false, true));
    }

    #[test]
    fn parses_named_keys() {
        assert_eq!(parse_key_chord("enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_key_chord("return").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_key_chord("tab").unwrap().code, KeyCode::Tab);
        assert_eq!(parse_key_chord("esc").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_key_chord("escape").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_key_chord("space").unwrap().code, KeyCode::Char(' '));
        assert_eq!(
            parse_key_chord("backspace").unwrap().code,
            KeyCode::Backspace
        );
        assert_eq!(parse_key_chord("del").unwrap().code, KeyCode::Delete);
        assert_eq!(parse_key_chord("delete").unwrap().code, KeyCode::Delete);
        assert_eq!(parse_key_chord("insert").unwrap().code, KeyCode::Insert);
    }

    #[test]
    fn parses_arrow_and_navigation_keys() {
        assert_eq!(parse_key_chord("up").unwrap().code, KeyCode::Up);
        assert_eq!(parse_key_chord("down").unwrap().code, KeyCode::Down);
        assert_eq!(parse_key_chord("left").unwrap().code, KeyCode::Left);
        assert_eq!(parse_key_chord("right").unwrap().code, KeyCode::Right);
        assert_eq!(parse_key_chord("home").unwrap().code, KeyCode::Home);
        assert_eq!(parse_key_chord("end").unwrap().code, KeyCode::End);
        assert_eq!(parse_key_chord("pageup").unwrap().code, KeyCode::PageUp);
        assert_eq!(parse_key_chord("pagedown").unwrap().code, KeyCode::PageDown);
    }

    #[test]
    fn is_case_insensitive_and_trims_whitespace() {
        assert_eq!(
            parse_key_chord("  CTRL + F12 ").unwrap(),
            chord(KeyCode::F(12), true, false, false)
        );
    }

    #[test]
    fn bare_f_is_the_letter_not_a_function_key() {
        assert_eq!(parse_key_chord("f").unwrap().code, KeyCode::Char('f'));
    }

    #[test]
    fn rejects_empty_and_malformed() {
        assert!(parse_key_chord("").is_none());
        assert!(parse_key_chord("ctrl").is_none(), "modifier with no key");
        assert!(parse_key_chord("ctrl++e").is_none(), "empty token");
        assert!(parse_key_chord("a+b").is_none(), "two key tokens");
        assert!(parse_key_chord("foo").is_none(), "unknown multi-char token");
        assert!(parse_key_chord("f0").is_none(), "f-key below range");
        assert!(parse_key_chord("f13").is_none(), "f-key above range");
    }

    #[test]
    fn matches_f_key_event() {
        let c = parse_key_chord("f2").unwrap();
        assert!(c.matches(ev(KeyCode::F(2), KeyModifiers::NONE)));
        assert!(!c.matches(ev(KeyCode::F(3), KeyModifiers::NONE)));
    }

    #[test]
    fn matches_ctrl_letter_case_folded_ignoring_shift() {
        let c = parse_key_chord("ctrl+e").unwrap();
        assert!(c.matches(ev(KeyCode::Char('e'), KeyModifiers::CONTROL)));
        // Terminal may fold an accompanying Shift into uppercase — for
        // a Char code Shift is ignored, so this still matches.
        assert!(c.matches(ev(
            KeyCode::Char('E'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )));
    }

    #[test]
    fn shift_gates_non_char_keys_exactly() {
        // `shift+up` must match Shift+Up and reject a bare Up.
        let c = parse_key_chord("shift+up").unwrap();
        assert!(c.matches(ev(KeyCode::Up, KeyModifiers::SHIFT)));
        assert!(!c.matches(ev(KeyCode::Up, KeyModifiers::NONE)));
        // ...and a bare `up` chord must reject Shift+Up.
        let bare = parse_key_chord("up").unwrap();
        assert!(bare.matches(ev(KeyCode::Up, KeyModifiers::NONE)));
        assert!(!bare.matches(ev(KeyCode::Up, KeyModifiers::SHIFT)));
    }

    #[test]
    fn rejects_wrong_modifier_state() {
        let c = parse_key_chord("ctrl+e").unwrap();
        assert!(
            !c.matches(ev(KeyCode::Char('e'), KeyModifiers::NONE)),
            "no ctrl"
        );
        assert!(
            !c.matches(ev(
                KeyCode::Char('e'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )),
            "stray alt"
        );
    }
}
