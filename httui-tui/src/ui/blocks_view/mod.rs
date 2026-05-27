use std::collections::HashMap;
use std::path::Path;

use ratatui::{layout::Rect, Frame};

use crate::app::{BlocksWorkspace, ResultPanelTab};
use crate::pane::PaneNode;
use crate::ui::VisualOverlay;

mod pane;
mod pane_tree;

/// Shared rendering context for BLOCKS view. Carries the borrowed
/// slices of `App` the renderers need without taking `&mut App`
/// directly (which would conflict with the split borrows in
/// `render_root`).
pub struct BlocksRenderCtx<'a> {
    pub vault: &'a Path,
    pub workspace: Option<&'a BlocksWorkspace>,
    pub connection_names: &'a HashMap<String, String>,
    pub result_tabs: &'a HashMap<crate::buffer::block::BlockId, ResultPanelTab>,
    pub result_viewport_top: &'a mut HashMap<usize, u16>,
    pub visual_overlay: Option<VisualOverlay>,
    pub running: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    node: &mut PaneNode,
    focused: &[u8],
    ctx: &mut BlocksRenderCtx<'_>,
) {
    pane_tree::render(frame, area, node, Some(focused), ctx);
}
