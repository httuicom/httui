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

// ---- auto-pairs ----------------------------------------------------

/// App whose note holds a db block; cursor parked at the end of the
/// SQL body line (a code context for auto-pairing).
async fn app_in_block_body() -> (crate::app::App, TempDir, TempDir) {
    let data = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    std::fs::write(
        vault.path().join("note.md"),
        "```db-postgres alias=q connection=c\nSELECT 1 \n```\n",
    )
    .unwrap();
    let pool = init_db(data.path()).await.unwrap();
    let resolved = ResolvedVault {
        vault: vault.path().to_path_buf(),
    };
    let mut app = crate::app::App::new(Config::default(), resolved, pool);
    let (seg, off) = {
        let doc = app.document().expect("note loaded");
        let seg = doc
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .expect("block segment");
        let raw = match &doc.segments()[seg] {
            crate::buffer::Segment::Block(b) => b.raw.to_string(),
            _ => unreachable!(),
        };
        // After the trailing space of "SELECT 1 " — neutral context.
        (seg, raw.find("SELECT 1 ").unwrap() + "SELECT 1 ".len())
    };
    if let Some(doc) = app.document_mut() {
        doc.set_cursor(Cursor::InBlock {
            segment_idx: seg,
            offset: off,
        });
    }
    (app, data, vault)
}

fn block_raw(app: &crate::app::App) -> String {
    let doc = app.document().unwrap();
    doc.segments()
        .iter()
        .find_map(|s| match s {
            crate::buffer::Segment::Block(b) => Some(b.raw.to_string()),
            _ => None,
        })
        .expect("block still parsed")
}

#[tokio::test(flavor = "multi_thread")]
async fn open_brace_in_block_body_auto_closes() {
    let (mut app, _d, _v) = app_in_block_body().await;
    apply_action(&mut app, Action::InsertChar('{'), true);
    assert!(block_raw(&app).contains("{}"), "pair inserted");
    let doc = app.document().unwrap();
    assert_eq!(doc.char_before_cursor(), Some('{'), "caret between pair");
    assert_eq!(doc.char_at_cursor(), Some('}'));
}

#[tokio::test(flavor = "multi_thread")]
async fn typing_closer_over_the_pair_skips_instead_of_duplicating() {
    let (mut app, _d, _v) = app_in_block_body().await;
    apply_action(&mut app, Action::InsertChar('{'), true);
    apply_action(&mut app, Action::InsertChar('}'), true);
    let raw = block_raw(&app);
    assert_eq!(raw.matches('}').count(), 1, "no duplicate closer: {raw}");
    let doc = app.document().unwrap();
    assert_eq!(doc.char_before_cursor(), Some('}'), "caret stepped over");
}

#[tokio::test(flavor = "multi_thread")]
async fn backspace_inside_empty_pair_removes_both_halves() {
    let (mut app, _d, _v) = app_in_block_body().await;
    apply_action(&mut app, Action::InsertChar('('), true);
    apply_action(&mut app, Action::DeleteBackward, true);
    let raw = block_raw(&app);
    assert!(!raw.contains('('), "opener gone: {raw}");
    assert!(!raw.contains(')'), "closer gone too: {raw}");
}

#[tokio::test(flavor = "multi_thread")]
async fn prose_typing_never_pairs() {
    let (mut app, _d, _v) = app_with_long_line().await;
    prime_pane(&mut app, 0);
    apply_action(&mut app, Action::InsertChar('('), true);
    apply_action(&mut app, Action::InsertChar('\''), true);
    let doc = app.document().unwrap();
    let text = doc.to_markdown();
    assert!(!text.contains(')'), "prose must not auto-close: {text}");
    assert_eq!(text.matches('\'').count(), 1, "apostrophe stays single");
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_pairs_off_disables_pairing_in_code() {
    let (mut app, _d, _v) = app_in_block_body().await;
    app.config.editor.auto_pairs = false;
    apply_action(&mut app, Action::InsertChar('{'), true);
    let raw = block_raw(&app);
    assert!(!raw.contains('}'), "no closer with the toggle off: {raw}");
}
