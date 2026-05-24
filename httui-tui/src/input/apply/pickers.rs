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

// ───────────── connection picker popup ─────────────

/// `Ctrl+L` — open the connection picker popup anchored to the DB
/// block at the cursor. Loads connections from
/// `<vault>/connections.toml` via `ConnectionsStore`
/// (V3 reordering 2026-05-23: was SQL, now reads the vault TOML so
/// desktop ↔ TUI share the same source). Returns `Err(msg)` on
/// validation failures so the caller can surface a status.
pub(crate) fn open_connection_picker(app: &mut App) -> Result<(), String> {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => return Err("no DB block at cursor".into()),
    };
    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => return Err("no DB block at cursor".into()),
    };
    if !block.is_db() {
        return Err(format!(
            "`{}` blocks don't have a connection",
            block.block_type
        ));
    }

    let store = app.connections_store.clone();
    let raw = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.list_public())
    });
    // TOML uses the connection name as the row key, so id == name —
    // ConnectionEntry's `id` field carries the lookup key callers
    // (status bar, schema cache) already expect.
    let connections: Vec<crate::app::ConnectionEntry> = match raw {
        Ok(list) => list
            .into_iter()
            .map(|c| crate::app::ConnectionEntry {
                id: c.name.clone(),
                name: c.name,
                kind: c.driver,
            })
            .collect(),
        Err(e) => return Err(format!("connection list failed: {e}")),
    };
    if connections.is_empty() {
        return Err("no connections registered yet".into());
    }

    // Pre-select the block's current connection so the user can hit
    // Enter to keep it (or arrow to switch). Falls back to the first
    // entry when the current value matches nothing.
    let current = block
        .params
        .get("connection_id")
        .or_else(|| block.params.get("connection"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let selected = connections
        .iter()
        .position(|c| c.id == current || c.name == current)
        .unwrap_or(0);

    app.modal = Some(crate::modal::Modal::ConnectionPicker(
        crate::app::ConnectionPickerState {
            segment_idx,
            connections,
            selected,
        },
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub(crate) fn apply_close_connection_picker(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::ConnectionPicker(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub(crate) fn apply_move_connection_picker_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::ConnectionPicker(state)) = app.modal.as_mut() else {
        return;
    };
    if state.connections.is_empty() {
        return;
    }
    let last = state.connections.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

/// `Enter` in the picker — write the selected connection's id to
/// the anchored block's params (`connection` field) and close. The
/// document is marked dirty via `snapshot()` so undo can restore
/// the previous value.
pub(crate) fn apply_confirm_connection_picker(app: &mut App) {
    let state = match app.modal.take() {
        Some(crate::modal::Modal::ConnectionPicker(s)) => s,
        other => {
            app.modal = other;
            app.vim.enter_normal();
            return;
        }
    };
    app.vim.enter_normal();
    let Some(picked) = state.connections.get(state.selected).cloned() else {
        return;
    };
    let segment_idx = state.segment_idx;
    let picked_id = picked.id.clone();
    let picked_name = picked.name.clone();
    if let Some(doc) = app.tabs.active_document_mut() {
        doc.snapshot();
        if let Some(block) = doc.block_at_mut(segment_idx) {
            if let Some(obj) = block.params.as_object_mut() {
                obj.insert(
                    "connection".into(),
                    serde_json::Value::String(picked_id.clone()),
                );
                // Drop the legacy alias so the next save serializes
                // the canonical `connection=<id>` form only — the
                // `connection_id` field was a JSON-body holdover from
                // pre-redesign blocks and gets resolved the same way
                // at run time.
                obj.remove("connection_id");
            }
        }
    }
    // Kick off schema introspection in the background. By the time
    // the user starts typing inside the SQL field, the
    // completion engine has tables/columns ready to suggest. Cheap to
    // call repeatedly — `ensure_schema_loaded` dedups on `pending`.
    app.ensure_schema_loaded(&picked_id);
    app.set_status(StatusKind::Info, format!("connection set to {picked_name}"));
}

/// `D` in the connection picker — drop the highlighted connection
/// from `<vault>/connections.toml` via `ConnectionsStore` (V3
/// reordering 2026-05-23: was SQL, now writes the vault TOML so
/// desktop ↔ TUI stay in sync). Blocks that referenced the deleted
/// name will surface a missing-connection error on next run.
pub(crate) fn apply_delete_connection_in_picker(app: &mut App) {
    let Some(crate::modal::Modal::ConnectionPicker(state)) = app.modal.as_ref() else {
        return;
    };
    let Some(picked) = state.connections.get(state.selected).cloned() else {
        return;
    };
    let store = app.connections_store.clone();
    let name = picked.name.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.delete(&name))
    });
    if let Err(e) = result {
        app.set_status(StatusKind::Error, format!("delete connection failed: {e}"));
        return;
    }

    // Reload the list so the picker reflects the deletion. If we
    // emptied the list, close the picker — there's nothing left to
    // pick.
    let raw = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.list_public())
    });
    match raw {
        Ok(list) => {
            let entries: Vec<crate::app::ConnectionEntry> = list
                .into_iter()
                .map(|c| crate::app::ConnectionEntry {
                    id: c.name.clone(),
                    name: c.name,
                    kind: c.driver,
                })
                .collect();
            if entries.is_empty() {
                apply_close_connection_picker(app);
                app.set_status(
                    StatusKind::Info,
                    format!("deleted \"{name}\" — no connections left"),
                );
                return;
            }
            if let Some(crate::modal::Modal::ConnectionPicker(state)) = app.modal.as_mut() {
                state.selected = state.selected.min(entries.len().saturating_sub(1));
                state.connections = entries;
            }
            // Refresh the global name lookup so block headers stop
            // showing the deleted connection's label.
            app.refresh_connection_names();
            app.set_status(StatusKind::Info, format!("deleted \"{name}\""));
        }
        Err(e) => {
            app.set_status(
                StatusKind::Error,
                format!("connection list reload failed: {e}"),
            );
        }
    }
}

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

