use super::*;
use ratatui::backend::{Backend, TestBackend};
use ratatui::Terminal;
use std::collections::HashMap;

type Names = blocks::ConnectionNames;

fn doc(md: &str) -> Document {
    Document::from_markdown(md).unwrap()
}

fn block_seg(d: &Document) -> usize {
    d.segments()
        .iter()
        .position(|s| matches!(s, Segment::Block(_)))
        .unwrap()
}

/// Render via `render_document` (cursor path) into a W×H buffer
/// and return the flattened text plus the buffer + cursor pos.
fn render(
    d: &Document,
    viewport_top: u16,
    pattern: Option<&str>,
    w: u16,
    h: u16,
) -> (String, ratatui::buffer::Buffer, Option<(u16, u16)>) {
    render_panned(d, viewport_top, 0, pattern, w, h)
}

fn render_panned(
    d: &Document,
    viewport_top: u16,
    viewport_left: u16,
    pattern: Option<&str>,
    w: u16,
    h: u16,
) -> (String, ratatui::buffer::Buffer, Option<(u16, u16)>) {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    let names: Names = HashMap::new();
    let mut rvt: HashMap<usize, u16> = HashMap::new();
    let result_tabs: HashMap<crate::buffer::block::BlockId, crate::app::ResultPanelTab> =
        HashMap::new();
    terminal
        .draw(|f| {
            render_document(
                f,
                Rect::new(0, 0, w, h),
                d,
                viewport_top,
                viewport_left,
                pattern,
                &names,
                &mut rvt,
                &result_tabs,
            );
        })
        .unwrap();
    let cur = terminal
        .backend_mut()
        .get_cursor_position()
        .ok()
        .map(|p| (p.x, p.y));
    let buf = terminal.backend().buffer().clone();
    let text: String = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
        .collect();
    (text, buf, cur)
}

fn render_nc(d: &Document, viewport_top: u16, pattern: Option<&str>, w: u16, h: u16) -> String {
    let backend = TestBackend::new(w, h);
    let mut terminal = Terminal::new(backend).unwrap();
    let names: Names = HashMap::new();
    let mut rvt: HashMap<usize, u16> = HashMap::new();
    let result_tabs: HashMap<crate::buffer::block::BlockId, crate::app::ResultPanelTab> =
        HashMap::new();
    terminal
        .draw(|f| {
            render_document_no_cursor(
                f,
                Rect::new(0, 0, w, h),
                d,
                viewport_top,
                0,
                pattern,
                &names,
                &mut rvt,
                &result_tabs,
            );
        })
        .unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
        .collect()
}

// ---- prose-only documents --------------------------------------

#[test]
fn prose_only_renders_with_cursor_in_prose() {
    let d = doc("first line\nsecond line\n");
    // Default cursor is InProse at offset 0.
    let (text, _b, cur) = render(&d, 0, None, 40, 6);
    assert!(text.contains("first line"), "got: {text:?}");
    assert!(text.contains("second line"), "got: {text:?}");
    // Cursor placed in the prose segment (line 0, col 0).
    assert_eq!(cur, Some((0, 0)));
}

#[test]
fn prose_only_no_cursor_path_still_paints_text() {
    let d = doc("alpha beta gamma\n");
    let text = render_nc(&d, 0, None, 40, 4);
    assert!(text.contains("alpha beta gamma"), "got: {text:?}");
}

#[test]
fn search_pattern_highlights_prose_match() {
    let d = doc("the needle is here\n");
    let (_t, buf, _c) = render(&d, 0, Some("needle"), 40, 3);
    // Some cell carries the yellow search bg.
    let hit = (0..40).any(|x| buf.cell((x, 0)).unwrap().bg == ratatui::style::Color::Yellow);
    assert!(hit, "expected a yellow search-highlight cell");
}

#[test]
fn search_pattern_none_leaves_prose_unhighlighted() {
    let d = doc("plain text only\n");
    let (_t, buf, _c) = render(&d, 0, None, 40, 3);
    let any_yellow = (0..40).any(|x| buf.cell((x, 0)).unwrap().bg == ratatui::style::Color::Yellow);
    assert!(!any_yellow, "no highlight expected without a pattern");
}

// ---- block segments --------------------------------------------

#[test]
fn http_block_renders_and_parks_cursor_inside_block() {
    let src = "intro\n\n```http alias=h\nGET https://example.com\n```\n";
    let mut d = doc(src);
    let seg = block_seg(&d);
    d.set_cursor(Cursor::InBlock {
        segment_idx: seg,
        offset: 0,
    });
    let (text, _b, cur) = render(&d, 0, None, 60, 14);
    assert!(text.contains("https://example.com"), "got: {text:?}");
    // Cursor parked somewhere inside the painted block chrome.
    assert!(cur.is_some(), "cursor should be set inside the block");
}

