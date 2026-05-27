use std::path::Path;

use ratatui::{layout::Rect, Frame};

use crate::app::BlocksWorkspace;
use crate::pane::PaneNode;

use super::pane;

pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    node: &mut PaneNode,
    focused_path: Option<&[u8]>,
    workspace: Option<&BlocksWorkspace>,
    vault: &Path,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    match node {
        PaneNode::Leaf(leaf) => {
            leaf.viewport_height = area.height;
            let is_focused = matches!(focused_path, Some(p) if p.is_empty());
            pane::render_leaf(frame, area, leaf, is_focused, workspace, vault);
        }
        PaneNode::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (rect_a, rect_b, sep_rect) =
                crate::ui::pane_tree::split_rect(area, *direction, *ratio);
            crate::ui::pane_tree::draw_separator(frame, sep_rect, *direction);
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
            render(frame, rect_a, first, path_first, workspace, vault);
            render(frame, rect_b, second, path_second, workspace, vault);
        }
    }
}
