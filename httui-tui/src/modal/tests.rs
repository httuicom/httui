// size:exclude file — modal handler test suite (cluster of small unit tests).

use super::handlers::*;
use super::*;
use crate::app::{EnvsPaneFocus, VarFormFocus, VaultCloneFormFocus, VaultCreateFormFocus};
use crossterm::event::{KeyCode, KeyModifiers};

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
fn envs_page_vars_focus_digit_also_emits_activate() {
    // V4 P6 refinamento: 1-9 ativam env por índice em qualquer
    // foco (UX: trocar env rápido sem precisar Tab pra Envs).
    let mut m = empty_envs_page(EnvsPaneFocus::Vars);
    match m.handle_key(k(KeyCode::Char('3'), KeyModifiers::NONE)) {
        ModalOutcome::Emit(Action::ActivateEnvByIndex(3)) => {}
        other => panic!("expected ActivateEnvByIndex(3), got {other:?}"),
    }
}

fn empty_envs_page(focus: EnvsPaneFocus) -> Modal {
    Modal::EnvsPage(EnvsPageState {
        envs: Vec::new(),
        active: None,
        selected_env: 0,
        vars: Vec::new(),
        selected_var: 0,
        focus,
        var_uses: Vec::new(),
    })
}

fn vault_picker(entries: Vec<&str>) -> Modal {
    Modal::VaultPicker(VaultPickerState {
        entries: entries.into_iter().map(String::from).collect(),
        selected: 0,
        active: None,
    })
}

#[test]
fn vault_picker_jk_arrows_navigate() {
    let mut m = vault_picker(vec!["/a", "/b"]);
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('j'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::MoveVaultPickerCursor(1))
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Down, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::MoveVaultPickerCursor(1))
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('k'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::MoveVaultPickerCursor(-1))
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Up, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::MoveVaultPickerCursor(-1))
    ));
}

#[test]
fn vault_picker_enter_confirms_esc_closes() {
    let mut m = vault_picker(vec!["/a"]);
    assert!(matches!(
        m.handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::ConfirmVaultPicker)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CloseVaultPicker)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        ModalOutcome::Emit(Action::CloseVaultPicker)
    ));
}

#[test]
fn vault_picker_n_opens_create_form() {
    // composição "vault" + verbo. `n` dentro do picker
    // dispara o form de criação. Mesmo padrão de ConnectionsPage.
    let mut m = vault_picker(vec!["/a"]);
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('n'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenVaultCreateForm)
    ));
}

fn empty_vault_create_form() -> Modal {
    Modal::VaultCreateForm(VaultCreateFormState::default())
}

#[test]
fn vault_picker_o_opens_open_picker() {
    let mut m = vault_picker(vec!["/a"]);
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('o'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenVaultOpenPicker)
    ));
}

fn vault_missing_secrets(editing: bool) -> Modal {
    Modal::VaultMissingSecrets(VaultMissingSecretsState {
        items: Vec::new(),
        selected: 0,
        editing,
    })
}

#[test]
fn vault_missing_secrets_browse_routes_navigation() {
    let mut m = vault_missing_secrets(false);
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('j'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::MoveVaultMissingSecretsCursor(1))
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultMissingSecretsEnterEdit)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('s'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultMissingSecretsSkip)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CloseVaultMissingSecrets)
    ));
}

#[test]
fn vault_missing_secrets_editing_routes_typing_and_save() {
    let mut m = vault_missing_secrets(true);
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('a'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultMissingSecretsChar('a'))
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Backspace, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultMissingSecretsBackspace)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultMissingSecretsSave)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultMissingSecretsCancelEdit)
    ));
}

fn vault_open_picker() -> Modal {
    Modal::VaultOpenPicker(VaultOpenPickerState {
        cwd: std::path::PathBuf::from("/tmp"),
        entries: Vec::new(),
        selected: 0,
    })
}

#[test]
fn vault_open_picker_routes_navigation() {
    let mut m = vault_open_picker();
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('j'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::MoveVaultOpenPickerCursor(1))
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Backspace, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultOpenPickerUp)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultOpenPickerEnter)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CloseVaultOpenPicker)
    ));
}

#[test]
fn vault_open_picker_o_emits_open_as_vault() {
    // V6 audit fix — `o`/`O` must be distinct from Enter so a
    // vault-as-parent doesn't trap the user inside it.
    let mut m = vault_open_picker();
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('o'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultOpenPickerOpenAsVault)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('O'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultOpenPickerOpenAsVault)
    ));
}

#[test]
fn vault_picker_c_opens_clone_form() {
    // composição "vault" + verbo. `c` dispara o
    // form de clone, complementando `n` (Create).
    let mut m = vault_picker(vec!["/a"]);
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('c'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenVaultCloneForm)
    ));
}

fn empty_vault_clone_form() -> Modal {
    Modal::VaultCloneForm(VaultCloneFormState::default())
}

