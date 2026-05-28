//! Per-vault view + pane snapshot persistence in `user.toml`.
//! Restore is best-effort: missing blocks drop to "no selection".

use std::path::PathBuf;

use httui_core::vault_config::{
    BlockKey, BlockSelection, BlocksWorkspaceSnapshot, PaneLeafSnapshot, PaneSnapshot,
    PaneSplitSnapshot, SidebarPos, TuiViewState, UserStore,
};

use crate::app::{App, AppView, BlockIndex, BlockMeta, BlockRef, BlocksWorkspace};
use crate::pane::{Pane, PaneNode, SplitDir, TabState};

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
    let expanded_files: Vec<String> = ws
        .expanded
        .iter()
        .filter_map(|&fi| ws.index.files.get(fi))
        .map(|f| f.display.clone())
        .collect();
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
        PaneNode::Leaf(pane) => PaneSnapshot::Leaf(PaneLeafSnapshot {
            file: pane
                .document_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            block: block_selection_from_pane(pane.block_selected, index),
            region: pane.block_region as u32,
            row: pane.block_row as u32,
            col: pane.block_col as u32,
        }),
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

pub fn restore(app: &mut App, snap: &TuiViewState) {
    let target_view = if snap.last_view == VIEW_BLOCKS {
        AppView::Blocks
    } else {
        AppView::Doc
    };

    app.tree.visible = snap.sidebar_open;
    if matches!(target_view, AppView::Blocks) {
        let mut index = BlockIndex::build(&app.vault_path);
        let vault = app.vault_path.clone();
        let pool = app.pool_manager.app_pool().clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                crate::app::enrich_last_runs(&mut index, &vault, &pool).await;
            });
        });
        app.blocks_workspace = Some(BlocksWorkspace::new(index.clone()));
        app.tree.block_index = Some(index);
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
    let new_root = restore_pane(&snap_blocks.root, &vault, &ws.index);
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

fn restore_pane(snap: &PaneSnapshot, vault: &std::path::Path, index: &BlockIndex) -> PaneNode {
    match snap {
        PaneSnapshot::Leaf(leaf) => PaneNode::Leaf(restore_leaf(leaf, vault, index)),
        PaneSnapshot::Split(split) => {
            let direction = if split.direction == SPLIT_HORIZONTAL {
                SplitDir::Horizontal
            } else {
                SplitDir::Vertical
            };
            PaneNode::Split {
                direction,
                ratio: split.ratio.clamp(0.1, 0.9),
                first: Box::new(restore_pane(&split.first, vault, index)),
                second: Box::new(restore_pane(&split.second, vault, index)),
            }
        }
    }
}

