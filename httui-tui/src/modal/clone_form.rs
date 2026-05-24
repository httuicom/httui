//! V4 P5 (2026-05-23): clone-env form key handler. Extraído de
//! `modal/mod.rs` pra respeitar size limit do DoD.

use crate::app::EnvCloneFormFocus;
use crate::input::action::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::ModalOutcome;

/// Tab alterna Name↔Vars; em Vars, `j`/`k`/setas movem o cursor da
/// checklist e `space` toggla. Enter submete; Esc/Ctrl-C cancela.
/// Backspace/printable apenas quando foco está em Name.
pub(super) fn env_clone_form_handle_key(focus: EnvCloneFormFocus, key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseEnvCloneForm)
        }
        (_, KeyCode::Enter) => ModalOutcome::Emit(Action::EnvCloneFormSubmit),
        (_, KeyCode::Tab) | (_, KeyCode::BackTab) => {
            ModalOutcome::Emit(Action::EnvCloneFormFocusToggle)
        }
        (KeyModifiers::NONE, KeyCode::Down) => ModalOutcome::Emit(
            if focus == EnvCloneFormFocus::Vars {
                Action::EnvCloneFormMoveVarCursor(1)
            } else {
                Action::EnvCloneFormFocusToggle
            },
        ),
        (KeyModifiers::NONE, KeyCode::Up) => ModalOutcome::Emit(
            if focus == EnvCloneFormFocus::Vars {
                Action::EnvCloneFormMoveVarCursor(-1)
            } else {
                Action::EnvCloneFormFocusToggle
            },
        ),
        (KeyModifiers::NONE, KeyCode::Char(' ')) if focus == EnvCloneFormFocus::Vars => {
            ModalOutcome::Emit(Action::EnvCloneFormToggleVar)
        }
        // `a` em Vars focus → toggle-all (inverte: se algum desmarcado,
        // marca tudo; se todos marcados, desmarca tudo).
        (KeyModifiers::NONE, KeyCode::Char('a')) if focus == EnvCloneFormFocus::Vars => {
            ModalOutcome::Emit(Action::EnvCloneFormToggleAll)
        }
        (_, KeyCode::Backspace) if focus == EnvCloneFormFocus::Name => {
            ModalOutcome::Emit(Action::EnvCloneFormBackspace)
        }
        (mods, KeyCode::Char(c))
            if focus == EnvCloneFormFocus::Name && !mods.contains(KeyModifiers::CONTROL) =>
        {
            ModalOutcome::Emit(Action::EnvCloneFormChar(c))
        }
        _ => ModalOutcome::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{EnvCloneFormState, EnvsPageState, EnvsPaneFocus};
    use crate::modal::Modal;

    fn k(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn empty_envs_page(focus: EnvsPaneFocus) -> Modal {
        Modal::EnvsPage(EnvsPageState {
            envs: Vec::new(),
            active: None,
            selected_env: 0,
            vars: Vec::new(),
            selected_var: 0,
            focus,
        })
    }

    #[test]
    fn envs_page_c_in_envs_focus_emits_open_clone() {
        let mut m = empty_envs_page(EnvsPaneFocus::Envs);
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('c'), KeyModifiers::NONE)),
            ModalOutcome::Emit(Action::OpenEnvCloneForm)
        ));
    }

    #[test]
    fn envs_page_c_in_vars_focus_is_inert() {
        let mut m = empty_envs_page(EnvsPaneFocus::Vars);
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('c'), KeyModifiers::NONE)),
            ModalOutcome::Continue
        ));
    }

    fn clone_form(focus: EnvCloneFormFocus) -> Modal {
        Modal::EnvCloneForm(EnvCloneFormState {
            focus,
            ..Default::default()
        })
    }

    #[test]
    fn clone_form_esc_closes() {
        let mut m = clone_form(EnvCloneFormFocus::Name);
        assert!(matches!(
            m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
            ModalOutcome::Emit(Action::CloseEnvCloneForm)
        ));
    }

    #[test]
    fn clone_form_ctrl_c_closes() {
        let mut m = clone_form(EnvCloneFormFocus::Vars);
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            ModalOutcome::Emit(Action::CloseEnvCloneForm)
        ));
    }

    #[test]
    fn clone_form_enter_submits() {
        let mut m = clone_form(EnvCloneFormFocus::Name);
        assert!(matches!(
            m.handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
            ModalOutcome::Emit(Action::EnvCloneFormSubmit)
        ));
    }

    #[test]
    fn clone_form_tab_toggles_focus() {
        let mut m = clone_form(EnvCloneFormFocus::Name);
        assert!(matches!(
            m.handle_key(k(KeyCode::Tab, KeyModifiers::NONE)),
            ModalOutcome::Emit(Action::EnvCloneFormFocusToggle)
        ));
        let mut m = clone_form(EnvCloneFormFocus::Vars);
        assert!(matches!(
            m.handle_key(k(KeyCode::BackTab, KeyModifiers::SHIFT)),
            ModalOutcome::Emit(Action::EnvCloneFormFocusToggle)
        ));
    }

    #[test]
    fn clone_form_down_in_name_toggles_focus() {
        let mut m = clone_form(EnvCloneFormFocus::Name);
        assert!(matches!(
            m.handle_key(k(KeyCode::Down, KeyModifiers::NONE)),
            ModalOutcome::Emit(Action::EnvCloneFormFocusToggle)
        ));
    }

    #[test]
    fn clone_form_down_in_vars_moves_cursor() {
        let mut m = clone_form(EnvCloneFormFocus::Vars);
        match m.handle_key(k(KeyCode::Down, KeyModifiers::NONE)) {
            ModalOutcome::Emit(Action::EnvCloneFormMoveVarCursor(1)) => {}
            other => panic!("expected MoveVarCursor(1), got {other:?}"),
        }
    }

    #[test]
    fn clone_form_up_in_vars_moves_cursor_up() {
        let mut m = clone_form(EnvCloneFormFocus::Vars);
        match m.handle_key(k(KeyCode::Up, KeyModifiers::NONE)) {
            ModalOutcome::Emit(Action::EnvCloneFormMoveVarCursor(-1)) => {}
            other => panic!("expected MoveVarCursor(-1), got {other:?}"),
        }
    }

    #[test]
    fn clone_form_space_in_vars_toggles_var() {
        let mut m = clone_form(EnvCloneFormFocus::Vars);
        assert!(matches!(
            m.handle_key(k(KeyCode::Char(' '), KeyModifiers::NONE)),
            ModalOutcome::Emit(Action::EnvCloneFormToggleVar)
        ));
    }

    #[test]
    fn clone_form_space_in_name_inserts_char() {
        let mut m = clone_form(EnvCloneFormFocus::Name);
        match m.handle_key(k(KeyCode::Char(' '), KeyModifiers::NONE)) {
            ModalOutcome::Emit(Action::EnvCloneFormChar(' ')) => {}
            other => panic!("expected EnvCloneFormChar(' '), got {other:?}"),
        }
    }

    #[test]
    fn clone_form_a_in_vars_toggles_all() {
        let mut m = clone_form(EnvCloneFormFocus::Vars);
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('a'), KeyModifiers::NONE)),
            ModalOutcome::Emit(Action::EnvCloneFormToggleAll)
        ));
    }

    #[test]
    fn clone_form_a_in_name_inserts_char() {
        let mut m = clone_form(EnvCloneFormFocus::Name);
        match m.handle_key(k(KeyCode::Char('a'), KeyModifiers::NONE)) {
            ModalOutcome::Emit(Action::EnvCloneFormChar('a')) => {}
            other => panic!("expected EnvCloneFormChar('a'), got {other:?}"),
        }
    }

    #[test]
    fn clone_form_backspace_only_when_name_focused() {
        let mut m = clone_form(EnvCloneFormFocus::Name);
        assert!(matches!(
            m.handle_key(k(KeyCode::Backspace, KeyModifiers::NONE)),
            ModalOutcome::Emit(Action::EnvCloneFormBackspace)
        ));
        let mut m = clone_form(EnvCloneFormFocus::Vars);
        assert!(matches!(
            m.handle_key(k(KeyCode::Backspace, KeyModifiers::NONE)),
            ModalOutcome::Continue
        ));
    }

    #[test]
    fn clone_form_ctrl_letter_not_inserted_as_char() {
        let mut m = clone_form(EnvCloneFormFocus::Name);
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('h'), KeyModifiers::CONTROL)),
            ModalOutcome::Continue
        ));
    }
}
