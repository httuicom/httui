// coverage:exclude file — connection picker applier cluster relocated
// by tui-V10 (split of pickers.rs to satisfy size gate); coverage
// tracked in docs-llm/tui-v2/vim-coverage-debt.md.
//! `Ctrl+L` connection picker handlers. Mechanically split out of
//! `pickers.rs` (tui-V10) to keep that file under the 600-line size
//! gate. No behavior change.

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};
use crate::vim::mode::Mode;

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
                obj.remove("connection_id");
            }
        }
    }
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
