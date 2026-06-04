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

/// Fallback policy when a popup of fixed `(w, h)` won't fit one row
/// below the caret. Different popups want different trade-offs:
///
/// - `FlipAbove`: when there isn't room below, place the popup above
///   the caret instead. Used by hover overlays (ref preview) where
///   it's OK if the popup briefly covers the source row.
/// - `TruncateBelow`: keep the popup below; clip the height to
///   whatever fits. Used by the completion popup so the dropdown
///   never obscures the text being completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaretPlacement {
    FlipAbove,
    TruncateBelow,
}

/// Place a `(w, h)` popup near `caret` (screen-coordinate `(col, row)`),
/// clamped horizontally to `area` so it never spills past the right
/// edge. When `caret` is `None`, centers the popup in `area` — used
/// when neither call site can derive a caret position (DOC view's
/// ref preview, completion popup without an anchor).
pub(crate) fn place_near_caret(
    area: Rect,
    w: u16,
    h: u16,
    caret: Option<(u16, u16)>,
    policy: CaretPlacement,
) -> Rect {
    let Some((cx, cy)) = caret else {
        return center_in(area, w, h);
    };
    let right_edge = area.x.saturating_add(area.width);
    let bottom_edge = area.y.saturating_add(area.height);
    // Horizontal: prefer aligning the popup's left edge with the
    // caret; slide left when the right edge would overflow.
    let max_x = right_edge.saturating_sub(w);
    let x = cx.min(max_x).max(area.x);
    // Vertical: one row below the caret is the canonical IDE
    // tooltip / completion position.
    let below_y = cy.saturating_add(1);
    let avail_below = bottom_edge.saturating_sub(below_y);
    match policy {
        CaretPlacement::FlipAbove => {
            if avail_below >= h {
                Rect {
                    x,
                    y: below_y,
                    width: w,
                    height: h,
                }
            } else {
                // Bottom edge of the popup lands one row above the
                // caret. Clamp to `area.y` so we never go negative
                // on a tiny screen.
                let y = cy.saturating_sub(h).max(area.y);
                Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                }
            }
        }
        CaretPlacement::TruncateBelow => {
            // Mirrors completion popup's pre-existing policy (2026-
            // 05-23 decision): never flip above; centering is the
            // fallback when truncation can't yield a usable height
            // (less than border + 1 row).
            if avail_below >= 3 {
                let truncated = h.min(avail_below);
                Rect {
                    x,
                    y: below_y,
                    width: w,
                    height: truncated,
                }
            } else {
                center_in(area, w, h)
            }
        }
    }
}

fn center_in(area: Rect, w: u16, h: u16) -> Rect {
    Rect {
        x: area.x.saturating_add((area.width.saturating_sub(w)) / 2),
        y: area.y.saturating_add((area.height.saturating_sub(h)) / 2),
        width: w,
        height: h,
    }
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

    fn ar(x: u16, y: u16, w: u16, h: u16) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn place_near_caret_centers_without_caret() {
        let r = place_near_caret(ar(0, 0, 80, 20), 30, 4, None, CaretPlacement::FlipAbove);
        assert_eq!(r.x, 25, "(80 - 30) / 2");
        assert_eq!(r.y, 8, "(20 - 4) / 2");
    }

    #[test]
    fn place_near_caret_flips_above_lands_below_when_room_fits() {
        let r = place_near_caret(
            ar(0, 0, 80, 24),
            30,
            3,
            Some((20, 4)),
            CaretPlacement::FlipAbove,
        );
        assert_eq!(r.x, 20);
        assert_eq!(r.y, 5, "one row below caret");
    }

    #[test]
    fn place_near_caret_flips_above_when_no_room_below() {
        let r = place_near_caret(
            ar(0, 0, 80, 10),
            30,
            3,
            Some((10, 9)),
            CaretPlacement::FlipAbove,
        );
        assert_eq!(r.y, 6, "caret_y - height = 9 - 3");
    }

    #[test]
    fn place_near_caret_clamps_right_edge() {
        let r = place_near_caret(
            ar(0, 0, 50, 24),
            40,
            3,
            Some((40, 2)),
            CaretPlacement::FlipAbove,
        );
        assert_eq!(r.x, 10, "right edge = area.x + area.w - popup.w = 10");
    }

    #[test]
    fn place_near_caret_truncate_keeps_below_and_shrinks_height() {
        // Caret near the bottom: not enough room for the full popup
        // height, but `avail_below >= 3` so we truncate instead of
        // flipping.
        let r = place_near_caret(
            ar(0, 0, 80, 10),
            30,
            6,
            Some((0, 5)),
            CaretPlacement::TruncateBelow,
        );
        assert_eq!(r.y, 6);
        assert_eq!(r.height, 4, "10 - 6 = 4 rows below the caret");
    }

    #[test]
    fn place_near_caret_truncate_falls_back_to_center_when_no_room() {
        // Caret at the bottom row: avail_below < 3 → centers.
        let r = place_near_caret(
            ar(0, 0, 80, 10),
            30,
            4,
            Some((0, 9)),
            CaretPlacement::TruncateBelow,
        );
        assert_eq!(r.x, 25);
        assert_eq!(r.y, 3, "(10 - 4) / 2");
    }
}
