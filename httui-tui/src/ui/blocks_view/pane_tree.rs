use std::path::Path;

use ratatui::{layout::Rect, Frame};

use crate::app::BlocksWorkspace;
use crate::pane::PaneNode;
use crate::ui::VisualOverlay;

use super::pane;

#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    node: &mut PaneNode,
    focused_path: Option<&[u8]>,
    workspace: Option<&BlocksWorkspace>,
    vault: &Path,
    visual_overlay: Option<VisualOverlay>,
    running: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let picker_active = workspace.is_some_and(|w| w.pane_picker.is_some());
    let mut counter = 0usize;
    render_inner(
        frame,
        area,
        node,
        focused_path,
        workspace,
        vault,
        picker_active,
        visual_overlay,
        running,
        &mut counter,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_inner(
    frame: &mut Frame,
    area: Rect,
    node: &mut PaneNode,
    focused_path: Option<&[u8]>,
    workspace: Option<&BlocksWorkspace>,
    vault: &Path,
    picker_active: bool,
    visual_overlay: Option<VisualOverlay>,
    running: bool,
    counter: &mut usize,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    match node {
        PaneNode::Leaf(leaf) => {
            leaf.viewport_height = area.height;
            let is_focused = matches!(focused_path, Some(p) if p.is_empty());
            // Visual overlay + spinner only on the focused leaf —
            // visual selection's moving end is the active cursor, and
            // there's only ever one running_query.
            let leaf_overlay = if is_focused { visual_overlay } else { None };
            let leaf_running = is_focused && running;
            pane::render_leaf(
                frame,
                area,
                leaf,
                is_focused,
                workspace,
                vault,
                leaf_overlay,
                leaf_running,
            );
            if picker_active {
                let n = *counter + 1;
                pane::paint_picker_overlay(frame, area, n);
            }
            *counter += 1;
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
            render_inner(
                frame,
                rect_a,
                first,
                path_first,
                workspace,
                vault,
                picker_active,
                visual_overlay,
                running,
                counter,
            );
            render_inner(
                frame,
                rect_b,
                second,
                path_second,
                workspace,
                vault,
                picker_active,
                visual_overlay,
                running,
                counter,
            );
        }
    }
}
