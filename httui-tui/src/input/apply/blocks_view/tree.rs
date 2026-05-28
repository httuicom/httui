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
    let used_aliases: std::collections::HashSet<String> = parsed
        .iter()
        .filter_map(|p| p.alias.clone())
        .collect();
    let mut idx = parsed.len() + 1;
    let alias = loop {
        let candidate = format!("untitled{idx}");
        if !used_aliases.contains(&candidate) {
            break candidate;
        }
        idx += 1;
    };
    let appended = if text.ends_with('\n') {
        format!(
            "{text}\n```http alias={alias}\nGET https://example.com\n```\n"
        )
    } else {
        format!(
            "{text}\n\n```http alias={alias}\nGET https://example.com\n```\n"
        )
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
pub(crate) fn tree_delete_block_confirmed(
    app: &mut App,
    rel_path: &str,
    block_idx: usize,
) {
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
    let insert_at = pane.block_row.saturating_add(1).min(draft.header_count());
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
            params.insert(
                "headers".to_string(),
                serde_json::Value::Array(Vec::new()),
            );
            params
                .get_mut("headers")
                .and_then(|v| v.as_array_mut())
                .expect("just inserted")
        }
    };
    arr.insert(insert_at, serde_json::json!({"key": "", "value": ""}));
    pane.block_row = insert_at;
    pane.block_col = 0;
}

/// `d`/`Delete` in HTTP `[2] Headers`: remove the focused row. Cursor
/// clamps to the row above (or 0 when the list went empty). No-op when
/// the headers array is empty.
pub(crate) fn delete_header_row(app: &mut App) {
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