// ───────────── environment picker (gE) ─────────────

/// `gE` — list envs from `<vault>/envs/*.toml` via `EnvironmentsStore`
/// (V4 P1, 2026-05-23: was SQL, now reads the vault TOML so
/// desktop ↔ TUI share the same source). Pre-selects the active env
/// so Enter is a no-op confirm.
pub(crate) fn open_environment_picker(app: &mut App) -> Result<(), String> {
    let store = app.environments_store.clone();
    let (entries, active_id) = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let envs = store
                .list_envs()
                .await
                .map_err(|e| format!("env list failed: {e}"))?;
            // `active_env()` returns `Result<Option<String>>`. Swallow
            // any read error so the picker still opens — falls back
            // to first entry pre-selected.
            let active = store.active_env().await.ok().flatten();
            Ok::<_, String>((envs, active))
        })
    })?;
    if entries.is_empty() {
        return Err("no environments registered yet".into());
    }
    // TOML keys envs by name so `id == name`. Preserves the legacy
    // EnvironmentEntry shape so the renderer and confirm path keep
    // working unchanged.
    let entries: Vec<crate::app::EnvironmentEntry> = entries
        .into_iter()
        .map(|e| crate::app::EnvironmentEntry {
            id: e.name.clone(),
            name: e.name,
        })
        .collect();
    let selected = active_id
        .as_deref()
        .and_then(|n| entries.iter().position(|e| e.id == n))
        .unwrap_or(0);

    app.modal = Some(crate::modal::Modal::EnvironmentPicker(
        crate::app::EnvironmentPickerState {
            entries,
            selected,
            active_id,
        },
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub(crate) fn apply_close_environment_picker(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::EnvironmentPicker(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub(crate) fn apply_move_environment_picker_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::EnvironmentPicker(state)) = app.modal.as_mut() else {
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

/// `Enter` in the env picker — flip the active flag in SQLite, refresh
/// the cached display name (so the status-bar chip updates), and
/// dismiss. A no-op when the highlighted entry is already active.
pub(crate) fn apply_confirm_environment_picker(app: &mut App) {
    let state = match app.modal.take() {
        Some(crate::modal::Modal::EnvironmentPicker(s)) => s,
        other => {
            app.modal = other;
            app.vim.enter_normal();
            return;
        }
    };
    app.vim.enter_normal();
    let Some(picked) = state.entries.get(state.selected).cloned() else {
        return;
    };
    if state.active_id.as_deref() == Some(picked.id.as_str()) {
        // Already active — silent no-op rather than a redundant
        // SQLite write. The display name is current.
        return;
    }
    let store = app.environments_store.clone();
    let name = picked.name.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.set_active_env(Some(&name)))
    });
    if let Err(e) = result {
        app.set_status(StatusKind::Error, format!("set active env failed: {e}"));
        return;
    }
    app.refresh_active_env_name();
    app.set_status(StatusKind::Info, format!("env: {}", picked.name));
}

/// `apply_action` sub-match for the picker-popup domain: connection
/// (`gc`), environment (`gE`), tab (`gb`), block-template (`gN`).
/// Mechanically split out of the `apply_action` router in
/// `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p6e) — arm bodies
/// copied verbatim. The outer router routes only this group's variants
/// here, so the `unreachable!` is a compile-time-backed invariant.
pub(crate) fn apply_pickers(app: &mut App, action: Action, _recording: bool) {
    match action {
        Action::OpenConnectionPicker => {
            if let Err(msg) = open_connection_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseConnectionPicker => apply_close_connection_picker(app),
        Action::MoveConnectionPickerCursor(delta) => {
            apply_move_connection_picker_cursor(app, delta)
        }
        Action::ConfirmConnectionPicker => apply_confirm_connection_picker(app),
        Action::DeleteConnectionInPicker => apply_delete_connection_in_picker(app),
        Action::OpenEnvironmentPicker => {
            if let Err(msg) = open_environment_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseEnvironmentPicker => apply_close_environment_picker(app),
        Action::MoveEnvironmentPickerCursor(delta) => {
            apply_move_environment_picker_cursor(app, delta)
        }
        Action::ConfirmEnvironmentPicker => apply_confirm_environment_picker(app),
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
            if let Err(msg) = open_vault_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseVaultPicker => apply_close_vault_picker(app),
        Action::MoveVaultPickerCursor(delta) => apply_move_vault_picker_cursor(app, delta),
        Action::ConfirmVaultPicker => apply_confirm_vault_picker(app),
        _ => unreachable!("apply_pickers: variante fora do grupo"),
    }
}

// ───────────── vault picker (V10 slice 8 — Alt+W) ─────────────

/// Open the vault picker. Reads every path registered via
/// `httui_core::vaults::list_vaults` and marks the active one with
/// `active`. Returns an error if the registry is empty (the
/// empty-state — slice 2 — handles first-run; the picker is a tool
/// for users who already have at least one vault).
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
        return Err("nenhum vault registrado ainda".into());
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
        app.set_status(StatusKind::Info, format!("já no vault {target}"));
        return;
    }
    match app.switch_vault(std::path::PathBuf::from(&target)) {
        Ok(()) => app.set_status(StatusKind::Info, format!("vault → {target}")),
        Err(e) => app.set_status(StatusKind::Error, format!("switch vault: {e}")),
    }
}
