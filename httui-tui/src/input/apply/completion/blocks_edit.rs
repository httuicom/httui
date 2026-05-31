//! Completion popup driver for the BLOCKS-view EDIT buffer.
//!
//! Reads body + cursor from the active `pane.block_edit` sub-doc
//! (the buffer the user is typing into) and assembles a popup the
//! same way the DOC view does, with two differences:
//!
//! 1. The segments fed to `complete_refs` come from `pane.document`
//!    (the real file), not `app.document()` — the latter redirects
//!    into the field sub-doc which has zero `Segment::Block` entries.
//! 2. `cached_result` on every upstream block is re-hydrated from
//!    SQLite via `block_hydrate::hydrate_segments_blocking` so a
//!    sibling pane that just re-ran the same alias is visible here.

use crate::app::App;
use crate::commands::db::{load_active_env_vars, resolve_connection_id_sync};

use super::close_completion_popup_if_open;

/// True when any BLOCKS-view EDIT buffer is active. SQL completion only
/// fires on DB query (gated downstream by `block_type.starts_with("db")`);
/// `{{ref}}` completion works in every field — URL, header value, HTTP
/// body, DB query — since refs are uniform across block kinds.
pub(super) fn blocks_edit_completion_active(app: &App) -> bool {
    if !matches!(app.view, crate::app::AppView::Blocks) {
        return false;
    }
    let Some(pane) = app.active_pane() else {
        return false;
    };
    pane.block_edit.is_some()
}

