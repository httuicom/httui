use super::*;

/// Open the field at `(region, row, col)` for inline editing. The
/// sub-`Document` is seeded from the draft's current value so vim
/// motions / search / undo land on something real from frame zero.
/// `EnterMode::Auto` defers the sub-mode to the active profile:
/// standard → INSERT, vim → NORMAL. `EnterMode::Insert` forces INSERT
/// (vim `i`/`a`/`o`).
pub(crate) fn enter_edit(app: &mut App, mode: EnterMode) {
    // Result region of a DB block: Enter opens the row-detail modal
    // (reuses the DOC view's `apply_open_db_row_detail` after parking
    // the cursor on the focused row).
    if let Some((segment_idx, row)) = focused_db_result_row(app) {
        if let Some(doc) = app.document_mut() {
            doc.set_cursor(crate::buffer::Cursor::InBlockResult { segment_idx, row });
        }
        crate::input::apply::modal_detail::apply_open_db_row_detail(app);
        return;
    }
    // Response region of an HTTP block: Enter opens the response-detail
    // modal (reuses the DOC view's `apply_open_http_response_detail`
    // after parking the cursor on the block's result).
    if let Some(segment_idx) = focused_http_response_segment(app) {
        if let Some(doc) = app.document_mut() {
            doc.set_cursor(crate::buffer::Cursor::InBlockResult {
                segment_idx,
                row: 0,
            });
        }
        crate::input::apply::modal_detail::apply_open_http_response_detail(app);
        return;
    }
    let Some(field) = field_for_focus(app) else {
        return;
    };
    let needs_hydrate = app
        .active_pane()
        .map(|p| p.block_draft.is_none())
        .unwrap_or(true);
    if needs_hydrate && !hydrate_draft(app) {
        app.set_status(StatusKind::Error, "block missing on disk");
        return;
    }
    let initial = current_field_value(app, &field).unwrap_or_default();
    let vim = app.config.editor.mode == EditorMode::Vim;
    let edit = match mode {
        EnterMode::Insert => RegionEdit::insert(field, initial),
        EnterMode::Auto if vim => RegionEdit::normal(field, initial),
        EnterMode::Auto => RegionEdit::insert(field, initial),
    };
    if let Some(pane) = app.active_pane_mut() {
        pane.block_edit = Some(Box::new(edit));
    }
    // Pin the vim engine to the mode matching the sub-doc so chords
    // arrive at the right parser (`parse_normal` for NORMAL,
    // `parse_insert` for INSERT). Standard profile ignores vim.mode.
    let mode = if matches!(
        app.active_pane().and_then(|p| p.block_edit.as_ref()).map(|e| e.sub_mode),
        Some(EditSubMode::Insert)
    ) {
        Mode::Insert
    } else {
        Mode::Normal
    };
    app.vim.mode = mode;
    app.vim.reset_pending();
}

/// Esc on an active buffer: serialize the sub-doc and write it into
/// the draft's matching field, then clear `block_edit`. The trailing
/// newline that `Document::to_markdown` appends is stripped — it's a
/// rendering artefact, not part of the user's value.
pub(crate) fn commit_edit(app: &mut App) {
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let Some(edit) = pane.block_edit.take() else {
        return;
    };
    let Some(draft) = pane.block_draft.as_mut() else {
        return;
    };
    let value = edit.current_text();
    match edit.field {
        EditField::HttpUrl => draft.set_url(value),
        EditField::HttpHeaderKey(row) => {
            draft.set_header(row, 0, value);
        }
        EditField::HttpHeaderValue(row) => {
            draft.set_header(row, 1, value);
        }
        EditField::HttpBody => draft.set_body(value),
        EditField::DbQuery => draft.set_query(value),
    }
    // Restore vim NORMAL on the (now hidden) field so the next Enter
    // doesn't land in stale Insert state.
    app.vim.enter_normal();
}

