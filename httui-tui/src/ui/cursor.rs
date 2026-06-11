//! Place the terminal cursor over the editor area.
//!
//! We use the real terminal cursor via [`Frame::set_cursor_position`]
//! — the shape (block / bar / underline) follows whatever the user
//! configured in their emulator, and it blinks natively. Painting
//! cells manually breaks when the chosen colors collide with the
//! terminal theme (why the cursor was invisible on dark backgrounds).
//!
//! `InBlock` lands the terminal cursor inside the block widget at the
//! requested `(line, offset)` — accounting for the 1-row top border.

use ratatui::{layout::Rect, Frame};
use ropey::Rope;

use crate::buffer::viewport2d::{display_col, project_x};
use crate::buffer::Cursor;

pub fn render_prose_cursor(
    frame: &mut Frame,
    area: Rect,
    rope: &Rope,
    cursor: Cursor,
    rope_top_line: usize,
    left: u16,
) {
    let Cursor::InProse { offset, .. } = cursor else {
        return;
    };

    let (line_idx, col_idx) = offset_to_line_col(rope, offset);
    if line_idx < rope_top_line {
        return;
    }
    let row_in_area = (line_idx - rope_top_line) as u16;
    if row_in_area >= area.height {
        return;
    }
    let cursor_x = display_col(rope.line(line_idx).chars(), col_idx);
    let Some(x) = project_x(cursor_x, left, area.x, area.width) else {
        return;
    };
    let y = area.y + row_in_area;
    frame.set_cursor_position((x, y));
}

/// Park the terminal cursor inside a focused block at the requested
/// body row — `line` is body-relative (line 0 = first body row),
/// `cursor_x` the display column inside that line, `left` the
/// horizontal pan of the body text. A cursor outside the panned
/// window hides instead of clamping (a clamped caret lies about its
/// column).
///
/// Card layout: top border → header bar → fence header → body →
/// fence closer → footer bar → bottom border. Body starts at
/// `area.y + 3` (border + chrome header + fence header) and the
/// first body cell is one column right of the left border.
pub fn render_inblock_cursor(frame: &mut Frame, area: Rect, line: usize, cursor_x: u16, left: u16) {
    if area.width <= 1 || area.height <= 4 {
        return;
    }
    let max_y = area.y.saturating_add(area.height.saturating_sub(3));
    let Some(x) = project_x(
        cursor_x,
        left,
        area.x.saturating_add(1),
        area.width.saturating_sub(2),
    ) else {
        return;
    };
    let y = area
        .y
        .saturating_add(3)
        .saturating_add(line as u16)
        .min(max_y);
    frame.set_cursor_position((x, y));
}

fn offset_to_line_col(rope: &Rope, offset: usize) -> (usize, usize) {
    let total = rope.len_chars();
    let off = offset.min(total);
    let line = rope.char_to_line(off);
    let line_start = rope.line_to_char(line);
    (line, off - line_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_zero_is_origin() {
        let r = Rope::from_str("abc\ndef\n");
        assert_eq!(offset_to_line_col(&r, 0), (0, 0));
    }

    #[test]
    fn offset_after_newline_is_next_line() {
        let r = Rope::from_str("abc\ndef\n");
        // 'd' is at offset 4 (a, b, c, \n)
        assert_eq!(offset_to_line_col(&r, 4), (1, 0));
    }

    #[test]
    fn offset_clamps_at_end() {
        let r = Rope::from_str("abc\n");
        let (line, _col) = offset_to_line_col(&r, 999);
        assert!(line <= r.len_lines());
    }
}