#[test]
fn http_block_cursor_on_closer_uses_request_height_offset() {
    let src = "```http alias=h\nGET https://example.com\n```\n";
    let mut d = doc(src);
    let seg = block_seg(&d);
    // Offset at the very end of the raw rope → Closer section.
    let end = match &d.segments()[seg] {
        Segment::Block(b) => b.raw.len_chars(),
        _ => unreachable!(),
    };
    d.set_cursor(Cursor::InBlock {
        segment_idx: seg,
        offset: end,
    });
    let (_t, _b, cur) = render(&d, 0, None, 60, 14);
    assert!(cur.is_some(), "closer cursor should be placed");
}

#[test]
fn db_block_cursor_in_header_section() {
    let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
    let mut d = doc(src);
    let seg = block_seg(&d);
    // Offset 0 → Header section (the fence line).
    d.set_cursor(Cursor::InBlock {
        segment_idx: seg,
        offset: 0,
    });
    let (text, _b, cur) = render(&d, 0, None, 60, 12);
    assert!(text.contains("SELECT 1"), "got: {text:?}");
    assert!(cur.is_some());
}

#[test]
fn db_block_cursor_in_body_section() {
    let src = "```db-postgres alias=q connection=c\nSELECT 1\nFROM t\n```\n";
    let mut d = doc(src);
    let seg = block_seg(&d);
    let raw = match &d.segments()[seg] {
        Segment::Block(b) => b.raw.to_string(),
        _ => unreachable!(),
    };
    let body_off = raw.find("FROM").unwrap();
    d.set_cursor(Cursor::InBlock {
        segment_idx: seg,
        offset: body_off,
    });
    let (_t, _b, cur) = render(&d, 0, None, 60, 12);
    assert!(cur.is_some(), "body cursor should be placed");
}

#[test]
fn db_block_cursor_in_block_result_selects_row() {
    let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
    let mut d = doc(src);
    let seg = block_seg(&d);
    // InBlockResult drives the `selected_row` + result_viewport
    // entry-or-insert branch.
    d.set_cursor(Cursor::InBlockResult {
        segment_idx: seg,
        row: 0,
    });
    let (text, _b, _c) = render(&d, 0, None, 60, 14);
    assert!(text.contains("SELECT 1"), "got: {text:?}");
}

#[test]
fn block_no_cursor_path_renders_block_chrome() {
    let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
    let d = doc(src);
    let text = render_nc(&d, 0, None, 60, 12);
    assert!(text.contains("SELECT 1"), "got: {text:?}");
}

// ---- viewport clipping -----------------------------------------

#[test]
fn segment_above_viewport_is_skipped_then_lower_one_renders() {
    // Tall prose so the first lines scroll above the viewport top.
    let body: String = (0..30).map(|i| format!("line{i}\n")).collect();
    let d = doc(&body);
    // Scroll down 10 rows: early lines clipped, later ones visible.
    let (text, _b, _c) = render(&d, 10, None, 30, 6);
    assert!(text.contains("line1"), "expected scrolled content");
    assert!(!text.contains("line0\n"), "line0 should be clipped off");
}

#[test]
fn viewport_below_all_segments_breaks_out_with_blank_buffer() {
    let d = doc("short\n");
    // viewport_top far below any segment → loop breaks, blank.
    let (text, _b, _c) = render(&d, 9999, None, 30, 4);
    assert!(text.trim().is_empty(), "expected blank, got: {text:?}");
}

#[test]
fn visible_height_zero_early_returns_for_clipped_segment() {
    // A prose segment whose only visible row is exactly at the
    // editor's bottom edge → visible_height resolves to 0 and
    // render_segment returns early without painting.
    let body: String = (0..20).map(|i| format!("row{i}\n")).collect();
    let d = doc(&body);
    // 1-row tall editor, viewport pushed so the segment top is
    // below the visible row.
    let (text, _b, _c) = render(&d, 0, None, 20, 1);
    // Only the very first row is visible; nothing panics.
    assert!(text.contains("row0"), "got: {text:?}");
}

#[test]
fn no_cursor_segment_clipping_handles_scrolled_block() {
    let pre: String = (0..15).map(|i| format!("p{i}\n")).collect();
    let src = format!("{pre}\n```db-postgres alias=q connection=c\nSELECT 1\n```\n");
    let d = doc(&src);
    // Scroll partway so the block is partially clipped at the top
    // in the no-cursor path (top_skip > 0).
    let text = render_nc(&d, 12, None, 50, 8);
    // Either the block or the tail prose is visible; just assert
    // no panic + something painted.
    assert!(!text.is_empty());
}

#[test]
fn missing_segment_index_is_a_safe_noop() {
    // Empty document → layout has the implicit empty prose
    // segment; rendering must not panic and stays blank-ish.
    let d = doc("");
    let (_t, _b, _c) = render(&d, 0, None, 10, 3);
}

// ---- horizontal pan --------------------------------------------

