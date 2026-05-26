// coverage:exclude file — vault sub-modal applier cluster relocated by
// tui-V10 (split of pickers.rs to satisfy size gate); coverage tracked
// in docs-llm/tui-v2/vim-coverage-debt.md.
//! Vault picker + Create/Clone/Open/MissingSecrets sub-modal
//! handlers. Mechanically split out of `pickers.rs` (tui-V10) to keep
//! that file under the 600-line size gate. No behavior change.

use crate::app::{App, StatusKind};
use crate::vim::mode::Mode;

use crate::vault::helpers::read_dir_entries;

// ───────────── vault sub-modal back-stack helper ─────────────

/// Dismiss the current sub-modal opened from the vault picker.
/// When `app.resume_vault_picker` is set (chord-driven flow), reopen
/// the picker so the user lands back at the menu instead of the
/// editor. When unset (auto-open paths like the first-run secrets
/// modal at startup), just close the modal.
fn dismiss_sub_modal(app: &mut App) {
    app.modal = None;
    if app.resume_vault_picker {
        app.resume_vault_picker = false;
        let _ = open_vault_picker(app);
        if app.modal.is_none() {
            app.vim.enter_normal();
        }
    } else {
        app.vim.enter_normal();
    }
}

// ───────────── vault picker (Alt+W) ─────────────

