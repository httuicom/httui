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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    const SEL_BG: Color = Color::Rgb(60, 70, 110);

    fn doc(md: &str) -> Document {
        Document::from_markdown(md).unwrap()
    }

    fn block_seg(d: &Document) -> usize {
        d.segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap()
    }

    /// Paint `overlay_visual_selection` over a W×H buffer and return
    /// the buffer for per-cell bg asserts.
    fn paint_sel(
        d: &Document,
        vp: u16,
        ov: VisualOverlay,
        w: u16,
        h: u16,
    ) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                overlay_visual_selection(f, Rect::new(0, 0, w, h), d, vp, ov);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn count_bg(buf: &ratatui::buffer::Buffer, w: u16, h: u16, want: Color) -> usize {
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|&(x, y)| buf.cell((x, y)).unwrap().bg == want)
            .count()
    }

    // ---- overlay_visual_selection: anchor variants -----------------

    #[test]
    fn visual_selection_in_block_result_anchor_early_returns() {
        let d = doc("some prose here\n");
        let ov = VisualOverlay {
            anchor: Cursor::InBlockResult {
                segment_idx: 0,
                row: 0,
            },
            linewise: false,
        };
        let buf = paint_sel(&d, 0, ov, 30, 4);
        // Early return → nothing highlighted.
        assert_eq!(count_bg(&buf, 30, 4, SEL_BG), 0);
    }

    #[test]
    fn visual_selection_cursor_in_block_result_early_returns() {
        let mut d = doc("```db-postgres alias=q connection=c\nSELECT 1\n```\n");
        let seg = block_seg(&d);
        // Cursor is InBlockResult while anchor is a valid InProse →
        // the cursor-side match arm early-returns.
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: seg,
            row: 0,
        });
        let ov = VisualOverlay {
            anchor: Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            linewise: false,
        };
        let buf = paint_sel(&d, 0, ov, 40, 8);
        assert_eq!(count_bg(&buf, 40, 8, SEL_BG), 0);
    }

    #[test]
    fn visual_selection_charwise_prose_highlights_inclusive_range() {
        let mut d = doc("hello world\n");
        // Anchor at col 0, cursor at col 4 → inclusive [0,4] = 5 cells.
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 4,
        });
        let ov = VisualOverlay {
            anchor: Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            linewise: false,
        };
        let buf = paint_sel(&d, 0, ov, 30, 3);
        let n = count_bg(&buf, 30, 3, SEL_BG);
        assert!(n >= 5, "expected ≥5 highlighted cells, got {n}");
    }

    #[test]
    fn visual_selection_anchor_after_cursor_swaps_lo_hi() {
        let mut d = doc("abcdef\n");
        // Anchor at offset 5, cursor at offset 1 → lo/hi swap path.
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 1,
        });
        let ov = VisualOverlay {
            anchor: Cursor::InProse {
                segment_idx: 0,
                offset: 5,
            },
            linewise: false,
        };
        let buf = paint_sel(&d, 0, ov, 20, 3);
        assert!(count_bg(&buf, 20, 3, SEL_BG) >= 4);
    }

    #[test]
    fn visual_selection_linewise_paints_full_width_rows() {
        let mut d = doc("line one\nline two\nline three\n");
        // Linewise selection spanning lines 0..2.
        let off = d.segments()[0]
            .as_prose()
            .map(|r| r.line_to_char(2))
            .unwrap_or(0);
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: off,
        });
        let ov = VisualOverlay {
            anchor: Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            linewise: true,
        };
        let buf = paint_sel(&d, 0, ov, 24, 5);
        // Linewise → every cell of selected rows highlighted; at
        // least the full first row width.
        assert!(count_bg(&buf, 24, 5, SEL_BG) >= 24);
    }

    #[test]
    fn visual_selection_in_block_uses_nonlinear_line_to_y() {
        let mut d = doc("```db-postgres alias=q connection=c\nSELECT 1\nFROM t\n```\n");
        let seg = block_seg(&d);
        let raw_end = match &d.segments()[seg] {
            Segment::Block(b) => b.raw.len_chars(),
            _ => unreachable!(),
        };
        // Anchor at block start, cursor near block end → block branch
        // of the line_to_y closure (header / body / closer mapping).
        d.set_cursor(Cursor::InBlock {
            segment_idx: seg,
            offset: raw_end.saturating_sub(1),
        });
        let ov = VisualOverlay {
            anchor: Cursor::InBlock {
                segment_idx: seg,
                offset: 0,
            },
            linewise: false,
        };
        let buf = paint_sel(&d, 0, ov, 50, 12);
        assert!(count_bg(&buf, 50, 12, SEL_BG) >= 1);
    }

    #[test]
    fn visual_selection_cross_segment_loops_lo_to_hi() {
        let src = "intro line\n\n```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let mut d = doc(src);
        let seg = block_seg(&d);
        // Anchor in the prose intro, cursor inside the block → the
        // lo_seg..=hi_seg loop covers a prose AND a block segment.
        d.set_cursor(Cursor::InBlock {
            segment_idx: seg,
            offset: 0,
        });
        let ov = VisualOverlay {
            anchor: Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            linewise: true,
        };
        let buf = paint_sel(&d, 0, ov, 50, 14);
        assert!(count_bg(&buf, 50, 14, SEL_BG) >= 1);
    }

    // ---- paint_segment_highlight clip paths ------------------------

    #[test]
    fn visual_selection_clipped_above_viewport_paints_nothing() {
        let body: String = (0..30).map(|i| format!("row{i}\n")).collect();
        let mut d = doc(&body);
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 3,
        });
        let ov = VisualOverlay {
            anchor: Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            linewise: false,
        };
        // viewport scrolled far past the selected rows → absolute_y <
        // viewport_top, every cell skipped.
        let buf = paint_sel(&d, 25, ov, 20, 4);
        assert_eq!(count_bg(&buf, 20, 4, SEL_BG), 0);
    }

    #[test]
    fn visual_selection_below_area_height_breaks_out() {
        let body: String = (0..40).map(|i| format!("L{i}\n")).collect();
        let mut d = doc(&body);
        // Select a wide line range; tiny 2-row area forces the
        // `y >= area.height` break.
        let off = d.segments()[0]
            .as_prose()
            .map(|r| r.line_to_char(30))
            .unwrap_or(0);
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: off,
        });
        let ov = VisualOverlay {
            anchor: Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            linewise: true,
        };
        let buf = paint_sel(&d, 0, ov, 10, 2);
        // Only the first 2 rows can be painted; no panic, ≤ 2*10 cells.
        assert!(count_bg(&buf, 10, 2, SEL_BG) <= 20);
    }

    #[test]
    fn visual_selection_empty_range_to_le_from_continues() {
        // anchor == cursor at same offset; charwise → to may collapse
        // to <= from on a zero-width line, exercising the continue.
        let mut d = doc("\n\nx\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        let ov = VisualOverlay {
            anchor: Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            linewise: false,
        };
        let buf = paint_sel(&d, 0, ov, 10, 4);
        // Selection of a single empty position — does not panic; the
        // highlight (if any) is minimal.
        assert!(count_bg(&buf, 10, 4, SEL_BG) <= 1);
    }

    // ---- overlay_search_highlights ---------------------------------

    fn paint_search(rope_text: &str, top: usize, pat: &str, w: u16, h: u16) -> usize {
        let d = doc(rope_text);
        let rope = d.segments()[0].as_prose().unwrap().clone();
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                overlay_search_highlights(f, Rect::new(0, 0, w, h), &rope, top, pat);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        count_bg(&buf, w, h, Color::Yellow)
    }

    #[test]
    fn search_highlight_empty_pattern_early_returns() {
        assert_eq!(paint_search("needle here\n", 0, "", 30, 3), 0);
    }

    #[test]
    fn search_highlight_marks_each_match_yellow() {
        let n = paint_search("find the needle now\n", 0, "needle", 40, 3);
        assert!(n >= 6, "expected ≥6 yellow cells, got {n}");
    }

    #[test]
    fn search_highlight_smartcase_lowercase_is_insensitive() {
        // Lowercase pattern → case-insensitive: matches "Needle".
        let n = paint_search("a Needle in here\n", 0, "needle", 40, 3);
        assert!(n >= 6, "smartcase should match Needle, got {n}");
    }

    #[test]
    fn search_highlight_uppercase_is_case_sensitive() {
        // Pattern with an uppercase char → case-sensitive: "needle"
        // (lowercase in text) must NOT match "Needle" pattern.
        let n = paint_search("only needle lower\n", 0, "Needle", 40, 3);
        assert_eq!(n, 0, "case-sensitive pattern should not match");
    }

    #[test]
    fn search_highlight_scrolled_lines_break_when_past_total() {
        // top_line beyond the rope's line count → loop breaks
        // immediately, no panic, nothing painted.
        let n = paint_search("one\ntwo\n", 99, "two", 20, 4);
        assert_eq!(n, 0);
    }

    #[test]
    fn search_highlight_match_clipped_at_area_right_edge() {
        // Match starts near the right edge; visible_width clamps so
        // x never exceeds max_x.
        let line = format!("{}needle\n", " ".repeat(8));
        let n = paint_search(&line, 0, "needle", 10, 2);
        // Area is only 10 wide; the match starting at col 8 is mostly
        // clipped — at most 2 cells painted, no panic.
        assert!(n <= 2, "expected clamp, got {n}");
    }
}
