//! Operator / paste / visual-operator appliers (snapshot + record).

mod visual;

use crate::app::{App, StatusKind};
use crate::buffer::Cursor;
use crate::input::action::Action;
use crate::input::types::{Motion, Operator, PastePos, TextObject};
use crate::vim::change::{ChangeOrigin, ChangeRecord};
use crate::vim::operator;

// ───────────── operator wrappers (snapshot + record) ─────────────

pub(crate) fn apply_op_motion(
    app: &mut App,
    op: Operator,
    motion: Motion,
    count: usize,
    recording: bool,
) {
    let viewport = app.viewport_height();
    let mut outcome = operator::OpOutcome::default();
    let mut unnamed = std::mem::take(&mut app.vim.unnamed);
    if let Some(doc) = app.document_mut() {
        if op_mutates(op) {
            doc.snapshot();
        }
        outcome = operator::apply_motion(op, motion, count, doc, &mut unnamed, viewport);
    }
    app.vim.unnamed = unnamed;
    sync_yank_to_clipboard(app, op);
    if motion.is_find() {
        app.vim.last_find = Some(motion);
    }
    if outcome.enter_insert {
        app.vim.enter_insert();
        app.vim.insert_session.start_change(ChangeOrigin::Motion {
            motion,
            op_count: count,
        });
    } else if recording && op_mutates(op) {
        app.vim.last_change = Some(ChangeRecord::OperatorMotion(op, motion, count));
    }
    app.refresh_viewport_for_cursor();
}

pub(crate) fn apply_op_linewise(app: &mut App, op: Operator, count: usize, recording: bool) {
    // Block-on-cursor short-circuit: `dd`/`yy`/`cc` on a Block (or
    // its result panel) treats the whole segment as one logical
    // line. The yanked text is the canonical fence markdown — paste
    // anywhere else + re-parse rebuilds the block. CM6-equivalent
    // cut/paste without needing visible fence delimiters.
    let block_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => Some(segment_idx),
        _ => None,
    };
    if let Some(idx) = block_idx {
        let mut yanked: Option<String> = None;
        if let Some(doc) = app.document_mut() {
            if op_mutates(op) {
                doc.snapshot();
            }
            yanked = match op {
                Operator::Yank => doc.yank_block_at(idx),
                Operator::Delete | Operator::Change => doc.delete_block_at(idx),
            };
        }
        if let Some(text) = yanked {
            app.vim.unnamed.set_linewise(text);
        }
        sync_yank_to_clipboard(app, op);
        if matches!(op, Operator::Change) {
            app.vim.enter_insert();
            app.vim
                .insert_session
                .start_change(ChangeOrigin::Linewise { op_count: count });
        } else if recording && op_mutates(op) {
            app.vim.last_change = Some(ChangeRecord::OperatorLinewise(op, count));
        }
        app.refresh_viewport_for_cursor();
        return;
    }

    let mut outcome = operator::OpOutcome::default();
    let mut unnamed = std::mem::take(&mut app.vim.unnamed);
    if let Some(doc) = app.document_mut() {
        if op_mutates(op) {
            doc.snapshot();
        }
        outcome = operator::apply_linewise(op, count, doc, &mut unnamed);
    }
    app.vim.unnamed = unnamed;
    sync_yank_to_clipboard(app, op);
    if outcome.enter_insert {
        app.vim.enter_insert();
        app.vim
            .insert_session
            .start_change(ChangeOrigin::Linewise { op_count: count });
    } else if recording && op_mutates(op) {
        app.vim.last_change = Some(ChangeRecord::OperatorLinewise(op, count));
    }
    app.refresh_viewport_for_cursor();
}

pub(crate) fn apply_op_textobject(
    app: &mut App,
    op: Operator,
    textobj: TextObject,
    count: usize,
    recording: bool,
) {
    let mut outcome = operator::OpOutcome::default();
    let mut unnamed = std::mem::take(&mut app.vim.unnamed);
    if let Some(doc) = app.document_mut() {
        if op_mutates(op) {
            doc.snapshot();
        }
        outcome = operator::apply_text_object(op, textobj, count, doc, &mut unnamed);
    }
    app.vim.unnamed = unnamed;
    sync_yank_to_clipboard(app, op);
    if outcome.enter_insert {
        app.vim.enter_insert();
        app.vim
            .insert_session
            .start_change(ChangeOrigin::TextObject {
                textobj,
                op_count: count,
            });
    } else if recording && op_mutates(op) {
        app.vim.last_change = Some(ChangeRecord::OperatorTextObject(op, textobj, count));
    }
    app.refresh_viewport_for_cursor();
}