/// `r` in BLOCKS NAV: ensure the focused pane has the block's file
/// loaded into `pane.document`, park the cursor on that block's
/// segment, then delegate to the existing run pipeline. The pipeline
/// reads `app.document()` → our redirect returns `pane.document` (NAV
/// has no `block_edit`), so the executor / cache / refs machinery
/// works without forking.
pub(crate) fn run_focused_block(app: &mut App) {
    if !matches!(app.view, AppView::Blocks) {
        return;
    }
    // Sync sub-doc → draft → pane.document in memory WITHOUT closing
    // the EDIT buffer. The user expects `r` mid-edit to run the
    // current text and return them to keep typing, like a REPL.
    sync_edit_to_doc_in_memory(app);
    let Some(pane) = app.active_pane() else { return };
    let Some(sel) = pane.block_selected else {
        app.set_status(StatusKind::Error, "no block selected");
        return;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return;
    };
    let Some(file) = ws.index.files.get(sel.file_idx) else {
        return;
    };
    let Some(block) = file.blocks.get(sel.block_idx) else {
        return;
    };
    let file_path_rel = file.path.clone();
    let target_alias = block.alias.clone();
    let target_type = block.block_type.clone();
    let abs_path = app.vault_path.join(&file_path_rel);

    // Load (or reload) the file into the pane's document if it
    // doesn't already point at the bloc's file. Stale results from a
    // previous file would carry over otherwise.
    let needs_load = app
        .active_pane()
        .map(|p| p.document_path.as_deref() != Some(abs_path.as_path()))
        .unwrap_or(true);
    if needs_load {
        // Load + hydrate via the shared helper so the loaded document
        // carries any persisted `cached_result` for blocks above the
        // focused one — the BLOCKS view Response card paints
        // immediately and `{{alias.body.…}}` resolves on first use.
        let doc = match crate::document_loader::load_and_hydrate(
            &app.vault_path,
            &file_path_rel,
            app.pool_manager.app_pool(),
            &app.environments_store,
        ) {
            Ok(d) => d,
            Err(e) => {
                app.set_status(StatusKind::Error, format!("read failed: {e}"));
                return;
            }
        };
        if let Some(p) = app.active_pane_mut() {
            p.document = Some(doc);
            p.document_path = Some(abs_path);
        }
    }

    // Apply any committed-but-unsaved draft onto the (possibly just
    // reloaded) pane document so the run uses the edited values without
    // requiring Ctrl+S first — edit→run→decide-to-save is the flow.
    sync_draft_to_doc_in_memory(app);

    // Map the selected block to a segment_idx in the pane's doc.
    // Bypass the `app.document()` redirect — in EDIT it returns the
    // sub-doc (a prose-only field buffer with zero blocks), so the
    // block lookup would always miss.
    let segment_idx = {
        let Some(pane) = app.active_pane() else { return };
        let Some(doc) = pane.document.as_ref() else { return };
        let mut found = None;
        for (idx, seg) in doc.segments().iter().enumerate() {
            if let crate::buffer::Segment::Block(b) = seg {
                if b.block_type == target_type && b.alias == target_alias {
                    found = Some(idx);
                    break;
                }
            }
        }
        match found {
            Some(s) => s,
            None => {
                app.set_status(StatusKind::Error, "block not found in file");
                return;
            }
        }
    };

    // Park the cursor on the block in the pane's doc (same bypass —
    // we want apply_run_block to see InBlock on the real document,
    // not the sub-doc).
    if let Some(pane) = app.active_pane_mut() {
        if let Some(doc) = pane.document.as_mut() {
            doc.set_cursor(crate::buffer::Cursor::InBlock {
                segment_idx,
                offset: 0,
            });
        }
    }

    // Temporarily detach `block_edit` so the `app.document()` redirect
    // points at the real pane document (which has the block) during
    // the run. The executor reads `doc.segments()` directly to fetch
    // the block — restoring the edit buffer after lets the user keep
    // typing without observing any flicker.
    let detached = app.active_pane_mut().and_then(|p| p.block_edit.take());
    crate::commands::refs::start_run_chain(app, segment_idx);
    if let Some(edit) = detached {
        if let Some(pane) = app.active_pane_mut() {
            pane.block_edit = Some(edit);
        }
    }
}

/// Copy the open sub-doc's text into the draft + into the matching
/// segment of `pane.document`, **without** closing `block_edit`.
/// Used by `r` mid-EDIT so a re-run sees the user's in-flight text
/// while the EDIT buffer stays open (REPL-like).
pub(crate) fn sync_edit_to_doc_in_memory(app: &mut App) {
    let (field, text) = {
        let Some(pane) = app.active_pane() else { return };
        let Some(edit) = pane.block_edit.as_ref() else { return };
        (edit.field.clone(), edit.current_text())
    };
    // Update the draft so the next `Ctrl+S` writes the right text.
    if let Some(pane) = app.active_pane_mut() {
        if let Some(draft) = pane.block_draft.as_mut() {
            match &field {
                EditField::HttpUrl => draft.set_url(text.clone()),
                EditField::HttpHeaderKey(row) => {
                    draft.set_header(*row, 0, text.clone());
                }
                EditField::HttpHeaderValue(row) => {
                    draft.set_header(*row, 1, text.clone());
                }
                EditField::HttpBody => draft.set_body(text.clone()),
                EditField::DbQuery => draft.set_query(text.clone()),
            }
        }
    }
    // Mirror into pane.document so apply_run_block sees the new
    // text — it reads the segment's params.query / params.body
    // directly. The block's `raw` rope stays stale (no re-serialize
    // mid-edit) but the executor reads params first.
    let field_key = match &field {
        EditField::HttpUrl => "url",
        EditField::HttpHeaderKey(_) | EditField::HttpHeaderValue(_) => "headers",
        EditField::HttpBody => "body",
        EditField::DbQuery => "query",
    };
    // Find the matching segment in pane.document by (type, alias) —
    // same key apply_run_block uses to identify the block.
    let target = {
        let Some(ws) = app.blocks_workspace.as_ref() else { return };
        let Some(sel) = app.active_pane().and_then(|p| p.block_selected) else { return };
        let Some(file) = ws.index.files.get(sel.file_idx) else { return };
        let Some(meta) = file.blocks.get(sel.block_idx) else { return };
        (meta.block_type.clone(), meta.alias.clone())
    };
    let Some(pane) = app.active_pane_mut() else { return };
    let Some(doc) = pane.document.as_mut() else { return };
    let segment_idx = doc.segments().iter().position(|s| {
        matches!(s, crate::buffer::Segment::Block(b)
            if b.block_type == target.0 && b.alias == target.1)
    });
    let Some(idx) = segment_idx else { return };
    if let Some(b) = doc.block_at_mut(idx) {
        if let Some(obj) = b.params.as_object_mut() {
            obj.insert(
                field_key.to_string(),
                serde_json::Value::String(text),
            );
        }
    }
}

