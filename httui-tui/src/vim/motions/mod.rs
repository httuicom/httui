//! Motion engine: take a [`Motion`] + count and move the cursor.
//! Sub-modules hold the per-family appliers (horizontal / vertical
//! / word); this module owns the dispatcher + `target` / `apply`
//! entry points.

mod helpers;
mod horizontal;
mod vertical;
mod word;

use crate::buffer::{Cursor, Document};
use crate::vim::parser::Motion;

/// Compute where a motion would land **without** keeping the change.
/// Internally calls [`apply`] against a snapshot — uses `&mut Document`
/// to reuse the existing engine, but restores the original cursor
/// before returning. Used by the operator engine to derive ranges.
pub fn target(motion: Motion, doc: &mut Document, count: usize, viewport_height: u16) -> Cursor {
    let saved = doc.cursor();
    apply(motion, doc, count, viewport_height);
    let result = doc.cursor();
    doc.set_cursor(saved);
    result
}

/// Apply a motion `count` times, mutating the document's cursor in place.
pub fn apply(motion: Motion, doc: &mut Document, count: usize, viewport_height: u16) {
    let count = count.max(1);
    match motion {
        Motion::HalfPageDown => {
            vertical::half_page(doc, (viewport_height as i32 / 2) * count as i32)
        }
        Motion::HalfPageUp => {
            vertical::half_page(doc, -(viewport_height as i32 / 2) * count as i32)
        }
        _ => {
            for _ in 0..count {
                let next = compute_next(motion, doc);
                if next == doc.cursor() {
                    break;
                }
                doc.set_cursor(next);
                if is_absolute(motion) {
                    break;
                }
            }
        }
    }
}

fn is_absolute(motion: Motion) -> bool {
    matches!(
        motion,
        Motion::LineStart
            | Motion::FirstNonBlank
            | Motion::LineEnd
            | Motion::DocStart
            | Motion::DocEnd
            | Motion::GotoLine(_)
    )
}

