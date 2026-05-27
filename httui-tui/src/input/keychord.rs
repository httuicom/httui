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

/// Canonical chord string for a [`KeyEvent`] — the inverse of
/// [`parse_key_chord`] for the subset of events the rebind UI emits.
///
/// Used by the Settings → Keymaps capture flow: when the user
/// presses a chord, we serialise it back into the same grammar
/// `[keymap]` expects in `config.toml`, so a freshly-captured chord
/// round-trips cleanly through save → reload.
///
/// Returns `None` for events that cannot be a binding on their own
/// (Esc, modifier-only "press", `KeyCode::Null`, mouse events the
/// terminal reports as `Press` of an unsupported code). Token order
/// is `ctrl+alt+shift+<key>`. Letters lowercase. `Shift` is omitted
/// for `Char` and `BackTab` codes because [`KeyChord::matches`]
/// ignores it there — emitting it would be a no-op that confuses
/// the user.
pub fn chord_string_from_key(key: KeyEvent) -> Option<String> {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    // Esc is reserved as "cancel capture" everywhere in the modal
    // stack — refuse to make it a binding from the UI.
    if matches!(code, KeyCode::Esc | KeyCode::Null) {
        return None;
    }
    let key_token = key_code_token(code)?;
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("ctrl");
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("alt");
    }
    // Only meaningful on non-`Char` / non-`BackTab` codes — match the
    // asymmetry in `KeyChord::matches`. `BackTab` is implicitly
    // `Shift+Tab`, so we don't append an explicit `shift` for it.
    let shift_meaningful = !matches!(code, KeyCode::Char(_) | KeyCode::BackTab);
    if shift_meaningful && modifiers.contains(KeyModifiers::SHIFT) {
        parts.push("shift");
    }
    parts.push(&key_token);
    Some(parts.join("+"))
}

fn key_code_token(code: KeyCode) -> Option<String> {
    Some(match code {
        KeyCode::Enter => "enter".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::BackTab => "shift+tab".into(),
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Delete => "delete".into(),
        KeyCode::Insert => "insert".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::PageUp => "pageup".into(),
        KeyCode::PageDown => "pagedown".into(),
        KeyCode::F(n) if (1..=12).contains(&n) => format!("f{n}"),
        KeyCode::Char(' ') => "space".into(),
        KeyCode::Char(c) => c.to_ascii_lowercase().to_string(),
        _ => return None,
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

    fn assert_roundtrip(s: &str, ev: KeyEvent) {
        let got = chord_string_from_key(ev).unwrap_or_else(|| panic!("None for {ev:?}"));
        assert_eq!(got, s, "stringified shape");
        let parsed = parse_key_chord(&got).unwrap_or_else(|| panic!("re-parse of {got}"));
        assert!(parsed.matches(ev), "parsed chord must match origin event");
    }

    #[test]
    fn from_key_ctrl_letter() {
        assert_roundtrip("ctrl+c", ev(KeyCode::Char('c'), KeyModifiers::CONTROL));
    }

    #[test]
    fn from_key_alt_letter_lowercases_uppercase_input() {
        // Some terminals report uppercase letters even without Shift —
        // canonical form is always lowercase, matching `parse_key_chord`.
        let stringified = chord_string_from_key(ev(KeyCode::Char('R'), KeyModifiers::ALT)).unwrap();
        assert_eq!(stringified, "alt+r");
    }

    #[test]
    fn from_key_shift_arrow_keeps_shift_token() {
        // Shift IS meaningful on non-Char codes.
        assert_roundtrip("shift+up", ev(KeyCode::Up, KeyModifiers::SHIFT));
        assert_roundtrip("shift+left", ev(KeyCode::Left, KeyModifiers::SHIFT));
    }

    #[test]
    fn from_key_drops_shift_token_for_char_code() {
        // `KeyChord::matches` ignores Shift on Char — emitting it
        // would round-trip to the same matcher.
        let s = chord_string_from_key(ev(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        ))
        .unwrap();
        assert_eq!(s, "ctrl+a");
    }

    #[test]
    fn from_key_named_keys_serialize_to_their_tokens() {
        for (token, code) in [
            ("enter", KeyCode::Enter),
            ("tab", KeyCode::Tab),
            ("space", KeyCode::Char(' ')),
            ("backspace", KeyCode::Backspace),
            ("delete", KeyCode::Delete),
            ("home", KeyCode::Home),
            ("end", KeyCode::End),
            ("pageup", KeyCode::PageUp),
            ("pagedown", KeyCode::PageDown),
        ] {
            assert_roundtrip(token, ev(code, KeyModifiers::NONE));
        }
    }

    #[test]
    fn from_key_f_keys_emit_canonical_form() {
        for n in 1..=12 {
            assert_roundtrip(&format!("f{n}"), ev(KeyCode::F(n), KeyModifiers::NONE));
        }
    }

    #[test]
    fn from_key_back_tab_unfolds_to_shift_tab() {
        // Terminals usually report Shift+Tab as `KeyCode::BackTab` —
        // our canonical form is `shift+tab` (the `parse_key_chord`
        // grammar normalizes shift+tab back to BackTab anyway).
        let s = chord_string_from_key(ev(KeyCode::BackTab, KeyModifiers::NONE)).unwrap();
        assert_eq!(s, "shift+tab");
        assert!(parse_key_chord(&s)
            .unwrap()
            .matches(ev(KeyCode::BackTab, KeyModifiers::NONE)));
    }

    #[test]
    fn from_key_canonical_modifier_order_is_ctrl_alt_shift_key() {
        let s = chord_string_from_key(ev(
            KeyCode::Up,
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT,
        ))
        .unwrap();
        assert_eq!(s, "ctrl+alt+shift+up");
    }

    #[test]
    fn from_key_rejects_esc_so_it_stays_as_cancel() {
        assert!(chord_string_from_key(ev(KeyCode::Esc, KeyModifiers::NONE)).is_none());
    }

    #[test]
    fn from_key_rejects_null_code() {
        assert!(chord_string_from_key(ev(KeyCode::Null, KeyModifiers::NONE)).is_none());
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