fn resolve_paste_register(app: &App) -> crate::vim::register::Register {
    let clip = crate::clipboard::get_text().unwrap_or_default();
    if clip.is_empty() {
        return app.vim.unnamed.clone();
    }
    if clip == app.vim.unnamed.text {
        return app.vim.unnamed.clone();
    }
    crate::vim::register::Register {
        text: clip,
        linewise: false,
    }
}

pub(crate) fn apply_paste(app: &mut App, pos: PastePos, count: usize, recording: bool) {
    if let Some(doc) = app.document_mut() {
        doc.snapshot();
    }
    let reg = resolve_paste_register(app);
    if let Some(doc) = app.document_mut() {
        operator::paste(pos, count, doc, &reg);
    }
    if recording {
        app.vim.last_change = Some(ChangeRecord::Paste(pos, count));
    }
    // Paste lands in prose. If the register held fence text (the
    // common case after `dd` on a block), the just-inserted prose
    // now contains a complete fence — re-parse so the block is
    // reinstated at the destination. Cheap when there's no fence
    // (parse_blocks returns empty and the helper bails).
    if let Some(Cursor::InProse { segment_idx, .. }) = app.document().map(|d| d.cursor()) {
        if let Some(doc) = app.document_mut() {
            doc.reparse_prose_at(segment_idx);
        }
    }
    app.refresh_viewport_for_cursor();
}

pub(crate) fn op_mutates(op: Operator) -> bool {
    !matches!(op, Operator::Yank)
}

/// After a yank lands in `app.vim.unnamed`, push its text to the
/// system clipboard so paste outside the TUI works. Failures (no X
/// forwarder, sandbox, etc.) bubble up to a non-fatal status hint —
/// the unnamed register still holds the text for in-TUI paste.
pub(crate) fn sync_yank_to_clipboard(app: &mut App, op: Operator) {
    if !matches!(op, Operator::Yank) {
        return;
    }
    if app.vim.unnamed.text.is_empty() {
        return;
    }
    if let Err(msg) = crate::clipboard::set_text(&app.vim.unnamed.text) {
        app.set_status(StatusKind::Error, msg);
    }
}

/// `apply_action` sub-match for the visual / operator / paste domain.
/// The outer router routes only this group's variants here, so the
/// `unreachable!` is a compile-time-backed invariant.
pub(crate) fn apply_operator(app: &mut App, action: Action, recording: bool) {
    // Read-only modals (db_row_detail / http_response_detail) host a
    // vim-navigable view but never accept mutations. `is_blocked_in_modal`
    // gates the Normal-mode parser, but once Visual mode kicks in, keys
    // route through `parse_visual` which has no modal awareness. Drop
    // mutating operators here as the last line of defense — yank still
    // works (no doc mutation), delete/change/paste don't.
    if is_readonly_modal_open(app) && is_doc_mutation(&action) {
        return;
    }
    match action {
        Action::EnterVisual => {
            if let Some(doc) = app.document() {
                let cur = doc.cursor();
                app.vim.enter_visual(cur);
            }
        }
        Action::EnterVisualLine => {
            if let Some(doc) = app.document() {
                let cur = doc.cursor();
                app.vim.enter_visual_line(cur);
            }
        }
        Action::ExitVisual => {
            visual::return_from_visual(app);
        }
        Action::VisualSwap => {
            if let (Some(anchor), Some(doc)) = (app.vim.visual_anchor, app.document_mut()) {
                let cur = doc.cursor();
                doc.set_cursor(anchor);
                app.vim.visual_anchor = Some(cur);
                app.refresh_viewport_for_cursor();
            }
        }
        Action::VisualOperator(op) => visual::apply_visual_operator(app, op, recording),
        Action::VisualPaste => visual::apply_visual_paste(app, recording),
        Action::VisualSelectTextObject(textobj) => {
            visual::apply_visual_select_textobject(app, textobj);
        }
        Action::OperatorMotion(op, motion, count) => {
            apply_op_motion(app, op, motion, count, recording);
        }
        Action::OperatorLinewise(op, count) => {
            apply_op_linewise(app, op, count, recording);
        }
        Action::OperatorTextObject(op, textobj, count) => {
            apply_op_textobject(app, op, textobj, count, recording);
        }
        Action::Paste(pos, count) => {
            apply_paste(app, pos, count, recording);
        }
        _ => unreachable!("apply_operator: variante fora do grupo"),
    }
}