/// Mirror the committed-but-unsaved draft's params onto the matching
/// segment of `pane.document` so a run uses the edited request without
/// a save first (edit → run → decide-to-save). `cached_result` lives
/// on a separate field, so it survives. No-op without a draft.
pub(crate) fn sync_draft_to_doc_in_memory(app: &mut App) {
    let (bt, alias, params) = {
        let Some(pane) = app.active_pane() else { return };
        let Some(draft) = pane.block_draft.as_ref() else { return };
        (
            draft.block.block_type.clone(),
            draft.block.alias.clone(),
            draft.block.params.clone(),
        )
    };
    let Some(pane) = app.active_pane_mut() else { return };
    let Some(doc) = pane.document.as_mut() else { return };
    let idx = doc.segments().iter().position(|s| {
        matches!(s, crate::buffer::Segment::Block(b)
            if b.block_type == bt && b.alias == alias)
    });
    let Some(idx) = idx else { return };
    if let Some(b) = doc.block_at_mut(idx) {
        b.params = params;
    }
}

pub(crate) fn cancel_edit(app: &mut App) {
    if let Some(pane) = app.active_pane_mut() {
        pane.block_edit = None;
    }
    app.vim.enter_normal();
}

/// Map `(region, row, col)` for the focused pane to an `EditField`.
/// Response is read-only (no editable field); DB Connection (`[1]`) is
/// picker-driven (Story 9) so it has no inline buffer.
pub(crate) fn field_for_focus(app: &App) -> Option<EditField> {
    let pane = app.active_pane()?;
    let ws = app.blocks_workspace.as_ref()?;
    let target = pane.block_selected?;
    let file = ws.index.files.get(target.file_idx)?;
    let block = file.blocks.get(target.block_idx)?;
    if block.block_type == "http" {
        return match pane.block_region {
            0 => Some(EditField::HttpUrl),
            1 => Some(if pane.block_col == 0 {
                EditField::HttpHeaderKey(pane.block_row)
            } else {
                EditField::HttpHeaderValue(pane.block_row)
            }),
            2 => Some(EditField::HttpBody),
            _ => None,
        };
    }
    if block.block_type.starts_with("db") {
        return match pane.block_region {
            1 => Some(EditField::DbQuery),
            _ => None,
        };
    }
    None
}

/// Read the current value of `field` from the focused pane's draft.
/// Falls back to the empty string when the draft hasn't been hydrated
/// yet (every caller hydrates first, but the empty default keeps the
/// function total).
pub(crate) fn current_field_value(app: &App, field: &EditField) -> Option<String> {
    let pane = app.active_pane()?;
    let draft = pane.block_draft.as_ref()?;
    let out = match field {
        EditField::HttpUrl => draft.url().to_string(),
        EditField::HttpHeaderKey(row) => draft.header_at(*row, 0).to_string(),
        EditField::HttpHeaderValue(row) => draft.header_at(*row, 1).to_string(),
        EditField::HttpBody => draft.body().to_string(),
        EditField::DbQuery => draft.query().to_string(),
    };
    Some(out)
}

/// First-edit lazy: build a `BlockDraft` from the on-disk parse of
/// the focused block. Returns `false` when the block can't be located
/// (file deleted, line offset stale).
pub(crate) fn hydrate_draft(app: &mut App) -> bool {
    let Some(pane) = app.active_pane() else {
        return false;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return false;
    };
    let Some(target) = pane.block_selected else {
        return false;
    };
    let Some(file) = ws.index.files.get(target.file_idx) else {
        return false;
    };
    let Some(block) = file.blocks.get(target.block_idx) else {
        return false;
    };
    let Ok(text) = httui_core::fs::read_note(
        &app.vault_path.to_string_lossy(),
        &file.path.to_string_lossy(),
    ) else {
        return false;
    };
    let parsed = httui_core::blocks::parse_blocks(&text);
    let Some(found) = parsed
        .into_iter()
        .find(|p| p.line_start == block.line_start && p.block_type == block.block_type)
    else {
        return false;
    };
    let draft = BlockDraft {
        file_path: file.path.clone(),
        block_line_start: block.line_start,
        block: found,
    };
    if let Some(pane) = app.active_pane_mut() {
        pane.block_draft = Some(Box::new(draft));
        return true;
    }
    false
}
