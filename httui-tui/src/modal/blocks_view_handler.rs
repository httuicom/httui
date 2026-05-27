use crate::input::action::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::ModalOutcome;

pub(super) fn blocks_view_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::CloseBlocksView),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseBlocksView)
        }
        (KeyModifiers::ALT, KeyCode::Char('m')) => {
            ModalOutcome::Emit(Action::CloseBlocksView)
        }
        (_, KeyCode::Tab) => ModalOutcome::Emit(Action::BlocksViewNextRegion),
        (_, KeyCode::BackTab) => ModalOutcome::Emit(Action::BlocksViewPrevRegion),
        (KeyModifiers::ALT, KeyCode::Char(c)) if c.is_ascii_digit() && c != '0' => {
            let n = (c as u8 - b'0') as usize;
            ModalOutcome::Emit(Action::BlocksViewJumpRegion(n))
        }
        _ => ModalOutcome::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn emitted(out: ModalOutcome) -> Option<Action> {
        match out {
            ModalOutcome::Emit(a) => Some(a),
            _ => None,
        }
    }

    #[test]
    fn esc_closes_view() {
        assert_eq!(
            emitted(blocks_view_handle_key(ev(KeyCode::Esc, KeyModifiers::NONE))),
            Some(Action::CloseBlocksView),
        );
    }

    #[test]
    fn ctrl_c_closes_view() {
        assert_eq!(
            emitted(blocks_view_handle_key(ev(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
            ))),
            Some(Action::CloseBlocksView),
        );
    }

    #[test]
    fn alt_m_closes_view() {
        assert_eq!(
            emitted(blocks_view_handle_key(ev(
                KeyCode::Char('m'),
                KeyModifiers::ALT,
            ))),
            Some(Action::CloseBlocksView),
        );
    }

    #[test]
    fn tab_cycles_next_region() {
        assert_eq!(
            emitted(blocks_view_handle_key(ev(KeyCode::Tab, KeyModifiers::NONE))),
            Some(Action::BlocksViewNextRegion),
        );
    }

    #[test]
    fn back_tab_cycles_prev_region() {
        assert_eq!(
            emitted(blocks_view_handle_key(ev(
                KeyCode::BackTab,
                KeyModifiers::NONE,
            ))),
            Some(Action::BlocksViewPrevRegion),
        );
    }

    #[test]
    fn alt_digit_jumps_to_region() {
        for n in 1..=9 {
            let c = char::from_digit(n as u32, 10).unwrap();
            assert_eq!(
                emitted(blocks_view_handle_key(ev(
                    KeyCode::Char(c),
                    KeyModifiers::ALT,
                ))),
                Some(Action::BlocksViewJumpRegion(n)),
                "Alt+{n} should emit jump",
            );
        }
    }

    #[test]
    fn alt_zero_is_not_a_jump() {
        assert!(matches!(
            blocks_view_handle_key(ev(KeyCode::Char('0'), KeyModifiers::ALT)),
            ModalOutcome::Continue,
        ));
    }

    #[test]
    fn unbound_keys_return_continue() {
        for key in [
            ev(KeyCode::Char('a'), KeyModifiers::NONE),
            ev(KeyCode::Char('1'), KeyModifiers::NONE),
            ev(KeyCode::Enter, KeyModifiers::NONE),
        ] {
            assert!(matches!(
                blocks_view_handle_key(key),
                ModalOutcome::Continue,
            ));
        }
    }
}
