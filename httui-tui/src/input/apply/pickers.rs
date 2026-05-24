// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
//! Picker-popup appliers: connection (`gc`), tab / block-template
//! (`gb` / `gN`), environment (`gE`). Mechanically moved out of
//! `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p5d) with no logic
//! change.

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};
use crate::input::action::Action;
use crate::vim::mode::Mode;


// ───────────── connections page (gC / Alt+P) ─────────────

/// V3 (2026-05-23): open the fullscreen Connections page. Loads
/// every entry from `<vault>/connections.toml` via `ConnectionsStore`
/// and seeds the state. `Err(msg)` bubbles up to the status bar; an
/// empty `connections.toml` is **not** an error — the page opens with
/// an empty list and a "press N to create" hint so the user can seed
/// the first entry from inside the TUI.
pub(crate) fn open_connections_page(app: &mut App) -> Result<(), String> {
    let store = app.connections_store.clone();
    let entries = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.list_public())
    })
    .map_err(|e| format!("connection list failed: {e}"))?;
    let connections: Vec<crate::app::ConnectionDetail> = entries
        .into_iter()
        .map(|c| crate::app::ConnectionDetail {
            name: c.name,
            driver: c.driver,
            host: c.host,
            port: c.port,
            database_name: c.database_name,
            username: c.username,
            has_password: c.has_password,
            ssl_mode: c.ssl_mode,
            is_readonly: c.is_readonly,
            description: c.description,
        })
        .collect();
    let vault_root = app.vault_path.clone();
    let first_name = connections.first().map(|c| c.name.clone());
    let uses = first_name
        .as_deref()
        .map(|n| httui_core::connection_uses::find_connection_uses(&vault_root, n))
        .unwrap_or_default();
    app.modal = Some(crate::modal::Modal::Connections(
        crate::app::ConnectionsPageState {
            connections,
            selected: 0,
            uses,
        },
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
    // V3 P5.2: kick off the schema introspection in the background;
    // the renderer reads from `app.schema_cache` and shows "loading…"
    // until `AppEvent::SchemaLoaded` lands and rerenders.
    if let Some(name) = first_name {
        app.ensure_schema_loaded(&name);
    }
    Ok(())
}

/// V3 P5.1: recompute `state.uses` for the currently-selected entry.
/// Called by the cursor-move applier so the detail pane stays in sync
/// without rebuilding the whole snapshot.
fn refresh_uses_for_selected(app: &mut App) {
    let vault_root = app.vault_path.clone();
    if let Some(crate::modal::Modal::Connections(state)) = app.modal.as_mut() {
        state.uses = state
            .connections
            .get(state.selected)
            .map(|c| httui_core::connection_uses::find_connection_uses(&vault_root, &c.name))
            .unwrap_or_default();
    }
}

pub(crate) fn apply_close_connections_page(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::Connections(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub(crate) fn apply_move_connections_page_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::Connections(state)) = app.modal.as_mut() else {
        return;
    };
    if state.connections.is_empty() {
        return;
    }
    let previous = state.selected;
    let last = state.connections.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
    // V3 P5.1/P5.2: refresh "used in" snapshot + trigger schema
    // introspection for the new selection. Both are no-ops when
    // selection didn't actually move (j past the end / k past the
    // start).
    if state.selected != previous {
        refresh_uses_for_selected(app);
        let selected_name = if let Some(crate::modal::Modal::Connections(s)) = app.modal.as_ref() {
            s.connections.get(s.selected).map(|c| c.name.clone())
        } else {
            None
        };
        if let Some(name) = selected_name {
            app.ensure_schema_loaded(&name);
        }
    }
}

// ───────────── tab picker (gb) ─────────────

/// `gb` — snapshot every tab's focused-leaf path + dirty flag and
/// open the picker. Pre-selects the currently-active tab so Enter
/// is a no-op confirm. Silent decline when there's only one tab
/// (the picker would just display that single row, no real choice).
pub(crate) fn apply_open_tab_picker(app: &mut App) {
    if app.tabs.len() <= 1 {
        app.set_status(StatusKind::Info, "only one tab open");
        return;
    }
    let active = app.tabs.active;
    let entries: Vec<crate::app::TabPickerEntry> = app
        .tabs
        .tabs
        .iter()
        .enumerate()
        .map(|(idx, tab)| {
            let leaf = tab.active_leaf();
            let label = leaf
                .document_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "(no file)".into());
            let dirty = leaf.document.as_ref().is_some_and(|d| d.is_dirty());
            crate::app::TabPickerEntry { idx, label, dirty }
        })
        .collect();
    app.modal = Some(crate::modal::Modal::TabPicker(
        crate::app::TabPickerState {
            entries,
            selected: active,
        },
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
}

pub(crate) fn apply_move_tab_picker_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::TabPicker(state)) = app.modal.as_mut() else {
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

/// `Enter` in the tab picker — flip `tabs.active` to the picked
/// index and dismiss. The `sync_file_watcher` call after every
/// keystroke (in the main loop) catches the new file in lockstep.
pub(crate) fn apply_confirm_tab_picker(app: &mut App) {
    let state = match app.modal.take() {
        Some(crate::modal::Modal::TabPicker(s)) => s,
        other => {
            app.modal = other;
            app.vim.enter_normal();
            return;
        }
    };
    app.vim.enter_normal();
    let Some(picked) = state.entries.get(state.selected) else {
        return;
    };
    if picked.idx < app.tabs.tabs.len() {
        app.tabs.active = picked.idx;
    }
}

// ───────────── block-template picker (gN) ─────────────

pub(crate) fn apply_move_block_template_picker_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::BlockTemplatePicker(state)) = app.modal.as_mut() else {
        return;
    };
    let len = crate::app::BlockTemplate::ALL.len();
    if len == 0 {
        return;
    }
    let last = len as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

/// `Enter` in the block-template picker — splice the picked
/// template's text into the active document at the cursor's
/// segment + line and re-parse so the typed fence promotes to a
/// `Segment::Block`. Three placement rules:
///
/// 1. Cursor in prose → insert at the start of the line *after*
///    the cursor's line (so we don't break a half-typed sentence
///    by injecting a fence mid-line).
/// 2. Cursor in a block → status error "exit block first" (the
///    template fence would corrupt the host block's `raw` rope).
/// 3. Cursor in a block result → same as (2).
///
/// Snapshot is taken before the splice so undo restores both the
/// inserted text and any cursor jump that follows. Cursor lands on
/// the freshly-promoted block's first body offset so the user can
/// immediately edit the URL / SQL.
pub(crate) fn apply_confirm_block_template_picker(app: &mut App) {
    let state = match app.modal.take() {
        Some(crate::modal::Modal::BlockTemplatePicker(s)) => s,
        other => {
            app.modal = other;
            app.vim.enter_normal();
            return;
        }
    };
    app.vim.enter_normal();
    let Some(tpl) = crate::app::BlockTemplate::ALL.get(state.selected).copied() else {
        return;
    };
    let cursor = match app.document().map(|d| d.cursor()) {
        Some(c) => c,
        None => return,
    };
    let (segment_idx, line_offset_for_insert) = match cursor {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            // Compute the char offset at the start of the line *after*
            // the cursor's line so the splice goes onto a fresh line.
            let Some(doc) = app.document() else { return };
            let rope = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => r,
                _ => return,
            };
            let total = rope.len_chars();
            let off = offset.min(total);
            let line = rope.char_to_line(off);
            // `line_to_char(line + 1)` is the start of the next line;
            // when the cursor is on the last line this hits `total`,
            // which is also "end of doc" — the splice appends.
            let next_line_start = if line + 1 < rope.len_lines() {
                rope.line_to_char(line + 1)
            } else {
                total
            };
            (segment_idx, next_line_start)
        }
        Cursor::InBlock { .. } | Cursor::InBlockResult { .. } => {
            app.set_status(
                StatusKind::Info,
                "exit block (Esc) before inserting a new block template",
            );
            return;
        }
    };

    if let Some(doc) = app.document_mut() {
        doc.snapshot();
        // Templates already include a trailing newline; if the cursor
        // is on a non-empty last line we prepend a `\n` so the fence
        // doesn't graft onto existing prose. `insert_text_in_segment`
        // is a raw rope insert — `reparse_prose_at` does the magic.
        let needs_leading_newline = {
            let rope = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => r,
                _ => return,
            };
            line_offset_for_insert == rope.len_chars()
                && rope.len_chars() > 0
                && rope.char(rope.len_chars() - 1) != '\n'
        };
        let to_insert: String = if needs_leading_newline {
            format!("\n{}", tpl.text)
        } else {
            tpl.text.to_string()
        };
        doc.insert_text_in_segment(segment_idx, line_offset_for_insert, &to_insert);
        // Promote the typed fence into a `Segment::Block`. After this
        // call the prose segment may have been split (or replaced),
        // and a new block segment exists in its slot.
        doc.reparse_prose_at(segment_idx);
        // Park the cursor at the start of the first new block, if
        // any — search forward from the original splice point.
        if let Some((new_idx, _)) = doc
            .segments()
            .iter()
            .enumerate()
            .skip(segment_idx)
            .find(|(_, s)| matches!(s, Segment::Block(_)))
        {
            doc.set_cursor(Cursor::InBlock {
                segment_idx: new_idx,
                offset: 0,
            });
        }
    }
    app.refresh_viewport_for_cursor();
    app.set_status(StatusKind::Info, format!("inserted {}", tpl.label));
}

