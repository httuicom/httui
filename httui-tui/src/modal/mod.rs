use crate::app::{
    BlockHistoryState, BlockTemplatePickerState, BlocksUnsavedPromptState, CompletionPopupState,
    ConfirmPromptState, ConnectionFormState, ConnectionPickerState, ConnectionsPageState,
    ContentSearchState, DbExportPickerState, DbRowDetailState, DbSettingsState, EnvCloneFormState,
    EnvFormState, EnvironmentPickerState, EnvsPageState, HttpResponseDetailState,
    SettingsPageState, TabPickerState, VarFormState, VaultCloneFormState, VaultCreateFormState,
    VaultMissingSecretsState, VaultOpenPickerState, VaultPickerState,
};
use crate::config::EditorMode;
use crate::vim::state::VimState;

mod clone_form;
/// Detail-modal handlers (`DbRowDetail`, `HttpResponseDetail`). They
/// route keys through the vim engine over a read-only sub-`Document`,
/// so they live apart from the simple per-variant dispatch table.
mod detail;
/// Git-specific modal handlers — split out of `handlers.rs` to keep
/// that file under the size gate.
mod git;
mod handlers;
/// Settings page handler — kept here (instead of inline in
/// `handlers.rs`) so each surface owns its own key vocabulary.
mod settings_handler;
mod util;

use crate::input::action::Action;
use crossterm::event::KeyEvent;
use handlers::*;
use settings_handler::settings_page_handle_key;

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
    /// Vars + Envs page.
    EnvsPage(EnvsPageState),
    /// Create / edit env form.
    EnvForm(EnvFormState),
    /// Create / edit var form.
    VarForm(VarFormState),
    /// Clone-env form with checkboxes per var. Opened from EnvsPage
    /// (`c`, focus = Envs); creates target env + bulk set_var of the
    /// ticked vars only (default: all ON).
    EnvCloneForm(EnvCloneFormState),
    /// Lists the vaults registered in the SQLite app registry. Confirm
    /// calls `App::switch_vault` (in-place swap). Opened by Alt+W.
    VaultPicker(VaultPickerState),
    /// Vault create form. Opened by `n` inside the VaultPicker.
    /// Submit runs mkdir + git init + scaffold + switch_vault.
    VaultCreateForm(VaultCreateFormState),
    /// Vault clone form. Opened by `c` inside the VaultPicker.
    /// Submit runs git clone + switch_vault.
    VaultCloneForm(VaultCloneFormState),
    /// Directory browser. Opened by `o` inside the VaultPicker.
    /// Enter on a dir descends; on a vault → switch_vault; Backspace
    /// goes up; Esc closes.
    VaultOpenPicker(VaultOpenPickerState),
    /// First-run secrets modal. Opens automatically after
    /// switch_vault when `scan_missing_secrets` returns refs without
    /// a keychain entry. Tab/jk navigates, typing edits the value,
    /// Enter saves, `s` skips, Esc closes.
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
    /// Single-line text prompt. Variants of [`PromptKind`] discriminate
    /// the open-source (inline fence-edit, ex-command, search) so the
    /// shared `LineEdit` field carries the buffer + caret while the
    /// kind tells `apply_action` where to commit on Enter.
    Prompt(PromptKind, crate::vim::lineedit::LineEdit),
    /// Full-text search panel (`<C-f>`). Carries the in-flight query
    /// plus the live result list + selection. Per-keystroke re-query
    /// happens in `commands::search`; this modal owns only the I/O.
    ContentSearch(ContentSearchState),
    /// `Ctrl+P` quick-open modal — fuzzy file picker over `.md` files
    /// in the vault. Owns query buffer + filtered candidate list +
    /// selection cursor.
    QuickOpen(crate::vim::quickopen::QuickOpen),
    /// SQL completion overlay — opens while the cursor is inside a DB
    /// block body during insert mode. Returns
    /// [`ModalOutcome::Forward`] for any key it doesn't bind so the
    /// editor below keeps typing into the doc; the post-action hook
    /// refreshes the popup against the new prefix.
    CompletionPopup(CompletionPopupState),
    /// DB block settings popup (`gs`). Tab / arrows cycle the focused
    /// LineEdit (`row_limit` / `timeout_ms`); typing routes into the
    /// focused field.
    DbSettings(DbSettingsState),
    /// Set-upstream confirm modal — opens when `Ctrl+Enter` (Sync) in
    /// the git panel hits a branch with no upstream. `y` / `Enter`
    /// runs `git push -u <remote> <branch>`; `n` / `Esc` cancels.
    GitSetUpstreamConfirm(crate::git::GitSetUpstreamConfirmState),
    /// Branch picker — opens with `Ctrl+B` while the git panel is
    /// focused. `j`/`k`/Up/Down navigate; Enter checks out the
    /// highlighted branch; Esc closes.
    GitBranchPicker(crate::git::GitBranchPickerState),
    /// Full-screen git log page — list of commits on the left, diff
    /// for the selected one on the right. Opened by `Ctrl+L` from
    /// inside the git panel.
    GitLogPage(crate::git::GitLogPageState),
    /// 3-way conflict resolver — opens when `Ctrl+R` from the panel
    /// finds unmerged files. List of conflicted paths on the left;
    /// three columns (base / ours / theirs) on the right. `1`/`2`/`3`
    /// pick the corresponding version, write it to disk, stage it,
    /// and drop the entry from the list.
    GitConflictResolver(crate::git::GitConflictResolverState),
    /// Fullscreen Settings page (Keymaps / Theme / Editor). Backed
    /// by `Config`; every mutation persists through
    /// `crate::config::save_config`. Opened by `Alt+,` when the
    /// cursor isn't on a DB/HTTP block (DB-block case routes to
    /// `DbSettings` instead — see [`Action::OpenSettings`]).
    Settings(SettingsPageState),
    /// "Unsaved blocks" guard — pops when the user toggles
    /// `Alt+M` (or attempts another view-switching action) with at
    /// least one pane carrying a `BlockDraft`. Save commits every
    /// dirty pane + replays the deferred toggle; Discard drops them.
    BlocksUnsavedPrompt(BlocksUnsavedPromptState),
    /// Generic y/n confirm. Owns its title + body + the actions to emit
    /// for confirm/cancel — see [`ConfirmPromptState`]. The per-flow
    /// data lives in `ConfirmPayload` so action appliers extract it.
    ConfirmPrompt(ConfirmPromptState),
    /// `K` (vim NORMAL) / `Alt+K` (standard): hover-preview of the
    /// `{{ref}}` under the cursor — shows the resolved value plus
    /// where it came from (env var / block alias). Esc / q / Ctrl-C
    /// close. Read-only, so no key gets forwarded back to the editor.
    RefPreview(crate::ref_preview::RefPreviewState),
}

