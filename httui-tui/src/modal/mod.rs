use crate::app::{
    BlockHistoryState, BlockTemplatePickerState, ConnectionDeleteConfirmState,
    ConnectionFormState, ConnectionPickerState, ConnectionsPageState, DbConfirmRunState,
    DbExportPickerState, EnvironmentPickerState, TabPickerState,
};
use crate::input::action::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug)]
pub enum Modal {
    Help,
    DbConfirmRun(DbConfirmRunState),
    BlockHistory(BlockHistoryState),
    DbExportPicker(DbExportPickerState),
    TabPicker(TabPickerState),
    BlockTemplatePicker(BlockTemplatePickerState),
    EnvironmentPicker(EnvironmentPickerState),
    ConnectionPicker(ConnectionPickerState),
    /// V3 (2026-05-23): fullscreen Connections page. Master-detail
    /// list of every entry in `<vault>/connections.toml`. `n` opens
    /// a create form (P3); `D` deletes the highlighted entry (P4);
    /// `e` edits (P3). Esc / Ctrl-C close.
    Connections(ConnectionsPageState),
    /// V3 P3 (2026-05-23): create-connection form, opened by `n` on
    /// the Connections page. Submits to `ConnectionsStore::create`.
    ConnectionForm(ConnectionFormState),
    /// V3 P4 (2026-05-23): destructive confirm for `D` on the
    /// Connections page. `y`/`Enter` runs store.delete; `n`/`Esc`
    /// reopens the page unchanged.
    ConnectionDeleteConfirm(ConnectionDeleteConfirmState),
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
            Modal::BlockHistory(_) => block_history_handle_key(key),
            Modal::DbExportPicker(_) => db_export_picker_handle_key(key),
            Modal::TabPicker(_) => tab_picker_handle_key(key),
            Modal::BlockTemplatePicker(_) => block_template_picker_handle_key(key),
            Modal::EnvironmentPicker(_) => environment_picker_handle_key(key),
            Modal::ConnectionPicker(_) => connection_picker_handle_key(key),
            Modal::Connections(_) => connections_page_handle_key(key),
            Modal::ConnectionForm(s) => connection_form_handle_key(s, key),
            Modal::ConnectionDeleteConfirm(_) => connection_delete_confirm_handle_key(key),
        }
    }
}

/// V3 P3: form key handling. Tab/Shift-Tab/Up/Down cycle focus;
/// Esc/Ctrl-C cancel; Enter submits; typing routes into the focused
/// `LineEdit` (or toggles the driver/readonly fields).
fn connection_form_handle_key(state: &ConnectionFormState, key: KeyEvent) -> ModalOutcome {
    use crate::app::ConnectionFormFocus;
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::CloseConnectionForm),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseConnectionForm)
        }
        (_, KeyCode::Enter) => ModalOutcome::Emit(Action::ConnectionFormSubmit),
        (_, KeyCode::Tab) | (_, KeyCode::Down) => {
            ModalOutcome::Emit(Action::ConnectionFormFocusNext)
        }
        (_, KeyCode::BackTab) | (_, KeyCode::Up) => {
            ModalOutcome::Emit(Action::ConnectionFormFocusPrev)
        }
        // Driver field is a 3-option dial: arrows / space cycle.
        (_, KeyCode::Char(' '))
            if matches!(state.focus, ConnectionFormFocus::Driver) =>
        {
            ModalOutcome::Emit(Action::ConnectionFormCycleDriver(1))
        }
        (_, KeyCode::Left) if matches!(state.focus, ConnectionFormFocus::Driver) => {
            ModalOutcome::Emit(Action::ConnectionFormCycleDriver(-1))
        }
        (_, KeyCode::Right) if matches!(state.focus, ConnectionFormFocus::Driver) => {
            ModalOutcome::Emit(Action::ConnectionFormCycleDriver(1))
        }
        // Readonly toggle (space flips).
        (_, KeyCode::Char(' '))
            if matches!(state.focus, ConnectionFormFocus::Readonly) =>
        {
            ModalOutcome::Emit(Action::ConnectionFormToggleReadonly)
        }
        // LineEdit ops on the focused text field.
        (_, KeyCode::Backspace) => ModalOutcome::Emit(Action::ConnectionFormBackspace),
        (_, KeyCode::Delete) => ModalOutcome::Emit(Action::ConnectionFormDelete),
        (_, KeyCode::Left) => ModalOutcome::Emit(Action::ConnectionFormCursorLeft),
        (_, KeyCode::Right) => ModalOutcome::Emit(Action::ConnectionFormCursorRight),
        (_, KeyCode::Home) => ModalOutcome::Emit(Action::ConnectionFormCursorHome),
        (_, KeyCode::End) => ModalOutcome::Emit(Action::ConnectionFormCursorEnd),
        // Printable char (no CONTROL) → insert into focused input.
        // Driver/Readonly are gated above; falling through here means
        // any non-special key is a no-op for those two fields.
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            if matches!(
                state.focus,
                ConnectionFormFocus::Driver | ConnectionFormFocus::Readonly
            ) {
                ModalOutcome::Continue
            } else {
                ModalOutcome::Emit(Action::ConnectionFormChar(c))
            }
        }
        _ => ModalOutcome::Continue,
    }
}

