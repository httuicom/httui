use crate::buffer::Cursor;
use crate::config::Config;
use crate::input::action::Action;
use crate::input::dispatch::apply_action;
use crate::vault::ResolvedVault;
use httui_core::db::init_db;
use tempfile::TempDir;

/// App over a vault whose only note is a single 100-char line.
/// Multi-thread runtime for the same reason as the accessors fixture:
/// `App::new` uses `block_in_place`.
async fn app_with_long_line() -> (crate::app::App, TempDir, TempDir) {
    let data = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    std::fs::write(
        vault.path().join("note.md"),
        format!("{}\n", "x".repeat(100)),
    )
    .unwrap();
    let pool = init_db(data.path()).await.unwrap();
    let resolved = ResolvedVault {
        vault: vault.path().to_path_buf(),
    };
    let app = crate::app::App::new(Config::default(), resolved, pool);
    (app, data, vault)
}

fn prime_pane(app: &mut crate::app::App, offset: usize) {
    if let Some(p) = app.active_pane_mut() {
        p.viewport_width = 40;
        p.viewport_height = 10;
    }
    if let Some(doc) = app.document_mut() {
        doc.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset,
        });
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn insert_char_past_the_edge_pans_the_viewport() {
    let (mut app, _d, _v) = app_with_long_line().await;
    prime_pane(&mut app, 100);
    assert_eq!(app.active_pane().unwrap().viewport_left, 0);
    apply_action(&mut app, Action::InsertChar('z'), true);
    let left = app.active_pane().unwrap().viewport_left;
    assert!(left > 0, "typing past the pane edge must pan, left={left}");
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_backward_pans_back_toward_the_line_start() {
    let (mut app, _d, _v) = app_with_long_line().await;
    prime_pane(&mut app, 100);
    apply_action(&mut app, Action::InsertChar('z'), true);
    let panned = app.active_pane().unwrap().viewport_left;
    assert!(panned > 0);
    for _ in 0..80 {
        apply_action(&mut app, Action::DeleteBackward, true);
    }
    let left = app.active_pane().unwrap().viewport_left;
    assert!(left < panned, "deleting back must shrink the pan");
    // The caret stays inside the window after the shrink.
    let doc = app.document().unwrap();
    let Cursor::InProse { offset, .. } = doc.cursor() else {
        panic!("cursor stays in prose");
    };
    assert!((offset as u16).saturating_sub(left) < 40);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_forward_refreshes_the_viewport() {
    let (mut app, _d, _v) = app_with_long_line().await;
    prime_pane(&mut app, 100);
    apply_action(&mut app, Action::InsertChar('z'), true);
    // Force a stale pan far beyond the cursor, then DeleteForward —
    // the refresh must clamp the pan back onto the cursor.
    if let Some(p) = app.active_pane_mut() {
        p.viewport_left = 200;
    }
    apply_action(&mut app, Action::DeleteForward, true);
    let left = app.active_pane().unwrap().viewport_left;
    assert!(left < 200, "DeleteForward must re-clamp the pan");
}
