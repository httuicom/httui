//! Per-vault view + pane snapshot persistence in `user.toml`.
//! Restore is best-effort: missing blocks drop to "no selection".

use std::path::PathBuf;

use httui_core::vault_config::{
    BlockKey, BlockSelection, BlockTabSnapshot, BlocksWorkspaceSnapshot, PaneLeafSnapshot,
    PaneSnapshot, PaneSplitSnapshot, SidebarPos, TuiViewState, UserStore,
};

use crate::app::{App, AppView, BlockIndex, BlockMeta, BlockRef, BlocksWorkspace};
use crate::pane::{Pane, PaneNode, SplitDir, TabState};
use crate::pane_tabs::BlockTab;

const VIEW_DOC: &str = "doc";
const VIEW_BLOCKS: &str = "blocks";
const SPLIT_VERTICAL: &str = "vertical";
const SPLIT_HORIZONTAL: &str = "horizontal";

pub fn capture(app: &App) -> TuiViewState {
    let last_view = match app.view {
        AppView::Doc => VIEW_DOC,
        AppView::Blocks => VIEW_BLOCKS,
    }
    .to_string();
    TuiViewState {
        last_view,
        sidebar_open: app.tree.visible,
        blocks: capture_blocks_workspace(app),
    }
}

fn capture_blocks_workspace(app: &App) -> Option<BlocksWorkspaceSnapshot> {
    let ws = app.blocks_workspace.as_ref()?;
    let tab = app.tabs.tabs.get(app.tabs.active)?;
    let mut expanded_files: Vec<String> = ws
        .expanded
        .iter()
        .filter_map(|&fi| ws.index.files.get(fi))
        .map(|f| f.display.clone())
        .collect();
    // The sidebar tree's expansion (directory AND file paths) rides
    // in the same list: on restore, entries matching an indexed file
    // feed `ws.expanded` and everything feeds `tree.expanded`, so old
    // snapshots (files only) stay valid.
    for path in &app.tree.expanded {
        if !expanded_files.contains(path) {
            expanded_files.push(path.clone());
        }
    }
    let cursor = sidebar_pos_for_cursor(ws);
    let root = capture_pane(&tab.root, &ws.index);
    Some(BlocksWorkspaceSnapshot {
        expanded_files,
        cursor,
        root,
        focused: tab.focused.clone(),
    })
}

fn sidebar_pos_for_cursor(ws: &BlocksWorkspace) -> Option<SidebarPos> {
    let row = ws.current_row()?;
    let file = ws.index.files.get(row.file_idx)?;
    let block = row.block_idx.and_then(|bi| {
        file.blocks.get(bi).map(|b| BlockKey {
            alias: b.alias.clone(),
            line_start: b.line_start as u32,
        })
    });
    Some(SidebarPos {
        file: file.display.clone(),
        block,
    })
}

fn capture_pane(node: &PaneNode, index: &BlockIndex) -> PaneSnapshot {
    match node {
        PaneNode::Leaf(pane) => {
            let tabs = capture_block_tabs(pane, index);
            // `tabs` always contains at least the active tab (we
            // synthesize it from the pane mirror). When there's only
            // one tab and no draft, the legacy single-mirror snapshot
            // is enough — emit an empty `tabs` so the diff against an
            // old snapshot stays minimal.
            let multi_tab = tabs.len() > 1;
            PaneSnapshot::Leaf(PaneLeafSnapshot {
                file: pane
                    .document_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
                block: block_selection_from_pane(pane.block_selected, index),
                region: pane.block_region as u32,
                row: pane.block_row as u32,
                col: pane.block_col as u32,
                tabs: if multi_tab { tabs } else { Vec::new() },
                active_tab: pane.block_tab_active as u32,
            })
        }
        PaneNode::Split {
            direction,
            ratio,
            first,
            second,
        } => PaneSnapshot::Split(PaneSplitSnapshot {
            direction: match direction {
                SplitDir::Vertical => SPLIT_VERTICAL.into(),
                SplitDir::Horizontal => SPLIT_HORIZONTAL.into(),
            },
            ratio: *ratio,
            first: Box::new(capture_pane(first, index)),
            second: Box::new(capture_pane(second, index)),
        }),
    }
}