#[test]
fn vault_clone_form_routes_typing_and_navigation() {
    let mut m = empty_vault_clone_form();
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('a'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultCloneFormChar('a'))
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Backspace, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultCloneFormBackspace)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Tab, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultCloneFormFocusNext)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultCloneFormSubmit)
    ));
    assert!(matches!(
        m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CloseVaultCloneForm)
    ));
}

#[test]
fn vault_create_form_routes_typing_and_navigation() {
    let mut m = empty_vault_create_form();
    // Char insertion.
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('a'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultCreateFormChar('a'))
    ));
    // Backspace.
    assert!(matches!(
        m.handle_key(k(KeyCode::Backspace, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultCreateFormBackspace)
    ));
    // Tab cycles focus forward.
    assert!(matches!(
        m.handle_key(k(KeyCode::Tab, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultCreateFormFocusNext)
    ));
    // Enter submits.
    assert!(matches!(
        m.handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VaultCreateFormSubmit)
    ));
    // Esc cancels.
    assert!(matches!(
        m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CloseVaultCreateForm)
    ));
}

// ───────────── tui-V10: coverage gaps in adjacent handlers
// (connection delete confirm, connections page, db confirm run). ─────

#[test]
fn connection_delete_confirm_routes_y_n_and_cancel() {
    let mut m = Modal::ConnectionDeleteConfirm(ConnectionDeleteConfirmState {
        name: String::new(),
    });
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('y'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::ConfirmConnectionDelete)
    ));
    let mut m = Modal::ConnectionDeleteConfirm(ConnectionDeleteConfirmState {
        name: String::new(),
    });
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('n'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CancelConnectionDelete)
    ));
    let mut m = Modal::ConnectionDeleteConfirm(ConnectionDeleteConfirmState {
        name: String::new(),
    });
    assert!(matches!(
        m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CancelConnectionDelete)
    ));
    let mut m = Modal::ConnectionDeleteConfirm(ConnectionDeleteConfirmState {
        name: String::new(),
    });
    assert!(matches!(
        m.handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::ConfirmConnectionDelete)
    ));
}

fn connections_page_modal() -> Modal {
    Modal::Connections(ConnectionsPageState::default())
}

#[test]
fn connections_page_routes_action_chords() {
    let mut m = connections_page_modal();
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('n'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenConnectionForm)
    ));
    let mut m = connections_page_modal();
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('e'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenConnectionEditForm)
    ));
    let mut m = connections_page_modal();
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('t'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::TestSelectedConnection)
    ));
    let mut m = connections_page_modal();
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('D'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenConnectionDeleteConfirm)
    ));
    let mut m = connections_page_modal();
    assert!(matches!(
        m.handle_key(k(KeyCode::Down, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::MoveConnectionsPageCursor(1))
    ));
    let mut m = connections_page_modal();
    assert!(matches!(
        m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CloseConnectionsPage)
    ));
}

