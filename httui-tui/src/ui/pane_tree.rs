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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;
    use crate::pane::{Pane, PaneNode, SplitDir};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::collections::HashMap;
    use std::path::PathBuf;

    type ConnectionNamesAlias = super::blocks::ConnectionNames;

    // ---- split_rect: pure math, no App / backend -------------------

    #[test]
    fn split_rect_vertical_carves_two_children_plus_separator() {
        let area = Rect::new(0, 0, 100, 30);
        let (a, b, sep) = split_rect(area, SplitDir::Vertical, 0.5);
        assert_eq!(sep.width, 1, "separator strip is 1 col wide");
        assert_eq!(a.height, 30);
        assert_eq!(b.height, 30);
        // a + sep + b reconstitutes the usable width.
        assert_eq!(a.width + sep.width + b.width, 100);
        assert_eq!(sep.x, a.x + a.width);
        assert_eq!(b.x, a.x + a.width + sep.width);
    }

    #[test]
    fn split_rect_horizontal_carves_top_and_bottom() {
        let area = Rect::new(2, 4, 40, 50);
        let (a, b, sep) = split_rect(area, SplitDir::Horizontal, 0.5);
        assert_eq!(sep.height, 1);
        assert_eq!(a.width, 40);
        assert_eq!(b.width, 40);
        assert_eq!(a.height + sep.height + b.height, 50);
        assert_eq!(sep.y, a.y + a.height);
        assert_eq!(b.y, a.y + a.height + sep.height);
    }

    #[test]
    fn split_rect_drops_separator_when_too_small_vertical() {
        // width < 3 → sep_w = 0.
        let (a, b, sep) = split_rect(Rect::new(0, 0, 2, 10), SplitDir::Vertical, 0.5);
        assert_eq!(sep.width, 0);
        assert_eq!(a.width + b.width, 2);
    }

    #[test]
    fn split_rect_drops_separator_when_too_small_horizontal() {
        let (a, b, sep) = split_rect(Rect::new(0, 0, 10, 2), SplitDir::Horizontal, 0.5);
        assert_eq!(sep.height, 0);
        assert_eq!(a.height + b.height, 2);
    }

    #[test]
    fn split_rect_clamps_ratio_low_extreme() {
        // ratio 0.0 clamps to 0.1 — first child stays ≥ 1 col.
        let (a, _b, _s) = split_rect(Rect::new(0, 0, 100, 10), SplitDir::Vertical, 0.0);
        assert!(a.width >= 1, "first width clamped to ≥1: {}", a.width);
        let usable = 100 - 1;
        assert_eq!(a.width, ((usable as f32) * 0.1).round() as u16);
    }

    #[test]
    fn split_rect_clamps_ratio_high_extreme() {
        // ratio 1.0 clamps to 0.9; first never eats the entire usable
        // width.
        let (a, b, _s) = split_rect(Rect::new(0, 0, 100, 10), SplitDir::Vertical, 1.0);
        assert!(a.width < 100);
        assert!(b.width >= 1 || a.width <= 98);
    }

    #[test]
    fn split_rect_handles_minimum_three_wide() {
        // Exactly 3 wide is the boundary where the separator survives.
        let (a, b, sep) = split_rect(Rect::new(0, 0, 3, 5), SplitDir::Vertical, 0.5);
        assert_eq!(sep.width, 1);
        assert_eq!(a.width + sep.width + b.width, 3);
        assert!(a.width >= 1);
    }

    #[test]
    fn split_rect_horizontal_ratio_clamp_keeps_first_row() {
        let (a, _b, _s) = split_rect(Rect::new(0, 0, 10, 100), SplitDir::Horizontal, 0.0);
        assert!(a.height >= 1);
    }

    // ---- render harness for the tree walk --------------------------

    fn doc(md: &str) -> Document {
        Document::from_markdown(md).unwrap()
    }

    fn leaf_with_doc(md: &str) -> PaneNode {
        PaneNode::Leaf(Pane::new(doc(md), PathBuf::from("a.md")))
    }

    fn render_tree(node: &mut PaneNode, focused: Option<&[u8]>, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let names: ConnectionNamesAlias = HashMap::new();
        let mut rvt: HashMap<usize, u16> = HashMap::new();
        terminal
            .draw(|f| {
                render_pane_tree(
                    f,
                    Rect::new(0, 0, w, h),
                    node,
                    focused,
                    false,
                    None,
                    None,
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

    #[test]
    fn render_pane_tree_zero_area_is_a_noop() {
        let mut node = leaf_with_doc("hello\n");
        // 0×0 area → early return, no panic, blank buffer.
        let backend = TestBackend::new(1, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        let names: ConnectionNamesAlias = HashMap::new();
        let mut rvt: HashMap<usize, u16> = HashMap::new();
        terminal
            .draw(|f| {
                render_pane_tree(
                    f,
                    Rect::new(0, 0, 0, 0),
                    &mut node,
                    Some(&[]),
                    false,
                    None,
                    None,
                    &names,
                    &mut rvt,
                    crate::app::ResultPanelTab::Result,
                );
            })
            .unwrap();
        // Nothing painted.
        let buf = terminal.backend().buffer().clone();
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), " ");
    }

    #[test]
    fn render_pane_tree_focused_leaf_paints_prose_and_sets_viewport_height() {
        let mut node = leaf_with_doc("focused content here\n");
        let text = render_tree(&mut node, Some(&[]), 40, 6);
        assert!(text.contains("focused content"), "got: {text:?}");
        if let PaneNode::Leaf(p) = &node {
            assert_eq!(p.viewport_height, 6, "viewport_height updated");
        } else {
            panic!("expected leaf");
        }
    }

    #[test]
    fn render_pane_tree_unfocused_leaf_uses_no_cursor_path() {
        let mut node = leaf_with_doc("unfocused branch\n");
        // focused_path None → not the focused leaf → render_document_no_cursor.
        let text = render_tree(&mut node, None, 40, 5);
        assert!(text.contains("unfocused branch"), "got: {text:?}");
    }

    #[test]
    fn render_pane_tree_empty_leaf_leaves_area_blank() {
        let mut node = PaneNode::Leaf(Pane::empty());
        let text = render_tree(&mut node, Some(&[]), 20, 4);
        assert!(text.trim().is_empty(), "expected blank, got: {text:?}");
        if let PaneNode::Leaf(p) = &node {
            assert_eq!(p.viewport_height, 4);
        }
    }

    #[test]
    fn render_pane_tree_vertical_split_paints_both_children_and_separator() {
        let mut node = PaneNode::Split {
            direction: SplitDir::Vertical,
            ratio: 0.5,
            first: Box::new(leaf_with_doc("LEFTSIDE\n")),
            second: Box::new(leaf_with_doc("RIGHTSIDE\n")),
        };
        // Focus path [0] → first child is the focused leaf.
        let text = render_tree(&mut node, Some(&[0]), 60, 6);
        assert!(text.contains("LEFTSIDE"), "left missing: {text:?}");
        assert!(text.contains("RIGHTSIDE"), "right missing: {text:?}");
        // Vertical separator glyph painted somewhere.
        assert!(text.contains('│'), "expected V separator: {text:?}");
    }

    #[test]
    fn render_pane_tree_horizontal_split_paints_separator_and_routes_focus_second() {
        let mut node = PaneNode::Split {
            direction: SplitDir::Horizontal,
            ratio: 0.5,
            first: Box::new(leaf_with_doc("TOPPANE\n")),
            second: Box::new(leaf_with_doc("BOTPANE\n")),
        };
        // Focus path [1] → second child focused (exercises the head==1 arm).
        let text = render_tree(&mut node, Some(&[1]), 50, 8);
        assert!(text.contains("TOPPANE"), "top missing: {text:?}");
        assert!(text.contains("BOTPANE"), "bottom missing: {text:?}");
        assert!(text.contains('─'), "expected H separator: {text:?}");
    }

    #[test]
    fn render_pane_tree_split_with_focus_in_other_subtree() {
        // focused_path None on a split → both children rendered
        // no-cursor (path_first / path_second both None).
        let mut node = PaneNode::Split {
            direction: SplitDir::Vertical,
            ratio: 0.5,
            first: Box::new(leaf_with_doc("AAA\n")),
            second: Box::new(leaf_with_doc("BBB\n")),
        };
        let text = render_tree(&mut node, None, 40, 5);
        assert!(
            text.contains("AAA") && text.contains("BBB"),
            "got: {text:?}"
        );
    }

    // ---- draw_separator + render_empty_state_inline ----------------

    #[test]
    fn draw_separator_zero_size_is_a_noop() {
        let backend = TestBackend::new(4, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                draw_separator(f, Rect::new(0, 0, 0, 3), SplitDir::Vertical);
                draw_separator(f, Rect::new(0, 0, 3, 0), SplitDir::Horizontal);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        // Nothing drawn → still blank.
        assert_eq!(buf.cell((0, 0)).unwrap().symbol(), " ");
    }

    #[test]
    fn render_empty_state_inline_paints_the_no_files_hint() {
        let backend = TestBackend::new(70, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_empty_state_inline(
                    f,
                    Rect::new(0, 0, 70, 10),
                    std::path::Path::new("/tmp/vault"),
                );
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let text: String = (0..10)
            .flat_map(|y| (0..70).map(move |x| (x, y)))
            .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
            .collect();
        assert!(
            text.contains("no markdown files yet"),
            "hint missing: {text:?}"
        );
        assert!(text.contains("/tmp/vault"), "vault path missing: {text:?}");
        assert!(text.contains("notes-tui"), "title missing: {text:?}");
    }
}
