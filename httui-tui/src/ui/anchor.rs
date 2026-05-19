//! Block-anchor geometry: translate a segment index into a
//! screen-coordinate rect so anchored popups know where to float.

use ratatui::layout::Rect;

use crate::app::App;
use crate::buffer::layout::layout_document;

/// Locate `segment_idx` in the active pane's layout and translate
/// to screen coordinates (subtract `pane.viewport_top`). Returns
/// `None` when the pane has no document, the segment isn't in the
/// layout, or it's entirely scrolled off-screen — caller falls
/// back to a centered popup.
pub(crate) fn compute_block_anchor(
    app: &App,
    editor_area: Rect,
    segment_idx: usize,
) -> Option<BlockAnchor> {
    let pane = app.active_pane()?;
    let doc = pane.document.as_ref()?;
    let layouts = layout_document(doc, editor_area.width);
    let layout = layouts.iter().find(|l| l.segment_idx == segment_idx)?;
    let viewport_top = pane.viewport_top;
    let block_bottom = layout.y_start.saturating_add(layout.height);
    if block_bottom <= viewport_top {
        return None;
    }
    let screen_top = editor_area
        .y
        .saturating_add(layout.y_start.saturating_sub(viewport_top));
    let visible_height = layout
        .height
        .saturating_sub(viewport_top.saturating_sub(layout.y_start))
        .min(
            editor_area
                .height
                .saturating_sub(screen_top.saturating_sub(editor_area.y)),
        );
    if visible_height == 0 {
        return None;
    }
    Some(BlockAnchor {
        screen_top,
        height: visible_height,
    })
}

/// Screen-coordinate rect of a focused block — used by the
/// connection picker popup to anchor itself.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockAnchor {
    pub screen_top: u16,
    pub height: u16,
}
