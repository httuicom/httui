use crate::app::{
    BlockHistoryState, BlockTemplatePickerState, ConnectionDeleteConfirmState,
    ConnectionFormState, ConnectionPickerState, ConnectionsPageState, DbConfirmRunState,
    DbExportPickerState, EnvCloneFormState, EnvDeleteConfirmState, EnvFormState,
    EnvironmentPickerState, EnvsPageState, EnvsPaneFocus, TabPickerState, VarDeleteConfirmState,
    VarFormFocus, VarFormState,
};

/// V4 P5: clone-env form key handler (extraído pra respeitar size limit).
mod clone_form;
/// V4 P6: utils compartilhados entre handlers de modal.
mod util;

use util::digit_1_9;
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
    /// V4 P2: Vars + Envs page (gV / Alt+V).
    EnvsPage(EnvsPageState),
    /// V4 P3: create/edit env form.
    EnvForm(EnvFormState),
    /// V4 P3: create/edit var form.
    VarForm(VarFormState),
    /// V4 P4: destructive confirm pra delete env.
    EnvDeleteConfirm(EnvDeleteConfirmState),
    /// V4 P4: destructive confirm pra delete var.
    VarDeleteConfirm(VarDeleteConfirmState),
    /// V4 P5 (2026-05-23): clone env form com checkboxes por var.
    /// Aberto por `c` na EnvsPage com focus Envs. Cria env destino +
    /// bulk set_var apenas das vars marcadas (default: todas ON).
    EnvCloneForm(EnvCloneFormState),
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
            Modal::EnvsPage(s) => envs_page_handle_key(s.focus, key),
            Modal::EnvForm(_) => env_form_handle_key(key),
            Modal::VarForm(s) => var_form_handle_key(s.focus, key),
            Modal::EnvDeleteConfirm(_) | Modal::VarDeleteConfirm(_) => env_or_var_confirm_handle_key(key),
            Modal::EnvCloneForm(s) => clone_form::env_clone_form_handle_key(s.focus, key),
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
    // V4 P6: numeric shortcuts 1-9 ativam env diretamente (modal —
    // sem conflito com vim count-prefix).
    if let Some(idx) = digit_1_9(key) {
        return ModalOutcome::Emit(Action::ActivateEnvByIndex(idx));
    }
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveEnvironmentPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveEnvironmentPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseEnvironmentPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmEnvironmentPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

// V4 P6: digit_1_9 vive em `modal/util.rs`.

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

// V4 P2-P4 handlers ----------

fn envs_page_handle_key(focus: EnvsPaneFocus, key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseEnvsPage)
        }
        (_, KeyCode::Tab) | (_, KeyCode::BackTab) | (KeyModifiers::NONE, KeyCode::Char('\t')) => {
            ModalOutcome::Emit(Action::EnvsPageFocusToggle)
        }
        (KeyModifiers::NONE, KeyCode::Char('h')) => ModalOutcome::Emit(Action::EnvsPageFocusEnvs),
        (KeyModifiers::NONE, KeyCode::Char('l')) => ModalOutcome::Emit(Action::EnvsPageFocusVars),
        (KeyModifiers::NONE, KeyCode::Char('j')) | (_, KeyCode::Down) => {
            ModalOutcome::Emit(if focus == EnvsPaneFocus::Envs {
                Action::EnvsPageMoveEnvCursor(1)
            } else {
                Action::EnvsPageMoveVarCursor(1)
            })
        }
        (KeyModifiers::NONE, KeyCode::Char('k')) | (_, KeyCode::Up) => {
            ModalOutcome::Emit(if focus == EnvsPaneFocus::Envs {
                Action::EnvsPageMoveEnvCursor(-1)
            } else {
                Action::EnvsPageMoveVarCursor(-1)
            })
        }
        // a (activate) on envs pane → switch active env.
        (KeyModifiers::NONE, KeyCode::Char('a')) if focus == EnvsPaneFocus::Envs => {
            ModalOutcome::Emit(Action::EnvsPageActivateEnv)
        }
        (KeyModifiers::NONE, KeyCode::Char('n')) => {
            ModalOutcome::Emit(if focus == EnvsPaneFocus::Envs {
                Action::OpenEnvForm
            } else {
                Action::OpenVarForm
            })
        }
        (KeyModifiers::NONE, KeyCode::Char('e')) => {
            ModalOutcome::Emit(if focus == EnvsPaneFocus::Envs {
                Action::OpenEnvEditForm
            } else {
                Action::OpenVarEditForm
            })
        }
        (mods, KeyCode::Char('D')) if !mods.contains(KeyModifiers::CONTROL) => {
            ModalOutcome::Emit(if focus == EnvsPaneFocus::Envs {
                Action::OpenEnvDeleteConfirm
            } else {
                Action::OpenVarDeleteConfirm
            })
        }
        // V4 P5: `c` na env-focused page → clone env form. No-op em
        // vars focus (clone é por-env, não por-var).
        (KeyModifiers::NONE, KeyCode::Char('c')) if focus == EnvsPaneFocus::Envs => {
            ModalOutcome::Emit(Action::OpenEnvCloneForm)
        }
        _ => {
            // V4 P6: 1-9 ativam env por índice (Envs-focus only;
            // em Vars-focus números não têm semantica).
            if focus == EnvsPaneFocus::Envs {
                if let Some(idx) = digit_1_9(key) {
                    return ModalOutcome::Emit(Action::ActivateEnvByIndex(idx));
                }
            }
            ModalOutcome::Continue
        }
    }
}

