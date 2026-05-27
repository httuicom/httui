use std::path::Path;

use ratatui::{layout::Rect, Frame};

use crate::app::BlocksWorkspace;
use crate::pane::PaneNode;

mod pane;
mod pane_tree;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    node: &mut PaneNode,
    focused: &[u8],
    workspace: Option<&BlocksWorkspace>,
    vault: &Path,
) {
    pane_tree::render(frame, area, node, Some(focused), workspace, vault);
}
