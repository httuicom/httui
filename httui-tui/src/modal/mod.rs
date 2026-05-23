use crate::app::DbConfirmRunState;
use crate::input::action::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug)]
pub enum Modal {
    Help,
    DbConfirmRun(DbConfirmRunState),
}

#[derive(Debug)]
pub enum ModalOutcome {
    Continue,
    Close,
    Emit(Action),
}

impl Modal {
    pub fn handle_key(&mut self, key: KeyEvent) -> ModalOutcome {
        match self {
            Modal::Help => help_handle_key(key),
            Modal::DbConfirmRun(_) => db_confirm_run_handle_key(key),
        }
    }
}

fn help_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Close,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => ModalOutcome::Close,
        (m, KeyCode::Char('q')) if !m.contains(KeyModifiers::CONTROL) => ModalOutcome::Close,
        _ => ModalOutcome::Continue,
    }
}

fn db_confirm_run_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::CancelDbRun),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => ModalOutcome::Emit(Action::CancelDbRun),
        (KeyModifiers::NONE, KeyCode::Char('n')) | (KeyModifiers::NONE, KeyCode::Char('N')) => {
            ModalOutcome::Emit(Action::CancelDbRun)
        }
        (KeyModifiers::NONE, KeyCode::Char('y'))
        | (KeyModifiers::NONE, KeyCode::Char('Y'))
        | (_, KeyCode::Enter) => ModalOutcome::Emit(Action::ConfirmDbRun),
        _ => ModalOutcome::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn help_closes_on_esc() {
        let mut m = Modal::Help;
        assert!(matches!(
            m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
            ModalOutcome::Close
        ));
    }

    #[test]
    fn help_closes_on_q() {
        let mut m = Modal::Help;
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('q'), KeyModifiers::NONE)),
            ModalOutcome::Close
        ));
    }

    #[test]
    fn help_closes_on_ctrl_c() {
        let mut m = Modal::Help;
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            ModalOutcome::Close
        ));
    }

    #[test]
    fn help_ignores_other_keys() {
        let mut m = Modal::Help;
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('j'), KeyModifiers::NONE)),
            ModalOutcome::Continue
        ));
    }
}
