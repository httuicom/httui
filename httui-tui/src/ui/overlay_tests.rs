use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn sel_bg() -> Color {
    crate::ui::palette::selection_bg()
}

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
fn paint_sel(d: &Document, vp: u16, ov: VisualOverlay, w: u16, h: u16) -> ratatui::buffer::Buffer {
    paint_sel_panned(d, vp, 0, ov, w, h)
}

fn paint_sel_panned(
    d: &Document,
    vp: u16,
    left: u16,
    ov: VisualOverlay,
    w: u16,
    h: u16,
) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            overlay_visual_selection(f, Rect::new(0, 0, w, h), d, vp, left, ov);
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
    assert_eq!(count_bg(&buf, 30, 4, sel_bg()), 0);
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
    assert_eq!(count_bg(&buf, 40, 8, sel_bg()), 0);
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
    let n = count_bg(&buf, 30, 3, sel_bg());
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
    assert!(count_bg(&buf, 20, 3, sel_bg()) >= 4);
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
    assert!(count_bg(&buf, 24, 5, sel_bg()) >= 24);
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
    assert!(count_bg(&buf, 50, 12, sel_bg()) >= 1);
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
    assert!(count_bg(&buf, 50, 14, sel_bg()) >= 1);
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
    assert_eq!(count_bg(&buf, 20, 4, sel_bg()), 0);
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
    assert!(count_bg(&buf, 10, 2, sel_bg()) <= 20);
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
    assert!(count_bg(&buf, 10, 4, sel_bg()) <= 1);
}

// ---- overlay_search_highlights ---------------------------------

fn paint_search(rope_text: &str, top: usize, pat: &str, w: u16, h: u16) -> usize {
    paint_search_panned(rope_text, top, pat, w, h, 0).0
}

/// Returns (yellow cell count, leftmost yellow x if any).
fn paint_search_panned(
    rope_text: &str,
    top: usize,
    pat: &str,
    w: u16,
    h: u16,
    left: u16,
) -> (usize, Option<u16>) {
    let d = doc(rope_text);
    let rope = d.segments()[0].as_prose().unwrap().clone();
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            overlay_search_highlights(f, Rect::new(0, 0, w, h), &rope, top, pat, left);
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    let n = count_bg(&buf, w, h, Color::Yellow);
    let first_x = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .find(|&(x, y)| buf.cell((x, y)).unwrap().bg == Color::Yellow)
        .map(|(x, _)| x);
    (n, first_x)
}

#[test]
fn search_highlight_shifts_left_under_pan() {
    // Match at cols 10..16; pan of 8 → highlight lands at x 2..8.
    let line = format!("{}needle tail\n", " ".repeat(10));
    let (n, first_x) = paint_search_panned(&line, 0, "needle", 20, 2, 8);
    assert_eq!(n, 6, "full match visible under pan");
    assert_eq!(first_x, Some(2), "highlight shifted by the pan");
}

#[test]
fn search_highlight_match_left_of_pan_is_clipped() {
    // Match at cols 0..6 entirely left of a pan of 10 → invisible.
    let (n, _x) = paint_search_panned("needle and more\n", 0, "needle", 20, 2, 10);
    assert_eq!(n, 0, "match scrolled off the left edge");
}

#[test]
fn visual_selection_shifts_with_pan_on_cursor_segment() {
    let line = format!("{}abcdef\n", " ".repeat(20));
    let mut d = doc(&line);
    // Select chars 20..=24 with the cursor at 24; pan of 18.
    d.set_cursor(Cursor::InProse {
        segment_idx: 0,
        offset: 24,
    });
    let ov = VisualOverlay {
        anchor: Cursor::InProse {
            segment_idx: 0,
            offset: 20,
        },
        linewise: false,
    };
    let buf = paint_sel_panned(&d, 0, 18, ov, 12, 2);
    // Display cols 20..25 minus pan 18 → x 2..7.
    assert_eq!(count_bg(&buf, 12, 2, sel_bg()), 5);
    assert_eq!(buf.cell((2, 0)).unwrap().bg, sel_bg());
    assert_ne!(buf.cell((1, 0)).unwrap().bg, sel_bg());
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
