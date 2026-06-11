use super::*;
use std::path::PathBuf;

fn pane_with_path(name: &str) -> Pane {
    Pane {
        document: None,
        document_path: Some(PathBuf::from(name)),
        viewport_top: 0,
        viewport_left: 0,
        viewport_height: 0,
        viewport_width: 0,
        block_selected: None,
        block_region: 0,
        block_row: 0,
        block_col: 1,
        block_req_tab: 0,
        block_edit: None,
        block_draft: None,
        block_tabs: vec![BlockTab::empty()],
        block_tab_active: 0,
    }
}

#[test]
fn note_req_tab_tracks_request_regions_only() {
    let mut p = Pane::empty();
    p.block_region = 2;
    p.note_req_tab();
    assert_eq!(p.block_req_tab, 1, "Body region records the Body tab");
    // Leaving for URL/Response keeps the memory.
    p.block_region = 3;
    p.note_req_tab();
    assert_eq!(p.block_req_tab, 1);
    p.block_region = 0;
    p.note_req_tab();
    assert_eq!(p.block_req_tab, 1);
    // Visiting Headers re-records.
    p.block_region = 1;
    p.note_req_tab();
    assert_eq!(p.block_req_tab, 0);
}

#[test]
fn snapshot_clone_preserves_cached_result_across_blocks() {
    // Build a pane whose Document has two HTTP blocks, populate
    // the second one's cached_result, then clone — the new pane
    // must keep the cached_result on the same block (split must
    // not wipe the response card).
    use crate::buffer::Segment;
    let md = "```http alias=a\nGET https://x.com\n```\n\n```http alias=b\nGET https://y.com\n```\n";
    let mut doc = Document::from_markdown(md).unwrap();
    let cached = serde_json::json!({"status": 200, "body": {"ok": true}});
    let target = doc
        .segments()
        .iter()
        .position(|s| matches!(s, Segment::Block(b) if b.alias.as_deref() == Some("b")))
        .unwrap();
    if let Some(b) = doc.block_at_mut(target) {
        b.cached_result = Some(cached.clone());
        b.state = crate::buffer::block::ExecutionState::Success;
    }
    let pane = Pane {
        document: Some(doc),
        document_path: Some(PathBuf::from("api.md")),
        viewport_top: 0,
        viewport_left: 0,
        viewport_height: 0,
        viewport_width: 0,
        block_selected: None,
        block_region: 0,
        block_row: 0,
        block_col: 0,
        block_req_tab: 0,
        block_edit: None,
        block_draft: None,
        block_tabs: vec![BlockTab::empty()],
        block_tab_active: 0,
    };
    let clone = pane.snapshot_clone();
    let cloned_doc = clone.document.expect("doc cloned");
    let cloned_b = cloned_doc
        .segments()
        .iter()
        .find_map(|s| match s {
            Segment::Block(b) if b.alias.as_deref() == Some("b") => Some(b),
            _ => None,
        })
        .expect("block b present");
    assert_eq!(cloned_b.cached_result.as_ref(), Some(&cached));
    assert!(matches!(
        cloned_b.state,
        crate::buffer::block::ExecutionState::Success
    ));
}

#[test]
fn new_tab_has_single_leaf() {
    let tab = TabState::new(pane_with_path("a.md"));
    assert_eq!(tab.leaf_count(), 1);
    assert_eq!(tab.focused, Vec::<u8>::new());
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("a.md")));
}

#[test]
fn split_vertical_focuses_new_pane() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    tab.split(SplitDir::Vertical, pane_with_path("b.md"));
    assert_eq!(tab.leaf_count(), 2);
    assert_eq!(tab.focused, vec![1u8]);
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("b.md")));
}

#[test]
fn focus_left_after_vertical_split() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    tab.split(SplitDir::Vertical, pane_with_path("b.md"));
    // We're on b (right). Press Left → land on a.
    assert!(tab.focus_dir(FocusDir::Left));
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("a.md")));
    // Already on the leftmost — Left is no-op.
    assert!(!tab.focus_dir(FocusDir::Left));
}

#[test]
fn focus_down_after_horizontal_split() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    tab.split(SplitDir::Horizontal, pane_with_path("b.md"));
    // We're on b (bottom). Up → a.
    assert!(tab.focus_dir(FocusDir::Up));
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("a.md")));
    // Down → b again.
    assert!(tab.focus_dir(FocusDir::Down));
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("b.md")));
}

#[test]
fn nested_focus_skips_unrelated_split() {
    // Layout:
    //   +-------+--------+
    //   |       | b.md   |
    //   | a.md  +--------+
    //   |       | c.md   |
    //   +-------+--------+
    let mut tab = TabState::new(pane_with_path("a.md"));
    tab.split(SplitDir::Vertical, pane_with_path("b.md")); // focus on b
    tab.split(SplitDir::Horizontal, pane_with_path("c.md")); // focus on c (below b)
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("c.md")));
    // From c, Left should walk past the H split and into a.
    assert!(tab.focus_dir(FocusDir::Left));
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("a.md")));
}

