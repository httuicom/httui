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
}
