//! Per-surface key handlers extracted from `modal::mod`.

use crate::app::{
    BlocksUnsavedPromptFocus, BlocksUnsavedPromptState, ConnectionFormState, EnvsPaneFocus,
    VarFormFocus, VaultCloneFormFocus, VaultCreateFormFocus,
};
use crate::input::action::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::util::digit_1_9;
use super::{ModalOutcome, PromptKind};

pub(super) fn blocks_unsaved_prompt_handle_key(
    state: &mut BlocksUnsavedPromptState,
    key: KeyEvent,
) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::BlocksUnsavedPromptCancel)
        }
        (KeyModifiers::NONE, KeyCode::Char('s')) | (KeyModifiers::NONE, KeyCode::Char('S')) => {
            ModalOutcome::Emit(Action::BlocksUnsavedPromptSave)
        }
        (KeyModifiers::NONE, KeyCode::Char('d')) | (KeyModifiers::NONE, KeyCode::Char('D')) => {
            ModalOutcome::Emit(Action::BlocksUnsavedPromptDiscard)
        }
        (KeyModifiers::NONE, KeyCode::Right) | (_, KeyCode::Tab) => {
            state.focus = state.focus.next();
            ModalOutcome::Continue
        }
        (KeyModifiers::NONE, KeyCode::Left) | (_, KeyCode::BackTab) => {
            state.focus = state.focus.prev();
            ModalOutcome::Continue
        }
        (_, KeyCode::Enter) => match state.focus {
            BlocksUnsavedPromptFocus::Save => ModalOutcome::Emit(Action::BlocksUnsavedPromptSave),
            BlocksUnsavedPromptFocus::Discard => {
                ModalOutcome::Emit(Action::BlocksUnsavedPromptDiscard)
            }
            BlocksUnsavedPromptFocus::Cancel => {
                ModalOutcome::Emit(Action::BlocksUnsavedPromptCancel)
            }
        },
        _ => ModalOutcome::Continue,
    }
}

pub(super) fn vault_missing_secrets_handle_key(editing: bool, key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    if editing {
        return match (modifiers, code) {
            (_, KeyCode::Esc) => ModalOutcome::Emit(Action::VaultMissingSecretsCancelEdit),
            (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
                ModalOutcome::Emit(Action::VaultMissingSecretsCancelEdit)
            }
            (KeyModifiers::CONTROL, KeyCode::Char('v')) => ModalOutcome::Emit(Action::PasteSystem),
            (_, KeyCode::Enter) => ModalOutcome::Emit(Action::VaultMissingSecretsSave),
            (_, KeyCode::Backspace) => ModalOutcome::Emit(Action::VaultMissingSecretsBackspace),
            (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
                ModalOutcome::Emit(Action::VaultMissingSecretsChar(c))
            }
            _ => ModalOutcome::Continue,
        };
    }
    // Browse mode.
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseVaultMissingSecrets)
        }
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) | (_, KeyCode::Tab) => {
            ModalOutcome::Emit(Action::MoveVaultMissingSecretsCursor(1))
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) | (_, KeyCode::BackTab) => {
            ModalOutcome::Emit(Action::MoveVaultMissingSecretsCursor(-1))
        }
        (_, KeyCode::Enter) | (KeyModifiers::NONE, KeyCode::Char('e')) => {
            ModalOutcome::Emit(Action::VaultMissingSecretsEnterEdit)
        }
        (KeyModifiers::NONE, KeyCode::Char('s')) => {
            ModalOutcome::Emit(Action::VaultMissingSecretsSkip)
        }
        _ => ModalOutcome::Continue,
    }
}

pub(super) fn vault_open_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    if let (_, KeyCode::Backspace) = (modifiers, code) {
        return ModalOutcome::Emit(Action::VaultOpenPickerUp);
    }
    if let (KeyModifiers::NONE, KeyCode::Char('o') | KeyCode::Char('O')) = (modifiers, code) {
        return ModalOutcome::Emit(Action::VaultOpenPickerOpenAsVault);
    }
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveVaultOpenPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveVaultOpenPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseVaultOpenPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::VaultOpenPickerEnter),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

pub(super) fn vault_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    // Sub-mode verbs (composes "vault" as the entry point with verbs
    // n=new / o=open / c=clone inside it — same shape as
    // ConnectionsPage: n=new, D=delete).
    if let (KeyModifiers::NONE, KeyCode::Char('n')) = (key.modifiers, key.code) {
        return ModalOutcome::Emit(Action::OpenVaultCreateForm);
    }
    if let (KeyModifiers::NONE, KeyCode::Char('c')) = (key.modifiers, key.code) {
        return ModalOutcome::Emit(Action::OpenVaultCloneForm);
    }
    if let (KeyModifiers::NONE, KeyCode::Char('o')) = (key.modifiers, key.code) {
        return ModalOutcome::Emit(Action::OpenVaultOpenPicker);
    }
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveVaultPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveVaultPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseVaultPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmVaultPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

