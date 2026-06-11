//! Cursor-Y projection + viewport clamping.
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-viewport) — pure code move, no behavior change. `SCROLL_OFF`,
//! `cursor_y`, `clamp_viewport` move together (the latter two are the
//! only `SCROLL_OFF` consumers). Re-exported `pub(crate)` from
//! `app/mod.rs` so `App::refresh_viewport_for_cursor` keeps resolving.
//! The `clamp_viewport` test suite moves along verbatim.

use crate::buffer::layout::SegmentLayout;
use crate::buffer::{Cursor, Document, Segment};

pub(crate) const SCROLL_OFF: u16 = 3;

/// Y row of the cursor in document-absolute coordinates.
pub(crate) fn cursor_y(doc: &Document, layouts: &[SegmentLayout]) -> u16 {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let layout = layouts
                .iter()
                .find(|l| l.segment_idx == segment_idx)
                .copied()
                .unwrap_or(SegmentLayout {
                    segment_idx,
                    y_start: 0,
                    height: 1,
                });
            let line_offset = if let Some(Segment::Prose(rope)) = doc.segments().get(segment_idx) {
                rope.char_to_line(offset.min(rope.len_chars())) as u16
            } else {
                0
            };
            layout.y_start.saturating_add(line_offset)
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => layouts
            .iter()
            .find(|l| l.segment_idx == segment_idx)
            .map(|l| {
                use crate::buffer::block::{raw_section_at, RawSection};
                use crate::buffer::Segment;
                let raw = match doc.segments().get(segment_idx) {
                    Some(Segment::Block(b)) => &b.raw,
                    _ => return l.y_start,
                };
                // Card layout: top border (y_start) → header bar
                // (y_start+1) → fence header (y_start+2, cursor on)
                // → body (y_start+3..) → fence closer (above footer)
                // → footer bar (y_start + height - 2) → bottom
                // border (y_start + height - 1).
                match raw_section_at(raw, offset) {
                    RawSection::Header => l.y_start.saturating_add(2),
                    RawSection::Body { line, .. } => {
                        l.y_start.saturating_add(3).saturating_add(line as u16)
                    }
                    RawSection::Closer => l.y_start.saturating_add(l.height.saturating_sub(3)),
                }
            })
            .unwrap_or(0),
        Cursor::InBlockResult { segment_idx, .. } => layouts
            .iter()
            .find(|l| l.segment_idx == segment_idx)
            // Park near the bottom of the block — refresh_viewport
            // already keeps the result table in view. A more precise
            // landing requires knowing each row's y inside the table.
            .map(|l| l.y_start.saturating_add(l.height.saturating_sub(2)))
            .unwrap_or(0),
    }
}

/// Adjust `viewport_top` so the cursor stays inside `[top + scrolloff,
/// top + height - scrolloff)`. Returns the new top.
pub(crate) fn clamp_viewport(viewport_top: u16, height: u16, cursor_y: u16) -> u16 {
    if height == 0 {
        return viewport_top;
    }
    let scrolloff = SCROLL_OFF.min(height / 2);
    let upper = cursor_y.saturating_sub(scrolloff);
    let lower = cursor_y
        .saturating_add(scrolloff + 1)
        .saturating_sub(height);
    if viewport_top > upper {
        upper
    } else if viewport_top < lower {
        lower
    } else {
        viewport_top
    }
}

