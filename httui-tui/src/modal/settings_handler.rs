//! Settings page key handler.

use crate::input::action::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::ModalOutcome;

/// Settings page handler. Two key flows:
///
/// 1. **Navigation** (default): `Tab` / `Shift+Tab` cycle sections,
///    `j`/`k`/arrows move the row cursor, `Enter` begins a rebind on
///    the highlighted Keymaps row, `r` resets it to the built-in
///    default, `Esc`/`Ctrl-C` closes.
/// 2. **Capture** (`is_capturing == true`): the very next key event
///    becomes the new chord — except `Esc`, which cancels capture
///    without committing. Every other key event is forwarded to the
///    applier as [`Action::SettingsCommitCapture`].
pub(super) fn settings_page_handle_key(is_capturing: bool, key: KeyEvent) -> ModalOutcome {
    if is_capturing {
        if matches!(key.code, KeyCode::Esc) {
            return ModalOutcome::Emit(Action::SettingsCancelCapture);
        }
        return ModalOutcome::Emit(Action::SettingsCommitCapture(key));
    }
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::CloseSettingsPage),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseSettingsPage)
        }
        (_, KeyCode::Tab) => ModalOutcome::Emit(Action::SettingsNextSection),
        (_, KeyCode::BackTab) => ModalOutcome::Emit(Action::SettingsPrevSection),
        // Section cycling with `h`/`l` matches the vim left/right
        // convention used by the connections page's column hint.
        (KeyModifiers::NONE, KeyCode::Char('l')) | (_, KeyCode::Right) => {
            ModalOutcome::Emit(Action::SettingsNextSection)
        }
        (KeyModifiers::NONE, KeyCode::Char('h')) | (_, KeyCode::Left) => {
            ModalOutcome::Emit(Action::SettingsPrevSection)
        }
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            ModalOutcome::Emit(Action::SettingsMoveCursor(1))
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            ModalOutcome::Emit(Action::SettingsMoveCursor(-1))
        }
        (_, KeyCode::Enter) => ModalOutcome::Emit(Action::SettingsActivateRow),
        (KeyModifiers::NONE, KeyCode::Char('r')) => {
            ModalOutcome::Emit(Action::SettingsResetBinding)
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
    fn capture_mode_esc_cancels() {
        let a = emitted(settings_page_handle_key(
            true,
            ev(KeyCode::Esc, KeyModifiers::NONE),
        ));
        assert_eq!(a, Some(Action::SettingsCancelCapture));
    }

    #[test]
    fn capture_mode_any_other_key_commits() {
        let ev = ev(KeyCode::Char('y'), KeyModifiers::CONTROL);
        let a = emitted(settings_page_handle_key(true, ev));
        assert_eq!(a, Some(Action::SettingsCommitCapture(ev)));
    }

    #[test]
    fn nav_esc_closes_page() {
        let a = emitted(settings_page_handle_key(
            false,
            ev(KeyCode::Esc, KeyModifiers::NONE),
        ));
        assert_eq!(a, Some(Action::CloseSettingsPage));
    }

    #[test]
    fn nav_ctrl_c_closes_page() {
        let a = emitted(settings_page_handle_key(
            false,
            ev(KeyCode::Char('c'), KeyModifiers::CONTROL),
        ));
        assert_eq!(a, Some(Action::CloseSettingsPage));
    }

    #[test]
    fn nav_tab_cycles_section_forward() {
        let a = emitted(settings_page_handle_key(
            false,
            ev(KeyCode::Tab, KeyModifiers::NONE),
        ));
        assert_eq!(a, Some(Action::SettingsNextSection));
    }

    #[test]
    fn nav_back_tab_cycles_section_back() {
        let a = emitted(settings_page_handle_key(
            false,
            ev(KeyCode::BackTab, KeyModifiers::NONE),
        ));
        assert_eq!(a, Some(Action::SettingsPrevSection));
    }

    #[test]
    fn nav_h_l_arrows_cycle_section() {
        for k in [
            ev(KeyCode::Char('l'), KeyModifiers::NONE),
            ev(KeyCode::Right, KeyModifiers::NONE),
        ] {
            assert_eq!(
                emitted(settings_page_handle_key(false, k)),
                Some(Action::SettingsNextSection),
            );
        }
        for k in [
            ev(KeyCode::Char('h'), KeyModifiers::NONE),
            ev(KeyCode::Left, KeyModifiers::NONE),
        ] {
            assert_eq!(
                emitted(settings_page_handle_key(false, k)),
                Some(Action::SettingsPrevSection),
            );
        }
    }

    #[test]
    fn nav_j_k_arrows_move_cursor() {
        for (k, delta) in [
            (ev(KeyCode::Char('j'), KeyModifiers::NONE), 1),
            (ev(KeyCode::Down, KeyModifiers::NONE), 1),
            (ev(KeyCode::Char('k'), KeyModifiers::NONE), -1),
            (ev(KeyCode::Up, KeyModifiers::NONE), -1),
        ] {
            assert_eq!(
                emitted(settings_page_handle_key(false, k)),
                Some(Action::SettingsMoveCursor(delta)),
            );
        }
    }

    #[test]
    fn nav_enter_begins_capture() {
        let a = emitted(settings_page_handle_key(
            false,
            ev(KeyCode::Enter, KeyModifiers::NONE),
        ));
        assert_eq!(a, Some(Action::SettingsActivateRow));
    }

    #[test]
    fn nav_r_resets_binding() {
        let a = emitted(settings_page_handle_key(
            false,
            ev(KeyCode::Char('r'), KeyModifiers::NONE),
        ));
        assert_eq!(a, Some(Action::SettingsResetBinding));
    }

    #[test]
    fn nav_unbound_key_is_continue() {
        let out = settings_page_handle_key(false, ev(KeyCode::Char('z'), KeyModifiers::NONE));
        assert!(matches!(out, ModalOutcome::Continue));
    }
}