// Environment picker (gE) handlers were mechanically split into
// `super::env_picker` (tui-V10) to keep this file under the 600-line
// size gate.

/// `apply_action` sub-match for the picker-popup domain: connection
/// (`gc`), environment (`gE`), tab (`gb`), block-template (`gN`).
/// Mechanically split out of the `apply_action` router in
/// `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p6e) — arm bodies
/// copied verbatim. The outer router routes only this group's variants
/// here, so the `unreachable!` is a compile-time-backed invariant.
pub(crate) fn apply_pickers(app: &mut App, action: Action, _recording: bool) {
    use super::connection_picker as cp;
    use super::env_picker as ep;
    use super::vault_modals as vm;
    match action {
        Action::OpenConnectionPicker => {
            if let Err(msg) = cp::open_connection_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseConnectionPicker => cp::apply_close_connection_picker(app),
        Action::MoveConnectionPickerCursor(delta) => {
            cp::apply_move_connection_picker_cursor(app, delta)
        }
        Action::ConfirmConnectionPicker => cp::apply_confirm_connection_picker(app),
        Action::DeleteConnectionInPicker => cp::apply_delete_connection_in_picker(app),
        Action::OpenEnvironmentPicker => {
            if let Err(msg) = ep::open_environment_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseEnvironmentPicker => ep::apply_close_environment_picker(app),
        Action::MoveEnvironmentPickerCursor(delta) => {
            ep::apply_move_environment_picker_cursor(app, delta)
        }
        Action::ConfirmEnvironmentPicker => ep::apply_confirm_environment_picker(app),
        Action::ActivateEnvByIndex(idx) => super::env_activate::apply_activate_env_by_index(app, idx),
        Action::OpenConnectionsPage => {
            if let Err(msg) = open_connections_page(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseConnectionsPage => apply_close_connections_page(app),
        Action::MoveConnectionsPageCursor(delta) => {
            apply_move_connections_page_cursor(app, delta)
        }
        Action::OpenBlockTemplatePicker => {
            app.modal = Some(crate::modal::Modal::BlockTemplatePicker(
                crate::app::BlockTemplatePickerState::new(),
            ));
            app.vim.mode = Mode::Modal;
            app.vim.reset_pending();
        }
        Action::CloseBlockTemplatePicker => {
            if matches!(app.modal, Some(crate::modal::Modal::BlockTemplatePicker(_))) {
                app.modal = None;
            }
            app.vim.enter_normal();
        }
        Action::MoveBlockTemplatePickerCursor(delta) => {
            apply_move_block_template_picker_cursor(app, delta)
        }
        Action::ConfirmBlockTemplatePicker => apply_confirm_block_template_picker(app),
        Action::OpenTabPicker => apply_open_tab_picker(app),
        Action::CloseTabPicker => {
            if matches!(app.modal, Some(crate::modal::Modal::TabPicker(_))) {
                app.modal = None;
            }
            app.vim.enter_normal();
        }
        Action::MoveTabPickerCursor(delta) => apply_move_tab_picker_cursor(app, delta),
        Action::ConfirmTabPicker => apply_confirm_tab_picker(app),
        Action::OpenVaultPicker => {
            if let Err(msg) = vm::open_vault_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseVaultPicker => vm::apply_close_vault_picker(app),
        Action::MoveVaultPickerCursor(delta) => vm::apply_move_vault_picker_cursor(app, delta),
        Action::ConfirmVaultPicker => vm::apply_confirm_vault_picker(app),
        Action::OpenVaultCreateForm => vm::open_vault_create_form(app),
        Action::CloseVaultCreateForm => vm::apply_close_vault_create_form(app),
        Action::VaultCreateFormFocusNext => {
            vm::with_vault_create_form(app, |f| f.focus = f.focus.next())
        }
        Action::VaultCreateFormFocusPrev => {
            vm::with_vault_create_form(app, |f| f.focus = f.focus.prev())
        }
        Action::VaultCreateFormChar(c) => vm::with_vault_create_form(app, |f| match f.focus {
            crate::app::VaultCreateFormFocus::Parent => f.parent.insert_char(c),
            crate::app::VaultCreateFormFocus::Name => f.name.insert_char(c),
        }),
        Action::VaultCreateFormBackspace => vm::with_vault_create_form(app, |f| match f.focus {
            crate::app::VaultCreateFormFocus::Parent => {
                f.parent.delete_before();
            }
            crate::app::VaultCreateFormFocus::Name => {
                f.name.delete_before();
            }
        }),
        Action::VaultCreateFormSubmit => vm::apply_vault_create_form_submit(app),
        Action::OpenVaultCloneForm => vm::open_vault_clone_form(app),
        Action::CloseVaultCloneForm => vm::apply_close_vault_clone_form(app),
        Action::VaultCloneFormFocusNext => {
            vm::with_vault_clone_form(app, |f| f.focus = f.focus.next())
        }
        Action::VaultCloneFormFocusPrev => {
            vm::with_vault_clone_form(app, |f| f.focus = f.focus.prev())
        }
        Action::VaultCloneFormChar(c) => vm::with_vault_clone_form(app, |f| match f.focus {
            crate::app::VaultCloneFormFocus::Url => f.url.insert_char(c),
            crate::app::VaultCloneFormFocus::Parent => f.parent.insert_char(c),
        }),
        Action::VaultCloneFormBackspace => vm::with_vault_clone_form(app, |f| match f.focus {
            crate::app::VaultCloneFormFocus::Url => {
                f.url.delete_before();
            }
            crate::app::VaultCloneFormFocus::Parent => {
                f.parent.delete_before();
            }
        }),
        Action::VaultCloneFormSubmit => vm::apply_vault_clone_form_submit(app),
        Action::OpenVaultOpenPicker => {
            if let Err(msg) = vm::open_vault_open_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseVaultOpenPicker => vm::apply_close_vault_open_picker(app),
        Action::MoveVaultOpenPickerCursor(delta) => {
            vm::apply_move_vault_open_picker_cursor(app, delta)
        }
        Action::VaultOpenPickerEnter => vm::apply_vault_open_picker_enter(app),
        Action::VaultOpenPickerUp => vm::apply_vault_open_picker_up(app),
        Action::CloseVaultMissingSecrets => vm::apply_close_vault_missing_secrets(app),
        Action::MoveVaultMissingSecretsCursor(delta) => {
            vm::apply_move_vault_missing_secrets_cursor(app, delta)
        }
        Action::VaultMissingSecretsEnterEdit => vm::with_missing_secrets(app, |s| s.editing = true),
        Action::VaultMissingSecretsCancelEdit => vm::with_missing_secrets(app, |s| {
            s.editing = false;
            if let Some(row) = s.items.get_mut(s.selected) {
                row.value = crate::vim::lineedit::LineEdit::new();
            }
        }),
        Action::VaultMissingSecretsChar(c) => vm::with_missing_secrets(app, |s| {
            if let Some(row) = s.items.get_mut(s.selected) {
                row.value.insert_char(c);
            }
        }),
        Action::VaultMissingSecretsBackspace => vm::with_missing_secrets(app, |s| {
            if let Some(row) = s.items.get_mut(s.selected) {
                row.value.delete_before();
            }
        }),
        Action::VaultMissingSecretsSave => vm::apply_vault_missing_secrets_save(app),
        Action::VaultMissingSecretsSkip => vm::apply_vault_missing_secrets_skip(app),
        _ => unreachable!("apply_pickers: variante fora do grupo"),
    }
}

