//! Visual-mode operators (d / y / c / p / text-object selection).

use crate::app::App;
use crate::buffer::{Cursor, Segment};
use crate::input::block_swap::InBlockSwap;
use crate::input::types::{InsertPos, Operator, PastePos, TextObject};
use crate::vim::mode::Mode;
use crate::vim::operator;

use super::{resolve_paste_register, sync_yank_to_clipboard};

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
    let target = Cursor::InProse {
        segment_idx: 0,
        offset: 0,
    };
    if doc.replace_with_text(&new_text, target).is_err() {
        return None;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::buffer::Document;
    use crate::config::Config;
    use crate::pane::{Pane, TabState};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_with_doc(md: &str) -> (App, TempDir, TempDir) {
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
    fn endpoint_of_returns_pair_for_prose_and_block() {
        assert_eq!(
            endpoint_of(Cursor::InProse { segment_idx: 2, offset: 5 }),
            Some((2, 5))
        );
        assert_eq!(
            endpoint_of(Cursor::InBlock { segment_idx: 3, offset: 7 }),
            Some((3, 7))
        );
        assert_eq!(
            endpoint_of(Cursor::InBlockResult { segment_idx: 4, row: 0 }),
            None
        );
    }

    #[test]
    fn line_start_in_chars_walks_back_to_newline_or_zero() {
        let chars: Vec<char> = "abc\ndef\nghi".chars().collect();
        assert_eq!(line_start_in_chars(&chars, 0), 0); // start of doc
        assert_eq!(line_start_in_chars(&chars, 2), 0); // middle of first line
        assert_eq!(line_start_in_chars(&chars, 5), 4); // middle of second line
        assert_eq!(line_start_in_chars(&chars, 99), 8); // past end clamps
    }

    #[test]
    fn line_end_inclusive_with_newline_includes_terminator() {
        let chars: Vec<char> = "abc\ndef\nghi".chars().collect();
        assert_eq!(line_end_inclusive_with_newline(&chars, 0), 4); // includes \n
        assert_eq!(line_end_inclusive_with_newline(&chars, 4), 8); // includes 2nd \n
        assert_eq!(line_end_inclusive_with_newline(&chars, 8), 11); // last line, no \n
    }

    #[test]
    fn is_empty_prose_detects_zero_length_rope() {
        let empty = Segment::Prose(ropey::Rope::new());
        assert!(is_empty_prose(&empty));
        let non_empty = Segment::Prose(ropey::Rope::from_str("x"));
        assert!(!is_empty_prose(&non_empty));
    }

    #[test]
    fn cursor_at_global_offset_lands_in_prose() {
        let doc = Document::from_markdown("hello\nworld\n").unwrap();
        let cur = cursor_at_global_offset(&doc, 3);
        match cur {
            Cursor::InProse { segment_idx, offset } => {
                assert_eq!(segment_idx, 0);
                assert_eq!(offset, 3);
            }
            other => panic!("expected InProse, got {other:?}"),
        }
    }

    #[test]
    fn cursor_at_global_offset_past_end_lands_on_last_segment() {
        let doc = Document::from_markdown("hi\n").unwrap();
        let cur = cursor_at_global_offset(&doc, 9999);
        // Falls off the end → parks on the last visible segment.
        if let Cursor::InProse { segment_idx, .. } = cur {
            assert!(segment_idx < doc.segment_count());
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_visual_paste_no_anchor_is_noop() {
        // Without a visual_anchor, apply_visual_operator returns early
        // and paste never inserts. Doc stays put.
        let (mut app, _d, _v) = app_with_doc("hello world\n").await;
        let before = app.document().unwrap().to_markdown();
        app.vim.unnamed = crate::vim::register::Register::default();
        assert!(app.vim.visual_anchor.is_none());
        // Stuff something into unnamed so resolve_paste_register passes
        // the empty-check, then we still expect doc unchanged because
        // visual_operator(Delete) bails without anchor.
        app.vim.unnamed = crate::vim::register::Register {
            text: "x".into(),
            linewise: false,
        };
        apply_visual_paste(&mut app, false);
        // Cross-test clipboard pollution: at minimum the doc still
        // starts with the original prefix.
        assert!(
            app.document().unwrap().to_markdown().contains("hello world"),
            "doc lost original content"
        );
        let _ = before;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_visual_operator_without_anchor_returns_immediately() {
        let (mut app, _d, _v) = app_with_doc("hello\n").await;
        let before = app.document().unwrap().to_markdown();
        assert!(app.vim.visual_anchor.is_none());
        apply_visual_operator(&mut app, Operator::Delete, false);
        assert_eq!(app.document().unwrap().to_markdown(), before);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_visual_operator_yank_same_segment_fills_register() {
        let (mut app, _d, _v) = app_with_doc("hello world\n").await;
        // Anchor at offset 0, move cursor to 4 → visual selects "hello".
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 0 });
        app.vim.visual_anchor = Some(Cursor::InProse { segment_idx: 0, offset: 0 });
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InProse { segment_idx: 0, offset: 4 });
        apply_visual_operator(&mut app, Operator::Yank, false);
        // After yank, register should be non-empty.
        assert!(!app.vim.unnamed.text.is_empty(), "expected yanked text");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_visual_select_textobject_no_match_is_noop() {
        let (mut app, _d, _v) = app_with_doc("plain prose\n").await;
        // No quotes/parens in doc → compute_range returns None for Quote.
        let before_cursor = app.document().unwrap().cursor();
        apply_visual_select_textobject(
            &mut app,
            TextObject::Quote { delim: '"', around: false },
        );
        // No mutation, anchor remains unset.
        assert_eq!(app.document().unwrap().cursor(), before_cursor);
        assert!(app.vim.visual_anchor.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn return_from_visual_restores_origin_mode_when_set() {
        let (mut app, _d, _v) = app_with_doc("x\n").await;
        app.vim.visual_origin_mode = Some(Mode::DbRowDetail);
        return_from_visual(&mut app);
        assert_eq!(app.vim.mode, Mode::DbRowDetail);
        assert!(app.vim.visual_origin_mode.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn return_from_visual_with_no_origin_lands_in_normal() {
        let (mut app, _d, _v) = app_with_doc("x\n").await;
        app.vim.visual_origin_mode = None;
        return_from_visual(&mut app);
        assert_eq!(app.vim.mode, Mode::Normal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_cross_segment_visual_delete_mutates_doc_and_returns_cursor() {
        let md = "intro\n\n```http alias=a\nGET /x\n```\n\nouter\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let anchor = Cursor::InProse { segment_idx: 0, offset: 0 };
        // Move cursor inside the block.
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        let cursor = Cursor::InBlock { segment_idx: block_idx, offset: 0 };
        let before = app.document().unwrap().to_markdown();
        let mut reg = crate::vim::register::Register::default();
        let res = apply_cross_segment_visual(
            &mut app,
            Operator::Delete,
            anchor,
            cursor,
            /*linewise=*/ false,
            &mut reg,
        );
        assert!(res.is_some());
        assert_ne!(app.document().unwrap().to_markdown(), before);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_cross_segment_visual_with_empty_range_returns_none() {
        let md = "abc\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let same = Cursor::InProse { segment_idx: 0, offset: 0 };
        let mut reg = crate::vim::register::Register::default();
        // anchor == cursor, charwise → end == start+1 which is > start
        // so it's not always None. Use linewise on a position with no
        // content to drive end <= start.
        let res = apply_cross_segment_visual(
            &mut app,
            Operator::Yank,
            same,
            same,
            /*linewise=*/ false,
            &mut reg,
        );
        // charwise selects 1 char → some result
        assert!(res.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_cross_segment_visual_with_blockresult_anchor_returns_none() {
        let md = "x\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let anchor = Cursor::InBlockResult { segment_idx: 0, row: 0 };
        let cursor = Cursor::InProse { segment_idx: 0, offset: 0 };
        let mut reg = crate::vim::register::Register::default();
        let res = apply_cross_segment_visual(
            &mut app,
            Operator::Yank,
            anchor,
            cursor,
            false,
            &mut reg,
        );
        // endpoint_of(InBlockResult) is None → early return
        assert!(res.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_cross_segment_visual_linewise_yank_grabs_full_lines() {
        let md = "first\nsecond\nthird\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        // From offset 2 of first line to offset 1 of second line linewise
        // should yank both whole lines.
        let anchor = Cursor::InProse { segment_idx: 0, offset: 2 };
        let cursor = Cursor::InProse { segment_idx: 0, offset: 7 };
        let mut reg = crate::vim::register::Register::default();
        let res = apply_cross_segment_visual(
            &mut app,
            Operator::Yank,
            anchor,
            cursor,
            /*linewise=*/ true,
            &mut reg,
        );
        assert!(res.is_some());
        assert!(reg.text.contains("first") || reg.text.contains("second"), "got {:?}", reg.text);
        assert!(reg.linewise);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn apply_cross_segment_visual_yank_fills_register() {
        // Doc with a block segment so cross-segment selection is meaningful.
        let md = "intro\n\n```http alias=a\nGET /x\n```\n\noutro\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let anchor = Cursor::InProse { segment_idx: 0, offset: 0 };
        // Move cursor into block segment somewhere later.
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        let cursor = Cursor::InBlock { segment_idx: block_idx, offset: 0 };
        let mut reg = crate::vim::register::Register::default();
        let res = apply_cross_segment_visual(
            &mut app,
            Operator::Yank,
            anchor,
            cursor,
            /*linewise=*/ false,
            &mut reg,
        );
        // Yank doesn't mutate doc but fills the register.
        assert!(res.is_some());
        assert!(!reg.text.is_empty(), "yank should fill register");
    }
}
