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
