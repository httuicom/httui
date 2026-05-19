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
}
