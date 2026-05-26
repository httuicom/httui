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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use ratatui::layout::Rect;
    use tempfile::TempDir;

    /// Build an `App` over a vault seeded with the given files.
    /// Mirrors the proven fixture in `ui::status` tests. `App::new`
    /// uses `block_in_place` → multi-thread runtime required.
    async fn app_with_files(files: &[(&str, &str)]) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        for (rel, body) in files {
            let p = vault.path().join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(p, body).unwrap();
        }
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    fn editor_area() -> Rect {
        Rect::new(0, 1, 80, 24)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn returns_none_when_pane_has_no_document() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        if let Some(p) = app.active_pane_mut() {
            p.document = None;
            p.document_path = None;
        }
        assert!(compute_block_anchor(&app, editor_area(), 0).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn returns_none_for_missing_segment_index() {
        let (app, _d, _v) = app_with_files(&[("a.md", "hello\n")]).await;
        // Segment 999 isn't in the layout → None.
        assert!(compute_block_anchor(&app, editor_area(), 999).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn anchors_a_visible_block_at_viewport_top_zero() {
        let src = "intro\n\n```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let (app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        let seg = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .unwrap();
        let a = compute_block_anchor(&app, editor_area(), seg).expect("visible block anchors");
        // Screen top is offset by editor_area.y (=1); height is non-zero.
        assert!(a.screen_top >= 1, "screen_top={}", a.screen_top);
        assert!(a.height > 0, "height={}", a.height);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn returns_none_when_block_scrolled_entirely_off_screen() {
        let src = "intro\n\n```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        let seg = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .unwrap();
        // Scroll the viewport far past the block bottom.
        if let Some(p) = app.active_pane_mut() {
            p.viewport_top = 9999;
        }
        assert!(compute_block_anchor(&app, editor_area(), seg).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn partial_visibility_clamps_height_to_viewport() {
        let src =
            "intro\n\n```db-postgres alias=q connection=c\nSELECT 1\nSELECT 2\nSELECT 3\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        let seg = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .unwrap();
        // Tiny editor area so the block can't fully fit → clamp path.
        let small = Rect::new(0, 0, 80, 4);
        if let Some(p) = app.active_pane_mut() {
            p.viewport_top = 1;
        }
        if let Some(a) = compute_block_anchor(&app, small, seg) {
            assert!(a.height <= small.height, "height clamp: {}", a.height);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn returns_none_when_visible_height_collapses_to_zero() {
        // viewport_top sits exactly at the block's last visible row so
        // the clamped visible_height resolves to 0.
        let src = "a\nb\nc\nd\ne\n\n```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        let seg = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .unwrap();
        let layouts = layout_document(app.document().unwrap(), 80);
        let layout = *layouts.iter().find(|l| l.segment_idx == seg).unwrap();
        // viewport_top sits one row above y_start: block_bottom stays
        // above viewport_top (not the scrolled-off branch), but
        // screen_top = editor.y + 1 lands at/below the 1-row editor
        // bottom, so the min() term clamps visible_height to 0.
        if let Some(p) = app.active_pane_mut() {
            p.viewport_top = layout.y_start.saturating_sub(1);
        }
        // 1-row editor: screen_top (= y_start - viewport_top = 1) is
        // already past the single visible row → visible_height 0.
        let one_row = Rect::new(0, 0, 80, 1);
        assert!(compute_block_anchor(&app, one_row, seg).is_none());
    }

    #[test]
    fn block_anchor_is_copy_and_debug() {
        let a = BlockAnchor {
            screen_top: 4,
            height: 9,
        };
        let b = a; // Copy — `a` stays usable.
        assert_eq!(a.screen_top, 4);
        assert_eq!(b.height, 9);
        assert!(format!("{a:?}").contains("BlockAnchor"));
    }
}
