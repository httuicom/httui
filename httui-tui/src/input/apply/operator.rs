// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
//! Operator / paste / visual-operator appliers (snapshot + record).
//! Mechanically moved out of `vim/dispatch.rs` (tui-v2 vertical 1,
//! fase 1 p5b) with no logic change.

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};
use crate::input::action::Action;
use crate::input::block_swap::InBlockSwap;
use crate::input::types::{InsertPos, Motion, Operator, PastePos, TextObject};
use crate::vim::change::{ChangeOrigin, ChangeRecord};
use crate::vim::mode::Mode;
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
    // Borrow the unnamed register out so we can use `app.document_mut()`
    // (which holds a mut borrow on the whole app) at the same time.
    // Restore at the end so yanks that landed in this call survive.
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

// ───────────── visual mode operators ─────────────

pub(crate) fn apply_visual_paste(app: &mut App, recording: bool) {
    let reg = resolve_paste_register(app);
    if reg.is_empty() {
        return;
    }
    apply_visual_operator(app, Operator::Delete, recording);
    if let Some(doc) = app.document_mut() {
        operator::paste(PastePos::Before, 1, doc, &reg);
    }
    if let Some(Cursor::InProse { segment_idx, .. }) = app.document().map(|d| d.cursor()) {
        if let Some(doc) = app.document_mut() {
            doc.reparse_prose_at(segment_idx);
        }
    }
    app.refresh_viewport_for_cursor();
}

pub(crate) fn apply_visual_operator(app: &mut App, op: Operator, _recording: bool) {
    let Some(anchor) = app.vim.visual_anchor else {
        return;
    };
    let linewise = matches!(app.vim.mode, Mode::VisualLine);
    let mut outcome = operator::OpOutcome::default();
    let mut unnamed = std::mem::take(&mut app.vim.unnamed);

    let cursor_now = app.document().map(|d| d.cursor());
    let cross_segment = matches!(
        (anchor, cursor_now),
        (
            Cursor::InProse { segment_idx: a, .. } | Cursor::InBlock { segment_idx: a, .. },
            Some(Cursor::InProse { segment_idx: c, .. } | Cursor::InBlock { segment_idx: c, .. })
        ) if a != c
    );

    if cross_segment {
        // Selection spans multiple segments (prose ↔ block, block ↔
        // prose, etc.). The single-segment operator engine doesn't
        // know about segment seams, so we round-trip the doc through
        // markdown: serialize → splice the selected range → re-parse.
        // Cached block state survives by alias matching (see
        // `Document::replace_with_text`).
        let cursor_after = apply_cross_segment_visual(
            app,
            op,
            anchor,
            cursor_now.unwrap_or(anchor),
            linewise,
            &mut unnamed,
        );
        app.vim.unnamed = unnamed;
        sync_yank_to_clipboard(app, op);
        if let Some(c) = cursor_after {
            if let Some(doc) = app.document_mut() {
                doc.set_cursor(c);
            }
        }
        if matches!(op, Operator::Change) {
            app.vim.enter_insert();
            app.vim.insert_session.start_plain(InsertPos::Current);
        } else {
            return_from_visual(app);
        }
        app.refresh_viewport_for_cursor();
        return;
    }

    // Same-segment branch. When the selection lives entirely inside
    // a single block, promote `b.raw` to a Prose segment via
    // InBlockSwap so the existing prose-only `apply_visual` engine
    // handles it.
    let in_block_swap = matches!(
        (anchor, cursor_now),
        (Cursor::InBlock { segment_idx: a, .. }, Some(Cursor::InBlock { segment_idx: c, .. })) if a == c
    );
    let swap = if in_block_swap {
        InBlockSwap::maybe_enter(app)
    } else {
        None
    };
    let translated_anchor = if let Some(swap) = swap.as_ref() {
        match anchor {
            Cursor::InBlock { offset, .. } => Cursor::InProse {
                segment_idx: swap.segment_idx,
                offset,
            },
            other => other,
        }
    } else {
        anchor
    };

    if let Some(doc) = app.document_mut() {
        if !matches!(op, Operator::Yank) {
            doc.snapshot();
        }
        let cursor = doc.cursor();
        outcome =
            operator::apply_visual(op, translated_anchor, cursor, linewise, doc, &mut unnamed);
    }
    if let Some(swap) = swap {
        swap.exit(app);
    }
    app.vim.unnamed = unnamed;
    sync_yank_to_clipboard(app, op);
    if outcome.enter_insert {
        app.vim.enter_insert();
        app.vim.insert_session.start_plain(InsertPos::Current);
    } else {
        return_from_visual(app);
    }
    app.refresh_viewport_for_cursor();
}

