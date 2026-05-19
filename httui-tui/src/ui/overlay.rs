//! Selection + search overlays painted over already-rendered prose
//! and block segments (visual-mode highlight, incremental search).

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    Frame,
};
use ropey::Rope;

use crate::buffer::{layout::layout_document, Cursor, Document, Segment};
use crate::vim::search;

/// Visual-mode selection state passed down through the pane tree.
/// Only painted on the *focused* leaf (the cursor is the moving end).
/// Re-used by the row-detail modal to paint its own selection
/// overlay when the modal is up + visual mode is active.
#[derive(Debug, Clone, Copy)]
pub(crate) struct VisualOverlay {
    pub anchor: Cursor,
    pub linewise: bool,
}

/// Paint a bg highlight under the active visual selection. Charwise:
/// inclusive char range `[min, max]`. Linewise: every cell of every
/// line from `min(line)` to `max(line)`. Cross-segment selections are
/// skipped (they're refused by the operator engine too).
pub(crate) fn overlay_visual_selection(
    frame: &mut Frame,
    area: Rect,
    doc: &Document,
    viewport_top: u16,
    overlay: VisualOverlay,
) {
    let (a_seg, a_off) = match overlay.anchor {
        Cursor::InProse {
            segment_idx,
            offset,
        } => (segment_idx, offset),
        Cursor::InBlock {
            segment_idx,
            offset,
        } => (segment_idx, offset),
        Cursor::InBlockResult { .. } => return,
    };
    let (c_seg, c_off) = match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => (segment_idx, offset),
        Cursor::InBlock {
            segment_idx,
            offset,
        } => (segment_idx, offset),
        Cursor::InBlockResult { .. } => return,
    };
    // Establish lo / hi by (segment, offset) so the highlight
    // sweeps in document order regardless of which end is the
    // anchor and which is the moving cursor.
    let (lo_seg, lo_off, hi_seg, hi_off) = if (a_seg, a_off) <= (c_seg, c_off) {
        (a_seg, a_off, c_seg, c_off)
    } else {
        (c_seg, c_off, a_seg, a_off)
    };

    let layouts = layout_document(doc, area.width);
    let style = Style::default().bg(Color::Rgb(60, 70, 110));

    for seg_idx in lo_seg..=hi_seg {
        let Some(seg) = doc.segments().get(seg_idx) else {
            break;
        };
        let layout = match layouts.iter().find(|l| l.segment_idx == seg_idx) {
            Some(l) => *l,
            None => continue,
        };
        // Synthesize an owned rope for blocks (their raw rope is
        // their on-screen content); prose segments hand us their
        // rope directly.
        let rope_owned: ropey::Rope;
        let rope: &ropey::Rope = match seg {
            Segment::Prose(r) => r,
            Segment::Block(b) => {
                rope_owned = b.raw.clone();
                &rope_owned
            }
        };
        let total = rope.len_chars();
        // What slice of this segment is selected?
        let seg_lo_off = if seg_idx == lo_seg {
            lo_off.min(total)
        } else {
            0
        };
        let seg_hi_off = if seg_idx == hi_seg {
            hi_off.min(total)
        } else {
            total
        };
        if seg_hi_off < seg_lo_off {
            continue;
        }

        let (start_line, start_col, end_line, end_col_inclusive) = if overlay.linewise {
            let lo_line = rope.char_to_line(seg_lo_off);
            let hi_line = if total == 0 {
                0
            } else {
                rope.char_to_line(seg_hi_off.saturating_sub(0).min(total))
            };
            (lo_line, 0usize, hi_line, usize::MAX)
        } else {
            let lo_line = rope.char_to_line(seg_lo_off);
            let lo_col = seg_lo_off - rope.line_to_char(lo_line);
            let hi_line = if total == 0 {
                0
            } else {
                rope.char_to_line(seg_hi_off.min(total))
            };
            let hi_col = seg_hi_off.saturating_sub(rope.line_to_char(hi_line));
            (lo_line, lo_col, hi_line, hi_col)
        };

        // Map raw line index → screen Y. For prose segments lines
        // are contiguous (rope_line N → y_start + N). Block segments
        // have chrome (top border + header bar) above the fence
        // header and a result panel between the body and the
        // closer, so the mapping is non-linear.
        let line_to_y: Box<dyn Fn(usize) -> u16> = match seg {
            Segment::Prose(_) => {
                let y_start = layout.y_start;
                Box::new(move |line: usize| y_start.saturating_add(line as u16))
            }
            Segment::Block(_) => {
                let y_start = layout.y_start;
                let height = layout.height;
                let last_raw = rope.len_lines().saturating_sub(1);
                Box::new(move |line: usize| {
                    if line == 0 {
                        // Fence header sits just inside the top
                        // border + chrome header bar.
                        y_start.saturating_add(2)
                    } else if line >= last_raw {
                        // Closer sits one row above the chrome
                        // footer bar and bottom border.
                        y_start.saturating_add(height.saturating_sub(3))
                    } else {
                        // Body line N (raw line N): right after
                        // the fence header.
                        y_start.saturating_add(2).saturating_add(line as u16)
                    }
                })
            }
        };

        paint_segment_highlight(
            frame,
            area,
            viewport_top,
            line_to_y.as_ref(),
            rope,
            start_line,
            start_col,
            end_line,
            end_col_inclusive,
            overlay.linewise,
            style,
            // Charwise selection is "inclusive at both ends" only
            // when this segment owns the hi endpoint; mid-segment
            // highlight paints the whole line.
            seg_idx == hi_seg,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_segment_highlight(
    frame: &mut Frame,
    area: Rect,
    viewport_top: u16,
    line_to_y: &dyn Fn(usize) -> u16,
    rope: &ropey::Rope,
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col_inclusive: usize,
    linewise: bool,
    style: Style,
    inclusive_hi: bool,
) {
    let buf = frame.buffer_mut();
    let total_lines = rope.len_lines();
    for line in start_line..=end_line {
        if line >= total_lines {
            break;
        }
        let absolute_y = line_to_y(line);
        if absolute_y < viewport_top {
            continue;
        }
        let y = absolute_y - viewport_top;
        if y >= area.height {
            break;
        }
        let line_text = rope.line(line).to_string();
        let line_chars = line_text.trim_end_matches('\n').chars().count();
        let from = if line == start_line { start_col } else { 0 };
        let to = if linewise {
            area.width as usize
        } else if line == end_line && inclusive_hi {
            (end_col_inclusive + 1).min(line_chars.max(1))
        } else {
            line_chars.max(1)
        };
        if to <= from {
            continue;
        }
        let max_x = area.x.saturating_add(area.width);
        for col in from..to {
            let x = area.x.saturating_add(col as u16);
            if x >= max_x {
                break;
            }
            let cell = &mut buf[(x, area.y + y)];
            cell.set_style(style);
        }
    }
}

/// Paint a yellow background under each match of `pattern` in the
/// visible portion of `rope`. Smartcase via [`search::is_case_sensitive`].
/// The overlay only sets the bg / fg fields, so existing markdown
/// styling (bold, italics, link colors) survives untouched in cells
/// that aren't matched.
pub(crate) fn overlay_search_highlights(
    frame: &mut Frame,
    area: Rect,
    rope: &Rope,
    top_line: usize,
    pattern: &str,
) {
    if pattern.is_empty() {
        return;
    }
    let case_sensitive = search::is_case_sensitive(pattern);
    let highlight = Style::default().bg(Color::Yellow).fg(Color::Black);
    let total = rope.len_lines();
    let buf = frame.buffer_mut();

    for row in 0..area.height as usize {
        let line_idx = top_line + row;
        if line_idx >= total {
            break;
        }
        let raw = rope.line(line_idx).to_string();
        let line_text = raw.trim_end_matches('\n');
        let matches = search::find_matches_in_line(line_text, pattern, case_sensitive);
        for (start, end) in matches {
            // `find_matches_in_line` returns char ranges; for ASCII
            // markdown the column is the same as the char count.
            let col_start = start as u16;
            let col_end = end as u16;
            let width = col_end.saturating_sub(col_start);
            if width == 0 {
                continue;
            }
            let x = area.x.saturating_add(col_start);
            let y = area.y.saturating_add(row as u16);
            let max_x = area.x.saturating_add(area.width);
            if x >= max_x || y >= area.y.saturating_add(area.height) {
                continue;
            }
            let visible_width = width.min(max_x - x);
            let rect = Rect {
                x,
                y,
                width: visible_width,
                height: 1,
            };
            buf.set_style(rect, highlight);
        }
    }
}
