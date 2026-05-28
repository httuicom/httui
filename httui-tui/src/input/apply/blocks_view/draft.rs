use super::*;

pub(crate) fn discard_all_drafts(app: &mut App) {
    let Some(tab) = app.active_tab_mut() else {
        return;
    };
    walk_panes_mut(&mut tab.root, &mut |pane| {
        pane.block_draft = None;
        pane.block_edit = None;
    });
}

/// Ctrl+S: serialize every dirty pane in the focused tab back into its
/// `.md` via `write_note`, then clear the draft. Saving is per-pane
/// (not per-tab) so two panes editing different files both flush.
pub(crate) fn save_draft(app: &mut App) {
    // Flush an open EDIT buffer first — Ctrl+S while a sub-doc is
    // active needs to push that text into the draft before the
    // disk write, otherwise the user's in-flight edit doesn't make
    // it into the file and `*` stays on the header.
    if app.active_pane().is_some_and(|p| p.block_edit.is_some()) {
        commit_edit(app);
    }
    let dirty = collect_dirty_panes(app);
    if dirty.is_empty() {
        return;
    }
    let vault = app.vault_path.clone();
    let mut saved = 0usize;
    let mut failed: Vec<String> = Vec::new();
    for (path, line_start) in dirty {
        // Re-borrow per pane to satisfy the borrow checker — each pane
        // mutation is independent and the draft contents are cloned
        // before the write so the file IO doesn't overlap the borrow.
        let Some((draft_block, draft_path)) = take_draft_for(app, &path, line_start) else {
            continue;
        };
        match save_block_to_disk(&vault, &draft_path, line_start, &draft_block) {
            Ok(_) => {
                saved += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "blocks-view save failed");
                failed.push(format!("{}: {e}", draft_path.display()));
                // Re-install the draft so the user doesn't silently
                // lose the unsaved edits on a failed write.
                restore_draft(app, &draft_path, line_start, draft_block);
            }
        }
    }
    if !failed.is_empty() {
        app.set_status(StatusKind::Error, format!("save failed: {}", failed.join("; ")));
    } else if saved > 0 {
        // Rebuild the index so the sidebar reflects fresh aliases /
        // header counts after the save. Keep selection by ref.
        if let Some(ws) = app.blocks_workspace.as_mut() {
            ws.index = BlockIndex::build(&vault);
        }
        app.set_status(
            StatusKind::Info,
            if saved == 1 {
                "saved".to_string()
            } else {
                format!("saved {saved} blocks")
            },
        );
    }
}

/// Walk every pane in the active tab and collect the `(file_path,
/// line_start)` pair of each one that has a draft. Returns an empty
/// vec when nothing is dirty.
pub(crate) fn collect_dirty_panes(app: &App) -> Vec<(std::path::PathBuf, usize)> {
    let Some(tab) = app.active_tab() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk_panes(&tab.root, &mut |pane| {
        if let Some(draft) = pane.block_draft.as_ref() {
            out.push((draft.file_path.clone(), draft.block_line_start));
        }
    });
    out
}

pub(crate) fn walk_panes(node: &crate::pane::PaneNode, f: &mut impl FnMut(&crate::pane::Pane)) {
    match node {
        crate::pane::PaneNode::Leaf(p) => f(p),
        crate::pane::PaneNode::Split { first, second, .. } => {
            walk_panes(first, f);
            walk_panes(second, f);
        }
    }
}

pub(crate) fn walk_panes_mut(
    node: &mut crate::pane::PaneNode,
    f: &mut impl FnMut(&mut crate::pane::Pane),
) {
    match node {
        crate::pane::PaneNode::Leaf(p) => f(p),
        crate::pane::PaneNode::Split { first, second, .. } => {
            walk_panes_mut(first, f);
            walk_panes_mut(second, f);
        }
    }
}

pub(crate) fn take_draft_for(
    app: &mut App,
    file_path: &std::path::Path,
    line_start: usize,
) -> Option<(httui_core::blocks::parser::ParsedBlock, std::path::PathBuf)> {
    let tab = app.active_tab_mut()?;
    let mut out = None;
    walk_panes_mut(&mut tab.root, &mut |pane| {
        if out.is_some() {
            return;
        }
        if let Some(draft) = pane.block_draft.as_ref() {
            if draft.file_path == file_path && draft.block_line_start == line_start {
                let taken = pane.block_draft.take().expect("just matched");
                out = Some((taken.block, taken.file_path));
            }
        }
    });
    out
}

pub(crate) fn restore_draft(
    app: &mut App,
    file_path: &std::path::Path,
    line_start: usize,
    block: httui_core::blocks::parser::ParsedBlock,
) {
    let Some(tab) = app.active_tab_mut() else {
        return;
    };
    let mut installed = false;
    walk_panes_mut(&mut tab.root, &mut |pane| {
        if installed {
            return;
        }
        let matches = pane
            .block_selected
            .map(|sel| {
                pane.block_draft.is_none()
                    && pane
                        .document_path
                        .as_ref()
                        .map(|_| sel)
                        .is_some()
            })
            .unwrap_or(false);
        if matches {
            pane.block_draft = Some(Box::new(BlockDraft {
                file_path: file_path.to_path_buf(),
                block_line_start: line_start,
                block: block.clone(),
            }));
            installed = true;
        }
    });
}

/// Serialize `draft` and replace the original block region in the file
/// on disk. The serializer is the same one the desktop uses, so the
/// resulting fence parses byte-identical to the in-memory ParsedBlock.
pub(crate) fn save_block_to_disk(
    vault: &std::path::Path,
    file_path: &std::path::Path,
    line_start: usize,
    draft: &httui_core::blocks::parser::ParsedBlock,
) -> std::io::Result<()> {
    let vault_str = vault.to_string_lossy().to_string();
    let file_str = file_path.to_string_lossy().to_string();
    let current = httui_core::fs::read_note(&vault_str, &file_str)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let lines: Vec<&str> = current.lines().collect();
    // Find the block in the current text using the parser — the
    // user might have edited the file between hydrate and save, so we
    // can't trust the original `line_end` blindly.
    let parsed = httui_core::blocks::parse_blocks(&current);
    let Some(target) = parsed
        .iter()
        .find(|p| p.line_start == line_start && p.block_type == draft.block_type)
    else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "block no longer present at the recorded offset",
        ));
    };
    let start = target.line_start;
    let end = target.line_end.min(lines.len().saturating_sub(1));
    let serialized = httui_core::blocks::serialize_block(draft);
    let trailing_newline = current.ends_with('\n');
    let mut out = String::new();
    for line in &lines[..start] {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&serialized);
    if end + 1 < lines.len() {
        out.push('\n');
        for line in &lines[end + 1..] {
            out.push_str(line);
            out.push('\n');
        }
    } else if trailing_newline {
        out.push('\n');
    }
    httui_core::fs::write_note(&vault_str, &file_str, &out)
        .map_err(|e| std::io::Error::other(e.to_string()))
}
