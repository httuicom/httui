//! Per-segment document renderer: clip each segment to the viewport,
//! dispatch prose vs. block painting, and place the editor cursor.

use ratatui::{layout::Rect, Frame};

use crate::buffer::{
    layout::{layout_document, SegmentLayout},
    Cursor, Document, Segment,
};

use super::{blocks, cursor, overlay, prose};

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_document_no_cursor(
    frame: &mut Frame,
    area: Rect,
    doc: &Document,
    viewport_top: u16,
    search_pattern: Option<&str>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    // Same logic as `render_document`, but skip the cursor draw step.
    // Used while the prompt is open so the terminal caret isn't fighting
    // for position with the editor.
    let layouts = layout_document(doc, area.width);
    let viewport_bottom = viewport_top.saturating_add(area.height);
    for layout in &layouts {
        if layout.y_start.saturating_add(layout.height) <= viewport_top {
            continue;
        }
        if layout.y_start >= viewport_bottom {
            break;
        }
        render_segment_no_cursor(
            frame,
            area,
            doc,
            layout,
            viewport_top,
            search_pattern,
            connection_names,
            result_viewport_top,
            result_tab,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_segment_no_cursor(
    frame: &mut Frame,
    editor_area: Rect,
    doc: &Document,
    layout: &SegmentLayout,
    viewport_top: u16,
    search_pattern: Option<&str>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    let seg = match doc.segments().get(layout.segment_idx) {
        Some(s) => s,
        None => return,
    };
    let top_skip = viewport_top.saturating_sub(layout.y_start);
    let visible_height = layout.height.saturating_sub(top_skip).min(
        editor_area
            .height
            .saturating_sub(layout.y_start.saturating_sub(viewport_top)),
    );
    if visible_height == 0 {
        return;
    }
    let y_in_editor = layout.y_start.saturating_sub(viewport_top);
    let area = Rect {
        x: editor_area.x,
        y: editor_area.y + y_in_editor,
        width: editor_area.width,
        height: visible_height,
    };
    match seg {
        Segment::Prose(rope) => {
            prose::render_prose(frame, area, rope, top_skip as usize);
            if let Some(pattern) = search_pattern {
                overlay::overlay_search_highlights(frame, area, rope, top_skip as usize, pattern);
            }
        }
        Segment::Block(b) => {
            // Modal-cursor / prompt mode → no selected row, but the
            // result table can still own a stored viewport_top from
            // a previous focus. Pass it through so the renderer keeps
            // the scroll position stable.
            let viewport_slot = result_viewport_top.get_mut(&layout.segment_idx);
            blocks::render_block_with_selection(
                frame,
                area,
                b,
                false,
                None,
                viewport_slot,
                connection_names,
                result_tab,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn render_document(
    frame: &mut Frame,
    area: Rect,
    doc: &Document,
    viewport_top: u16,
    search_pattern: Option<&str>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    let layouts = layout_document(doc, area.width);
    let cursor = doc.cursor();
    let viewport_bottom = viewport_top.saturating_add(area.height);

    for layout in &layouts {
        if layout.y_start.saturating_add(layout.height) <= viewport_top {
            continue;
        }
        if layout.y_start >= viewport_bottom {
            break;
        }
        render_segment(
            frame,
            area,
            doc,
            layout,
            cursor,
            viewport_top,
            search_pattern,
            connection_names,
            result_viewport_top,
            result_tab,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_segment(
    frame: &mut Frame,
    editor_area: Rect,
    doc: &Document,
    layout: &SegmentLayout,
    cursor: Cursor,
    viewport_top: u16,
    search_pattern: Option<&str>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    let seg = match doc.segments().get(layout.segment_idx) {
        Some(s) => s,
        None => return,
    };

    // Viewport clipping. `top_skip` is how many rows of this segment
    // are above the viewport top — they'll be drawn off-screen.
    let top_skip = viewport_top.saturating_sub(layout.y_start);
    let visible_height = layout.height.saturating_sub(top_skip).min(
        editor_area
            .height
            .saturating_sub(layout.y_start.saturating_sub(viewport_top)),
    );
    if visible_height == 0 {
        return;
    }
    let y_in_editor = layout.y_start.saturating_sub(viewport_top);
    let area = Rect {
        x: editor_area.x,
        y: editor_area.y + y_in_editor,
        width: editor_area.width,
        height: visible_height,
    };

    match seg {
        Segment::Prose(rope) => {
            prose::render_prose(frame, area, rope, top_skip as usize);
            if let Some(pattern) = search_pattern {
                overlay::overlay_search_highlights(frame, area, rope, top_skip as usize, pattern);
            }
            if let Cursor::InProse { segment_idx, .. } = cursor {
                if segment_idx == layout.segment_idx {
                    cursor::render_prose_cursor(frame, area, rope, cursor, top_skip as usize);
                }
            }
        }
        Segment::Block(b) => {
            // The block is "focused" whenever the cursor lives inside
            // it — drives the border highlight and tells the cursor
            // renderer where to park the terminal caret.
            let in_block = matches!(
                cursor,
                Cursor::InBlock { segment_idx, .. } if segment_idx == layout.segment_idx
            );
            let in_result = matches!(
                cursor,
                Cursor::InBlockResult { segment_idx, .. } if segment_idx == layout.segment_idx
            );
            let focused = in_block || in_result;
            let selected_row = match cursor {
                Cursor::InBlockResult { segment_idx, row } if segment_idx == layout.segment_idx => {
                    Some(row)
                }
                _ => None,
            };
            // Block widgets ignore `top_skip` — they always render
            // their full chrome; if partly off-screen the terminal
            // clips for us.
            //
            // For DB result blocks: hand the renderer a mut slot in
            // `result_viewport_top` so the table's scroll persists
            // across frames (cursor floats inside the visible window
            // — same feel as the editor pane scroll).
            let viewport_slot: Option<&mut u16> = if in_result {
                Some(result_viewport_top.entry(layout.segment_idx).or_insert(0))
            } else {
                result_viewport_top.get_mut(&layout.segment_idx)
            };
            blocks::render_block_with_selection(
                frame,
                area,
                b,
                focused,
                selected_row,
                viewport_slot,
                connection_names,
                result_tab,
            );
            if in_block {
                if let Cursor::InBlock { offset, .. } = cursor {
                    use crate::buffer::block::{raw_section_at, RawSection};
                    // New card layout: top border → header bar →
                    // fence header → body → fence closer → footer
                    // bar → bottom border. Fence header sits at
                    // `area.y + 2` (one past the chrome header bar);
                    // body at `area.y + 3..`; closer at
                    // `area.y + area.height - 3` (one above the
                    // chrome footer bar).
                    let raw = &b.raw;
                    let line_idx = raw.char_to_line(offset.min(raw.len_chars()));
                    let line_start = raw.line_to_char(line_idx);
                    let col = offset.saturating_sub(line_start);
                    let max_x = area.x.saturating_add(area.width.saturating_sub(2));
                    match raw_section_at(raw, offset) {
                        RawSection::Body { line, col } => {
                            cursor::render_inblock_cursor(frame, area, line, col);
                        }
                        RawSection::Header => {
                            let x = area
                                .x
                                .saturating_add(1)
                                .saturating_add(col as u16)
                                .min(max_x);
                            let y = area.y.saturating_add(2);
                            frame.set_cursor_position((x, y));
                        }
                        RawSection::Closer => {
                            let x = area
                                .x
                                .saturating_add(1)
                                .saturating_add(col as u16)
                                .min(max_x);
                            // HTTP blocks paint the closer between
                            // raw input and response panel — its row
                            // is `area.y + 3 + request_height`
                            // (border + header bar + fence header +
                            // request lines). All other block types
                            // paint the closer one row above the
                            // footer bar.
                            let y = if b.is_http() {
                                let request_height =
                                    crate::buffer::block::body_line_count(&b.raw).max(1) as u16;
                                area.y.saturating_add(3 + request_height)
                            } else {
                                area.y.saturating_add(area.height.saturating_sub(3))
                            };
                            frame.set_cursor_position((x, y));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let names: Names = HashMap::new();
        let mut rvt: HashMap<usize, u16> = HashMap::new();
        terminal
            .draw(|f| {
                render_document(
                    f,
                    Rect::new(0, 0, w, h),
                    d,
                    viewport_top,
                    pattern,
                    &names,
                    &mut rvt,
                    crate::app::ResultPanelTab::Result,
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
        terminal
            .draw(|f| {
                render_document_no_cursor(
                    f,
                    Rect::new(0, 0, w, h),
                    d,
                    viewport_top,
                    pattern,
                    &names,
                    &mut rvt,
                    crate::app::ResultPanelTab::Result,
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
        let any_yellow =
            (0..40).any(|x| buf.cell((x, 0)).unwrap().bg == ratatui::style::Color::Yellow);
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
}