fn block_selection_from_pane(
    selected: Option<BlockRef>,
    index: &BlockIndex,
) -> Option<BlockSelection> {
    let sel = selected?;
    let file = index.files.get(sel.file_idx)?;
    let block = file.blocks.get(sel.block_idx)?;
    Some(BlockSelection {
        file: file.display.clone(),
        key: BlockKey {
            alias: block.alias.clone(),
            line_start: block.line_start as u32,
        },
    })
}

/// Build the per-tab snapshot list in left-to-right strip order. The
/// pane mirror is the canonical source for the ACTIVE slot — inactive
/// slots come straight from `pane.block_tabs[i]` (which we know is the
/// truth for non-active entries by construction).
fn capture_block_tabs(pane: &Pane, index: &BlockIndex) -> Vec<BlockTabSnapshot> {
    (0..pane.tab_count())
        .map(|i| {
            if i == pane.block_tab_active {
                BlockTabSnapshot {
                    block: block_selection_from_pane(pane.block_selected, index),
                    region: pane.block_region as u32,
                    row: pane.block_row as u32,
                    col: pane.block_col as u32,
                }
            } else {
                let snap = pane.inactive_tab(i);
                let (sel, region, row, col) = match snap {
                    Some(t) => (t.block_selected, t.block_region, t.block_row, t.block_col),
                    None => (None, 0usize, 0usize, 1usize),
                };
                BlockTabSnapshot {
                    block: block_selection_from_pane(sel, index),
                    region: region as u32,
                    row: row as u32,
                    col: col as u32,
                }
            }
        })
        .collect()
}

/// Resolve a snapshot's `BlockSelection` back to an in-memory `BlockRef`
/// via the workspace index. Drops gracefully when the file/block no
/// longer exists (typical after vault edits between sessions).
fn block_ref_from_selection(sel: Option<&BlockSelection>, index: &BlockIndex) -> Option<BlockRef> {
    let sel = sel?;
    let file_idx = index.files.iter().position(|f| f.display == sel.file)?;
    let file = index.files.get(file_idx)?;
    let block_idx = file
        .blocks
        .iter()
        .position(|b| block_matches(b, &sel.key))?;
    Some(BlockRef {
        file_idx,
        block_idx,
    })
}

pub fn restore(app: &mut App, snap: &TuiViewState) {
    let target_view = if snap.last_view == VIEW_BLOCKS {
        AppView::Blocks
    } else {
        AppView::Doc
    };

    app.tree.visible = snap.sidebar_open;
    if matches!(target_view, AppView::Blocks) {
        let index = BlockIndex::build(&app.vault_path);
        app.blocks_workspace = Some(BlocksWorkspace::new(index.clone()));
        app.tree.block_index = Some(index);
        let vault = app.vault_path.clone();
        app.tree.refresh(&vault);
        // Land focus on the pane (not the sidebar) — Ctrl+W h/j/k/l
        // depend on a non-Tree vim mode. Ctrl+E toggles sidebar focus
        // when the user wants it.
    }
    app.view = target_view;

    let Some(snap_blocks) = snap.blocks.as_ref() else {
        return;
    };
    let Some(ws) = app.blocks_workspace.as_mut() else {
        return;
    };

    ws.expanded = snap_blocks
        .expanded_files
        .iter()
        .filter_map(|disp| ws.index.files.iter().position(|f| &f.display == disp))
        .collect();

    if let Some(pos) = snap_blocks.cursor.as_ref() {
        if let Some(idx) = sidebar_cursor_index(ws, pos) {
            ws.cursor = idx;
        }
    }

    let vault = app.vault_path.clone();
    let pool = app.pool_manager.app_pool().clone();
    let env_store = app.environments_store.clone();
    let new_root = restore_pane(&snap_blocks.root, &vault, &ws.index, &pool, &env_store);
    let focused = sanitize_focus(&new_root, &snap_blocks.focused);
    let focused_block = focused_leaf(&new_root, &focused).and_then(|p| p.block_selected);
    if let Some(target) = focused_block {
        ws.selected = Some(target);
    }

    match app.tabs.tabs.get_mut(app.tabs.active) {
        Some(tab) => {
            tab.root = new_root;
            tab.focused = focused;
        }
        None => {
            let mut tab = TabState::new(Pane::empty());
            tab.root = new_root;
            tab.focused = focused;
            app.tabs.tabs.push(tab);
            app.tabs.active = 0;
        }
    }

    // Re-open the sidebar tree exactly as the user left it: every
    // persisted path (directories and files alike) goes back into the
    // tree's expansion set, then a refresh rebuilds the visible rows.
    app.tree.expanded = snap_blocks.expanded_files.iter().cloned().collect();
    app.tree.refresh(&vault);
}