fn restore_leaf(leaf: &PaneLeafSnapshot, vault: &std::path::Path, index: &BlockIndex) -> Pane {
    let mut pane = match leaf.file.as_deref() {
        Some(rel) => match crate::document_loader::load_document(vault, &PathBuf::from(rel)) {
            Ok(doc) => Pane::new(doc, PathBuf::from(rel)),
            Err(_) => Pane::empty(),
        },
        None => Pane::empty(),
    };
    pane.block_selected = leaf.block.as_ref().and_then(|sel| {
        let file_idx = index.files.iter().position(|f| f.display == sel.file)?;
        let file = index.files.get(file_idx)?;
        let block_idx = file.blocks.iter().position(|b| block_matches(b, &sel.key))?;
        Some(BlockRef {
            file_idx,
            block_idx,
        })
    });
    pane.block_region = leaf.region as usize;
    pane.block_row = leaf.row as usize;
    pane.block_col = leaf.col as usize;
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
mod tests {
    use super::*;
    use crate::app::{BlockIndex, BlocksWorkspace, FileBlocks};
    use crate::config::Config;
    use crate::pane::{Pane, SplitDir};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use std::path::PathBuf;
    use tempfile::TempDir;

    async fn app_with_blocks(body: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("api.md"), body).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn capture_records_doc_view_by_default() {
        let (app, _d, _v) = app_with_blocks("# api\n\n```http alias=login\nGET https://x.com\n```\n").await;
        let snap = capture(&app);
        assert_eq!(snap.last_view, "doc");
        assert!(snap.blocks.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn capture_records_blocks_view_with_pane_state() {
        let (mut app, _d, _v) =
            app_with_blocks("# api\n\n```http alias=login\nGET https://x.com\n```\n").await;
        app.view = AppView::Blocks;
        app.blocks_workspace = Some(BlocksWorkspace::new(BlockIndex::build(&app.vault_path)));
        if let Some(ws) = app.blocks_workspace.as_mut() {
            // Single-file vaults auto-expand, so row 1 is the first
            // block under the (only) file row.
            ws.cursor = 1;
            ws.activate();
        }
        if let Some(pane) = app
            .tabs
            .tabs
            .get_mut(app.tabs.active)
            .map(|t| t.active_leaf_mut())
        {
            pane.block_selected = Some(BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
            pane.block_region = 2;
        }
        let snap = capture(&app);
        assert_eq!(snap.last_view, "blocks");
        let blocks = snap.blocks.expect("blocks snapshot");
        assert_eq!(blocks.expanded_files, vec!["api.md".to_string()]);
        match blocks.root {
            PaneSnapshot::Leaf(leaf) => {
                assert_eq!(leaf.file.as_deref(), Some("api.md"));
                let sel = leaf.block.expect("block selection");
                assert_eq!(sel.file, "api.md");
                assert_eq!(sel.key.alias.as_deref(), Some("login"));
                assert_eq!(leaf.region, 2);
            }
            PaneSnapshot::Split(_) => panic!("expected leaf snapshot"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restore_sets_up_blocks_sidebar_and_tree_mode() {
        let (mut app, _d, _v) =
            app_with_blocks("# api\n\n```http alias=login\nGET https://x.com\n```\n").await;
        app.view = AppView::Blocks;
        app.blocks_workspace = Some(BlocksWorkspace::new(BlockIndex::build(&app.vault_path)));
        app.tree.visible = true;
        let snap = capture(&app);
        assert!(snap.sidebar_open);

        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: app.vault_path.clone(),
        };
        let mut app2 = App::new(Config::default(), resolved, pool);
        restore(&mut app2, &snap);

        assert!(matches!(app2.view, AppView::Blocks));
        assert!(app2.blocks_workspace.is_some());
        assert!(app2.tree.block_index.is_some());
        assert!(app2.tree.visible);
        // Restore lands on the pane, not the sidebar — Ctrl+W h/j/k/l
        // require a non-Tree vim mode.
        assert!(!matches!(app2.vim.mode, crate::vim::mode::Mode::Tree));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restore_round_trips_blocks_view() {
        let body = "# api\n\n```http alias=login\nGET https://x.com\n```\n";
        let (mut app, _d, _v) = app_with_blocks(body).await;
        app.view = AppView::Blocks;
        app.blocks_workspace = Some(BlocksWorkspace::new(BlockIndex::build(&app.vault_path)));
        if let Some(pane) = app
            .tabs
            .tabs
            .get_mut(app.tabs.active)
            .map(|t| t.active_leaf_mut())
        {
            pane.block_selected = Some(BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
            pane.block_region = 1;
        }
        let snap = capture(&app);

        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: app.vault_path.clone(),
        };
        let mut app2 = App::new(Config::default(), resolved, pool);
        restore(&mut app2, &snap);

        assert!(matches!(app2.view, AppView::Blocks));
        let ws = app2.blocks_workspace.as_ref().expect("workspace restored");
        assert!(ws.expanded.contains(&0));
        let pane = app2
            .tabs
            .tabs
            .get(app2.tabs.active)
            .map(|t| t.active_leaf())
            .expect("active leaf");
        assert_eq!(pane.block_region, 1);
        let sel = pane.block_selected.expect("block selected");
        assert_eq!(sel.block_idx, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restore_falls_back_when_block_alias_missing() {
        let (mut app, _d, _v) =
            app_with_blocks("# api\n\n```http alias=login\nGET https://x.com\n```\n").await;
        app.view = AppView::Blocks;
        app.blocks_workspace = Some(BlocksWorkspace::new(BlockIndex::build(&app.vault_path)));
        if let Some(pane) = app
            .tabs
            .tabs
            .get_mut(app.tabs.active)
            .map(|t| t.active_leaf_mut())
        {
            pane.block_selected = Some(BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
        }
        let mut snap = capture(&app);
        if let Some(b) = snap.blocks.as_mut() {
            if let PaneSnapshot::Leaf(leaf) = &mut b.root {
                if let Some(sel) = leaf.block.as_mut() {
                    sel.key.alias = Some("missing-alias".into());
                    sel.key.line_start = 9999;
                }
            }
        }

        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: app.vault_path.clone(),
        };
        let mut app2 = App::new(Config::default(), resolved, pool);
        restore(&mut app2, &snap);
        let pane = app2
            .tabs
            .tabs
            .get(app2.tabs.active)
            .map(|t| t.active_leaf())
            .expect("leaf");
        assert!(pane.block_selected.is_none());
        assert!(matches!(app2.view, AppView::Blocks));
    }

    #[test]
    fn sanitize_focus_collapses_to_leftmost_when_stale() {
        let root = PaneNode::Split {
            direction: SplitDir::Vertical,
            ratio: 0.5,
            first: Box::new(PaneNode::Leaf(Pane::empty())),
            second: Box::new(PaneNode::Leaf(Pane::empty())),
        };
        let cleaned = sanitize_focus(&root, &[0, 1, 1]);
        assert_eq!(cleaned, vec![0]);
    }

    #[test]
    fn sanitize_focus_keeps_valid_path() {
        let root = PaneNode::Split {
            direction: SplitDir::Vertical,
            ratio: 0.5,
            first: Box::new(PaneNode::Leaf(Pane::empty())),
            second: Box::new(PaneNode::Leaf(Pane::empty())),
        };
        assert_eq!(sanitize_focus(&root, &[1]), vec![1]);
    }

    #[test]
    fn block_matches_prefers_alias_when_present() {
        let meta = BlockMeta {
            alias: Some("login".into()),
            block_type: "http".into(),
            line_start: 5,
            last_run: None,
        };
        let alias_match = BlockKey {
            alias: Some("login".into()),
            line_start: 999,
        };
        assert!(block_matches(&meta, &alias_match));
        // Mismatching aliases never fall back to line_start — a
        // renamed block is treated as gone, not as an accidental
        // line-number coincidence.
        let alias_miss = BlockKey {
            alias: Some("other".into()),
            line_start: 5,
        };
        assert!(!block_matches(&meta, &alias_miss));
    }

    #[test]
    fn block_matches_falls_back_to_line_start_when_alias_missing() {
        let meta = BlockMeta {
            alias: None,
            block_type: "http".into(),
            line_start: 7,
            last_run: None,
        };
        assert!(block_matches(
            &meta,
            &BlockKey {
                alias: None,
                line_start: 7,
            }
        ));
        assert!(!block_matches(
            &meta,
            &BlockKey {
                alias: None,
                line_start: 8,
            }
        ));
    }

    #[test]
    fn pane_snapshot_split_round_trips_in_capture() {
        let mut tab = TabState::new(Pane::empty());
        tab.split(SplitDir::Vertical, Pane::empty());
        let snap = capture_pane(
            &tab.root,
            &BlockIndex {
                files: vec![FileBlocks {
                    path: PathBuf::from("api.md"),
                    display: "api.md".into(),
                    blocks: vec![],
                }],
            },
        );
        match snap {
            PaneSnapshot::Split(split) => {
                assert_eq!(split.direction, "vertical");
                assert!((split.ratio - 0.5).abs() < 1e-6);
            }
            PaneSnapshot::Leaf(_) => panic!("expected split"),
        }
    }
}