/// Open the vault picker. Reads every path registered via
/// `httui_core::vaults::list_vaults` and marks the active one with
/// `active`. Returns an error if the registry is empty (the
/// empty-state handles first-run; the picker is a tool for users who
/// already have at least one vault).
pub(crate) fn open_vault_picker(app: &mut App) -> Result<(), String> {
    let pool = app.pool_manager.app_pool().clone();
    let (entries, active) = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let vs = httui_core::vaults::list_vaults(&pool)
                .await
                .map_err(|e| format!("list vaults: {e}"))?;
            let active = httui_core::vaults::get_active_vault(&pool)
                .await
                .ok()
                .flatten();
            Ok::<_, String>((vs, active))
        })
    })?;
    if entries.is_empty() {
        return Err("no vaults registered yet".into());
    }
    let selected = active
        .as_deref()
        .and_then(|a| entries.iter().position(|v| v == a))
        .unwrap_or(0);
    app.modal = Some(crate::modal::Modal::VaultPicker(
        crate::app::VaultPickerState {
            entries,
            selected,
            active,
        },
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub(crate) fn apply_close_vault_picker(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::VaultPicker(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub(crate) fn apply_move_vault_picker_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::VaultPicker(state)) = app.modal.as_mut() else {
        return;
    };
    if state.entries.is_empty() {
        return;
    }
    let last = state.entries.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

/// `Enter` in the vault picker — call `App::switch_vault` for the
/// highlighted path. No-op (just close) when the highlighted entry is
/// already active.
pub(crate) fn apply_confirm_vault_picker(app: &mut App) {
    let state = match app.modal.take() {
        Some(crate::modal::Modal::VaultPicker(s)) => s,
        other => {
            app.modal = other;
            app.vim.enter_normal();
            return;
        }
    };
    app.vim.enter_normal();
    let Some(target) = state.entries.get(state.selected).cloned() else {
        return;
    };
    if state.active.as_deref() == Some(target.as_str()) {
        app.set_status(StatusKind::Info, format!("already on vault {target}"));
        return;
    }
    match app.switch_vault(std::path::PathBuf::from(&target)) {
        Ok(()) => app.set_status(StatusKind::Info, format!("vault → {target}")),
        Err(e) => app.set_status(StatusKind::Error, format!("switch vault: {e}")),
    }
}

// ───────────── vault create form ─────────────

/// Open the Create form. Default parent is `$HOME` so the user just
/// needs to pick a name; can be edited if they want a different root.
pub(crate) fn open_vault_create_form(app: &mut App) {
    use crate::vim::lineedit::LineEdit;
    let default_parent = std::env::var("HOME")
        .ok()
        .unwrap_or_else(|| ".".to_string());
    app.resume_vault_picker = true;
    app.modal = Some(crate::modal::Modal::VaultCreateForm(
        crate::app::VaultCreateFormState {
            parent: LineEdit::from_str(default_parent),
            name: LineEdit::new(),
            focus: crate::app::VaultCreateFormFocus::Name,
            error: None,
        },
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
}

pub(crate) fn apply_close_vault_create_form(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::VaultCreateForm(_))) {
        dismiss_sub_modal(app);
    }
}

pub(crate) fn with_vault_create_form(
    app: &mut App,
    f: impl FnOnce(&mut crate::app::VaultCreateFormState),
) {
    if let Some(crate::modal::Modal::VaultCreateForm(s)) = app.modal.as_mut() {
        f(s);
    }
}

/// Validate + scaffold + switch. Errors stay inline on the form so
/// the user can fix and resubmit. Success closes the modal and the
/// status bar reflects the new active vault.
pub(crate) fn apply_vault_create_form_submit(app: &mut App) {
    let (parent_raw, name_raw) =
        if let Some(crate::modal::Modal::VaultCreateForm(s)) = app.modal.as_ref() {
            (s.parent.as_str().to_string(), s.name.as_str().to_string())
        } else {
            return;
        };
    let target = match crate::vault::helpers::submit_create(&parent_raw, &name_raw) {
        Ok(p) => p,
        Err(msg) => {
            if let Some(crate::modal::Modal::VaultCreateForm(s)) = app.modal.as_mut() {
                s.error = Some(msg);
            }
            return;
        }
    };
    app.modal = None;
    app.vim.enter_normal();
    let display = target.display().to_string();
    match app.switch_vault(target) {
        Ok(()) => app.set_status(StatusKind::Info, format!("vault created → {display}")),
        Err(e) => app.set_status(
            StatusKind::Error,
            format!("vault created at {display} but switch failed: {e}"),
        ),
    }
}

// ───────────── vault clone form ─────────────

pub(crate) fn open_vault_clone_form(app: &mut App) {
    use crate::vim::lineedit::LineEdit;
    let default_parent = std::env::var("HOME")
        .ok()
        .unwrap_or_else(|| ".".to_string());
    app.resume_vault_picker = true;
    app.modal = Some(crate::modal::Modal::VaultCloneForm(
        crate::app::VaultCloneFormState {
            url: LineEdit::new(),
            parent: LineEdit::from_str(default_parent),
            focus: crate::app::VaultCloneFormFocus::Url,
            error: None,
        },
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
}

pub(crate) fn apply_close_vault_clone_form(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::VaultCloneForm(_))) {
        dismiss_sub_modal(app);
    }
}

pub(crate) fn with_vault_clone_form(
    app: &mut App,
    f: impl FnOnce(&mut crate::app::VaultCloneFormState),
) {
    if let Some(crate::modal::Modal::VaultCloneForm(s)) = app.modal.as_mut() {
        f(s);
    }
}

/// Validate URL + parent → git_clone → switch_vault. Errors inline.
pub(crate) fn apply_vault_clone_form_submit(app: &mut App) {
    let (url_raw, parent_raw) =
        if let Some(crate::modal::Modal::VaultCloneForm(s)) = app.modal.as_ref() {
            (s.url.as_str().to_string(), s.parent.as_str().to_string())
        } else {
            return;
        };
    let target = match crate::vault::helpers::submit_clone(&url_raw, &parent_raw) {
        Ok(p) => p,
        Err(msg) => {
            if let Some(crate::modal::Modal::VaultCloneForm(s)) = app.modal.as_mut() {
                s.error = Some(msg);
            }
            return;
        }
    };
    app.modal = None;
    app.vim.enter_normal();
    let display = target.display().to_string();
    match app.switch_vault(target) {
        Ok(()) => app.set_status(StatusKind::Info, format!("vault cloned → {display}")),
        Err(e) => app.set_status(
            StatusKind::Error,
            format!("vault cloned at {display} but switch failed: {e}"),
        ),
    }
}

// ───────────── vault open picker ─────────────

/// Open the directory navigator rooted at `$HOME` (or `.` when HOME
/// isn't set). The user can Enter to descend, Backspace to ascend,
/// Enter on a vault root to switch into it.
pub(crate) fn open_vault_open_picker(app: &mut App) -> Result<(), String> {
    let start = std::env::var("HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let canonical = start
        .canonicalize()
        .map_err(|e| format!("resolve start dir {}: {e}", start.display()))?;
    let entries = read_dir_entries(&canonical)?;
    app.resume_vault_picker = true;
    app.modal = Some(crate::modal::Modal::VaultOpenPicker(
        crate::app::VaultOpenPickerState {
            cwd: canonical,
            entries,
            selected: 0,
        },
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub(crate) fn apply_close_vault_open_picker(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::VaultOpenPicker(_))) {
        dismiss_sub_modal(app);
    }
}

pub(crate) fn apply_move_vault_open_picker_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::VaultOpenPicker(state)) = app.modal.as_mut() else {
        return;
    };
    if state.entries.is_empty() {
        return;
    }
    let last = state.entries.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

/// Enter: always descend (or ascend on `..`). Never opens as vault —
/// that's a separate action (`o`/`O`) so a vault-as-parent doesn't
/// trap navigation when the user wants to reach a deeper subdir.
pub(crate) fn apply_vault_open_picker_enter(app: &mut App) {
    let (cwd, entry) = match app.modal.as_ref() {
        Some(crate::modal::Modal::VaultOpenPicker(s)) => match s.entries.get(s.selected).cloned() {
            Some(e) => (s.cwd.clone(), e),
            None => return,
        },
        _ => return,
    };
    use crate::app::VaultOpenEntryKind;
    match entry.kind {
        VaultOpenEntryKind::Parent => navigate_to(app, cwd.parent().map(|p| p.to_path_buf())),
        VaultOpenEntryKind::Directory | VaultOpenEntryKind::Vault => {
            let target = cwd.join(&entry.name);
            navigate_to(app, Some(target));
        }
    }
}

/// `o`/`O`: open the highlighted entry as the active vault. Works on
/// Directory AND Vault entries — a vault inside another vault is
/// allowed. No-op for `..` (ascending isn't a vault target).
pub(crate) fn apply_vault_open_picker_open_as_vault(app: &mut App) {
    let (cwd, entry) = match app.modal.as_ref() {
        Some(crate::modal::Modal::VaultOpenPicker(s)) => match s.entries.get(s.selected).cloned() {
            Some(e) => (s.cwd.clone(), e),
            None => return,
        },
        _ => return,
    };
    use crate::app::VaultOpenEntryKind;
    if matches!(entry.kind, VaultOpenEntryKind::Parent) {
        return;
    }
    let target = cwd.join(&entry.name);
    app.modal = None;
    app.vim.enter_normal();
    let display = target.display().to_string();
    match app.switch_vault(target) {
        Ok(()) => app.set_status(StatusKind::Info, format!("vault → {display}")),
        Err(e) => app.set_status(StatusKind::Error, format!("switch vault: {e}")),
    }
}

pub(crate) fn apply_vault_open_picker_up(app: &mut App) {
    let cwd = match app.modal.as_ref() {
        Some(crate::modal::Modal::VaultOpenPicker(s)) => s.cwd.clone(),
        _ => return,
    };
    navigate_to(app, cwd.parent().map(|p| p.to_path_buf()));
}

/// Shared helper: re-read entries for `target` and update the picker
/// state. No-op when `target` is `None` (already at filesystem root).
fn navigate_to(app: &mut App, target: Option<std::path::PathBuf>) {
    let Some(target) = target else { return };
    let canonical = match target.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            app.set_status(StatusKind::Error, format!("dir {}: {e}", target.display()));
            return;
        }
    };
    if !canonical.is_dir() {
        return;
    }
    let entries = match read_dir_entries(&canonical) {
        Ok(es) => es,
        Err(e) => {
            app.set_status(StatusKind::Error, e);
            return;
        }
    };
    if let Some(crate::modal::Modal::VaultOpenPicker(state)) = app.modal.as_mut() {
        state.cwd = canonical;
        state.entries = entries;
        state.selected = 0;
    }
}

// ───────────── vault missing-secrets modal ─────────────

pub(crate) fn apply_close_vault_missing_secrets(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::VaultMissingSecrets(_))) {
        dismiss_sub_modal(app);
    }
}

