use crate::app::{App, BlockRef};
use crate::config::Config;
use crate::input::action::Action;
use crate::input::apply::blocks_view::apply_blocks_view;
use crate::input::apply::tree_nav::apply_tree_nav;
use crate::vault::ResolvedVault;
use httui_core::db::init_db;
use tempfile::TempDir;

/// App in BLOCKS view with one HTTP block selected. Multi-thread
/// runtime: `App::new` uses `block_in_place`.
async fn blocks_app() -> (App, TempDir, TempDir) {
    let data = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    std::fs::write(
        vault.path().join("api.md"),
        "# api\n\n```http alias=login\nGET https://x.com\n```\n",
    )
    .unwrap();
    let pool = init_db(data.path()).await.unwrap();
    let resolved = ResolvedVault {
        vault: vault.path().to_path_buf(),
    };
    let mut app = App::new(Config::default(), resolved, pool);
    apply_blocks_view(&mut app, Action::ToggleAppView);
    if let Some(pane) = app.active_pane_mut() {
        pane.block_selected = Some(BlockRef {
            file_idx: 0,
            block_idx: 0,
        });
        pane.block_region = 0;
    }
    (app, data, vault)
}

#[tokio::test(flavor = "multi_thread")]
async fn visiting_body_is_remembered_when_focus_leaves() {
    let (mut app, _d, _v) = blocks_app().await;
    // Request band lands on Headers, Tab flips to Body.
    apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
    assert_eq!(app.active_pane().unwrap().block_region, 1);
    assert_eq!(app.active_pane().unwrap().block_req_tab, 0);
    apply_blocks_view(&mut app, Action::BlocksPaneNextRegion);
    assert_eq!(app.active_pane().unwrap().block_region, 2);
    assert_eq!(app.active_pane().unwrap().block_req_tab, 1);
    // Jump to the URL band — the Body memory must survive.
    apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(1));
    let pane = app.active_pane().unwrap();
    assert_eq!(pane.block_region, 0);
    assert_eq!(pane.block_req_tab, 1, "Body tab stays remembered");
}

#[tokio::test(flavor = "multi_thread")]
async fn re_entering_the_request_band_lands_on_the_remembered_tab() {
    let (mut app, _d, _v) = blocks_app().await;
    // Visit Body, leave for the URL band.
    apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
    apply_blocks_view(&mut app, Action::BlocksPaneNextRegion);
    assert_eq!(app.active_pane().unwrap().block_region, 2);
    apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(1));
    // Digit jump back into the Request band → Body, not Headers.
    apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
    assert_eq!(
        app.active_pane().unwrap().block_region,
        2,
        "digit re-entry lands on the remembered Body tab"
    );
    // Same via vertical band motion: leave upward to URL, walk back
    // down — still Body.
    apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(1));
    apply_blocks_view(&mut app, Action::BlocksPaneRowDown);
    assert_eq!(
        app.active_pane().unwrap().block_region,
        2,
        "band motion re-entry lands on the remembered Body tab"
    );
    // Headers memory works the same way in reverse.
    apply_blocks_view(&mut app, Action::BlocksPaneNextRegion);
    assert_eq!(app.active_pane().unwrap().block_region, 1);
    apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(1));
    apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
    assert_eq!(
        app.active_pane().unwrap().block_region,
        1,
        "Headers re-entry after visiting Headers"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn reactivating_the_open_block_keeps_region_and_tabs() {
    let (mut app, _d, _v) = blocks_app().await;
    apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
    apply_blocks_view(&mut app, Action::BlocksPaneNextRegion);
    assert_eq!(app.active_pane().unwrap().block_region, 2);
    let tabs_before = app.active_pane().unwrap().tab_count();
    // Walk the sidebar down to the open block's row. Expanding a
    // collapsed row needs a refresh to materialize its children —
    // same dance the TreeActivate handler does.
    let vault_path = app.vault_path.clone();
    app.tree.select_first();
    for _ in 0..50 {
        let on_block = app
            .tree
            .current()
            .map(|n| n.block.is_some())
            .unwrap_or(false);
        if on_block {
            break;
        }
        if app.tree.toggle_expand() {
            app.tree.refresh(&vault_path);
        }
        app.tree.select_next();
    }
    assert!(
        app.tree
            .current()
            .map(|n| n.block.is_some())
            .unwrap_or(false),
        "fixture: tree cursor must land on a block row"
    );
    apply_tree_nav(&mut app, Action::TreeActivate, true);
    let pane = app.active_pane().unwrap();
    assert_eq!(
        pane.block_region, 2,
        "region survives re-activating the same block"
    );
    assert_eq!(pane.tab_count(), tabs_before, "no duplicate tab stacked");
}