/// User's actual bug: three columns laid out as V[A, V[B, C]].
/// From C (rightmost), Ctrl+W h must land on B (immediate left
/// neighbour) — the old `descend_first` always went to A (leftmost
/// of the whole layout) by descending into the sibling's `first`
/// child no matter what direction we came from.
#[test]
fn focus_left_descends_toward_origin_in_nested_vertical_splits() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    tab.split(SplitDir::Vertical, pane_with_path("b.md"));
    // Now V[A, B], focused on B = [1].
    tab.split(SplitDir::Vertical, pane_with_path("c.md"));
    // Now V[A, V[B, C]], focused on C = [1, 1].
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("c.md")));
    assert!(tab.focus_dir(FocusDir::Left));
    assert_eq!(
        tab.active_leaf().document_path,
        Some(PathBuf::from("b.md")),
        "expected B (immediate left), got: focused path = {:?}",
        tab.focused
    );
}

/// Symmetric: H[A, H[B, C]] — from C (bottom) press Up, must
/// land on B (the row immediately above), not A (topmost leaf).
#[test]
fn focus_up_descends_toward_origin_in_nested_horizontal_splits() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    tab.split(SplitDir::Horizontal, pane_with_path("b.md"));
    tab.split(SplitDir::Horizontal, pane_with_path("c.md"));
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("c.md")));
    assert!(tab.focus_dir(FocusDir::Up));
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("b.md")),);
}

/// Layout that matches the user's report: vertical root, left
/// column is a horizontal split (A top / B bottom), right is a
/// vertical split (C / D). From D, `Ctrl+W h` must land on C
/// (the adjacent left neighbour), not on A (leftmost leaf).
#[test]
fn focus_left_from_rightmost_in_4pane_layout_lands_on_immediate_neighbour() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    // Build: V[ H[A, B], V[C, D] ].
    // Start: just A. After split V → focus is on second leaf [1].
    tab.split(SplitDir::Vertical, pane_with_path("c.md"));
    // Now layout: V[A, C]. focused=[1] on C.
    // Split V from C to create D (right of C) → V[A, V[C, D]],
    // focused=[1,1] on D.
    tab.split(SplitDir::Vertical, pane_with_path("d.md"));
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("d.md")));
    // Move focus to A and split horizontally to add B below.
    assert!(tab.focus_dir(FocusDir::Left));
    assert!(tab.focus_dir(FocusDir::Left));
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("a.md")));
    tab.split(SplitDir::Horizontal, pane_with_path("b.md"));
    // Layout now: V[ H[A, B], V[C, D] ]; focus on B = [0,1].
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("b.md")));
    // Move focus back to D and test the failing motion.
    assert!(tab.focus_dir(FocusDir::Right));
    assert!(tab.focus_dir(FocusDir::Right));
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("d.md")));
    // Ctrl+W h from D → expect C (immediate neighbour), not A
    // (which is the leftmost leaf of the whole layout).
    assert!(tab.focus_dir(FocusDir::Left));
    assert_eq!(
        tab.active_leaf().document_path,
        Some(PathBuf::from("c.md")),
        "expected C, got: focused path = {:?}",
        tab.focused
    );
}

#[test]
fn close_focused_promotes_sibling() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    tab.split(SplitDir::Vertical, pane_with_path("b.md"));
    // Close b → only a remains, focus on a.
    assert_eq!(tab.close_focused(), CloseResult::Closed);
    assert_eq!(tab.leaf_count(), 1);
    assert_eq!(tab.active_leaf().document_path, Some(PathBuf::from("a.md")));
}

#[test]
fn close_focused_on_root_returns_last_leaf() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    assert_eq!(tab.close_focused(), CloseResult::LastLeaf);
    assert_eq!(tab.leaf_count(), 1);
}

#[test]
fn cycle_focus_visits_all_leaves() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    tab.split(SplitDir::Vertical, pane_with_path("b.md"));
    tab.split(SplitDir::Horizontal, pane_with_path("c.md"));
    // Cycle from c → a → b → c.
    let mut seen = vec![tab.active_leaf().document_path.clone()];
    for _ in 0..3 {
        assert!(tab.cycle_focus());
        seen.push(tab.active_leaf().document_path.clone());
    }
    assert_eq!(seen[0], Some(PathBuf::from("c.md")));
    assert_eq!(seen[3], Some(PathBuf::from("c.md")));
    let names: Vec<String> = seen
        .iter()
        .filter_map(|p| p.as_ref().map(|p| p.display().to_string()))
        .collect();
    assert!(names.contains(&"a.md".to_string()));
    assert!(names.contains(&"b.md".to_string()));
}

#[test]
fn equalize_resets_ratios() {
    let mut tab = TabState::new(pane_with_path("a.md"));
    tab.split(SplitDir::Vertical, pane_with_path("b.md"));
    if let PaneNode::Split { ratio, .. } = &mut tab.root {
        *ratio = 0.8;
    }
    tab.equalize();
    if let PaneNode::Split { ratio, .. } = &tab.root {
        assert!((ratio - 0.5).abs() < f32::EPSILON);
    } else {
        panic!("root should be a split");
    }
}