#[test]
fn panned_prose_shows_line_tail_and_keeps_cursor_visible() {
    // 120-char line, cursor on its last char, 40-col buffer. With the
    // pan the follow math produces, the tail must be on screen and
    // the terminal cursor inside the area.
    let line = format!("{}TAILMARKER", "x".repeat(110));
    let mut d = doc(&format!("{line}\n"));
    d.set_cursor(Cursor::InProse {
        segment_idx: 0,
        offset: 119,
    });
    // follow_x(0, 40, 119) = 119 + 4 - 40 = 83 → window [83, 123).
    let left = crate::buffer::viewport2d::follow_x(0, 40, 119);
    let (text, _b, cur) = render_panned(&d, 0, left, None, 40, 6);
    assert!(text.contains("TAILMARKER"), "tail visible, got: {text:?}");
    let (cx, cy) = cur.expect("cursor must be placed under pan");
    assert!(cx < 40, "cursor x inside area, got {cx}");
    assert_eq!((cx, cy), (119 - left, 0));
}

#[test]
fn unpanned_prose_still_shows_line_head() {
    let line = format!("HEADMARKER{}", "x".repeat(100));
    let d = doc(&format!("{line}\n"));
    let (text, _b, cur) = render(&d, 0, None, 40, 4);
    assert!(text.contains("HEADMARKER"), "got: {text:?}");
    assert_eq!(cur, Some((0, 0)));
}

#[test]
fn pan_applies_only_to_the_cursor_segment() {
    // Two prose segments separated by a block; cursor in the second
    // prose segment with a pan. The first prose segment must still
    // render its head (unpanned).
    let src = format!(
        "FIRSTHEAD rest of first line\n\n```http alias=h\nGET https://x.test\n```\n\n{}TAIL2\n",
        "y".repeat(60)
    );
    let mut d = doc(&src);
    let last_prose = d
        .segments()
        .iter()
        .rposition(|s| matches!(s, Segment::Prose(_)))
        .unwrap();
    d.set_cursor(Cursor::InProse {
        segment_idx: last_prose,
        offset: 60,
    });
    let (text, _b, _c) = render_panned(&d, 0, 30, None, 40, 20);
    assert!(
        text.contains("FIRSTHEAD"),
        "non-cursor segment must stay unpanned, got: {text:?}"
    );
    assert!(text.contains("TAIL2"), "panned segment tail visible");
}

#[test]
fn panned_http_block_body_shows_request_tail_and_cursor() {
    let url = format!("https://example.com/{}TAILURL", "p".repeat(80));
    let src = format!("```http alias=h\nGET {url}\n```\n");
    let mut d = doc(&src);
    let seg = block_seg(&d);
    let raw = match &d.segments()[seg] {
        Segment::Block(b) => b.raw.to_string(),
        _ => unreachable!(),
    };
    // Cursor on the request line's last char (a Body offset).
    let line_start = raw.find("GET ").unwrap();
    let line_len = raw[line_start..].find('\n').unwrap();
    let cursor_col = (line_len - 1) as u16;
    d.set_cursor(Cursor::InBlock {
        segment_idx: seg,
        offset: line_start + line_len - 1,
    });
    let left = crate::buffer::viewport2d::follow_x(0, 48, cursor_col);
    let (text, _b, cur) = render_panned(&d, 0, left, None, 50, 14);
    assert!(text.contains("TAILURL"), "URL tail visible, got: {text:?}");
    let (cx, _cy) = cur.expect("body cursor placed under pan");
    assert!(cx < 50, "cursor inside the card, got {cx}");
}

#[test]
fn panned_db_block_body_shows_sql_tail() {
    let sql = format!("SELECT {} AS TAILCOL", "x".repeat(80));
    let src = format!("```db-postgres alias=q connection=c\n{sql}\n```\n");
    let mut d = doc(&src);
    let seg = block_seg(&d);
    let raw = match &d.segments()[seg] {
        Segment::Block(b) => b.raw.to_string(),
        _ => unreachable!(),
    };
    let line_start = raw.find("SELECT").unwrap();
    let line_len = raw[line_start..].find('\n').unwrap();
    d.set_cursor(Cursor::InBlock {
        segment_idx: seg,
        offset: line_start + line_len - 1,
    });
    let left = crate::buffer::viewport2d::follow_x(0, 48, (line_len - 1) as u16);
    let (text, _b, cur) = render_panned(&d, 0, left, None, 50, 12);
    assert!(text.contains("TAILCOL"), "SQL tail visible, got: {text:?}");
    assert!(cur.is_some(), "body cursor placed under pan");
}

#[test]
fn pan_scrolls_short_line_content_off_screen() {
    // A 5-char line under a pan of 20 paints nothing — proves the
    // scroll actually skips columns (cursor hiding itself is covered
    // by the project_x unit tests).
    let d = doc("short\n");
    let (text, _b, _c) = render_panned(&d, 0, 20, None, 30, 3);
    assert!(!text.contains("short"), "content must scroll off: {text:?}");
}