fn sidebar_cursor_index(ws: &BlocksWorkspace, pos: &SidebarPos) -> Option<usize> {
    let fi = ws.index.files.iter().position(|f| f.display == pos.file)?;
    let rows = ws.rows();
    rows.iter().position(|r| match (&pos.block, r.block_idx) {
        (None, None) => r.file_idx == fi,
        (Some(key), Some(bi)) => {
            r.file_idx == fi
                && ws
                    .index
                    .files
                    .get(fi)
                    .and_then(|f| f.blocks.get(bi))
                    .map(|m| block_matches(m, key))
                    .unwrap_or(false)
        }
        _ => false,
    })
}

fn block_matches(meta: &BlockMeta, key: &BlockKey) -> bool {
    // Prefer alias when both sides have it — survives line-number
    // Alias-only when both sides have it; otherwise line_start.
    match (meta.alias.as_ref(), key.alias.as_ref()) {
        (Some(a), Some(b)) => a == b,
        _ => meta.line_start as u32 == key.line_start,
    }
}

fn restore_pane(
    snap: &PaneSnapshot,
    vault: &std::path::Path,
    index: &BlockIndex,
    pool: &sqlx::sqlite::SqlitePool,
    env_store: &std::sync::Arc<httui_core::vault_config::EnvironmentsStore>,
) -> PaneNode {
    match snap {
        PaneSnapshot::Leaf(leaf) => {
            PaneNode::Leaf(restore_leaf(leaf, vault, index, pool, env_store))
        }
        PaneSnapshot::Split(split) => {
            let direction = if split.direction == SPLIT_HORIZONTAL {
                SplitDir::Horizontal
            } else {
                SplitDir::Vertical
            };
            PaneNode::Split {
                direction,
                ratio: split.ratio.clamp(0.1, 0.9),
                first: Box::new(restore_pane(&split.first, vault, index, pool, env_store)),
                second: Box::new(restore_pane(&split.second, vault, index, pool, env_store)),
            }
        }
    }
}

