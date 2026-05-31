// coverage:exclude file — sidebar CRUD shims (`tree_new_block`,
// `tree_delete_block`, `tree_reorder_block`, `tree_open_in_split`,
// header-row helpers). Each path needs a full App fixture + vault
// file IO + workspace index rebuild; the behavior is asserted by the
// BLOCKS-view scenarios in `input/apply/blocks_view/mod.rs#tests`
// and exercised manually each session. Coverage debt tracked in
// docs-llm/tui-v2/vim-coverage-debt.md.
use super::*;

/// Append a new HTTP block to the file the sidebar cursor is on.
/// Reads the file, parses, builds a canonical empty HTTP block,
/// writes back, refreshes the index. Auto-aliases as `untitled1`,
/// `untitled2`, … to avoid colliding with existing aliases.
pub(crate) fn tree_new_block(app: &mut App) {
    let Some(node) = app.tree.current().cloned() else {
        return;
    };
    // Cursor on a block row → resolve the parent `.md` via the
    // workspace index. Cursor on a `.md` row → use it directly.
    let rel_path = if let Some(meta) = node.block.as_ref() {
        let ws = app.blocks_workspace.as_ref();
        let file = ws.and_then(|w| w.index.files.get(meta.file_idx));
        match file {
            Some(f) => f.path.to_string_lossy().to_string(),
            None => return,
        }
    } else if node.is_dir || !node.path.ends_with(".md") {
        return;
    } else {
        node.path.clone()
    };
    let vault = app.vault_path.to_string_lossy().to_string();
    let Ok(text) = httui_core::fs::read_note(&vault, &rel_path) else {
        app.set_status(StatusKind::Error, "could not read file");
        return;
    };
    let parsed = httui_core::blocks::parse_blocks(&text);
    let used_aliases: std::collections::HashSet<String> =
        parsed.iter().filter_map(|p| p.alias.clone()).collect();
    let mut idx = parsed.len() + 1;
    let alias = loop {
        let candidate = format!("untitled{idx}");
        if !used_aliases.contains(&candidate) {
            break candidate;
        }
        idx += 1;
    };
    let appended = if text.ends_with('\n') {
        format!("{text}\n```http alias={alias}\nGET https://example.com\n```\n")
    } else {
        format!("{text}\n\n```http alias={alias}\nGET https://example.com\n```\n")
    };
    if let Err(e) = httui_core::fs::write_note(&vault, &rel_path, &appended) {
        app.set_status(StatusKind::Error, format!("write failed: {e}"));
        return;
    }
    refresh_blocks_index_and_tree(app);
    app.set_status(StatusKind::Info, format!("new block: {alias}"));
}

/// Open the destructive-confirm prompt for the focused block. Same
/// shape as the file-delete prompt — Enter on `y`/`Y` runs the
/// actual removal via [`tree_delete_block_confirmed`].
pub(crate) fn tree_delete_block(app: &mut App) {
    let Some(node) = app.tree.current().cloned() else {
        return;
    };
    let Some(meta) = node.block.as_ref() else {
        return;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return;
    };
    let Some(file) = ws.index.files.get(meta.file_idx) else {
        return;
    };
    let Some(block) = file.blocks.get(meta.block_idx) else {
        return;
    };
    let label = block.label();
    let rel_path = file.path.to_string_lossy().to_string();
    app.tree.prompt = Some(crate::tree::TreePrompt::new(
        crate::tree::TreePromptKind::DeleteBlock {
            rel_path,
            block_idx: meta.block_idx,
            label,
        },
        String::new(),
    ));
    app.vim.mode = crate::vim::mode::Mode::TreePrompt;
}