/// Connections page (`gC` / `Alt+P`). Vocab: navigate with `j`/`k`/
/// arrows/`Ctrl+n`/`Ctrl+p`; `n` opens the create form (P3); `D`
/// opens the delete-confirm modal (P4); `Esc`/`Ctrl+C` close.
fn connections_page_handle_key(key: KeyEvent) -> ModalOutcome {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char('n')) => {
            return ModalOutcome::Emit(Action::OpenConnectionForm);
        }
        (KeyModifiers::NONE, KeyCode::Char('e')) => {
            return ModalOutcome::Emit(Action::OpenConnectionEditForm);
        }
        (KeyModifiers::NONE, KeyCode::Char('t')) => {
            return ModalOutcome::Emit(Action::TestSelectedConnection);
        }
        // Capital D — matches the picker's destructive chord style
        // (lowercase 'd' would conflict with vim's `dd` reflex).
        (mods, KeyCode::Char('D')) if !mods.contains(KeyModifiers::CONTROL) => {
            return ModalOutcome::Emit(Action::OpenConnectionDeleteConfirm);
        }
        _ => {}
    }
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveConnectionsPageCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveConnectionsPageCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseConnectionsPage),
        ListPickerKey::Confirm | ListPickerKey::Other => ModalOutcome::Continue,
    }
}

/// V3 P4: y/Enter → confirm delete; n/Esc/Ctrl-C → cancel; other → noop.
fn connection_delete_confirm_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::CancelConnectionDelete),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CancelConnectionDelete)
        }
        (KeyModifiers::NONE, KeyCode::Char('n')) | (KeyModifiers::NONE, KeyCode::Char('N')) => {
            ModalOutcome::Emit(Action::CancelConnectionDelete)
        }
        (KeyModifiers::NONE, KeyCode::Char('y'))
        | (KeyModifiers::NONE, KeyCode::Char('Y'))
        | (_, KeyCode::Enter) => ModalOutcome::Emit(Action::ConfirmConnectionDelete),
        _ => ModalOutcome::Continue,
    }
}

/// ConnectionPicker reusa o vocab `list_picker_key`, mas adiciona
/// `Shift+D` para deletar a conexão highlighted (mantém o picker
/// aberto com a lista recarregada).
fn connection_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    if let (mods, KeyCode::Char('D')) = (key.modifiers, key.code) {
        if !mods.contains(KeyModifiers::CONTROL) {
            return ModalOutcome::Emit(Action::DeleteConnectionInPicker);
        }
    }
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveConnectionPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveConnectionPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseConnectionPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmConnectionPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

fn environment_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveEnvironmentPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveEnvironmentPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseEnvironmentPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmEnvironmentPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

fn block_template_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveBlockTemplatePickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveBlockTemplatePickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseBlockTemplatePicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmBlockTemplatePicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

fn tab_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveTabPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveTabPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseTabPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmTabPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

fn block_history_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveBlockHistoryCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveBlockHistoryCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseBlockHistory),
        ListPickerKey::Confirm | ListPickerKey::Other => ModalOutcome::Continue,
    }
}

fn db_export_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveDbExportPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveDbExportPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseDbExportPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmDbExportPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ListPickerKey {
    Up,
    Down,
    Confirm,
    Cancel,
    Other,
}

fn list_picker_key(key: KeyEvent) -> ListPickerKey {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ListPickerKey::Cancel,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => ListPickerKey::Cancel,
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => ListPickerKey::Down,
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => ListPickerKey::Up,
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => ListPickerKey::Down,
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => ListPickerKey::Up,
        (_, KeyCode::Enter) => ListPickerKey::Confirm,
        _ => ListPickerKey::Other,
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

    fn empty_conn_picker() -> Modal {
        Modal::ConnectionPicker(ConnectionPickerState {
            segment_idx: 0,
            connections: Vec::new(),
            selected: 0,
        })
    }

    #[test]
    fn connection_picker_capital_d_emits_delete() {
        let mut m = empty_conn_picker();
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('D'), KeyModifiers::SHIFT)),
            ModalOutcome::Emit(Action::DeleteConnectionInPicker)
        ));
        let mut m = empty_conn_picker();
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('D'), KeyModifiers::NONE)),
            ModalOutcome::Emit(Action::DeleteConnectionInPicker)
        ));
    }

    #[test]
    fn connection_picker_lowercase_d_is_inert() {
        let mut m = empty_conn_picker();
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('d'), KeyModifiers::NONE)),
            ModalOutcome::Continue
        ));
    }

    #[test]
    fn connection_picker_ctrl_d_does_not_delete() {
        let mut m = empty_conn_picker();
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('D'), KeyModifiers::CONTROL)),
            ModalOutcome::Continue
        ));
    }
}
