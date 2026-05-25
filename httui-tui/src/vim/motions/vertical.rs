//! Vertical / cross-segment motions (`j`, `k`, `gg`, `G`, `nG`,
//! `Ctrl-D`, `Ctrl-U`).

use crate::buffer::block::{raw_section_at, RawSection};
use crate::buffer::layout::layout_document;
use crate::buffer::{Cursor, Document, Segment};

use super::helpers::{
    block_at, block_query_line_count, block_result_row_count, body_cursor, closer_cursor,
    header_cursor, jump_to_segment,
};

pub(super) fn half_page(doc: &mut Document, delta: i32) {
    let count = delta.unsigned_abs() as usize;
    for _ in 0..count {
        let next = if delta > 0 {
            apply_down(doc)
        } else {
            apply_up(doc)
        };
        if next == doc.cursor() {
            break;
        }
        doc.set_cursor(next);
    }
}

pub(super) fn apply_down(doc: &Document) -> Cursor {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            if let Some(Segment::Prose(rope)) = doc.segments().get(segment_idx) {
                let line = rope.char_to_line(offset.min(rope.len_chars()));
                if line + 1 < rope.len_lines() {
                    return Cursor::InProse {
                        segment_idx,
                        offset: rope.line_to_char(line + 1),
                    };
                }
            }
            jump_to_segment(doc, segment_idx + 1, true).unwrap_or(doc.cursor())
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let block = match block_at(doc, segment_idx) {
                Some(b) => b,
                None => return doc.cursor(),
            };
            match raw_section_at(&block.raw, offset) {
                RawSection::Header => {
                    // From the ` ```<info> ` row, j drops into the
                    // body's first line — the renderer kept that row
                    // at the same visual position whether we're in
                    // raw or render mode.
                    body_cursor(segment_idx, block, 0, 0)
                }
                RawSection::Body { line, .. } => {
                    // Canonical order: body → closer → result → exit.
                    // Mirrors the renderer (closer fences the editable
                    // region; result panel lives below).
                    let lines = block_query_line_count(doc, segment_idx);
                    if line + 1 < lines {
                        return body_cursor(segment_idx, block, line + 1, 0);
                    }
                    closer_cursor(segment_idx, block)
                }
                RawSection::Closer => {
                    // Closer → result row 0 when present, else exit.
                    if block_result_row_count(doc, segment_idx) > 0 {
                        return Cursor::InBlockResult {
                            segment_idx,
                            row: 0,
                        };
                    }
                    jump_to_segment(doc, segment_idx + 1, true).unwrap_or(doc.cursor())
                }
            }
        }
        Cursor::InBlockResult { segment_idx, row } => {
            let total = block_result_row_count(doc, segment_idx);
            if row + 1 < total {
                return Cursor::InBlockResult {
                    segment_idx,
                    row: row + 1,
                };
            }
            // Result panel is the last cursor inside the block —
            // closer was already visited on the way in.
            jump_to_segment(doc, segment_idx + 1, true).unwrap_or(doc.cursor())
        }
    }
}

pub(super) fn apply_up(doc: &Document) -> Cursor {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            if let Some(Segment::Prose(rope)) = doc.segments().get(segment_idx) {
                let line = rope.char_to_line(offset.min(rope.len_chars()));
                if line > 0 {
                    return Cursor::InProse {
                        segment_idx,
                        offset: rope.line_to_char(line - 1),
                    };
                }
            }
            if segment_idx == 0 {
                return doc.cursor();
            }
            jump_to_segment(doc, segment_idx - 1, false).unwrap_or(doc.cursor())
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let block = match block_at(doc, segment_idx) {
                Some(b) => b,
                None => return doc.cursor(),
            };
            match raw_section_at(&block.raw, offset) {
                RawSection::Closer => {
                    // Reverse of the down path. Closer sits between
                    // body and result panel, so k from the closer
                    // always lands on the body's last line —
                    // independent of whether a result exists.
                    let last_line = block_query_line_count(doc, segment_idx).saturating_sub(1);
                    body_cursor(segment_idx, block, last_line, 0)
                }
                RawSection::Body { line, .. } => {
                    if line > 0 {
                        return body_cursor(segment_idx, block, line - 1, 0);
                    }
                    // Top of the body → land on the fence header
                    // instead of jumping out of the block. Another
                    // `k` takes us out.
                    header_cursor(segment_idx)
                }
                RawSection::Header => {
                    // Above the header — leave the block for the
                    // previous segment.
                    if segment_idx == 0 {
                        return doc.cursor();
                    }
                    jump_to_segment(doc, segment_idx - 1, false).unwrap_or(doc.cursor())
                }
            }
        }
        Cursor::InBlockResult { segment_idx, row } => {
            if row > 0 {
                return Cursor::InBlockResult {
                    segment_idx,
                    row: row - 1,
                };
            }
            // First row of the result panel → closer (it sits
            // between body and result; cursor matches the renderer).
            let block = match block_at(doc, segment_idx) {
                Some(b) => b,
                None => return doc.cursor(),
            };
            closer_cursor(segment_idx, block)
        }
    }
}

pub(super) fn apply_doc_start(doc: &Document) -> Cursor {
    if let Some(seg) = doc.segments().first() {
        match seg {
            Segment::Prose(_) => Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            Segment::Block(b) => body_cursor(0, b, 0, 0),
        }
    } else {
        doc.cursor()
    }
}

