//! Keychord parsing — turns a human string like `"f2"`, `"ctrl+e"` or
//! `"alt+shift+x"` into a [`KeyChord`] that can be matched against an
//! incoming `crossterm` key event.
//!
//! Used by config-driven keybindings (`EditorConfig::toggle_mode_key`)
//! so the default UX doesn't depend on a hard-coded chord — see
//! tui-V03 / cenário 0, where `Ctrl+Shift+M` proved unreachable on a
//! terminal without the kitty keyboard protocol.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A parsed keychord: a key code plus the Ctrl / Alt modifier state.
///
/// `Shift` is intentionally NOT part of a chord. `Ctrl+Shift+<letter>`
/// is unreliable across terminals (one modifier collapses), and
/// `Shift` on a letter is already folded into its uppercase form — so
/// [`KeyChord::matches`] compares letters case-insensitively and
/// ignores `Shift` entirely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyChord {
    pub code: KeyCode,
    pub ctrl: bool,
    pub alt: bool,
}

impl KeyChord {
    /// `true` when `key` is this chord. Letter codes compare
    /// case-insensitively; `Ctrl` and `Alt` must match exactly;
    /// `Shift` is ignored.
    pub fn matches(&self, key: KeyEvent) -> bool {
        let code_ok = match (self.code, key.code) {
            (KeyCode::Char(a), KeyCode::Char(b)) => a.eq_ignore_ascii_case(&b),
            (a, b) => a == b,
        };
        code_ok
            && key.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl
            && key.modifiers.contains(KeyModifiers::ALT) == self.alt
    }
}

/// Parse a keychord string into a [`KeyChord`]. Returns `None` for
/// empty or malformed input so callers can fall back to a default.
///
/// Grammar: modifier tokens joined to exactly one key token by `+`,
/// e.g. `"ctrl+e"`, `"alt+shift+f4"`, `"f2"`. Case-insensitive,
/// whitespace around tokens is trimmed.
///
/// - Modifiers: `ctrl`/`control`, `alt`/`opt`/`option`. `shift` is
///   accepted but ignored (see [`KeyChord`]).
/// - Keys: `f1`..`f12`, a single character, or one of `enter`/
///   `return`, `tab`, `esc`/`escape`, `space`, `backspace`.
pub fn parse_key_chord(s: &str) -> Option<KeyChord> {
    let mut ctrl = false;
    let mut alt = false;
    let mut code: Option<KeyCode> = None;

    for raw in s.split('+') {
        let part = raw.trim().to_ascii_lowercase();
        if part.is_empty() {
            return None;
        }
        match part.as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "opt" | "option" => alt = true,
            "shift" => {}
            other => {
                // A key token — exactly one is allowed per chord.
                if code.is_some() {
                    return None;
                }
                code = Some(parse_key_code(other)?);
            }
        }
    }

    Some(KeyChord {
        code: code?,
        ctrl,
        alt,
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

    #[test]
    fn parses_plain_f_key() {
        let c = parse_key_chord("f2").unwrap();
        assert_eq!(c, KeyChord { code: KeyCode::F(2), ctrl: false, alt: false });
    }

    #[test]
    fn parses_ctrl_letter() {
        let c = parse_key_chord("ctrl+e").unwrap();
        assert_eq!(c, KeyChord { code: KeyCode::Char('e'), ctrl: true, alt: false });
    }

    #[test]
    fn parses_modifier_aliases_and_alt() {
        assert_eq!(parse_key_chord("control+e").unwrap().ctrl, true);
        assert_eq!(parse_key_chord("alt+x").unwrap().alt, true);
        assert_eq!(parse_key_chord("opt+x").unwrap().alt, true);
        assert_eq!(parse_key_chord("option+x").unwrap().alt, true);
    }

    #[test]
    fn shift_token_is_accepted_but_ignored() {
        // `shift` parses fine but never lands in the chord.
        let c = parse_key_chord("ctrl+shift+m").unwrap();
        assert_eq!(c, KeyChord { code: KeyCode::Char('m'), ctrl: true, alt: false });
        assert_eq!(parse_key_chord("shift+f2").unwrap().code, KeyCode::F(2));
    }

    #[test]
    fn parses_named_keys() {
        assert_eq!(parse_key_chord("enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_key_chord("return").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_key_chord("tab").unwrap().code, KeyCode::Tab);
        assert_eq!(parse_key_chord("esc").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_key_chord("escape").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_key_chord("space").unwrap().code, KeyCode::Char(' '));
        assert_eq!(parse_key_chord("backspace").unwrap().code, KeyCode::Backspace);
    }

    #[test]
    fn is_case_insensitive_and_trims_whitespace() {
        let c = parse_key_chord("  CTRL + F12 ").unwrap();
        assert_eq!(c, KeyChord { code: KeyCode::F(12), ctrl: true, alt: false });
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
    fn matches_ctrl_letter_case_folded() {
        let c = parse_key_chord("ctrl+e").unwrap();
        assert!(c.matches(ev(KeyCode::Char('e'), KeyModifiers::CONTROL)));
        // Terminal may fold an accompanying Shift into uppercase —
        // still a match (Shift is ignored).
        assert!(c.matches(ev(
            KeyCode::Char('E'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )));
    }

    #[test]
    fn rejects_wrong_modifier_state() {
        let c = parse_key_chord("ctrl+e").unwrap();
        assert!(!c.matches(ev(KeyCode::Char('e'), KeyModifiers::NONE)), "no ctrl");
        assert!(
            !c.matches(ev(KeyCode::Char('e'), KeyModifiers::CONTROL | KeyModifiers::ALT)),
            "stray alt"
        );
    }
}