/// Visual operator (d / y / c) on a selection that crosses segment
/// boundaries — heading + block + paragraph, etc. Round-trips the
/// doc through markdown so the operator doesn't have to teach every
/// segment kind about every cut. Cached block state (state /
/// cached_result) survives via alias matching in `replace_with_text`.
///
/// Returns the cursor's new position (so the caller can apply it
/// after dropping its mutable doc borrow), or `None` when nothing
/// changed.
pub(crate) fn apply_cross_segment_visual(
    app: &mut App,
    op: Operator,
    anchor: Cursor,
    cursor: Cursor,
    linewise: bool,
    reg: &mut crate::vim::register::Register,
) -> Option<Cursor> {
    let (a_seg, a_off) = endpoint_of(anchor)?;
    let (c_seg, c_off) = endpoint_of(cursor)?;

    let doc = app.tabs.active_document_mut()?;
    if !matches!(op, Operator::Yank) {
        doc.snapshot();
    }
    let full_text = doc.to_markdown();
    let chars: Vec<char> = full_text.chars().collect();
    let total = chars.len();
    let lo_global =
        doc.global_offset_for(a_seg.min(c_seg), if a_seg <= c_seg { a_off } else { c_off });
    let hi_global =
        doc.global_offset_for(a_seg.max(c_seg), if a_seg <= c_seg { c_off } else { a_off });
    let (lo, hi) = (lo_global.min(total), hi_global.min(total));

    // Resolve the inclusive char range to delete / yank. Linewise
    // expands to whole lines; charwise is inclusive at hi.
    let (start, end) = if linewise {
        let lo_line_start = line_start_in_chars(&chars, lo);
        let hi_line_end = line_end_inclusive_with_newline(&chars, hi);
        (lo_line_start, hi_line_end)
    } else {
        (lo, (hi + 1).min(total))
    };
    if end <= start {
        return None;
    }

    let yanked: String = chars[start..end].iter().collect();
    reg.text = yanked.clone();
    reg.linewise = linewise;

    if matches!(op, Operator::Yank) {
        // Yank doesn't mutate the doc — restore the original cursor
        // (visual mode collapses to anchor end on yank, vim convention).
        return Some(anchor);
    }

    // Splice the range out and rebuild the doc. The cursor lands
    // at `start` (vim's convention for d / c).
    let new_text: String = chars[..start].iter().chain(chars[end..].iter()).collect();
    // We need a Cursor for the new doc; pre-compute as InProse
    // segment 0 offset 0; replace_with_text clamps it sanely.
    let target = Cursor::InProse {
        segment_idx: 0,
        offset: 0,
    };
    if doc.replace_with_text(&new_text, target).is_err() {
        return None;
    }
    // Now find the segment + offset where `start` falls in the
    // *new* doc by walking segments. global_offset_for is the
    // forward map; we need the inverse.
    let new_cursor = cursor_at_global_offset(doc, start);
    doc.set_cursor(new_cursor);
    Some(new_cursor)
}

pub(crate) fn endpoint_of(c: Cursor) -> Option<(usize, usize)> {
    match c {
        Cursor::InProse {
            segment_idx,
            offset,
        } => Some((segment_idx, offset)),
        Cursor::InBlock {
            segment_idx,
            offset,
        } => Some((segment_idx, offset)),
        Cursor::InBlockResult { .. } => None,
    }
}

pub(crate) fn line_start_in_chars(chars: &[char], offset: usize) -> usize {
    let off = offset.min(chars.len());
    let mut i = off;
    while i > 0 && chars[i - 1] != '\n' {
        i -= 1;
    }
    i
}

pub(crate) fn line_end_inclusive_with_newline(chars: &[char], offset: usize) -> usize {
    let mut i = offset.min(chars.len());
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    if i < chars.len() {
        i + 1
    } else {
        i
    }
}

/// Find the (segment_idx, offset) cursor that maps to `target_global`
/// in `doc.to_markdown()`. Inverse of `Document::global_offset_for`.
pub(crate) fn cursor_at_global_offset(
    doc: &crate::buffer::Document,
    target_global: usize,
) -> Cursor {
    let mut global = 0usize;
    let last_visible_idx = doc
        .segments()
        .iter()
        .enumerate()
        .filter(|(_, s)| !is_empty_prose(s))
        .map(|(i, _)| i)
        .next_back()
        .unwrap_or(0);
    let mut emitted_so_far = String::new();
    for (i, seg) in doc.segments().iter().enumerate() {
        if is_empty_prose(seg) {
            continue;
        }
        let seg_text = match seg {
            Segment::Prose(r) => r.to_string(),
            Segment::Block(b) => {
                let adapter = httui_core::blocks::parser::ParsedBlock {
                    block_type: b.block_type.clone(),
                    alias: b.alias.clone(),
                    display_mode: b.display_mode.clone(),
                    params: b.params.clone(),
                    line_start: 0,
                    line_end: 0,
                };
                httui_core::blocks::serialize_block(&adapter)
            }
        };
        let seg_len = seg_text.chars().count();
        if target_global >= global && target_global <= global + seg_len {
            let local = target_global - global;
            return match seg {
                Segment::Prose(_) => Cursor::InProse {
                    segment_idx: i,
                    offset: local,
                },
                Segment::Block(_) => Cursor::InBlock {
                    segment_idx: i,
                    offset: local,
                },
            };
        }
        global += seg_len;
        emitted_so_far.push_str(&seg_text);
        if i < last_visible_idx && !emitted_so_far.ends_with('\n') {
            global += 1;
            emitted_so_far.push('\n');
        }
    }
    // Fell off the end — park on the last segment.
    Cursor::InProse {
        segment_idx: doc.segment_count().saturating_sub(1),
        offset: 0,
    }
}