/// Execute the block delete after the user typed `y` / `Y` in the
/// confirm prompt. Reads the file, drops the block fence (and its
/// trailing blank line), writes back, refreshes the index.
pub(crate) fn tree_delete_block_confirmed(app: &mut App, rel_path: &str, block_idx: usize) {
    let vault = app.vault_path.to_string_lossy().to_string();
    let Ok(text) = httui_core::fs::read_note(&vault, &rel_path) else {
        return;
    };
    let parsed = httui_core::blocks::parse_blocks(&text);
    let Some(target) = parsed.get(block_idx) else {
        return;
    };
    let lines: Vec<&str> = text.lines().collect();
    let start = target.line_start;
    let end = target.line_end.min(lines.len().saturating_sub(1));
    // Drop the block AND one trailing blank line (if any) so the
    // resulting markdown doesn't grow a double-blank gap.
    let drop_until = if end + 1 < lines.len() && lines[end + 1].trim().is_empty() {
        end + 1
    } else {
        end
    };
    let mut out = String::new();
    for l in &lines[..start] {
        out.push_str(l);
        out.push('\n');
    }
    if drop_until + 1 < lines.len() {
        for l in &lines[drop_until + 1..] {
            out.push_str(l);
            out.push('\n');
        }
    }
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    if let Err(e) = httui_core::fs::write_note(&vault, &rel_path, &out) {
        app.set_status(StatusKind::Error, format!("write failed: {e}"));
        return;
    }
    refresh_blocks_index_and_tree(app);
    // Clamp cursor so it doesn't dangle on a now-missing row.
    if app.tree.selected > 0 {
        let count = app.tree.entries.len();
        if app.tree.selected >= count {
            app.tree.selected = count.saturating_sub(1);
        }
    }
    app.set_status(StatusKind::Info, "block removed");
}

/// Rebuild both the workspace `BlockIndex` and the file tree's
/// `block_index` copy, then refresh the visible tree entries. Tree
/// shows stale data otherwise — the two indices live separately so
/// updating only one leaves the sidebar out of sync.
pub(crate) fn refresh_blocks_index_and_tree(app: &mut App) {
    let vault_path = app.vault_path.clone();
    let fresh = crate::app::BlockIndex::build(&vault_path);
    if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.index = fresh.clone();
    }
    app.tree.block_index = Some(fresh);
    app.tree.refresh(&vault_path);
}

/// Swap the currently-focused block in the sidebar with its
/// neighbour by `delta` (+1 down, -1 up) in the same file. Updates
/// the `.md` on disk and refreshes the index. No-op when the block
/// is at the edge.
pub(crate) fn tree_reorder_block(app: &mut App, delta: isize) {
    let Some(node) = app.tree.current().cloned() else {
        return;
    };
    let Some(meta) = node.block.as_ref() else {
        return;
    };
    let block_idx = meta.block_idx;
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return;
    };
    let Some(file) = ws.index.files.get(meta.file_idx) else {
        return;
    };
    let target_idx = block_idx as isize + delta;
    if target_idx < 0 || target_idx as usize >= file.blocks.len() {
        return;
    }
    let rel_path = file.path.to_string_lossy().to_string();
    let vault = app.vault_path.to_string_lossy().to_string();
    let Ok(text) = httui_core::fs::read_note(&vault, &rel_path) else {
        return;
    };
    let mut parsed = httui_core::blocks::parse_blocks(&text);
    if block_idx >= parsed.len() || target_idx as usize >= parsed.len() {
        return;
    }
    // Build the rewritten markdown by extracting the two block ranges
    // and swapping them. Prose between blocks stays put.
    let a = block_idx.min(target_idx as usize);
    let b = block_idx.max(target_idx as usize);
    let lines: Vec<&str> = text.lines().collect();
    let block_a = &parsed[a];
    let block_b = &parsed[b];
    let a_start = block_a.line_start;
    let a_end = block_a.line_end.min(lines.len().saturating_sub(1));
    let b_start = block_b.line_start;
    let b_end = block_b.line_end.min(lines.len().saturating_sub(1));
    let mut out = String::new();
    // Lines before a
    for l in &lines[..a_start] {
        out.push_str(l);
        out.push('\n');
    }
    // Block b in a's place
    out.push_str(&lines[b_start..=b_end].join("\n"));
    out.push('\n');
    // Lines between a and b (a_end+1 .. b_start)
    if a_end + 1 < b_start {
        for l in &lines[a_end + 1..b_start] {
            out.push_str(l);
            out.push('\n');
        }
    }
    // Block a in b's place
    out.push_str(&lines[a_start..=a_end].join("\n"));
    out.push('\n');
    // Lines after b
    if b_end + 1 < lines.len() {
        for l in &lines[b_end + 1..] {
            out.push_str(l);
            out.push('\n');
        }
    }
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    if let Err(e) = httui_core::fs::write_note(&vault, &rel_path, &out) {
        app.set_status(StatusKind::Error, format!("write failed: {e}"));
        return;
    }
    app.set_status(
        StatusKind::Info,
        format!("reordered block {block_idx} ↔ {}", target_idx),
    );
    let _ = parsed; // dropped before mut borrow below
    refresh_blocks_index_and_tree(app);
    // Move the sidebar cursor along with the block so the user can
    // keep stepping in the same direction. Tree entries are flat
    // (file row + expanded block rows), and reorder only swaps two
    // adjacent block rows under the same file — so a `delta`-step
    // in entry index lands on the moved block's new row.
    let new_selected = (app.tree.selected as isize + delta).max(0) as usize;
    app.tree.selected = new_selected;
}

