use crate::app::{
    BlockHistoryState, BlockTemplatePickerState, ConnectionDeleteConfirmState,
    ConnectionFormState, ConnectionPickerState, ConnectionsPageState, DbConfirmRunState,
    DbExportPickerState, DbRowDetailState, EnvCloneFormState, EnvDeleteConfirmState, EnvFormState,
    EnvironmentPickerState, EnvsPageState, EnvsPaneFocus, HttpResponseDetailState, TabPickerState,
    VarDeleteConfirmState, VarFormFocus, VarFormState, VaultCloneFormFocus, VaultCloneFormState,
    VaultCreateFormFocus, VaultCreateFormState, VaultMissingSecretsState, VaultOpenPickerState,
    VaultPickerState,
};
use crate::config::EditorMode;
use crate::vim::state::VimState;

mod clone_form;
/// Detail-modal handlers (`DbRowDetail`, `HttpResponseDetail`). They
/// route keys through the vim engine over a read-only sub-`Document`,
/// so they live apart from the simple per-variant dispatch table.
mod detail;
mod util;

use util::digit_1_9;
use crate::input::action::Action;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Cross-cutting context handed to [`Modal::handle_key_with_ctx`]. Only
/// the detail variants (`DbRowDetail`, `HttpResponseDetail`) consult it
/// today — they need `&mut VimState` to drive the read-only vim engine
/// over their sub-`Document`, and `editor_mode` to decide between
/// owning the key (vim profile in detail mode) or forwarding to the
/// editor (standard profile, or a transient vim mode like Visual that
/// must reach `parse_visual`).
pub struct ModalKeyCtx<'a> {
    pub vim: &'a mut VimState,
    pub editor_mode: EditorMode,
}

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
    /// lista os vaults registrados no SQLite app
    /// registry. Confirm chama `App::switch_vault` (in-place swap).
    /// Aberto por Alt+W (configurável via keymap.toml).
    VaultPicker(VaultPickerState),
    /// form de criação de vault. Aberto por `n` dentro
    /// do VaultPicker. Submit faz mkdir + git init + scaffold +
    /// switch_vault (in-place).
    VaultCreateForm(VaultCreateFormState),
    /// form de clone. Aberto por `c` dentro do
    /// VaultPicker. Submit faz git clone + switch_vault.
    VaultCloneForm(VaultCloneFormState),
    /// navegador de diretório. Aberto por `o` dentro
    /// do VaultPicker. Enter num dir desce; Enter num vault ativa
    /// (switch_vault); Backspace sobe um nível; Esc fecha.
    VaultOpenPicker(VaultOpenPickerState),
    /// first-run secrets modal. Aberto automaticamente
    /// após switch_vault quando scan_missing_secrets retorna refs
    /// sem entrada no keychain local. Tab/jk navega, type edita
    /// value, Enter salva, `s` skip, Esc fecha.
    VaultMissingSecrets(VaultMissingSecretsState),
    /// DB row-detail modal. `<CR>` on a result row opens it with a
    /// sub-`Document` carrying the row's columns + values; the modal
    /// hosts the full vim motion engine over that doc (read-only).
    /// `Ctrl-c` closes; `Y` copies row as JSON. Visual mode INSIDE
    /// the modal is allowed — the renderer paints while state is
    /// `Some`, regardless of `vim.mode`.
    DbRowDetail(DbRowDetailState),
    /// HTTP response-detail modal. Mirrors `DbRowDetail`: status +
    /// headers + body in a sub-`Document` driven by the vim motion
    /// engine (read-only). `Ctrl-c` closes; `Y` copies the raw body.
    HttpResponseDetail(HttpResponseDetailState),
}

#[derive(Debug)]
pub enum ModalOutcome {
    Continue,
    Close,
    Emit(Action),
    /// Modal doesn't own this key — let the scope walker pass it to
    /// the editor below. Used by detail modals (`DbRowDetail` /
    /// `HttpResponseDetail`) to delegate transient vim modes (Visual,
    /// Search) and the entire standard profile to the editor scope,
    /// which then operates on `app.document_mut()` (redirected to the
    /// modal's sub-doc by [`crate::app::App::document_mut`]).
    Forward,
}

impl Modal {
    /// Context-free dispatch. Kept for tests and for the simple
    /// modals (most variants) that don't need `&mut VimState` or
    /// the editor profile. Detail variants return `Continue` here —
    /// production code calls [`Modal::handle_key_with_ctx`] from the
    /// scope walker, which threads the missing context.
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
            Modal::VaultPicker(_) => vault_picker_handle_key(key),
            Modal::VaultCreateForm(s) => vault_create_form_handle_key(s.focus, key),
            Modal::VaultCloneForm(s) => vault_clone_form_handle_key(s.focus, key),
            Modal::VaultOpenPicker(_) => vault_open_picker_handle_key(key),
            Modal::VaultMissingSecrets(s) => vault_missing_secrets_handle_key(s.editing, key),
            Modal::DbRowDetail(_) | Modal::HttpResponseDetail(_) => ModalOutcome::Continue,
        }
    }

    /// Context-aware dispatch used by the scope walker. Detail
    /// variants consume `ctx` to drive the read-only vim engine or
    /// forward to the editor; every other variant ignores it and
    /// delegates to [`Modal::handle_key`].
    pub fn handle_key_with_ctx(
        &mut self,
        key: KeyEvent,
        ctx: &mut ModalKeyCtx<'_>,
    ) -> ModalOutcome {
        match self {
            Modal::DbRowDetail(_) => detail::db_row_handle_key(key, ctx),
            Modal::HttpResponseDetail(_) => detail::http_response_handle_key(key, ctx),
            _ => self.handle_key(key),
        }
    }

    /// Borrow the active `DbRowDetail` state if that's the current
    /// modal. Used by the renderer + accessors that need the sub-doc.
    pub fn as_db_row_detail(&self) -> Option<&DbRowDetailState> {
        match self {
            Modal::DbRowDetail(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_db_row_detail_mut(&mut self) -> Option<&mut DbRowDetailState> {
        match self {
            Modal::DbRowDetail(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_http_response_detail(&self) -> Option<&HttpResponseDetailState> {
        match self {
            Modal::HttpResponseDetail(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_http_response_detail_mut(&mut self) -> Option<&mut HttpResponseDetailState> {
        match self {
            Modal::HttpResponseDetail(s) => Some(s),
            _ => None,
        }
    }
}

fn vault_missing_secrets_handle_key(editing: bool, key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
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

fn vault_open_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
    if let (_, KeyCode::Backspace) = (modifiers, code) {
        return ModalOutcome::Emit(Action::VaultOpenPickerUp);
    }
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveVaultOpenPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveVaultOpenPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseVaultOpenPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::VaultOpenPickerEnter),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

fn vault_picker_handle_key(key: KeyEvent) -> ModalOutcome {
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

fn vault_create_form_handle_key(focus: VaultCreateFormFocus, key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
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

fn vault_clone_form_handle_key(focus: VaultCloneFormFocus, key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
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

fn env_form_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
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

fn var_form_handle_key(focus: VarFormFocus, key: KeyEvent) -> ModalOutcome {
    let KeyEvent { code, modifiers, .. } = key;
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
}
