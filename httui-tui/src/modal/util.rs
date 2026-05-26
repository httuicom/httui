//! V4 P6 (2026-05-23): utilitários compartilhados entre handlers de
//! modal. Extraído de `modal/mod.rs` pra respeitar size limit do DoD.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Extrai dígito 1-9 (NONE modifier) de uma KeyEvent. Retorna None
/// pra `0` ou qualquer outra coisa. Usado pelo environment_picker e
/// envs_page (focus=Envs) pra ativar env por índice.
pub(super) fn digit_1_9(key: KeyEvent) -> Option<usize> {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    if modifiers != KeyModifiers::NONE {
        return None;
    }
    if let KeyCode::Char(c) = code {
        if ('1'..='9').contains(&c) {
            return Some((c as u8 - b'0') as usize);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn recognizes_1_to_9() {
        assert_eq!(
            digit_1_9(k(KeyCode::Char('1'), KeyModifiers::NONE)),
            Some(1)
        );
        assert_eq!(
            digit_1_9(k(KeyCode::Char('5'), KeyModifiers::NONE)),
            Some(5)
        );
        assert_eq!(
            digit_1_9(k(KeyCode::Char('9'), KeyModifiers::NONE)),
            Some(9)
        );
    }

    #[test]
    fn rejects_zero_and_letters() {
        assert_eq!(digit_1_9(k(KeyCode::Char('0'), KeyModifiers::NONE)), None);
        assert_eq!(digit_1_9(k(KeyCode::Char('a'), KeyModifiers::NONE)), None);
        assert_eq!(digit_1_9(k(KeyCode::Esc, KeyModifiers::NONE)), None);
    }

    #[test]
    fn requires_no_modifiers() {
        assert_eq!(
            digit_1_9(k(KeyCode::Char('3'), KeyModifiers::CONTROL)),
            None
        );
        assert_eq!(digit_1_9(k(KeyCode::Char('3'), KeyModifiers::SHIFT)), None);
    }
}