/// New horizontal pan for the cursor's segment. Pans only where a
/// caret can actually sit on a long text line: prose lines and block
/// BODY lines. Fence header / closer rows and result tables stay at
/// 0 — their cursor clamps to the card edge as before.
pub(crate) fn follow_cursor_x(doc: &Document, left: u16, width: u16) -> u16 {
    use crate::buffer::viewport2d::{display_col, follow_x};
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let Some(Segment::Prose(rope)) = doc.segments().get(segment_idx) else {
                return 0;
            };
            let off = offset.min(rope.len_chars());
            let line = rope.char_to_line(off);
            let col = off - rope.line_to_char(line);
            let cursor_x = display_col(rope.line(line).chars(), col);
            follow_x(left, width, cursor_x)
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            use crate::buffer::block::{raw_section_at, RawSection};
            let Some(Segment::Block(b)) = doc.segments().get(segment_idx) else {
                return 0;
            };
            let raw = &b.raw;
            let off = offset.min(raw.len_chars());
            if !matches!(raw_section_at(raw, off), RawSection::Body { .. }) {
                return 0;
            }
            let line = raw.char_to_line(off);
            let col = off - raw.line_to_char(line);
            let cursor_x = display_col(raw.line(line).chars(), col);
            // Body text sits one cell inside the card's left border and
            // one short of the right border (see render_inblock_cursor).
            follow_x(left, width.saturating_sub(2), cursor_x)
        }
        Cursor::InBlockResult { .. } => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_viewport_keeps_cursor_visible() {
        let new_top = clamp_viewport(0, 10, 50);
        assert!(new_top > 0);
        let no_change = clamp_viewport(40, 10, 45);
        assert_eq!(no_change, 40);
    }

    #[test]
    fn clamp_viewport_handles_zero_height() {
        assert_eq!(clamp_viewport(7, 0, 100), 7);
    }

    #[test]
    fn clamp_viewport_scrolls_up_when_cursor_above_window() {
        // Cursor sits above the current top — viewport snaps up so
        // the cursor lands `scrolloff` rows from the top edge.
        let new_top = clamp_viewport(40, 10, 20);
        // scrolloff = min(3, 10/2) = 3 → upper = 20 - 3 = 17.
        assert_eq!(new_top, 17);
    }

    #[test]
    fn clamp_viewport_clamps_scrolloff_to_half_height() {
        // With a 4-row window, scrolloff is min(3, 2) = 2. Cursor at
        // row 10 → lower = 10 + 3 - 4 = 9.
        let new_top = clamp_viewport(0, 4, 10);
        assert_eq!(new_top, 9);
    }

    fn doc_with_block() -> Document {
        // prose / block / prose so segment indices are stable.
        let md =
            "intro\n\n```http alias=a\nGET https://a.test\nAccept: application/json\n```\n\nend\n";
        Document::from_markdown(md).unwrap()
    }

    fn block_segment_idx(doc: &Document) -> usize {
        doc.segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("a block segment")
    }

    #[test]
    fn cursor_y_in_prose_adds_line_offset_to_layout_y_start() {
        let mut doc = Document::from_markdown("line0\nline1\nline2\n").unwrap();
        // Single prose segment at index 0.
        let layouts = vec![SegmentLayout {
            segment_idx: 0,
            y_start: 5,
            height: 3,
        }];
        // Cursor at the very start → y_start, no line offset.
        doc.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        assert_eq!(cursor_y(&doc, &layouts), 5);
        // Move the cursor onto the third line (offset past two
        // newlines) → y_start + 2.
        doc.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 12,
        });
        assert_eq!(cursor_y(&doc, &layouts), 7);
    }

    #[test]
    fn cursor_y_in_prose_falls_back_when_no_layout_matches() {
        let mut doc = Document::from_markdown("hello\n").unwrap();
        doc.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        // Empty layout slice → default SegmentLayout { y_start: 0 }.
        assert_eq!(cursor_y(&doc, &[]), 0);
    }

    #[test]
    fn cursor_y_in_prose_handles_offset_past_rope_end() {
        let mut doc = Document::from_markdown("ab\n").unwrap();
        doc.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 9_999,
        });
        let layouts = vec![SegmentLayout {
            segment_idx: 0,
            y_start: 2,
            height: 1,
        }];
        // Offset is clamped to rope length — single-line rope → 0
        // line offset → y_start.
        assert_eq!(cursor_y(&doc, &layouts), 2);
    }

    #[test]
    fn cursor_y_in_prose_for_non_prose_segment_uses_zero_offset() {
        let doc = doc_with_block();
        let blk = block_segment_idx(&doc);
        let layouts = vec![SegmentLayout {
            segment_idx: blk,
            y_start: 4,
            height: 8,
        }];
        // An InProse cursor pointing at a Block segment is degenerate
        // but defended: line_offset is 0 so it lands on y_start.
        let mut d = doc;
        d.set_cursor(Cursor::InProse {
            segment_idx: blk,
            offset: 0,
        });
        assert_eq!(cursor_y(&d, &layouts), 4);
    }

    #[test]
    fn cursor_y_in_block_maps_sections_to_card_rows() {
        let mut doc = doc_with_block();
        let blk = block_segment_idx(&doc);
        let layouts = vec![SegmentLayout {
            segment_idx: blk,
            y_start: 10,
            height: 9,
        }];
        // Header (offset 0) → y_start + 2.
        doc.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: 0,
        });
        assert_eq!(cursor_y(&doc, &layouts), 12);

        // A body offset → y_start + 3 + body-line. Find an offset
        // that lands on the first body line via raw_section_at.
        use crate::buffer::block::{raw_section_at, RawSection};
        let raw = match &doc.segments()[blk] {
            Segment::Block(b) => b.raw.clone(),
            _ => unreachable!(),
        };
        // Walk offsets until we hit Body { line: 0 } then Closer.
        let mut body0: Option<usize> = None;
        let mut closer: Option<usize> = None;
        for off in 0..raw.len_chars() {
            match raw_section_at(&raw, off) {
                RawSection::Body { line: 0, .. } if body0.is_none() => body0 = Some(off),
                RawSection::Closer if closer.is_none() => closer = Some(off),
                _ => {}
            }
        }
        doc.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: body0.expect("a body offset"),
        });
        assert_eq!(cursor_y(&doc, &layouts), 13); // 10 + 3 + 0

        doc.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: closer.expect("a closer offset"),
        });
        // Closer → y_start + (height - 3) = 10 + 6 = 16.
        assert_eq!(cursor_y(&doc, &layouts), 16);
    }

    #[test]
    fn cursor_y_in_block_without_layout_is_zero() {
        let mut doc = doc_with_block();
        let blk = block_segment_idx(&doc);
        doc.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: 0,
        });
        assert_eq!(cursor_y(&doc, &[]), 0);
    }

    #[test]
    fn follow_cursor_x_pans_for_long_prose_line() {
        let line = format!("{}end", "x".repeat(97));
        let mut d = Document::from_markdown(&format!("{line}\n")).unwrap();
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 99,
        });
        // width 40, cursor_x 99 → lower = 99 + 4 - 40 = 63.
        assert_eq!(follow_cursor_x(&d, 0, 40), 63);
        // Back to column 0 → pan returns to 0.
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        assert_eq!(follow_cursor_x(&d, 63, 40), 0);
    }

    #[test]
    fn follow_cursor_x_zero_width_keeps_left() {
        let mut d = Document::from_markdown("abc\n").unwrap();
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 2,
        });
        assert_eq!(follow_cursor_x(&d, 7, 0), 7);
    }

    #[test]
    fn follow_cursor_x_pans_block_body_with_border_allowance() {
        let url = format!("https://example.com/{}", "p".repeat(90));
        let md = format!("```http alias=a\nGET {url}\n```\n");
        let mut d = Document::from_markdown(&md).unwrap();
        let blk = block_segment_idx(&d);
        let raw = match &d.segments()[blk] {
            Segment::Block(b) => b.raw.to_string(),
            _ => unreachable!(),
        };
        // Cursor at the end of the GET line (a Body section offset).
        let line_start = raw.find("GET ").unwrap();
        let line_len = raw[line_start..].find('\n').unwrap();
        d.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: line_start + line_len - 1,
        });
        let left = follow_cursor_x(&d, 0, 40);
        // Effective width is 40 - 2 (card borders); the pan must put
        // the cursor inside that window.
        let cursor_col = (line_len - 1) as u16;
        assert!(left > 0, "long body line must pan");
        assert!(cursor_col - left < 38, "cursor inside the body window");
    }

    #[test]
    fn follow_cursor_x_is_zero_on_fence_header_and_result() {
        let mut d = doc_with_block();
        let blk = block_segment_idx(&d);
        // Offset 0 → fence header → no pan even with stale left.
        d.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: 0,
        });
        assert_eq!(follow_cursor_x(&d, 9, 40), 0);
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: blk,
            row: 0,
        });
        assert_eq!(follow_cursor_x(&d, 9, 40), 0);
    }

    #[test]
    fn cursor_y_in_block_result_parks_near_bottom() {
        let mut doc = doc_with_block();
        let blk = block_segment_idx(&doc);
        let layouts = vec![SegmentLayout {
            segment_idx: blk,
            y_start: 20,
            height: 12,
        }];
        doc.set_cursor(Cursor::InBlockResult {
            segment_idx: blk,
            row: 3,
        });
        // y_start + (height - 2) = 20 + 10 = 30.
        assert_eq!(cursor_y(&doc, &layouts), 30);
        // No layout → 0.
        assert_eq!(cursor_y(&doc, &[]), 0);
    }
}
