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
    let (app, _d, _v) =
        app_with_blocks("# api\n\n```http alias=login\nGET https://x.com\n```\n").await;
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
async fn restore_round_trips_sidebar_tree_expansion() {
    let (mut app, _d, _v) =
        app_with_blocks("# api\n\n```http alias=login\nGET https://x.com\n```\n").await;
    // A second note inside a subdirectory so the sidebar tree has
    // a real directory node to expand.
    std::fs::create_dir_all(app.vault_path.join("sub")).unwrap();
    std::fs::write(
        app.vault_path.join("sub/inner.md"),
        "```http alias=ping\nGET https://y.com\n```\n",
    )
    .unwrap();
    app.view = AppView::Blocks;
    let index = BlockIndex::build(&app.vault_path);
    app.blocks_workspace = Some(BlocksWorkspace::new(index.clone()));
    app.tree.block_index = Some(index);
    app.tree.visible = true;
    // User opened the dir and one file before quitting.
    app.tree.expanded.insert("sub".to_string());
    app.tree.expanded.insert("sub/inner.md".to_string());
    let snap = capture(&app);
    let persisted = &snap.blocks.as_ref().unwrap().expanded_files;
    assert!(persisted.contains(&"sub".to_string()), "{persisted:?}");
    assert!(
        persisted.contains(&"sub/inner.md".to_string()),
        "{persisted:?}"
    );

    let data = TempDir::new().unwrap();
    let pool = init_db(data.path()).await.unwrap();
    let resolved = ResolvedVault {
        vault: app.vault_path.clone(),
    };
    let mut app2 = App::new(Config::default(), resolved, pool);
    restore(&mut app2, &snap);

    assert!(app2.tree.expanded.contains("sub"), "dir stays open");
    assert!(app2.tree.expanded.contains("sub/inner.md"));
    // The rebuilt rows actually show the re-opened hierarchy.
    let names: Vec<&str> = app2.tree.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"sub"), "rows: {names:?}");
    assert!(names.contains(&"inner.md"), "rows: {names:?}");
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
async fn capture_and_restore_multi_tab_pane_strip() {
    // Two blocks open as tabs in the same pane. After capture +
    // restore in a fresh App, the strip should rebuild with the
    // same tabs in the same order and the persisted active tab
    // still active.
    let body = "# api\n\n\
        ```http alias=ping\nGET https://x.com\n```\n\n\
        ```http alias=save\nPOST https://x.com\n```\n";
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
        let new_tab = BlockTab {
            block_selected: Some(BlockRef {
                file_idx: 0,
                block_idx: 1,
            }),
            block_region: 3,
            block_row: 0,
            block_col: 1,
            ..BlockTab::empty()
        };
        pane.push_block_tab(new_tab);
    }
    let snap = capture(&app);

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
        .expect("active leaf");
    assert_eq!(pane.tab_count(), 2, "tab strip restored");
    assert_eq!(pane.block_tab_active, 1, "active tab persisted");
    let active_sel = pane.block_selected.expect("active mirror block");
    assert_eq!(active_sel.block_idx, 1);
    // The inactive tab carries the original `ping` selection.
    let inactive = pane.inactive_tab(0).expect("inactive slot 0");
    assert_eq!(inactive.block_selected.map(|b| b.block_idx), Some(0));
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