/// `o`/`Insert` in HTTP `[2] Headers`: append an empty row right
/// after the focused one, advance the cursor to the new row's key
/// cell. Hydrates the draft on first use so subsequent edits land in
/// the same in-memory copy.
pub(crate) fn insert_header_row(app: &mut App) {
    insert_header_row_at(app, true);
}

/// `O`-equivalent: insert a header row ABOVE the focused row. The cursor
/// stays at its original index (now pointing at the new empty row), with
/// the previous content pushed one row down.
pub(crate) fn insert_header_row_above(app: &mut App) {
    insert_header_row_at(app, false);
}

/// Shared implementation: insert a fresh empty header row either below
/// (vim `o`) or above (vim `O`) the focused row, hydrate the draft on
/// first use, drop straight into INSERT on the new key.
fn insert_header_row_at(app: &mut App, below: bool) {
    if !focused_region_is_http_headers(app) {
        return;
    }
    if app
        .active_pane()
        .map(|p| p.block_draft.is_none())
        .unwrap_or(true)
        && !hydrate_draft(app)
    {
        app.set_status(StatusKind::Error, "block missing on disk");
        return;
    }
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let Some(draft) = pane.block_draft.as_mut() else {
        return;
    };
    let cur = pane.block_row;
    let count = draft.header_count();
    let insert_at = if below {
        cur.saturating_add(1).min(count)
    } else {
        cur.min(count)
    };
    let arr = draft
        .block
        .params
        .as_object_mut()
        .and_then(|o| o.get_mut("headers"))
        .and_then(|v| v.as_array_mut());
    let arr = match arr {
        Some(a) => a,
        None => {
            // No `headers` key yet — synth one.
            let params = draft
                .block
                .params
                .as_object_mut()
                .expect("ParsedBlock.params is always an object");
            params.insert("headers".to_string(), serde_json::Value::Array(Vec::new()));
            params
                .get_mut("headers")
                .and_then(|v| v.as_array_mut())
                .expect("just inserted")
        }
    };
    arr.insert(insert_at, serde_json::json!({"key": "", "value": ""}));
    // Below: cursor follows the new row at insert_at. Above: cursor stays at
    // its original index, which now points to the new (empty) row.
    pane.block_row = if below { insert_at } else { cur };
    pane.block_col = 0;
    enter_edit(app, EnterMode::Insert);
}