pub(super) fn vault_create_form_handle_key(
    focus: VaultCreateFormFocus,
    key: KeyEvent,
) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    let _ = focus;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseVaultCreateForm)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('v')) => ModalOutcome::Emit(Action::PasteSystem),
        (_, KeyCode::Enter) => ModalOutcome::Emit(Action::VaultCreateFormSubmit),
        (_, KeyCode::Tab) | (_, KeyCode::Down) => {
            ModalOutcome::Emit(Action::VaultCreateFormFocusNext)
        }
        (_, KeyCode::BackTab) | (_, KeyCode::Up) => {
            ModalOutcome::Emit(Action::VaultCreateFormFocusPrev)
        }
        (_, KeyCode::Backspace) => ModalOutcome::Emit(Action::VaultCreateFormBackspace),
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            ModalOutcome::Emit(Action::VaultCreateFormChar(c))
        }
        _ => ModalOutcome::Continue,
    }
}

pub(super) fn vault_clone_form_handle_key(
    focus: VaultCloneFormFocus,
    key: KeyEvent,
) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    let _ = focus;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseVaultCloneForm)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('v')) => ModalOutcome::Emit(Action::PasteSystem),
        (_, KeyCode::Enter) => ModalOutcome::Emit(Action::VaultCloneFormSubmit),
        (_, KeyCode::Tab) | (_, KeyCode::Down) => {
            ModalOutcome::Emit(Action::VaultCloneFormFocusNext)
        }
        (_, KeyCode::BackTab) | (_, KeyCode::Up) => {
            ModalOutcome::Emit(Action::VaultCloneFormFocusPrev)
        }
        (_, KeyCode::Backspace) => ModalOutcome::Emit(Action::VaultCloneFormBackspace),
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            ModalOutcome::Emit(Action::VaultCloneFormChar(c))
        }
        _ => ModalOutcome::Continue,
    }
}

/// V3 P3: form key handling. Tab/Shift-Tab/Up/Down cycle focus;
/// Esc/Ctrl-C cancel; Enter submits; typing routes into the focused
/// `LineEdit` (or toggles the driver/readonly fields).
pub(super) fn connection_form_handle_key(
    state: &ConnectionFormState,
    key: KeyEvent,
) -> ModalOutcome {
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
        (_, KeyCode::Char(' ')) if matches!(state.focus, ConnectionFormFocus::Driver) => {
            ModalOutcome::Emit(Action::ConnectionFormCycleDriver(1))
        }
        (_, KeyCode::Left) if matches!(state.focus, ConnectionFormFocus::Driver) => {
            ModalOutcome::Emit(Action::ConnectionFormCycleDriver(-1))
        }
        (_, KeyCode::Right) if matches!(state.focus, ConnectionFormFocus::Driver) => {
            ModalOutcome::Emit(Action::ConnectionFormCycleDriver(1))
        }
        // Readonly toggle (space flips).
        (_, KeyCode::Char(' ')) if matches!(state.focus, ConnectionFormFocus::Readonly) => {
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
pub(super) fn connections_page_handle_key(key: KeyEvent) -> ModalOutcome {
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
        (KeyModifiers::NONE, KeyCode::Char('o')) => {
            return ModalOutcome::Emit(Action::OpenSessionOverrideForm);
        }
        (mods, KeyCode::Char('O')) if !mods.contains(KeyModifiers::CONTROL) => {
            return ModalOutcome::Emit(Action::ClearSessionOverride);
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

/// ConnectionPicker reusa o vocab `list_picker_key`, mas adiciona
/// `Shift+D` para deletar a conexão highlighted (mantém o picker
/// aberto com a lista recarregada).
pub(super) fn connection_picker_handle_key(key: KeyEvent) -> ModalOutcome {
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

pub(super) fn environment_picker_handle_key(key: KeyEvent) -> ModalOutcome {
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

pub(super) fn block_template_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveBlockTemplatePickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveBlockTemplatePickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseBlockTemplatePicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmBlockTemplatePicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

pub(super) fn tab_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveTabPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveTabPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseTabPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmTabPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

pub(super) fn block_history_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveBlockHistoryCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveBlockHistoryCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseBlockHistory),
        ListPickerKey::Confirm | ListPickerKey::Other => ModalOutcome::Continue,
    }
}

pub(super) fn db_export_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveDbExportPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveDbExportPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseDbExportPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmDbExportPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ListPickerKey {
    Up,
    Down,
    Confirm,
    Cancel,
    Other,
}

pub(super) fn list_picker_key(key: KeyEvent) -> ListPickerKey {
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

pub(super) fn help_handle_key(key: KeyEvent) -> ModalOutcome {
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

/// Hover-tooltip dismissal: explicit-close chords swallow the key
/// (Esc / `q` / Ctrl+C), everything else dismisses AND forwards so a
/// motion like `j` closes the popup AND moves the cursor in the
/// same tap — matches IDE tooltip behaviour.
pub(super) fn ref_preview_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Close,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => ModalOutcome::Close,
        (m, KeyCode::Char('q')) if !m.contains(KeyModifiers::CONTROL) => ModalOutcome::Close,
        _ => ModalOutcome::CloseAndForward,
    }
}

// V4 P2-P4 handlers ----------

pub(super) fn envs_page_handle_key(focus: EnvsPaneFocus, key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
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
            // V4 P6: 1-9 ativam env por índice em qualquer foco.
            // Em Vars-focus o número não compete com input de char
            // porque a página em si não captura texto livre — só o
            // VarForm modal (esse usa handler próprio).
            let _ = focus;
            if let Some(idx) = digit_1_9(key) {
                return ModalOutcome::Emit(Action::ActivateEnvByIndex(idx));
            }
            ModalOutcome::Continue
        }
    }
}

pub(super) fn env_form_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseEnvForm)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('v')) => ModalOutcome::Emit(Action::PasteSystem),
        (_, KeyCode::Enter) => ModalOutcome::Emit(Action::EnvFormSubmit),
        (_, KeyCode::Left) => ModalOutcome::Emit(Action::EnvFormCursorLeft),
        (_, KeyCode::Right) => ModalOutcome::Emit(Action::EnvFormCursorRight),
        (_, KeyCode::Home) => ModalOutcome::Emit(Action::EnvFormHome),
        (_, KeyCode::End) => ModalOutcome::Emit(Action::EnvFormEnd),
        (_, KeyCode::Delete) => ModalOutcome::Emit(Action::EnvFormDelete),
        (_, KeyCode::Backspace) => ModalOutcome::Emit(Action::EnvFormBackspace),
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            ModalOutcome::Emit(Action::EnvFormChar(c))
        }
        _ => ModalOutcome::Continue,
    }
}