/// `rebuild_completion_popup` for BLOCKS view EDIT on a DB Query.
/// Mirrors the DOC view path but reads body + cursor from the
/// sub-doc rather than `Cursor::InBlock` on the pane document.
pub(super) fn rebuild_completion_popup_blocks_edit(app: &mut App, allow_empty_prefix: bool) {
    // Pull the sub-doc text + cursor (line, col) from the edit
    // buffer the user is typing into. Also snapshot the file-wide
    // segments + the focused block's index so ref completion sees
    // every block above this one (not just the field sub-doc, which
    // has zero `Segment::Block` entries).
    let (body, line, offset, block_type, conn_raw, mut file_segments, current_segment, abs_path) = {
        let Some(pane) = app.active_pane() else {
            close_completion_popup_if_open(app);
            return;
        };
        let Some(edit) = pane.block_edit.as_ref() else {
            close_completion_popup_if_open(app);
            return;
        };
        let body = edit.current_text();
        let cursor_offset = match edit.doc.cursor() {
            crate::buffer::Cursor::InProse { offset, .. } => offset,
            crate::buffer::Cursor::InBlock { offset, .. } => offset,
            _ => 0,
        };
        let mut line = 0usize;
        let mut col = 0usize;
        for ch in body.chars().take(cursor_offset) {
            if ch == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        // Resolve the block_type + connection raw from the workspace
        // index — we need both to pick the dialect and the schema.
        let Some(ws) = app.blocks_workspace.as_ref() else {
            close_completion_popup_if_open(app);
            return;
        };
        let Some(sel) = pane.block_selected else {
            close_completion_popup_if_open(app);
            return;
        };
        let Some(file) = ws.index.files.get(sel.file_idx) else {
            close_completion_popup_if_open(app);
            return;
        };
        let Some(meta) = file.blocks.get(sel.block_idx) else {
            close_completion_popup_if_open(app);
            return;
        };
        let block_type = meta.block_type.clone();
        let alias = meta.alias.clone();
        let conn_raw = read_block_connection(&app.vault_path, &file.path, meta.line_start);
        let (file_segments, current_segment) = pane
            .document
            .as_ref()
            .map(|doc| {
                let segs: Vec<crate::buffer::Segment> = doc.segments().to_vec();
                let idx = segs
                    .iter()
                    .position(|s| matches!(
                        s,
                        crate::buffer::Segment::Block(b)
                            if b.block_type == block_type && b.alias == alias
                    ))
                    .unwrap_or(segs.len());
                (segs, idx)
            })
            .unwrap_or_else(|| (Vec::new(), 0));
        let abs_path = app.vault_path.join(&file.path);
        (
            body,
            line,
            col,
            block_type,
            conn_raw,
            file_segments,
            current_segment,
            abs_path,
        )
    };

    // Re-attach `cached_result` from SQLite onto the segments snapshot
    // so the popup sees the latest captured response — another pane (or
    // a previous session) may have refreshed it after this pane's
    // `pane.document` was last loaded. Without this the popup paths
    // would drift after every cross-pane rerun.
    let env_vars_for_hydrate: std::collections::HashMap<String, String> =
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(load_active_env_vars(&app.environments_store))
        })
        .unwrap_or_default();
    crate::block_hydrate::hydrate_segments_blocking(
        app.pool_manager.app_pool(),
        &mut file_segments,
        &env_vars_for_hydrate,
        &abs_path,
    );

    // Stable anchor key for the popup state — the renderer overrides
    // position via `popup_cursor_cell` published from the BLOCKS pane,
    // so this index just keys the modal.
    let segment_idx = 0usize;

    // Ref completion (`{{...}}`) — works in any block kind. Same
    // engine the DOC view uses.
    if let Some(ref_detect) = crate::sql_completion::detect_ref_context(&body, line, offset) {
        let items = crate::sql_completion::complete_refs(
            &ref_detect,
            &file_segments,
            current_segment,
            &env_vars_for_hydrate,
        );
        if items.is_empty() {
            close_completion_popup_if_open(app);
            return;
        }
        let prior_label = app
            .completion_popup()
            .and_then(|p| p.items.get(p.selected))
            .map(|i| i.label.clone());
        let selected = prior_label
            .and_then(|lbl| items.iter().position(|i| i.label == lbl))
            .unwrap_or(0);
        app.modal = Some(crate::modal::Modal::CompletionPopup(
            crate::app::CompletionPopupState {
                segment_idx,
                items,
                selected,
                anchor_line: line,
                anchor_offset: ref_detect.anchor_offset,
                prefix: ref_detect.prefix,
            },
        ));
        return;
    }

    // SQL completion — DB blocks only.
    if !block_type.starts_with("db") {
        close_completion_popup_if_open(app);
        return;
    }
    let (anchor_offset, prefix) =
        match crate::sql_completion::prefix_at_cursor(&body, line, offset) {
            Some(p) => p,
            None if allow_empty_prefix => (offset, String::new()),
            None => {
                close_completion_popup_if_open(app);
                return;
            }
        };
    let dialect = crate::sql_completion::Dialect::from_block_type(&block_type);
    let context = crate::sql_completion::detect_context(&body, line, anchor_offset);
    let conn_id = conn_raw
        .as_deref()
        .map(|raw| resolve_connection_id_sync(raw, &app.connection_names));
    let schema_tables: Option<Vec<crate::schema::SchemaTable>> = conn_id
        .as_deref()
        .and_then(|id| app.schema_cache.get(id))
        .map(|e| e.tables.clone());
    if let Some(id) = conn_id.as_deref() {
        if schema_tables.is_none() {
            app.ensure_schema_loaded(id);
        }
    }
    let items =
        crate::sql_completion::complete(dialect, &prefix, context, schema_tables.as_deref());
    if items.is_empty() {
        close_completion_popup_if_open(app);
        return;
    }
    let prior_label = app
        .completion_popup()
        .and_then(|p| p.items.get(p.selected))
        .map(|i| i.label.clone());
    let selected = prior_label
        .and_then(|lbl| items.iter().position(|i| i.label == lbl))
        .unwrap_or(0);
    app.modal = Some(crate::modal::Modal::CompletionPopup(
        crate::app::CompletionPopupState {
            segment_idx,
            items,
            selected,
            anchor_line: line,
            anchor_offset,
            prefix,
        },
    ));
}

fn read_block_connection(
    vault: &std::path::Path,
    file: &std::path::Path,
    line_start: usize,
) -> Option<String> {
    let text =
        httui_core::fs::read_note(&vault.to_string_lossy(), &file.to_string_lossy()).ok()?;
    let parsed = httui_core::blocks::parse_blocks(&text);
    let p = parsed.iter().find(|p| p.line_start == line_start)?;
    p.params
        .get("connection")
        .or_else(|| p.params.get("connection_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}