#[test]
fn envs_page_routes_navigation_and_action_chords() {
    let envs_focus = || EnvsPaneFocus::Envs;
    let vars_focus = || EnvsPaneFocus::Vars;
    // Tab toggles focus.
    assert!(matches!(
        envs_page_handle_key(envs_focus(), k(KeyCode::Tab, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::EnvsPageFocusToggle)
    ));
    // h/l switch panes.
    assert!(matches!(
        envs_page_handle_key(vars_focus(), k(KeyCode::Char('h'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::EnvsPageFocusEnvs)
    ));
    assert!(matches!(
        envs_page_handle_key(envs_focus(), k(KeyCode::Char('l'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::EnvsPageFocusVars)
    ));
    // j/k move within the focused pane.
    assert!(matches!(
        envs_page_handle_key(envs_focus(), k(KeyCode::Char('j'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::EnvsPageMoveEnvCursor(1))
    ));
    assert!(matches!(
        envs_page_handle_key(vars_focus(), k(KeyCode::Char('k'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::EnvsPageMoveVarCursor(-1))
    ));
    // Per-pane n/e/D/c verbs.
    assert!(matches!(
        envs_page_handle_key(envs_focus(), k(KeyCode::Char('n'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenEnvForm)
    ));
    assert!(matches!(
        envs_page_handle_key(vars_focus(), k(KeyCode::Char('n'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenVarForm)
    ));
    assert!(matches!(
        envs_page_handle_key(envs_focus(), k(KeyCode::Char('e'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenEnvEditForm)
    ));
    assert!(matches!(
        envs_page_handle_key(envs_focus(), k(KeyCode::Char('D'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenEnvDeleteConfirm)
    ));
    assert!(matches!(
        envs_page_handle_key(envs_focus(), k(KeyCode::Char('c'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::OpenEnvCloneForm)
    ));
    assert!(matches!(
        envs_page_handle_key(envs_focus(), k(KeyCode::Char('a'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::EnvsPageActivateEnv)
    ));
    // Esc closes.
    assert!(matches!(
        envs_page_handle_key(envs_focus(), k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CloseEnvsPage)
    ));
    // 1-9 activate env by index (works in either focus).
    match envs_page_handle_key(envs_focus(), k(KeyCode::Char('3'), KeyModifiers::NONE)) {
        ModalOutcome::Emit(Action::ActivateEnvByIndex(3)) => {}
        other => panic!("expected ActivateEnvByIndex(3), got {other:?}"),
    }
}

#[test]
fn env_form_routes_typing_save_and_cancel() {
    assert!(matches!(
        env_form_handle_key(k(KeyCode::Char('a'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::EnvFormChar('a'))
    ));
    assert!(matches!(
        env_form_handle_key(k(KeyCode::Backspace, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::EnvFormBackspace)
    ));
    assert!(matches!(
        env_form_handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::EnvFormSubmit)
    ));
    assert!(matches!(
        env_form_handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CloseEnvForm)
    ));
    assert!(matches!(
        env_form_handle_key(k(KeyCode::Char('v'), KeyModifiers::CONTROL)),
        ModalOutcome::Emit(Action::PasteSystem)
    ));
}

#[test]
fn var_form_routes_typing_focus_and_secret_toggle() {
    assert!(matches!(
        var_form_handle_key(VarFormFocus::Key, k(KeyCode::Char('x'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VarFormChar('x'))
    ));
    assert!(matches!(
        var_form_handle_key(VarFormFocus::Value, k(KeyCode::Tab, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VarFormFocusNext)
    ));
    assert!(matches!(
        var_form_handle_key(VarFormFocus::Key, k(KeyCode::BackTab, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VarFormFocusPrev)
    ));
    assert!(matches!(
        var_form_handle_key(VarFormFocus::Secret, k(KeyCode::Char(' '), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VarFormToggleSecret)
    ));
    // Char on Secret focus is inert.
    assert!(matches!(
        var_form_handle_key(VarFormFocus::Secret, k(KeyCode::Char('z'), KeyModifiers::NONE)),
        ModalOutcome::Continue
    ));
    assert!(matches!(
        var_form_handle_key(VarFormFocus::Key, k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::VarFormSubmit)
    ));
    assert!(matches!(
        var_form_handle_key(VarFormFocus::Key, k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CloseVarForm)
    ));
}

#[test]
fn env_or_var_confirm_routes_y_n_enter_esc() {
    assert!(matches!(
        env_or_var_confirm_handle_key(k(KeyCode::Char('y'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::ConfirmEnvOrVarDelete)
    ));
    assert!(matches!(
        env_or_var_confirm_handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::ConfirmEnvOrVarDelete)
    ));
    assert!(matches!(
        env_or_var_confirm_handle_key(k(KeyCode::Char('n'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CancelEnvOrVarDelete)
    ));
    assert!(matches!(
        env_or_var_confirm_handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CancelEnvOrVarDelete)
    ));
}

#[test]
fn db_confirm_run_routes_y_n_and_enter() {
    let mut m = Modal::DbConfirmRun(DbConfirmRunState {
        segment_idx: 0,
        reason: String::new(),
    });
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('y'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::ConfirmDbRun)
    ));
    let mut m = Modal::DbConfirmRun(DbConfirmRunState {
        segment_idx: 0,
        reason: String::new(),
    });
    assert!(matches!(
        m.handle_key(k(KeyCode::Enter, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::ConfirmDbRun)
    ));
    let mut m = Modal::DbConfirmRun(DbConfirmRunState {
        segment_idx: 0,
        reason: String::new(),
    });
    assert!(matches!(
        m.handle_key(k(KeyCode::Char('n'), KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CancelDbRun)
    ));
    let mut m = Modal::DbConfirmRun(DbConfirmRunState {
        segment_idx: 0,
        reason: String::new(),
    });
    assert!(matches!(
        m.handle_key(k(KeyCode::Esc, KeyModifiers::NONE)),
        ModalOutcome::Emit(Action::CancelDbRun)
    ));
}

#[test]
fn accessors_return_none_when_modal_kind_mismatches() {
    let mut m = Modal::Help;
    assert!(m.as_db_row_detail().is_none());
    assert!(m.as_db_row_detail_mut().is_none());
    assert!(m.as_http_response_detail().is_none());
    assert!(m.as_http_response_detail_mut().is_none());
    assert!(m.as_prompt().is_none());
    assert!(m.as_prompt_mut().is_none());
    assert!(m.as_content_search().is_none());
    assert!(m.as_content_search_mut().is_none());
    assert!(m.as_quickopen().is_none());
    assert!(m.as_quickopen_mut().is_none());
    assert!(m.as_completion_popup().is_none());
    assert!(m.as_completion_popup_mut().is_none());
    assert!(m.as_db_settings().is_none());
    assert!(m.as_db_settings_mut().is_none());
}

#[test]
fn as_prompt_returns_some_when_prompt_active() {
    let mut m = Modal::Prompt(PromptKind::Cmdline, crate::vim::lineedit::LineEdit::new());
    assert!(m.as_prompt().is_some());
    assert!(m.as_prompt_mut().is_some());
}

#[test]
fn as_quickopen_returns_some_when_quickopen_active() {
    let mut m = Modal::QuickOpen(crate::vim::quickopen::QuickOpen::default());
    assert!(m.as_quickopen().is_some());
    assert!(m.as_quickopen_mut().is_some());
}