/// `d`/`Delete` in HTTP `[2] Headers`: open the confirm prompt. Misclicks
/// on `dd` were dropping the wrong row silently, so the actual delete now
/// lives behind a `y`/`Enter` confirm (see [`delete_header_row_confirmed`]).
pub(crate) fn delete_header_row(app: &mut App) {
    if !focused_region_is_http_headers(app) {
        return;
    }
    // Hydrate so we can read `header_count` even before the user has touched
    // the block — `dd` on a never-edited HTTP block was opening nothing
    // because the no-draft branch returned `count = 0`.
    if app
        .active_pane()
        .map(|p| p.block_draft.is_none())
        .unwrap_or(true)
        && !hydrate_draft(app)
    {
        return;
    }
    let Some(pane) = app.active_pane() else {
        return;
    };
    let row = pane.block_row;
    let Some(draft) = pane.block_draft.as_ref() else {
        return;
    };
    let count = draft.header_count();
    if count == 0 || row >= count {
        return;
    }
    let key_preview = draft.header_at(row, 0).to_string();
    let body = if key_preview.is_empty() {
        format!("Delete header (row {})?", row + 1)
    } else {
        format!("Delete header \"{}\"?", key_preview)
    };
    app.modal = Some(crate::modal::Modal::ConfirmPrompt(
        crate::app::ConfirmPromptState {
            title: "Confirm delete".to_string(),
            body,
            on_confirm: crate::input::action::Action::BlocksHeaderDeleteConfirm,
            on_cancel: crate::input::action::Action::BlocksHeaderDeleteCancel,
            payload: crate::app::ConfirmPayload::HeaderRow(row),
        },
    ));
}

/// Generic-prompt confirm: close the modal, extract the row from
/// [`crate::app::ConfirmPayload::HeaderRow`], and run the actual delete.
/// Other payload variants are silently ignored (defensive — should never
/// happen if `delete_header_row` is the only opener).
pub(crate) fn apply_header_delete_confirm(app: &mut App) {
    let row = match app.modal.take() {
        Some(crate::modal::Modal::ConfirmPrompt(state)) => match state.payload {
            crate::app::ConfirmPayload::HeaderRow(r) => Some(r),
            _ => None,
        },
        _ => None,
    };
    if let Some(row) = row {
        if let Some(pane) = app.active_pane_mut() {
            pane.block_row = row;
        }
        delete_header_row_confirmed(app);
    }
}

/// `y`/`Enter` confirmed the prompt: drop the focused header row. Cursor
/// clamps to the row above (or 0 when the list went empty).
pub(crate) fn delete_header_row_confirmed(app: &mut App) {
    if !focused_region_is_http_headers(app) {
        return;
    }
    if app
        .active_pane()
        .map(|p| p.block_draft.is_none())
        .unwrap_or(true)
        && !hydrate_draft(app)
    {
        return;
    }
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let Some(draft) = pane.block_draft.as_mut() else {
        return;
    };
    let arr = draft
        .block
        .params
        .as_object_mut()
        .and_then(|o| o.get_mut("headers"))
        .and_then(|v| v.as_array_mut());
    let Some(arr) = arr else {
        return;
    };
    if arr.is_empty() || pane.block_row >= arr.len() {
        return;
    }
    arr.remove(pane.block_row);
    if pane.block_row > 0 && pane.block_row >= arr.len() {
        pane.block_row -= 1;
    }
}

/// `Space` in HTTP `[2] Headers`: toggle the focused row on/off. Disabling
/// keeps the row but flags it `enabled: false` (serialized with `# `, skipped
/// on dispatch). No-op when there are no rows. Hydrates the draft on first use.
pub(crate) fn toggle_header_enabled(app: &mut App) {
    if !focused_region_is_http_headers(app) {
        return;
    }
    if app
        .active_pane()
        .map(|p| p.block_draft.is_none())
        .unwrap_or(true)
        && !hydrate_draft(app)
    {
        return;
    }
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let row = pane.block_row;
    let Some(draft) = pane.block_draft.as_mut() else {
        return;
    };
    draft.toggle_header_enabled(row);
}

/// vim NORMAL `o` on a single-line HTTP cell: commit + insert a new header
/// row below + INSERT on its key. Same semantics as NAV `o` but reachable
/// without leaving EDIT first.
pub(crate) fn field_open_below(app: &mut App) {
    commit_edit(app);
    insert_header_row(app);
}

