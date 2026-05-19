//! Pane-tree walk + split geometry: recurse the binary pane tree,
//! carve each split into child rects, and paint the empty-vault
//! placeholder for a lone empty leaf.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::pane::{PaneNode, SplitDir};

use super::blocks;
use super::overlay::{self, VisualOverlay};
use super::{render_document, render_document_no_cursor};

/// Recursively paint a pane tree into `area`. Each leaf's
/// `viewport_height` is updated to match the rect it was given so
/// motions like `Ctrl+D` know how far to jump on the next tick.
///
/// `focused_path` is the path from the current node to the focused
/// leaf, or `None` if focus lives in another subtree. When the path is
/// `Some([])` the current node *is* the focused leaf.
#[allow(clippy::too_many_arguments)] // bundle into RenderContext if it grows further.
pub(crate) fn render_pane_tree(
    frame: &mut Frame,
    area: Rect,
    node: &mut PaneNode,
    focused_path: Option<&[u8]>,
    suppress_cursor: bool,
    search_pattern: Option<&str>,
    visual_overlay: Option<VisualOverlay>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    match node {
        PaneNode::Leaf(pane) => {
            pane.viewport_height = area.height;
            let is_focused = matches!(focused_path, Some(p) if p.is_empty());
            match pane.document.as_ref() {
                Some(doc) => {
                    if is_focused && !suppress_cursor {
                        render_document(
                            frame,
                            area,
                            doc,
                            pane.viewport_top,
                            search_pattern,
                            connection_names,
                            result_viewport_top,
                            result_tab,
                        );
                    } else {
                        render_document_no_cursor(
                            frame,
                            area,
                            doc,
                            pane.viewport_top,
                            search_pattern,
                            connection_names,
                            result_viewport_top,
                            result_tab,
                        );
                    }
                    // Selection highlight only on the focused leaf —
                    // visual mode is single-pane.
                    if is_focused {
                        if let Some(overlay) = visual_overlay {
                            overlay::overlay_visual_selection(
                                frame,
                                area,
                                doc,
                                pane.viewport_top,
                                overlay,
                            );
                        }
                    }
                }
                None => {
                    // Empty pane — leave the area blank. A future iteration
                    // could surface a `(no buffer)` hint here.
                }
            }
        }
        PaneNode::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (rect_a, rect_b, sep_rect) = split_rect(area, *direction, *ratio);
            draw_separator(frame, sep_rect, *direction);
            let (path_first, path_second) = match focused_path {
                Some(p) if !p.is_empty() => {
                    let head = p[0];
                    let rest = &p[1..];
                    if head == 0 {
                        (Some(rest), None)
                    } else {
                        (None, Some(rest))
                    }
                }
                _ => (None, None),
            };
            render_pane_tree(
                frame,
                rect_a,
                first,
                path_first,
                suppress_cursor,
                search_pattern,
                visual_overlay,
                connection_names,
                result_viewport_top,
                result_tab,
            );
            render_pane_tree(
                frame,
                rect_b,
                second,
                path_second,
                suppress_cursor,
                search_pattern,
                visual_overlay,
                connection_names,
                result_viewport_top,
                result_tab,
            );
        }
    }
}

/// Carve `area` into two child rects plus a 1-cell separator strip.
/// The `ratio` is clamped so neither child gets less than one row /
/// column; the separator is dropped when `area` is too small to fit
/// it.
fn split_rect(area: Rect, dir: SplitDir, ratio: f32) -> (Rect, Rect, Rect) {
    let ratio = ratio.clamp(0.1, 0.9);
    match dir {
        SplitDir::Vertical => {
            let total = area.width;
            let sep_w = if total >= 3 { 1 } else { 0 };
            let usable = total.saturating_sub(sep_w);
            let mut first_w = (usable as f32 * ratio).round() as u16;
            first_w = first_w.clamp(1, usable.saturating_sub(1).max(1));
            let second_w = usable.saturating_sub(first_w);
            let a = Rect {
                x: area.x,
                y: area.y,
                width: first_w,
                height: area.height,
            };
            let sep = Rect {
                x: area.x.saturating_add(first_w),
                y: area.y,
                width: sep_w,
                height: area.height,
            };
            let b = Rect {
                x: area.x.saturating_add(first_w + sep_w),
                y: area.y,
                width: second_w,
                height: area.height,
            };
            (a, b, sep)
        }
        SplitDir::Horizontal => {
            let total = area.height;
            let sep_h = if total >= 3 { 1 } else { 0 };
            let usable = total.saturating_sub(sep_h);
            let mut first_h = (usable as f32 * ratio).round() as u16;
            first_h = first_h.clamp(1, usable.saturating_sub(1).max(1));
            let second_h = usable.saturating_sub(first_h);
            let a = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: first_h,
            };
            let sep = Rect {
                x: area.x,
                y: area.y.saturating_add(first_h),
                width: area.width,
                height: sep_h,
            };
            let b = Rect {
                x: area.x,
                y: area.y.saturating_add(first_h + sep_h),
                width: area.width,
                height: second_h,
            };
            (a, b, sep)
        }
    }
}

fn draw_separator(frame: &mut Frame, area: Rect, dir: SplitDir) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default().fg(Color::DarkGray);
    let glyph = match dir {
        SplitDir::Vertical => "│",
        SplitDir::Horizontal => "─",
    };
    let buf = frame.buffer_mut();
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &mut buf[(area.x + x, area.y + y)];
            cell.set_symbol(glyph);
            cell.set_style(style);
        }
    }
}

pub(crate) fn render_empty_state_inline(frame: &mut Frame, area: Rect, vault: &std::path::Path) {
    let vault = vault.to_string_lossy().into_owned();
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "This vault has no markdown files yet.",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("vault: {vault}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Create a note from the file tree (Ctrl+E, then `a`) or open one.",
            Style::default().add_modifier(Modifier::DIM),
        )),
    ];
    let block = Block::default().borders(Borders::ALL).title("notes-tui");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