/// Tag for the open [`Modal::Prompt`]. Carries the per-kind context
/// (e.g. which block a fence-edit targets, which direction a search
/// runs) that survives until the prompt confirms or cancels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    /// `<C-a>` on a block → edit its alias used in `{{alias.path}}`
    /// refs and shown in the block title.
    FenceEditAlias { segment_idx: usize },
    /// `:` ex-command prompt. On Enter the buffer is fed through
    /// `vim::ex::run`; status bar paints `:<buf>`.
    Cmdline,
    /// `/` (forward) or `?` (backward) search prompt. `forward`
    /// selects the prompt sigil and the executed direction.
    Search { forward: bool },
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
    /// Close the modal AND let the same keystroke reach the editor
    /// scope underneath. Used by transient hover popups (RefPreview)
    /// so motion keys like `hjkl` dismiss the popup and move the
    /// cursor in a single tap — same UX as IDE tooltips.
    CloseAndForward,
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
            Modal::BlockHistory(_) => block_history_handle_key(key),
            Modal::DbExportPicker(_) => db_export_picker_handle_key(key),
            Modal::TabPicker(_) => tab_picker_handle_key(key),
            Modal::BlockTemplatePicker(_) => block_template_picker_handle_key(key),
            Modal::EnvironmentPicker(_) => environment_picker_handle_key(key),
            Modal::ConnectionPicker(_) => connection_picker_handle_key(key),
            Modal::Connections(_) => connections_page_handle_key(key),
            Modal::ConnectionForm(s) => connection_form_handle_key(s, key),
            Modal::EnvsPage(s) => envs_page_handle_key(s.focus, key),
            Modal::EnvForm(_) => env_form_handle_key(key),
            Modal::VarForm(s) => var_form_handle_key(s.focus, key),
            Modal::EnvCloneForm(s) => clone_form::env_clone_form_handle_key(s.focus, key),
            Modal::VaultPicker(_) => vault_picker_handle_key(key),
            Modal::VaultCreateForm(s) => vault_create_form_handle_key(s.focus, key),
            Modal::VaultCloneForm(s) => vault_clone_form_handle_key(s.focus, key),
            Modal::VaultOpenPicker(_) => vault_open_picker_handle_key(key),
            Modal::VaultMissingSecrets(s) => vault_missing_secrets_handle_key(s.editing, key),
            Modal::DbRowDetail(_) | Modal::HttpResponseDetail(_) => ModalOutcome::Continue,
            Modal::Prompt(kind, _) => prompt_handle_key(*kind, key),
            Modal::ContentSearch(_) => {
                ModalOutcome::Emit(crate::input::parser::modals::parse_content_search(key))
            }
            Modal::QuickOpen(_) => {
                ModalOutcome::Emit(crate::input::parser::lineedit::parse_quickopen(key))
            }
            Modal::CompletionPopup(_) => {
                match crate::input::apply::completion::parse_completion_popup_key(key) {
                    Some(action) => ModalOutcome::Emit(action),
                    None => ModalOutcome::Forward,
                }
            }
            Modal::DbSettings(_) => {
                ModalOutcome::Emit(crate::input::parser::modals::parse_db_settings_modal(key))
            }
            Modal::GitSetUpstreamConfirm(_) => git::set_upstream_confirm_handle_key(key),
            Modal::GitBranchPicker(_) => git::branch_picker_handle_key(key),
            Modal::GitLogPage(_) => git::log_page_handle_key(key),
            Modal::GitConflictResolver(_) => git::conflict_resolver_handle_key(key),
            Modal::Settings(s) => settings_page_handle_key(s.capture.is_some(), key),
            Modal::BlocksUnsavedPrompt(s) => blocks_unsaved_prompt_handle_key(s, key),
            Modal::ConfirmPrompt(s) => confirm_prompt_handle_key(s, key),
            Modal::RefPreview(_) => ref_preview_handle_key(key),
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

    pub fn as_prompt(&self) -> Option<(PromptKind, &crate::vim::lineedit::LineEdit)> {
        match self {
            Modal::Prompt(kind, le) => Some((*kind, le)),
            _ => None,
        }
    }

    pub fn as_prompt_mut(&mut self) -> Option<(PromptKind, &mut crate::vim::lineedit::LineEdit)> {
        match self {
            Modal::Prompt(kind, le) => Some((*kind, le)),
            _ => None,
        }
    }

    pub fn as_content_search(&self) -> Option<&ContentSearchState> {
        match self {
            Modal::ContentSearch(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_content_search_mut(&mut self) -> Option<&mut ContentSearchState> {
        match self {
            Modal::ContentSearch(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_quickopen(&self) -> Option<&crate::vim::quickopen::QuickOpen> {
        match self {
            Modal::QuickOpen(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_quickopen_mut(&mut self) -> Option<&mut crate::vim::quickopen::QuickOpen> {
        match self {
            Modal::QuickOpen(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_completion_popup(&self) -> Option<&CompletionPopupState> {
        match self {
            Modal::CompletionPopup(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_completion_popup_mut(&mut self) -> Option<&mut CompletionPopupState> {
        match self {
            Modal::CompletionPopup(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_db_settings(&self) -> Option<&DbSettingsState> {
        match self {
            Modal::DbSettings(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_db_settings_mut(&mut self) -> Option<&mut DbSettingsState> {
        match self {
            Modal::DbSettings(s) => Some(s),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