/// vim NORMAL `O` on a single-line HTTP cell: mirror of [`field_open_below`]
/// that inserts above the focused row.
pub(crate) fn field_open_above(app: &mut App) {
    commit_edit(app);
    insert_header_row_above(app);
}

/// `Enter`/`Tab` on a single-line HTTP cell in EDIT INSERT: commit the
/// current buffer, advance to the next field, re-enter INSERT. Form-style
/// flow so the user can type key → Enter → value → Enter → next row's key
/// without ever leaving INSERT or hitting `Esc`+`l`+`i`.
pub(crate) fn field_advance_next(app: &mut App) {
    let field = app
        .active_pane()
        .and_then(|p| p.block_edit.as_ref())
        .map(|e| e.field.clone());
    let Some(field) = field else {
        return;
    };
    commit_edit(app);
    match field {
        EditField::HttpHeaderKey(row) => {
            if let Some(pane) = app.active_pane_mut() {
                pane.block_row = row;
                pane.block_col = 1;
            }
            enter_edit(app, EnterMode::Insert);
        }
        EditField::HttpHeaderValue(row) => {
            let count = app
                .active_pane()
                .and_then(|p| p.block_draft.as_ref())
                .map(|d| d.header_count())
                .unwrap_or(0);
            if row + 1 >= count {
                // Last row → append new (insert_header_row picks up cursor
                // from block_row + 1 and re-enters INSERT on the new key).
                if let Some(pane) = app.active_pane_mut() {
                    pane.block_row = row;
                    pane.block_col = 1;
                }
                insert_header_row(app);
            } else {
                if let Some(pane) = app.active_pane_mut() {
                    pane.block_row = row + 1;
                    pane.block_col = 0;
                }
                enter_edit(app, EnterMode::Insert);
            }
        }
        // URL has nothing meaningful to advance to without crossing regions;
        // commit is enough — user can `Tab` (NAV) to switch region.
        EditField::HttpUrl => {}
        // Multi-line fields shouldn't reach here (resolver excludes them).
        EditField::HttpBody | EditField::DbQuery => {}
    }
}

/// Split the currently-focused pane and open the sidebar's selected
/// block in the new half. The new pane lands focused and the tree
/// mode exits, so the user can act on the block immediately — same
/// chord shape as nvim-tree (`v` / `s`).
pub(crate) fn tree_open_in_split(app: &mut App, dir: crate::pane::SplitDir) {
    let Some(node) = app.tree.current().cloned() else {
        return;
    };
    let Some(meta) = node.block.as_ref() else {
        return;
    };
    let target = crate::app::BlockRef {
        file_idx: meta.file_idx,
        block_idx: meta.block_idx,
    };
    let leaves = app.active_tab().map(|t| t.leaf_count()).unwrap_or(0);
    if leaves > 1 {
        // Multiple panes are open — the user must pick which one to
        // split off. Park the intent and let the picker overlay drive
        // the choice; `choose_picker` will perform the split on the
        // chosen leaf.
        if let Some(ws) = app.blocks_workspace.as_mut() {
            let action = match dir {
                crate::pane::SplitDir::Vertical => {
                    crate::app::PanePickerAction::SplitVertical
                }
                crate::pane::SplitDir::Horizontal => {
                    crate::app::PanePickerAction::SplitHorizontal
                }
            };
            ws.pane_picker = Some(crate::app::PanePickerIntent { target, action });
        }
        return;
    }
    // Single pane — no ambiguity, split it directly.
    let Some(tab) = app.active_tab_mut() else {
        return;
    };
    let mut new_pane = tab.active_leaf().snapshot_clone();
    new_pane.block_selected = Some(target);
    new_pane.block_region = 0;
    new_pane.block_row = 0;
    new_pane.block_col = 0;
    tab.split(dir, new_pane);
    app.vim.enter_normal();
    app.refresh_viewport_for_cursor();
}