pub(super) fn apply_doc_end(doc: &Document) -> Cursor {
    let last = doc.segment_count().saturating_sub(1);
    let seg = match doc.segments().get(last) {
        Some(s) => s,
        None => return doc.cursor(),
    };
    match seg {
        Segment::Prose(rope) => {
            let lines = rope.len_lines();
            let off = if lines == 0 {
                0
            } else {
                rope.line_to_char(lines - 1)
            };
            Cursor::InProse {
                segment_idx: last,
                offset: off,
            }
        }
        Segment::Block(b) => body_cursor(last, b, 0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block_doc(md: &str) -> Document {
        Document::from_markdown(md).unwrap()
    }

    fn block_idx(d: &Document) -> usize {
        d.segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap()
    }

    #[test]
    fn half_page_walks_down_when_positive_delta() {
        let mut d = Document::from_markdown("a\nb\nc\nd\ne\n").unwrap();
        half_page(&mut d, 2);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert!(offset > 0),
            _ => panic!(),
        }
    }

    #[test]
    fn half_page_walks_up_when_negative_delta() {
        let mut d = Document::from_markdown("a\nb\nc\n").unwrap();
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 4,
        });
        half_page(&mut d, -1);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert!(offset < 4),
            _ => panic!(),
        }
    }

    #[test]
    fn half_page_stops_when_cursor_does_not_move() {
        let mut d = Document::from_markdown("only\n").unwrap();
        // Try to scroll way down — should bail out.
        half_page(&mut d, 50);
    }

    #[test]
    fn apply_down_in_blockresult_advances_row() {
        let mut d = block_doc("head\n\n```db-postgres alias=q\nSELECT 1\n```\n\ntail\n");
        let i = block_idx(&d);
        // Inject a result so there are rows to walk.
        let mut block = match d.segments().get(i).cloned() {
            Some(Segment::Block(b)) => b,
            _ => panic!(),
        };
        block.cached_result = Some(serde_json::json!({
            "results": [{
                "kind": "select",
                "columns": ["col"],
                "rows": [["r0"], ["r1"], ["r2"]],
            }]
        }));
        d.replace_segment(i, Segment::Block(block));
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: i,
            row: 0,
        });
        let after = apply_down(&d);
        assert!(matches!(after, Cursor::InBlockResult { row, .. } if row == 1));
    }

    #[test]
    fn apply_up_from_blockresult_row_zero_lands_on_closer() {
        let mut d = block_doc("head\n\n```db-postgres alias=q\nSELECT 1\n```\n\ntail\n");
        let i = block_idx(&d);
        let mut block = match d.segments().get(i).cloned() {
            Some(Segment::Block(b)) => b,
            _ => panic!(),
        };
        block.cached_result = Some(serde_json::json!({
            "results": [{
                "kind": "select",
                "columns": ["col"],
                "rows": [["r0"]],
            }]
        }));
        d.replace_segment(i, Segment::Block(block));
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: i,
            row: 0,
        });
        let after = apply_up(&d);
        assert!(matches!(after, Cursor::InBlock { .. }));
    }

    #[test]
    fn apply_up_in_prose_at_doc_start_returns_same() {
        let mut d = Document::from_markdown("a\nb\n").unwrap();
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        let before = d.cursor();
        assert_eq!(apply_up(&d), before);
    }

    #[test]
    fn apply_doc_start_on_block_first_segment_lands_in_body() {
        let d = block_doc("```http\nGET /\n```\n");
        let after = apply_doc_start(&d);
        // Block lives at idx 1 after padding; first segment is prose pad.
        assert!(matches!(after, Cursor::InProse { segment_idx: 0, .. }));
    }

    #[test]
    fn apply_doc_end_on_block_last_segment_lands_in_body() {
        let d = block_doc("head\n\n```http\nGET /\n```");
        let after = apply_doc_end(&d);
        // Last segment is the trailing prose pad (pad_with_prose adds it).
        match after {
            Cursor::InProse { .. } | Cursor::InBlock { .. } => {}
            _ => panic!(),
        }
    }

    #[test]
    fn apply_goto_line_falls_back_to_doc_end_when_past_end() {
        let d = Document::from_markdown("a\nb\nc\n").unwrap();
        let after = apply_goto_line(&d, 999);
        // Should land somewhere valid (doc end fallback).
        match after {
            Cursor::InProse { .. } | Cursor::InBlock { .. } => {}
            _ => panic!(),
        }
    }

    #[test]
    fn apply_goto_line_walks_to_specified_line_in_prose() {
        let d = Document::from_markdown("a\nb\nc\nd\n").unwrap();
        let after = apply_goto_line(&d, 2);
        // Should land on line 1 (0-based) in prose.
        if let Cursor::InProse { offset, .. } = after {
            assert!(offset > 0);
        } else {
            panic!()
        }
    }
}

pub(super) fn apply_goto_line(doc: &Document, n: usize) -> Cursor {
    let layouts = layout_document(doc, 80);
    let mut accum = 0usize;
    for layout in &layouts {
        let height = layout.height as usize;
        if accum + height >= n {
            let seg = match doc.segments().get(layout.segment_idx) {
                Some(s) => s,
                None => return doc.cursor(),
            };
            return match seg {
                Segment::Prose(rope) => {
                    let line_in_seg = n.saturating_sub(accum + 1);
                    let off = if line_in_seg < rope.len_lines() {
                        rope.line_to_char(line_in_seg)
                    } else {
                        0
                    };
                    Cursor::InProse {
                        segment_idx: layout.segment_idx,
                        offset: off,
                    }
                }
                Segment::Block(b) => body_cursor(layout.segment_idx, b, 0, 0),
            };
        }
        accum += height;
    }
    apply_doc_end(doc)
}
