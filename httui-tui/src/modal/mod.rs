use crate::input::action::Action;

#[derive(Debug)]
pub enum Modal {}

#[derive(Debug)]
#[allow(dead_code)]
pub enum ModalOutcome {
    Continue,
    Close,
    Emit(Action),
}

impl Modal {
    pub fn handle_key(
        &mut self,
        _key: crossterm::event::KeyEvent,
    ) -> ModalOutcome {
        match *self {}
    }
}