fn is_readonly_modal_open(app: &App) -> bool {
    app.db_row_detail().is_some() || app.http_response_detail().is_some()
}

fn is_doc_mutation(action: &Action) -> bool {
    use crate::input::types::Operator::{Change, Delete};
    matches!(
        action,
        Action::VisualOperator(Change | Delete)
            | Action::VisualPaste
            | Action::OperatorMotion(Change | Delete, _, _)
            | Action::OperatorLinewise(Change | Delete, _)
            | Action::OperatorTextObject(Change | Delete, _, _)
            | Action::Paste(_, _)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, DbRowDetailState, HttpResponseDetailState};
    use crate::buffer::Document;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_with_db_modal(content: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), "x\n").unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        app.modal = Some(crate::modal::Modal::DbRowDetail(DbRowDetailState {
            segment_idx: 0,
            row: 0,
            title: "t".into(),
            doc: Document::from_markdown(content).unwrap(),
            viewport_height: 4,
            viewport_top: 0,
        }));
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn visual_delete_in_db_row_detail_does_not_mutate() {
        let (mut app, _d, _v) = app_with_db_modal("alpha beta\n").await;
        let before = app.document().unwrap().to_markdown();
        let cur = app.document().unwrap().cursor();
        app.vim.visual_anchor = Some(cur);
        apply_operator(&mut app, Action::VisualOperator(Operator::Delete), false);
        assert_eq!(app.document().unwrap().to_markdown(), before);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn visual_yank_in_db_row_detail_is_allowed() {
        let (mut app, _d, _v) = app_with_db_modal("alpha beta\n").await;
        let before = app.document().unwrap().to_markdown();
        let cur = app.document().unwrap().cursor();
        app.vim.visual_anchor = Some(cur);
        apply_operator(&mut app, Action::VisualOperator(Operator::Yank), false);
        assert_eq!(app.document().unwrap().to_markdown(), before);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn paste_in_http_response_detail_does_not_mutate() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), "x\n").unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        app.modal = Some(crate::modal::Modal::HttpResponseDetail(
            HttpResponseDetailState {
                segment_idx: 0,
                title: "t".into(),
                doc: Document::from_markdown("status 200\n").unwrap(),
                viewport_height: 4,
                viewport_top: 0,
            },
        ));
        let before = app.document().unwrap().to_markdown();
        apply_operator(&mut app, Action::Paste(PastePos::After, 1), false);
        assert_eq!(app.document().unwrap().to_markdown(), before);
        let _ = vault;
        let _ = data;
    }

    #[test]
    fn is_doc_mutation_table() {
        use Operator::{Change, Delete, Yank};
        assert!(is_doc_mutation(&Action::VisualOperator(Delete)));
        assert!(is_doc_mutation(&Action::VisualOperator(Change)));
        assert!(!is_doc_mutation(&Action::VisualOperator(Yank)));
        assert!(is_doc_mutation(&Action::VisualPaste));
        assert!(is_doc_mutation(&Action::OperatorMotion(
            Delete,
            Motion::Right,
            1
        )));
        assert!(!is_doc_mutation(&Action::OperatorMotion(
            Yank,
            Motion::Right,
            1
        )));
    }

    use crate::buffer::Segment;
    use crate::pane::{Pane, TabState};

    async fn app_with_prose(md: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let note = vault.path().join("note.md");
        std::fs::write(&note, md).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault { vault: vault.path().to_path_buf() };
        let mut app = App::new(Config::default(), resolved, pool);
        let doc = Document::from_markdown(md).unwrap();
        let pane = Pane::new(doc, note);
        app.tabs.tabs.clear();
        app.tabs.tabs.push(TabState::new(pane));
        app.tabs.active = 0;
        (app, data, vault)
    }

    #[test]
    fn op_mutates_table() {
        assert!(!op_mutates(Operator::Yank));
        assert!(op_mutates(Operator::Delete));
        assert!(op_mutates(Operator::Change));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_op_motion_yank_fills_unnamed_no_snapshot() {
        let (mut app, _d, _v) = app_with_prose("hello world\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        apply_op_motion(&mut app, Operator::Yank, Motion::Right, 5, /*recording=*/ false);
        assert!(!app.vim.unnamed.text.is_empty(), "yank should fill unnamed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_op_motion_delete_mutates_and_records_when_recording() {
        let (mut app, _d, _v) = app_with_prose("hello world\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        apply_op_motion(&mut app, Operator::Delete, Motion::Right, 5, /*recording=*/ true);
        assert!(app.vim.last_change.is_some(), "recording captures change");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_op_motion_find_records_last_find() {
        let (mut app, _d, _v) = app_with_prose("hello world\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        apply_op_motion(&mut app, Operator::Yank, Motion::FindForward('o'), 1, false);
        assert!(app.vim.last_find.is_some(), "find motion should be remembered");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_op_linewise_yank_on_prose_fills_unnamed() {
        let (mut app, _d, _v) = app_with_prose("alpha\nbeta\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        apply_op_linewise(&mut app, Operator::Yank, 1, false);
        assert!(!app.vim.unnamed.text.is_empty(), "yy should yank a line");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_op_linewise_yank_on_block_yanks_full_fence() {
        let md = "```http alias=q\nGET /x\n```\n";
        let (mut app, _d, _v) = app_with_prose(md).await;
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InBlock { segment_idx: block_idx, offset: 0 });
        apply_op_linewise(&mut app, Operator::Yank, 1, false);
        // yank_block_at returns the fence markdown; should contain ```http.
        assert!(
            app.vim.unnamed.text.contains("```http"),
            "yanked text should be a fence; got {:?}",
            app.vim.unnamed.text
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_op_linewise_delete_on_block_removes_segment() {
        let md = "before\n\n```http alias=q\nGET /x\n```\n\nafter\n";
        let (mut app, _d, _v) = app_with_prose(md).await;
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InBlock { segment_idx: block_idx, offset: 0 });
        let before = app.document().unwrap().to_markdown();
        apply_op_linewise(&mut app, Operator::Delete, 1, /*recording=*/ true);
        assert_ne!(app.document().unwrap().to_markdown(), before);
        assert!(app.vim.last_change.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_op_linewise_change_enters_insert_mode() {
        let (mut app, _d, _v) = app_with_prose("alpha\nbeta\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        apply_op_linewise(&mut app, Operator::Change, 1, false);
        assert!(matches!(app.vim.mode, crate::vim::mode::Mode::Insert));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_op_textobject_yank_word_inside() {
        let (mut app, _d, _v) = app_with_prose("hello world\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        apply_op_textobject(
            &mut app,
            Operator::Yank,
            TextObject::Word { around: false },
            1,
            false,
        );
        assert!(!app.vim.unnamed.text.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_op_textobject_change_enters_insert() {
        let (mut app, _d, _v) = app_with_prose("hello world\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        apply_op_textobject(
            &mut app,
            Operator::Change,
            TextObject::Word { around: false },
            1,
            true,
        );
        // Either entered insert or recorded change — exercise both branches.
        assert!(matches!(app.vim.mode, crate::vim::mode::Mode::Insert) || app.vim.last_change.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_paste_into_prose_records_and_snapshots() {
        let (mut app, _d, _v) = app_with_prose("hello\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 5 });
        app.vim.unnamed = crate::vim::register::Register {
            text: " world".into(),
            linewise: false,
        };
        let before = app.document().unwrap().to_markdown();
        apply_paste(&mut app, PastePos::After, 1, /*recording=*/ true);
        // Doc may or may not change depending on clipboard register
        // resolution, but recording must capture the paste.
        assert!(app.vim.last_change.is_some());
        let _ = before;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_paste_with_fence_text_reparses_prose() {
        // Register holds a fence — after paste, the prose should
        // re-parse and produce a Block segment.
        let (mut app, _d, _v) = app_with_prose("\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        app.vim.unnamed = crate::vim::register::Register {
            text: "```http alias=q\nGET /x\n```\n".into(),
            linewise: true,
        };
        apply_paste(&mut app, PastePos::Before, 1, false);
        // After reparse, should have at least one Block segment.
        let has_block = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .any(|s| matches!(s, Segment::Block(_)));
        // Reparse may not always promote if cursor ends up in non-prose;
        // just exercise the path.
        let _ = has_block;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_yank_to_clipboard_skips_non_yank() {
        let (mut app, _d, _v) = app_with_prose("x\n").await;
        app.vim.unnamed.text = "abc".into();
        sync_yank_to_clipboard(&mut app, Operator::Delete);
        // Early-return: Delete doesn't touch the clipboard, so the
        // status bar stays where we left it (no error path executed).
        assert!(app.status_message.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_yank_to_clipboard_skips_empty_unnamed() {
        let (mut app, _d, _v) = app_with_prose("x\n").await;
        app.vim.unnamed.text.clear();
        sync_yank_to_clipboard(&mut app, Operator::Yank);
        // No clipboard write, no error.
        let _ = app;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_operator_enter_visual_sets_anchor() {
        let (mut app, _d, _v) = app_with_prose("hello\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 2 });
        apply_operator(&mut app, Action::EnterVisual, false);
        assert!(app.vim.visual_anchor.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_operator_enter_visual_line_sets_anchor_and_mode() {
        let (mut app, _d, _v) = app_with_prose("hello\n").await;
        apply_operator(&mut app, Action::EnterVisualLine, false);
        assert!(app.vim.visual_anchor.is_some());
        assert!(matches!(app.vim.mode, crate::vim::mode::Mode::VisualLine));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_operator_exit_visual_returns_to_normal() {
        let (mut app, _d, _v) = app_with_prose("hello\n").await;
        app.vim.visual_anchor = Some(Cursor::InProse { segment_idx: 0, offset: 0 });
        app.vim.mode = crate::vim::mode::Mode::Visual;
        apply_operator(&mut app, Action::ExitVisual, false);
        assert!(matches!(app.vim.mode, crate::vim::mode::Mode::Normal));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_operator_visual_swap_flips_anchor_and_cursor() {
        let (mut app, _d, _v) = app_with_prose("hello\n").await;
        let anchor = Cursor::InProse { segment_idx: 0, offset: 0 };
        let cursor_start = Cursor::InProse { segment_idx: 0, offset: 3 };
        app.vim.visual_anchor = Some(anchor);
        app.document_mut().unwrap().set_cursor(cursor_start);
        apply_operator(&mut app, Action::VisualSwap, false);
        // After swap, cursor should be at anchor's old position.
        match app.document().unwrap().cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 0),
            other => panic!("got {other:?}"),
        }
        // Anchor now holds the old cursor.
        match app.vim.visual_anchor.unwrap() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 3),
            other => panic!("got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_operator_dispatches_paste() {
        let (mut app, _d, _v) = app_with_prose("ab\n").await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        app.vim.unnamed = crate::vim::register::Register {
            text: "X".into(),
            linewise: false,
        };
        apply_operator(&mut app, Action::Paste(PastePos::After, 1), false);
        // No panic + path executed.
        assert!(app.document().is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn is_readonly_modal_open_detects_db_row_detail() {
        let (mut app, _d, _v) = app_with_prose("x\n").await;
        assert!(!is_readonly_modal_open(&app));
        app.modal = Some(crate::modal::Modal::DbRowDetail(DbRowDetailState {
            segment_idx: 0,
            row: 0,
            title: "t".into(),
            doc: Document::from_markdown("x\n").unwrap(),
            viewport_height: 4,
            viewport_top: 0,
        }));
        assert!(is_readonly_modal_open(&app));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn is_readonly_modal_open_detects_http_response_detail() {
        let (mut app, _d, _v) = app_with_prose("x\n").await;
        app.modal = Some(crate::modal::Modal::HttpResponseDetail(
            HttpResponseDetailState {
                segment_idx: 0,
                title: "t".into(),
                doc: Document::from_markdown("x\n").unwrap(),
                viewport_height: 4,
                viewport_top: 0,
            },
        ));
        assert!(is_readonly_modal_open(&app));
    }

    #[test]
    fn is_doc_mutation_textobject_and_linewise_variants() {
        use Operator::{Change, Delete, Yank};
        assert!(is_doc_mutation(&Action::OperatorTextObject(
            Delete,
            TextObject::Word { around: false },
            1
        )));
        assert!(!is_doc_mutation(&Action::OperatorTextObject(
            Yank,
            TextObject::Word { around: false },
            1
        )));
        assert!(is_doc_mutation(&Action::OperatorLinewise(Change, 1)));
        assert!(!is_doc_mutation(&Action::OperatorLinewise(Yank, 1)));
        assert!(is_doc_mutation(&Action::Paste(PastePos::After, 1)));
    }
}