fn env_form_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseEnvForm)
        }
        (_, KeyCode::Enter) => ModalOutcome::Emit(Action::EnvFormSubmit),
        (_, KeyCode::Backspace) => ModalOutcome::Emit(Action::EnvFormBackspace),
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            ModalOutcome::Emit(Action::EnvFormChar(c))
        }
        _ => ModalOutcome::Continue,
    }
}

fn var_form_handle_key(focus: VarFormFocus, key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseVarForm)
        }
        (_, KeyCode::Enter) => ModalOutcome::Emit(Action::VarFormSubmit),
        (_, KeyCode::Tab) | (_, KeyCode::Down) => ModalOutcome::Emit(Action::VarFormFocusNext),
        (_, KeyCode::BackTab) | (_, KeyCode::Up) => ModalOutcome::Emit(Action::VarFormFocusPrev),
        (KeyModifiers::NONE, KeyCode::Char(' ')) if focus == VarFormFocus::Secret => {
            ModalOutcome::Emit(Action::VarFormToggleSecret)
        }
        (_, KeyCode::Backspace) => ModalOutcome::Emit(Action::VarFormBackspace),
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            if focus == VarFormFocus::Secret {
                ModalOutcome::Continue
            } else {
                ModalOutcome::Emit(Action::VarFormChar(c))
            }
        }
        _ => ModalOutcome::Continue,
    }
}

fn env_or_var_confirm_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c'))
        | (KeyModifiers::NONE, KeyCode::Char('n')) | (KeyModifiers::NONE, KeyCode::Char('N')) => {
            ModalOutcome::Emit(Action::CancelEnvOrVarDelete)
        }
        (_, KeyCode::Enter)
        | (KeyModifiers::NONE, KeyCode::Char('y'))
        | (KeyModifiers::NONE, KeyCode::Char('Y')) => {
            ModalOutcome::Emit(Action::ConfirmEnvOrVarDelete)
        }
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

    // V4 P5: tests do clone form ficam em `modal/clone_form.rs`.

    // V4 P6: tests de digit_1_9 ficam em `modal/util.rs`.

    fn env_picker_modal() -> Modal {
        Modal::EnvironmentPicker(EnvironmentPickerState {
            entries: Vec::new(),
            selected: 0,
            active_id: None,
        })
    }

    #[test]
    fn env_picker_digit_emits_activate() {
        let mut m = env_picker_modal();
        match m.handle_key(k(KeyCode::Char('2'), KeyModifiers::NONE)) {
            ModalOutcome::Emit(Action::ActivateEnvByIndex(2)) => {}
            other => panic!("expected ActivateEnvByIndex(2), got {other:?}"),
        }
    }

    #[test]
    fn envs_page_envs_focus_digit_emits_activate() {
        let mut m = empty_envs_page(EnvsPaneFocus::Envs);
        match m.handle_key(k(KeyCode::Char('7'), KeyModifiers::NONE)) {
            ModalOutcome::Emit(Action::ActivateEnvByIndex(7)) => {}
            other => panic!("expected ActivateEnvByIndex(7), got {other:?}"),
        }
    }

    #[test]
    fn envs_page_vars_focus_digit_is_inert() {
        let mut m = empty_envs_page(EnvsPaneFocus::Vars);
        assert!(matches!(
            m.handle_key(k(KeyCode::Char('3'), KeyModifiers::NONE)),
            ModalOutcome::Continue
        ));
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
}
