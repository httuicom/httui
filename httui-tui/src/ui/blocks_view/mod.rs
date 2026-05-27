use std::path::Path;

use ratatui::{layout::Rect, Frame};

use crate::app::BlocksWorkspace;
use crate::pane::PaneNode;
use crate::ui::VisualOverlay;

mod pane;
mod pane_tree;

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    node: &mut PaneNode,
    focused: &[u8],
    workspace: Option<&BlocksWorkspace>,
    vault: &Path,
    visual_overlay: Option<VisualOverlay>,
    running: Option<String>,
) {
    pane_tree::render(
        frame,
        area,
        node,
        Some(focused),
        workspace,
        vault,
        visual_overlay,
        running.as_deref(),
    );
}