pub(crate) fn apply_move_vault_missing_secrets_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::VaultMissingSecrets(state)) = app.modal.as_mut() else {
        return;
    };
    if state.items.is_empty() {
        return;
    }
    let last = state.items.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
    state.editing = false;
}

pub(crate) fn with_missing_secrets(
    app: &mut App,
    f: impl FnOnce(&mut crate::app::VaultMissingSecretsState),
) {
    if let Some(crate::modal::Modal::VaultMissingSecrets(s)) = app.modal.as_mut() {
        f(s);
    }
}

/// Enter while editing — push the entered value into the keychain
/// and drop the row from `pending_secrets`. Closes the modal when
/// nothing pending is left.
pub(crate) fn apply_vault_missing_secrets_save(app: &mut App) {
    let (key, value) = {
        let Some(crate::modal::Modal::VaultMissingSecrets(state)) = app.modal.as_ref() else {
            return;
        };
        let Some(row) = state.items.get(state.selected) else {
            return;
        };
        (row.keychain_key.clone(), row.value.as_str().to_string())
    };
    if value.is_empty() {
        app.set_status(StatusKind::Error, "value cannot be empty");
        return;
    }
    if let Err(e) = httui_core::db::keychain::store_secret(&key, &value) {
        app.set_status(StatusKind::Error, format!("keychain store: {e}"));
        return;
    }
    app.pending_secrets.retain(|r| r.keychain_key != key);
    let modal_drop = {
        let Some(crate::modal::Modal::VaultMissingSecrets(state)) = app.modal.as_mut() else {
            return;
        };
        if let Some(row) = state.items.get_mut(state.selected) {
            row.saved = true;
            row.value = crate::vim::lineedit::LineEdit::new();
        }
        state.editing = false;
        let next_unsaved = state
            .items
            .iter()
            .enumerate()
            .find(|(_, r)| !r.saved)
            .map(|(i, _)| i);
        match next_unsaved {
            Some(i) => {
                state.selected = i;
                false
            }
            None => true,
        }
    };
    if modal_drop {
        app.modal = None;
        app.vim.enter_normal();
        app.set_status(StatusKind::Info, "all secrets saved");
    }
}

/// `s` in browse mode — mark current row as skipped (just advance to
/// the next item). The pending list isn't touched; the badge in the
/// status bar surfaces it later.
pub(crate) fn apply_vault_missing_secrets_skip(app: &mut App) {
    let Some(crate::modal::Modal::VaultMissingSecrets(state)) = app.modal.as_mut() else {
        return;
    };
    if state.items.is_empty() {
        return;
    }
    let last = state.items.len() as i64 - 1;
    let next = ((state.selected as i64).saturating_add(1)).clamp(0, last);
    state.selected = next as usize;
}