fn compute_next(motion: Motion, doc: &Document) -> Cursor {
    match motion {
        Motion::Left => horizontal::apply_left(doc),
        Motion::Right => horizontal::apply_right(doc),
        Motion::Down => vertical::apply_down(doc),
        Motion::Up => vertical::apply_up(doc),
        Motion::LineStart => horizontal::apply_line_start(doc),
        Motion::FirstNonBlank => horizontal::apply_first_non_blank(doc),
        Motion::LineEnd => horizontal::apply_line_end(doc),
        Motion::WordForward => word::apply_word_forward(doc),
        Motion::WordBackward => word::apply_word_backward(doc),
        Motion::WordEnd => word::apply_word_end(doc),
        Motion::DocStart => vertical::apply_doc_start(doc),
        Motion::DocEnd => vertical::apply_doc_end(doc),
        Motion::GotoLine(n) => vertical::apply_goto_line(doc, n),
        Motion::FindForward(c) => word::apply_find(doc, c, true, false),
        Motion::FindBackward(c) => word::apply_find(doc, c, false, false),
        Motion::TillForward(c) => word::apply_find(doc, c, true, true),
        Motion::TillBackward(c) => word::apply_find(doc, c, false, true),
        // half-page handled by `apply` directly
        Motion::HalfPageDown | Motion::HalfPageUp => doc.cursor(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::{raw_section_at, RawSection};
    use crate::buffer::{Document, Segment};
    use serde_json::Value;

    fn doc(md: &str) -> Document {
        Document::from_markdown(md).unwrap()
    }

    // Canonical cursor contract: the cursor walks every block of any
    // kind in the SAME order — header → body → result (if any) →
    // closer. `j` follows that order top-to-bottom, `k` is its exact
    // reverse. HTTP and DB must produce identical sequences for
    // matching `(body_lines, result_rows)` shapes so muscle memory
    // carries between block types. The renderer is free to paint the
    // closer wherever it makes visual sense (orthogonal concern).

    /// Label cursors by their semantic location so we can compare
    /// walks across block types without caring about offsets. Prose
    /// segments are bucketed into Above/Below relative to the block.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Loc {
        ProseAbove,
        ProseBelow,
        Header,
        Body(usize),
        Closer,
        Result(usize),
    }

    fn classify(d: &Document, block_idx: usize) -> Loc {
        match d.cursor() {
            Cursor::InProse { segment_idx, .. } => {
                if segment_idx < block_idx {
                    Loc::ProseAbove
                } else {
                    Loc::ProseBelow
                }
            }
            Cursor::InBlock {
                segment_idx,
                offset,
            } => {
                assert_eq!(segment_idx, block_idx, "cursor escaped the target block");
                let Segment::Block(b) = &d.segments()[segment_idx] else {
                    panic!("expected block at {segment_idx}")
                };
                match raw_section_at(&b.raw, offset) {
                    RawSection::Header => Loc::Header,
                    RawSection::Body { line, .. } => Loc::Body(line),
                    RawSection::Closer => Loc::Closer,
                }
            }
            Cursor::InBlockResult { segment_idx, row } => {
                assert_eq!(segment_idx, block_idx, "cursor escaped the target block");
                Loc::Result(row)
            }
        }
    }

    /// Apply `motion` from the current cursor until it crosses the
    /// block boundary on the far side (or stops moving), returning
    /// the labeled stops.
    fn walk(d: &mut Document, block_idx: usize, motion: Motion) -> Vec<Loc> {
        let mut path = vec![classify(d, block_idx)];
        let mut entered = false;
        for _ in 0..32 {
            let before = d.cursor();
            apply(motion, d, 1, 10);
            if d.cursor() == before {
                break;
            }
            let loc = classify(d, block_idx);
            let in_block = !matches!(loc, Loc::ProseAbove | Loc::ProseBelow);
            if in_block {
                entered = true;
            }
            let leaving = entered && !in_block;
            path.push(loc);
            if leaving {
                break;
            }
        }
        path
    }

    fn block_pos(d: &Document) -> usize {
        d.segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("fixture must contain a block")
    }

    fn attach_http_response(d: &mut Document, block_idx: usize) {
        let mut block = match d.segments()[block_idx].clone() {
            Segment::Block(b) => b,
            _ => unreachable!(),
        };
        block.cached_result = Some(serde_json::json!({
            "status": 200,
            "status_text": "OK",
            "headers": [],
            "body": serde_json::json!({"ok": true}),
            "size_bytes": 13,
            "timing": {"total_ms": 50, "ttfb_ms": 30},
        }));
        d.replace_segment(block_idx, Segment::Block(block));
    }

    fn attach_db_rows(d: &mut Document, block_idx: usize, row_count: usize) {
        let mut block = match d.segments()[block_idx].clone() {
            Segment::Block(b) => b,
            _ => unreachable!(),
        };
        let rows: Vec<Value> = (0..row_count)
            .map(|i| serde_json::json!([format!("r{i}")]))
            .collect();
        block.cached_result = Some(serde_json::json!({
            "results": [{
                "kind": "select",
                "columns": ["col"],
                "rows": rows,
            }],
        }));
        d.replace_segment(block_idx, Segment::Block(block));
    }

    /// Park the cursor on the last character of the prose segment
    /// directly above the block, so the next `j` enters the block.
    fn park_above(d: &mut Document, block_idx: usize) {
        let prose_idx = block_idx
            .checked_sub(1)
            .expect("fixture needs prose above the block");
        let rope = match &d.segments()[prose_idx] {
            Segment::Prose(r) => r.clone(),
            _ => panic!("expected prose above block"),
        };
        let last_line = rope.len_lines().saturating_sub(1);
        d.set_cursor(Cursor::InProse {
            segment_idx: prose_idx,
            offset: rope.line_to_char(last_line),
        });
    }

    /// Park the cursor on the first character of the prose segment
    /// directly below the block, so the next `k` enters the block.
    fn park_below(d: &mut Document, block_idx: usize) {
        let prose_idx = block_idx + 1;
        assert!(
            matches!(d.segments().get(prose_idx), Some(Segment::Prose(_))),
            "fixture needs prose below the block"
        );
        d.set_cursor(Cursor::InProse {
            segment_idx: prose_idx,
            offset: 0,
        });
    }

    fn http_doc(body: &str) -> Document {
        let md = format!("head\n\n```http alias=req1\n{body}\n```\n\ntail\n");
        doc(&md)
    }

    fn db_doc(body: &str) -> Document {
        let md = format!("head\n\n```db-postgres alias=q\n{body}\n```\n\ntail\n");
        doc(&md)
    }

    fn expect_canon(body_lines: usize, result_rows: usize) -> (Vec<Loc>, Vec<Loc>) {
        let mut down = vec![Loc::ProseAbove, Loc::Header];
        for i in 0..body_lines {
            down.push(Loc::Body(i));
        }
        down.push(Loc::Closer);
        for i in 0..result_rows {
            down.push(Loc::Result(i));
        }
        down.push(Loc::ProseBelow);
        let mut up = down.clone();
        up.reverse();
        (down, up)
    }

    #[test]
    fn canonical_down_http_idle() {
        let mut d = http_doc("GET https://example.com/users");
        let idx = block_pos(&d);
        park_above(&mut d, idx);
        let (expected, _) = expect_canon(1, 0);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_up_http_idle() {
        let mut d = http_doc("GET https://example.com/users");
        let idx = block_pos(&d);
        park_below(&mut d, idx);
        let (_, expected) = expect_canon(1, 0);
        assert_eq!(walk(&mut d, idx, Motion::Up), expected);
    }

    #[test]
    fn canonical_down_http_with_result() {
        let mut d = http_doc("GET https://example.com/users");
        let idx = block_pos(&d);
        attach_http_response(&mut d, idx);
        park_above(&mut d, idx);
        // HTTP result panel is a single landing row — the inner
        // viewport is internally scrollable, j/k only park on it.
        let (expected, _) = expect_canon(1, 1);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_up_http_with_result() {
        let mut d = http_doc("GET https://example.com/users");
        let idx = block_pos(&d);
        attach_http_response(&mut d, idx);
        park_below(&mut d, idx);
        let (_, expected) = expect_canon(1, 1);
        assert_eq!(walk(&mut d, idx, Motion::Up), expected);
    }

    #[test]
    fn canonical_down_db_idle() {
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        park_above(&mut d, idx);
        let (expected, _) = expect_canon(1, 0);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_up_db_idle() {
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        park_below(&mut d, idx);
        let (_, expected) = expect_canon(1, 0);
        assert_eq!(walk(&mut d, idx, Motion::Up), expected);
    }

    #[test]
    fn canonical_down_db_with_result() {
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        attach_db_rows(&mut d, idx, 3);
        park_above(&mut d, idx);
        let (expected, _) = expect_canon(1, 3);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_up_db_with_result() {
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        attach_db_rows(&mut d, idx, 3);
        park_below(&mut d, idx);
        let (_, expected) = expect_canon(1, 3);
        assert_eq!(walk(&mut d, idx, Motion::Up), expected);
    }

    #[test]
    fn canonical_down_multi_body_db_with_result() {
        let mut d = db_doc("SELECT id, name\nFROM users\nWHERE id = 1");
        let idx = block_pos(&d);
        attach_db_rows(&mut d, idx, 2);
        park_above(&mut d, idx);
        let (expected, _) = expect_canon(3, 2);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_http_and_db_idle_have_same_shape() {
        let mut h = http_doc("GET https://example.com/users");
        let mut q = db_doc("SELECT 1");
        let hi = block_pos(&h);
        let qi = block_pos(&q);
        park_above(&mut h, hi);
        park_above(&mut q, qi);
        assert_eq!(walk(&mut h, hi, Motion::Down), walk(&mut q, qi, Motion::Down));
    }

    #[test]
    fn canonical_up_is_reverse_of_down() {
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        attach_db_rows(&mut d, idx, 3);
        park_above(&mut d, idx);
        let down = walk(&mut d, idx, Motion::Down);
        park_below(&mut d, idx);
        let mut up = walk(&mut d, idx, Motion::Up);
        up.reverse();
        assert_eq!(down, up);
    }

    #[test]
    fn left_stops_at_line_start() {
        let mut d = doc("hello\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        apply(Motion::Left, &mut d, 1, 10);
        assert_eq!(
            d.cursor(),
            Cursor::InProse {
                segment_idx: 0,
                offset: 0
            }
        );
    }

    #[test]
    fn right_advances_inside_line() {
        let mut d = doc("ab\n");
        apply(Motion::Right, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 1),
            _ => panic!(),
        }
    }

    #[test]
    fn line_end_lands_before_newline() {
        let mut d = doc("hello world\n");
        apply(Motion::LineEnd, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => {
                assert_eq!(offset, "hello world".len());
            }
            _ => panic!(),
        }
    }

    #[test]
    fn line_start_resets_offset() {
        let mut d = doc("hello\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 4,
        });
        apply(Motion::LineStart, &mut d, 1, 10);
        assert_eq!(
            d.cursor(),
            Cursor::InProse {
                segment_idx: 0,
                offset: 0
            }
        );
    }

    #[test]
    fn first_non_blank_skips_indent() {
        let mut d = doc("   indented\n");
        apply(Motion::FirstNonBlank, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 3),
            _ => panic!(),
        }
    }

    #[test]
    fn down_advances_line() {
        let mut d = doc("a\nb\nc\n");
        apply(Motion::Down, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert!(offset > 0),
            _ => panic!(),
        }
    }

    #[test]
    fn count_amplifies_down() {
        let mut d = doc("a\nb\nc\nd\ne\n");
        apply(Motion::Down, &mut d, 3, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => {
                let line = d.segments()[0].as_prose().unwrap().char_to_line(offset);
                assert_eq!(line, 3);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn doc_start_and_end() {
        let mut d = doc("a\nb\nc\n");
        apply(Motion::DocEnd, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert!(offset > 0),
            _ => panic!(),
        }
        apply(Motion::DocStart, &mut d, 1, 10);
        assert_eq!(
            d.cursor(),
            Cursor::InProse {
                segment_idx: 0,
                offset: 0
            }
        );
    }

    #[test]
    fn word_forward_skips_to_next_word() {
        let mut d = doc("hello world foo\n");
        apply(Motion::WordForward, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 6),
            _ => panic!(),
        }
    }

    #[test]
    fn word_backward_returns_to_previous() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 6,
        });
        apply(Motion::WordBackward, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 0),
            _ => panic!(),
        }
    }

    #[test]
    fn word_end_lands_on_last_char() {
        let mut d = doc("hello world\n");
        apply(Motion::WordEnd, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 4),
            _ => panic!(),
        }
    }

    #[test]
    fn half_page_down_walks_lines() {
        let md = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let mut d = doc(md);
        apply(Motion::HalfPageDown, &mut d, 1, 8);
        match d.cursor() {
            Cursor::InProse { offset, .. } => {
                let line = d.segments()[0].as_prose().unwrap().char_to_line(offset);
                assert_eq!(line, 4); // half of 8
            }
            _ => panic!(),
        }
    }

    #[test]
    fn f_lands_on_target_char() {
        let mut d = doc("hello world\n");
        apply(Motion::FindForward('o'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 4),
            _ => panic!(),
        }
    }

    #[test]
    fn f_with_count_finds_nth() {
        let mut d = doc("a-b-c-d\n");
        apply(Motion::FindForward('-'), &mut d, 2, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 3),
            _ => panic!(),
        }
    }

    #[test]
    fn t_lands_one_before_target() {
        let mut d = doc("hello world\n");
        apply(Motion::TillForward('o'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 3),
            _ => panic!(),
        }
    }

    #[test]
    fn capital_f_searches_backward() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 8,
        });
        apply(Motion::FindBackward('o'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 7),
            _ => panic!(),
        }
    }

    #[test]
    fn capital_t_lands_one_after_backward_target() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 8,
        });
        apply(Motion::TillBackward('o'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 8),
            _ => panic!(),
        }
    }

    #[test]
    fn find_does_not_cross_newline() {
        let mut d = doc("abc\nxyz\n");
        apply(Motion::FindForward('x'), &mut d, 1, 10);
        // 'x' is on line 2; forward find from line 1 must not match.
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 0),
            _ => panic!(),
        }
    }

    #[test]
    fn find_no_match_keeps_cursor() {
        let mut d = doc("hello\n");
        apply(Motion::FindForward('z'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 0),
            _ => panic!(),
        }
    }

    fn cursor_section(d: &Document) -> Option<RawSection> {
        let Cursor::InBlock {
            segment_idx,
            offset,
        } = d.cursor()
        else {
            return None;
        };
        let Segment::Block(b) = d.segments().get(segment_idx)? else {
            return None;
        };
        Some(raw_section_at(&b.raw, offset))
    }

    #[test]
    fn h_l_walk_the_fence_header() {
        // Horizontal motions on a fence row walk char-by-char like
        // prose so the user can edit `alias=foo` without leaving
        // the block.
        let mut d = doc("```db-postgres alias=q\nSELECT 1\n```\n");
        let block_idx = d
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        d.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: 0,
        });
        assert!(is_header(&d));
        for _ in 0..5 {
            apply(Motion::Right, &mut d, 1, 10);
        }
        assert!(is_header(&d));
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, 5);
        }
        apply(Motion::LineStart, &mut d, 1, 10);
        assert!(is_header(&d));
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, 0);
        }
        // Header is "```db-postgres alias=q" (22 chars); `$` lands
        // one before the newline.
        apply(Motion::LineEnd, &mut d, 1, 10);
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, 21);
        }
    }

    #[test]
    fn h_at_column_zero_does_not_cross_lines_in_block() {
        // Vim's `h` never folds into the previous line — same
        // contract for InBlock now that motions walk the raw rope.
        let mut d = doc("```db-postgres alias=q\nSELECT 1\n```\n");
        let block_idx = d
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        let raw = match d.segments().get(block_idx) {
            Some(Segment::Block(b)) => b.raw.clone(),
            _ => panic!(),
        };
        let body_start = crate::buffer::block::body_line_to_raw_offset(&raw, 0);
        d.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: body_start,
        });
        let before = d.cursor();
        apply(Motion::Left, &mut d, 1, 10);
        // Cursor stays put — refused to cross into the header line.
        assert_eq!(d.cursor(), before);
    }

    fn is_header(d: &Document) -> bool {
        matches!(cursor_section(d), Some(RawSection::Header))
    }
}