pub(super) fn var_form_handle_key(focus: VarFormFocus, key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseVarForm)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('v')) => ModalOutcome::Emit(Action::PasteSystem),
        (_, KeyCode::Enter) => ModalOutcome::Emit(Action::VarFormSubmit),
        (_, KeyCode::Tab) | (_, KeyCode::Down) => ModalOutcome::Emit(Action::VarFormFocusNext),
        (_, KeyCode::BackTab) | (_, KeyCode::Up) => ModalOutcome::Emit(Action::VarFormFocusPrev),
        (KeyModifiers::NONE, KeyCode::Char(' ')) if focus == VarFormFocus::Secret => {
            ModalOutcome::Emit(Action::VarFormToggleSecret)
        }
        (_, KeyCode::Left) => ModalOutcome::Emit(Action::VarFormCursorLeft),
        (_, KeyCode::Right) => ModalOutcome::Emit(Action::VarFormCursorRight),
        (_, KeyCode::Home) => ModalOutcome::Emit(Action::VarFormHome),
        (_, KeyCode::End) => ModalOutcome::Emit(Action::VarFormEnd),
        (_, KeyCode::Delete) => ModalOutcome::Emit(Action::VarFormDelete),
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

/// Generic y/n confirm dispatcher. Emits the modal's stored
/// `on_confirm` / `on_cancel` action so each flow's applier picks up
/// its specific side effects (delete a connection, drop a header row, …).
pub(super) fn confirm_prompt_handle_key(
    state: &crate::app::ConfirmPromptState,
    key: KeyEvent,
) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) | (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(state.on_cancel)
        }
        (KeyModifiers::NONE, KeyCode::Char('n')) | (KeyModifiers::NONE, KeyCode::Char('N')) => {
            ModalOutcome::Emit(state.on_cancel)
        }
        (KeyModifiers::NONE, KeyCode::Char('y'))
        | (KeyModifiers::NONE, KeyCode::Char('Y'))
        | (_, KeyCode::Enter) => ModalOutcome::Emit(state.on_confirm),
        _ => ModalOutcome::Continue,
    }
}

/// Dispatch one key into the active prompt. The shared
/// `parse_lineedit_prompt` returns a kind-agnostic `LineEditAction`;
/// this function maps it to the right concrete `Action` based on the
/// prompt's `PromptKind`.
pub(super) fn prompt_handle_key(kind: PromptKind, key: KeyEvent) -> ModalOutcome {
    let action = match kind {
        PromptKind::FenceEditAlias { .. } => crate::input::parser::lineedit::parse_fence_edit(key),
        PromptKind::Cmdline => crate::input::parser::lineedit::parse_cmdline(key),
        PromptKind::Search { .. } => crate::input::parser::lineedit::parse_search(key),
    };
    ModalOutcome::Emit(action)
}