pub(crate) fn is_empty_prose(s: &Segment) -> bool {
    matches!(s, Segment::Prose(r) if r.len_chars() == 0)
}

/// `va{` / `vi{` / `vaw` / `vi"` etc. — extend the current visual
/// selection to cover the resolved text object. Reuses the same
/// `textobject::compute_range` the operator engine uses, so the
/// notion of what's "inside" / "around" stays consistent. The
/// returned range is `[start, end)` (end exclusive); we snap the
/// anchor to `start` and the moving cursor to `end - 1` so the
/// selection paints inclusively at both ends. Mode stays Visual /
/// VisualLine — user can layer more motions on top.
pub(crate) fn apply_visual_select_textobject(app: &mut App, textobj: TextObject) {
    let Some(doc) = app.document_mut() else {
        return;
    };
    let Some((segment_idx, start, end)) = crate::vim::textobject::compute_range(textobj, doc)
    else {
        return;
    };
    if end == 0 || end <= start {
        return;
    }
    app.vim.visual_anchor = Some(Cursor::InProse {
        segment_idx,
        offset: start,
    });
    if let Some(doc) = app.document_mut() {
        doc.set_cursor(Cursor::InProse {
            segment_idx,
            offset: end - 1,
        });
    }
    app.refresh_viewport_for_cursor();
}

/// Leave Visual / VisualLine and pick the right "back" mode. When
/// the row-detail modal is the active surface (it owns its own
/// `Document` via `App::document_mut`'s redirect), we restore
/// `Mode::DbRowDetail` so the modal keeps rendering and key input
/// keeps flowing through `parse_db_row_detail`. Otherwise the
/// editor's normal mode is the natural exit.
pub(crate) fn return_from_visual(app: &mut App) {
    let origin = app.vim.visual_origin_mode.take();
    app.vim.enter_normal();
    if let Some(mode) = origin {
        if mode != Mode::Normal {
            app.vim.mode = mode;
        }
    }
}

/// `apply_action` sub-match for the visual / operator / paste domain.
/// Mechanically split out of the `apply_action` router in
/// `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p6d) — arm bodies
/// copied verbatim (including the `recording` plumbing). The outer
/// router routes only this group's variants here, so the
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
            return_from_visual(app);
        }
        Action::VisualSwap => {
            if let (Some(anchor), Some(doc)) = (app.vim.visual_anchor, app.document_mut()) {
                let cur = doc.cursor();
                doc.set_cursor(anchor);
                app.vim.visual_anchor = Some(cur);
                app.refresh_viewport_for_cursor();
            }
        }
        Action::VisualOperator(op) => apply_visual_operator(app, op, recording),
        Action::VisualPaste => apply_visual_paste(app, recording),
        Action::VisualSelectTextObject(textobj) => {
            apply_visual_select_textobject(app, textobj);
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
    app.db_row_detail.is_some() || app.http_response_detail.is_some()
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
        app.db_row_detail = Some(DbRowDetailState {
            segment_idx: 0,
            row: 0,
            title: "t".into(),
            doc: Document::from_markdown(content).unwrap(),
            viewport_height: 4,
            viewport_top: 0,
        });
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn visual_delete_in_db_row_detail_does_not_mutate() {
        let (mut app, _d, _v) = app_with_db_modal("alpha beta\n").await;
        let before = app.document().unwrap().to_markdown();
        // Seed visual anchor + cursor so `apply_visual_operator` has
        // a real range to act on.
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
        // Yank doesn't mutate the doc but should land in the unnamed
        // register. We only assert the doc invariant — the register
        // path is covered by yank-specific tests.
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
        app.http_response_detail = Some(HttpResponseDetailState {
            segment_idx: 0,
            title: "t".into(),
            doc: Document::from_markdown("status 200\n").unwrap(),
            viewport_height: 4,
            viewport_top: 0,
        });
        let before = app.document().unwrap().to_markdown();
        apply_operator(
            &mut app,
            Action::Paste(PastePos::After, 1),
            false,
        );
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