fn restore_leaf(
    leaf: &PaneLeafSnapshot,
    vault: &std::path::Path,
    index: &BlockIndex,
    pool: &sqlx::sqlite::SqlitePool,
    env_store: &std::sync::Arc<httui_core::vault_config::EnvironmentsStore>,
) -> Pane {
    let mut pane = match leaf.file.as_deref() {
        Some(rel) => match crate::document_loader::load_and_hydrate(
            vault,
            &PathBuf::from(rel),
            pool,
            env_store,
        ) {
            Ok(doc) => Pane::new(doc, PathBuf::from(rel)),
            Err(_) => Pane::empty(),
        },
        None => Pane::empty(),
    };
    pane.block_selected = block_ref_from_selection(leaf.block.as_ref(), index);
    pane.block_region = leaf.region as usize;
    pane.block_row = leaf.row as usize;
    pane.block_col = leaf.col as usize;
    // The request-tab memory isn't persisted — derive it from the
    // restored region so the card opens on the tab the focus implies.
    pane.note_req_tab();
    // Multi-tab restore: drop the default single-empty strip and
    // rebuild from the snapshot, then activate the persisted index.
    // Snapshots from old TUIs leave `tabs` empty — the mirror
    // assignments above already cover the single-tab case.
    if !leaf.tabs.is_empty() {
        pane.block_tabs.clear();
        for snap in &leaf.tabs {
            pane.block_tabs.push(BlockTab {
                document: None,
                document_path: None,
                viewport_top: 0,
                block_selected: block_ref_from_selection(snap.block.as_ref(), index),
                block_region: snap.region as usize,
                block_row: snap.row as usize,
                block_col: snap.col as usize,
                block_req_tab: if snap.region == 2 { 1 } else { 0 },
                block_edit: None,
                block_draft: None,
            });
        }
        // Clamp the active index so a malformed snapshot can't panic.
        let len = pane.block_tabs.len();
        let target_idx = (leaf.active_tab as usize).min(len.saturating_sub(1));
        pane.block_tab_active = target_idx;
        // Pull the active tab's snapshot into the mirror (the
        // restore loop above stuffed the active slot too; pull from
        // there so the mirror == active slot invariant holds).
        if let Some(active) = pane.block_tabs.get_mut(target_idx) {
            let pulled = std::mem::replace(active, BlockTab::empty());
            pane.block_selected = pulled.block_selected;
            pane.block_region = pulled.block_region;
            pane.block_row = pulled.block_row;
            pane.block_col = pulled.block_col;
            pane.block_req_tab = pulled.block_req_tab;
            pane.viewport_top = pulled.viewport_top;
        }
    }
    pane
}

fn focused_leaf<'a>(root: &'a PaneNode, focused: &[u8]) -> Option<&'a Pane> {
    let mut node = root;
    for &step in focused {
        match node {
            PaneNode::Leaf(_) => break,
            PaneNode::Split { first, second, .. } => {
                node = if step == 0 { first } else { second };
            }
        }
    }
    match node {
        PaneNode::Leaf(pane) => Some(pane),
        _ => None,
    }
}

// `active_leaf()` requires focus to land on a real leaf.
fn sanitize_focus(root: &PaneNode, focused: &[u8]) -> Vec<u8> {
    let mut prefix: Vec<u8> = Vec::new();
    let mut node = root;
    for &step in focused {
        match node {
            PaneNode::Leaf(_) => break,
            PaneNode::Split { first, second, .. } => {
                let go_first = step == 0;
                prefix.push(if go_first { 0 } else { 1 });
                node = if go_first { first } else { second };
            }
        }
    }
    loop {
        match node {
            PaneNode::Leaf(_) => return prefix,
            PaneNode::Split { first, .. } => {
                prefix.push(0);
                node = first;
            }
        }
    }
}

pub fn load_snapshot(store: &UserStore, vault_path: &std::path::Path) -> Option<TuiViewState> {
    let key = canonical_key(vault_path)?;
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.tui_view_state(&key))
    });
    result.ok().flatten()
}

pub fn save_snapshot(store: &UserStore, vault_path: &std::path::Path, snap: TuiViewState) {
    let Some(key) = canonical_key(vault_path) else {
        return;
    };
    let _ = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.set_tui_view_state(&key, snap))
    });
}

fn canonical_key(vault_path: &std::path::Path) -> Option<String> {
    let canonical = vault_path.canonicalize().ok()?;
    Some(canonical.to_string_lossy().into_owned())
}

#[cfg(test)]
#[path = "persist_tests.rs"]
mod tests;
