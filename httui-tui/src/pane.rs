//! Per-tab pane tree. Each tab holds a binary tree of [`Pane`]s; splits
//! are introduced by `Ctrl+W v` (vertical separator → side-by-side) and
//! `Ctrl+W s` (horizontal separator → top/bottom). Focus moves by path
//! through the tree via `Ctrl+W h/j/k/l`.

use std::path::PathBuf;

use crate::app::{BlockDraft, BlockRef, RegionEdit};
use crate::buffer::Document;

pub struct Pane {
    pub document: Option<Document>,
    pub document_path: Option<PathBuf>,
    pub viewport_top: u16,
    /// Horizontal pan, in display columns, applied to the segment the
    /// cursor lives in (other segments always render unpanned).
    /// Follows the cursor on refresh; never persisted — it is
    /// recomputed on the next cursor move, so a stale value is never
    /// visible.
    pub viewport_left: u16,
    /// Height of the editor area allocated to this pane on the most
    /// recent frame. Updated by the renderer; read by motion code that
    /// needs page-relative scroll amounts (e.g. `Ctrl+D`). Shared
    /// across BLOCKS tabs — the renderer overwrites it every frame.
    pub viewport_height: u16,
    /// Width of the editor area on the most recent frame. Updated by
    /// the renderer like `viewport_height`; read by the horizontal
    /// cursor-follow, which runs in the update path before render and
    /// would otherwise have to guess the pane width.
    pub viewport_width: u16,
    /// Currently-rendered block in BLOCKS view (`AppView::Blocks`).
    /// Ignored when the app is in DOC view; survives the round-trip so
    /// re-entering BLOCKS restores the per-pane selection.
    pub block_selected: Option<BlockRef>,
    /// Focused region inside the displayed block (0-based; clamped to
    /// the block kind's region count by the applier).
    pub block_region: usize,
    /// Row index inside a table-shaped region (Headers). Clamped by the
    /// applier to the region's current row count. Ignored by regions
    /// that aren't table-shaped (Request line, Connection, Body…).
    pub block_row: usize,
    /// Column index inside a table-shaped region: `0 = key`, `1 = value`.
    /// Default `1` so a fresh focus into Headers lands on "value" — the
    /// most-edited field in practice (Cenário 4 enters on `value`).
    pub block_col: usize,
    /// `Some` while a region field is being edited. The applier captures
    /// every keystroke into the buffer until Esc (commit) or Ctrl+C
    /// (discard); the renderer paints the buffer in place of the field
    /// value plus an "EDIT" label on the focused region's border.
    /// Boxed so an idle pane doesn't carry the multi-line buffer's
    /// `Vec<String>` on the stack (keeps `PaneNode::Leaf` lean).
    pub block_edit: Option<Box<RegionEdit>>,
    /// `Some` once the pane has any committed-but-not-saved edit. Saving
    /// (Ctrl+S) re-serializes the draft into the `.md` and clears this
    /// field. The header shows `*` next to the alias while this is set.
    /// Boxed for the same reason as `block_edit` — `ParsedBlock` carries
    /// a `serde_json::Value` whose worst case is non-trivial.
    pub block_draft: Option<Box<BlockDraft>>,
    /// BLOCKS-view tab strip. Always non-empty — `block_tabs[block_tab_active]`
    /// is the canonical home of the active tab's snapshot; the eight
    /// fields above (document/document_path/viewport_top + the six
    /// `block_*`) MIRROR that slot so the 200+ existing call sites that
    /// read/write the pane directly keep working unchanged. Swapping tabs
    /// commits the current pane state into the active slot, then restores
    /// the target slot into the pane. DOC view ignores this strip.
    pub block_tabs: Vec<BlockTab>,
    pub block_tab_active: usize,
}

// `BlockTab` + the BLOCKS-view tab-strip helpers (`swap_to_tab`,
// `push_blank_tab`, `close_active_tab`, …) live in `pane_tabs.rs` so
// this module stays focused on the binary pane tree + per-tab pane
// state.
pub use crate::pane_tabs::BlockTab;

impl Pane {
    pub fn empty() -> Self {
        Self {
            document: None,
            document_path: None,
            viewport_top: 0,
            viewport_left: 0,
            viewport_height: 0,
            viewport_width: 0,
            block_selected: None,
            block_region: 0,
            block_row: 0,
            block_col: 1,
            block_edit: None,
            block_draft: None,
            block_tabs: vec![BlockTab::empty()],
            block_tab_active: 0,
        }
    }

    pub fn new(document: Document, path: PathBuf) -> Self {
        Self {
            document: Some(document),
            document_path: Some(path),
            viewport_top: 0,
            viewport_left: 0,
            viewport_height: 0,
            viewport_width: 0,
            block_selected: None,
            block_region: 0,
            block_row: 0,
            block_col: 1,
            block_edit: None,
            block_draft: None,
            // The mirror IS the active tab's truth; the inactive slot
            // starts empty and only gets populated on the first swap.
            block_tabs: vec![BlockTab::empty()],
            block_tab_active: 0,
        }
    }

    /// Snapshot the pane's state into a fresh independent pane via a
    /// markdown roundtrip. Used by `Ctrl+W v/s` so the new split shows
    /// the same content as the current one without sharing buffers.
    /// Cursor returns to the document start; viewport is reset. The
    /// BLOCKS-view edit buffer and draft do NOT carry into the split —
    /// each pane edits and saves independently (mirrors how unsaved
    /// changes in DOC are tied to the source pane).
    pub fn snapshot_clone(&self) -> Self {
        // Roundtrip via markdown rebuilds the segment tree fresh
        // (cheap, deterministic). The roundtrip drops in-memory
        // `BlockNode.cached_result` + `state`, so we re-attach them
        // by index — the new doc has the same block ordering by
        // construction since serialization is lossless.
        let document = self.document.as_ref().and_then(|src| {
            let mut new_doc = Document::from_markdown(&src.to_markdown()).ok()?;
            let original_blocks: Vec<(
                crate::buffer::block::ExecutionState,
                Option<serde_json::Value>,
            )> = src
                .segments()
                .iter()
                .filter_map(|s| match s {
                    crate::buffer::Segment::Block(b) => {
                        Some((b.state.clone(), b.cached_result.clone()))
                    }
                    _ => None,
                })
                .collect();
            let mut block_idx = 0usize;
            for seg_idx in 0..new_doc.segments().len() {
                if matches!(
                    new_doc.segments().get(seg_idx),
                    Some(crate::buffer::Segment::Block(_))
                ) {
                    if let Some((state, cached)) = original_blocks.get(block_idx).cloned() {
                        if let Some(b) = new_doc.block_at_mut(seg_idx) {
                            b.state = state;
                            b.cached_result = cached;
                        }
                    }
                    block_idx += 1;
                }
            }
            Some(new_doc)
        });
        Self {
            document,
            document_path: self.document_path.clone(),
            viewport_top: 0,
            viewport_left: 0,
            viewport_height: 0,
            viewport_width: 0,
            block_selected: self.block_selected,
            block_region: self.block_region,
            block_row: self.block_row,
            block_col: self.block_col,
            block_edit: None,
            block_draft: None,
            // Splits start with a single empty inactive slot — the
            // mirror carries the active tab's state. The new pane gets
            // its own tab strip, independent from the source's.
            block_tabs: vec![BlockTab::empty()],
            block_tab_active: 0,
        }
    }
}

/// Orientation of the *separator line* between split children.
///
/// - [`SplitDir::Vertical`] — vertical line; children placed left / right.
///   Matches vim `:vsplit` / `<C-w>v`.
/// - [`SplitDir::Horizontal`] — horizontal line; children placed top / bottom.
///   Matches vim `:split` / `<C-w>s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDir {
    Vertical,
    Horizontal,
}

/// Node in a pane tree. Either a leaf carrying a [`Pane`] or an inner
/// node holding two children separated by a [`SplitDir`].
//
// Story 5 grew `Pane` with the BLOCKS-view draft + edit fields (both
// boxed), so the size delta between `Leaf` and `Split` is acceptable
// — both variants live in `Box<PaneNode>` slots from the parent split
// anyway, so the extra slack only hits the single root `PaneNode`.
#[allow(clippy::large_enum_variant)]
pub enum PaneNode {
    Leaf(Pane),
    Split {
        direction: SplitDir,
        /// Fraction of the parent area assigned to `first`; clamped to
        /// `[0.1, 0.9]` so neither child gets squeezed out.
        ratio: f32,
        first: Box<PaneNode>,
        second: Box<PaneNode>,
    },
}

/// Direction for `Ctrl+W h/j/k/l` focus movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDir {
    Left,
    Right,
    Up,
    Down,
}

/// Outcome of [`TabState::close_focused`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseResult {
    /// Focused leaf removed; sibling promoted; focus moved.
    Closed,
    /// The tab had a single leaf — caller is responsible for closing
    /// the tab itself (we don't manage tab lifetime here).
    LastLeaf,
}

impl PaneNode {
    /// Walk to the node at `path`. Returns `None` if any step indexes
    /// into a leaf (path overshoots).
    pub fn walk(&self, path: &[u8]) -> Option<&PaneNode> {
        let mut node = self;
        for &step in path {
            match node {
                PaneNode::Leaf(_) => return None,
                PaneNode::Split { first, second, .. } => {
                    node = if step == 0 { first } else { second };
                }
            }
        }
        Some(node)
    }

    pub fn walk_mut(&mut self, path: &[u8]) -> Option<&mut PaneNode> {
        let mut node = self;
        for &step in path {
            match node {
                PaneNode::Leaf(_) => return None,
                PaneNode::Split { first, second, .. } => {
                    node = if step == 0 { first } else { second };
                }
            }
        }
        Some(node)
    }

    /// Extend `path` from `self` into the leftmost (`first`-child)
    /// leaf. Used after a structural rewrite to land on a deterministic
    /// leaf when the previous focus path is no longer valid.
    fn descend_first(&self, path: &mut Vec<u8>) {
        let mut node = self;
        loop {
            match node {
                PaneNode::Leaf(_) => return,
                PaneNode::Split { first, .. } => {
                    path.push(0);
                    node = first;
                }
            }
        }
    }

    /// Extend `path` into the leaf nearest to the side we just came
    /// from, given a motion `dir`. When the sibling subtree is itself
    /// split, pick the child closest in the motion's axis so a
    /// rightmost pane → Left lands on the immediate neighbour (not
    /// the leftmost leaf of the entire layout). Transversal splits
    /// (e.g. Horizontal split while moving Left) keep the default
    /// `first` choice — neither child is "closer" along the motion.
    fn descend_toward(&self, dir: FocusDir, path: &mut Vec<u8>) {
        let mut node = self;
        loop {
            match node {
                PaneNode::Leaf(_) => return,
                PaneNode::Split {
                    direction,
                    first,
                    second,
                    ..
                } => {
                    let go_second = match (dir, *direction) {
                        // Left motion into a Vertical sibling: pick its
                        // right half — that's the column visually
                        // adjacent to where we came from.
                        (FocusDir::Left, SplitDir::Vertical) => true,
                        (FocusDir::Right, SplitDir::Vertical) => false,
                        // Up motion into a Horizontal sibling: pick its
                        // bottom half (closest row to where we came
                        // from).
                        (FocusDir::Up, SplitDir::Horizontal) => true,
                        (FocusDir::Down, SplitDir::Horizontal) => false,
                        // Transversal: neither child is closer; keep
                        // `first` for determinism.
                        _ => false,
                    };
                    if go_second {
                        path.push(1);
                        node = second;
                    } else {
                        path.push(0);
                        node = first;
                    }
                }
            }
        }
    }

    fn leaf_count(&self) -> usize {
        match self {
            PaneNode::Leaf(_) => 1,
            PaneNode::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    /// Borrow every leaf pane in depth-first order.
    pub fn leaf_panes(&self) -> Vec<&Pane> {
        let mut out = Vec::new();
        self.collect_leaf_panes(&mut out);
        out
    }

    fn collect_leaf_panes<'a>(&'a self, out: &mut Vec<&'a Pane>) {
        match self {
            PaneNode::Leaf(p) => out.push(p),
            PaneNode::Split { first, second, .. } => {
                first.collect_leaf_panes(out);
                second.collect_leaf_panes(out);
            }
        }
    }
}

/// A tab's pane tree plus the path of the currently focused leaf.
///
/// `focused` is a sequence of `0`/`1` directions from `root` down to a
/// leaf (`0` = `first` child, `1` = `second` child). The empty path
/// means the root itself is the focused leaf.
pub struct TabState {
    pub root: PaneNode,
    pub focused: Vec<u8>,
}

impl TabState {
    pub fn new(pane: Pane) -> Self {
        Self {
            root: PaneNode::Leaf(pane),
            focused: Vec::new(),
        }
    }

    pub fn active_leaf(&self) -> &Pane {
        match self.root.walk(&self.focused) {
            Some(PaneNode::Leaf(p)) => p,
            _ => panic!("focused path does not point to a leaf"),
        }
    }

    pub fn active_leaf_mut(&mut self) -> &mut Pane {
        match self.root.walk_mut(&self.focused) {
            Some(PaneNode::Leaf(p)) => p,
            _ => panic!("focused path does not point to a leaf"),
        }
    }

    pub fn leaf_count(&self) -> usize {
        self.root.leaf_count()
    }

    /// Split the focused leaf along `direction`, inserting `new_pane`
    /// as the second child and moving focus there.
    pub fn split(&mut self, direction: SplitDir, new_pane: Pane) {
        let target = self.root.walk_mut(&self.focused).expect("focus path stale");
        let PaneNode::Leaf(_) = target else {
            panic!("focused path does not point to a leaf");
        };
        // Take the existing leaf out by swapping in a placeholder, then
        // wrap with a Split node.
        let placeholder = PaneNode::Leaf(Pane::empty());
        let existing = std::mem::replace(target, placeholder);
        *target = PaneNode::Split {
            direction,
            ratio: 0.5,
            first: Box::new(existing),
            second: Box::new(PaneNode::Leaf(new_pane)),
        };
        self.focused.push(1);
    }

    /// Close the focused split. When the focused leaf is the root,
    /// returns [`CloseResult::LastLeaf`] without mutating; otherwise the
    /// focused leaf is removed, its sibling is promoted in place of the
    /// parent split, and focus descends into the promoted subtree's
    /// first leaf.
    pub fn close_focused(&mut self) -> CloseResult {
        if self.focused.is_empty() {
            return CloseResult::LastLeaf;
        }
        let mut path = self.focused.clone();
        let last = path.pop().expect("focused.is_empty() checked above");
        // Walk to the parent split.
        let parent = self.root.walk_mut(&path).expect("focus parent path stale");
        // Replace the parent Split with its surviving sibling subtree.
        let placeholder = PaneNode::Leaf(Pane::empty());
        let split = std::mem::replace(parent, placeholder);
        let PaneNode::Split { first, second, .. } = split else {
            panic!("parent on focus path was not a Split");
        };
        let sibling = if last == 0 { *second } else { *first };
        *parent = sibling;
        // Re-anchor focus to a real leaf inside the promoted subtree.
        self.focused = path;
        let promoted = self
            .root
            .walk(&self.focused)
            .expect("promoted subtree path missing");
        promoted.descend_first(&mut self.focused);
        CloseResult::Closed
    }

    /// Cycle focus forward in depth-first leaf order, wrapping around.
    /// No-op when the tab has only one leaf.
    pub fn cycle_focus(&mut self) -> bool {
        let leaves: Vec<Vec<u8>> = collect_leaf_paths(&self.root);
        if leaves.len() <= 1 {
            return false;
        }
        let pos = leaves.iter().position(|p| p == &self.focused).unwrap_or(0);
        let next = (pos + 1) % leaves.len();
        self.focused = leaves[next].clone();
        true
    }

    /// Move focus in the requested direction. Walks up the focus path
    /// looking for the nearest ancestor whose split direction matches
    /// the motion AND whose child slot we're on is the "wrong" side
    /// relative to the motion (e.g. `h` requires we're on `second` of a
    /// Vertical split). Found → jump into the sibling subtree and
    /// descend to its leftmost leaf. Not found → no-op.
    pub fn focus_dir(&mut self, dir: FocusDir) -> bool {
        let (want_dir, from_idx) = match dir {
            FocusDir::Left => (SplitDir::Vertical, 1u8),
            FocusDir::Right => (SplitDir::Vertical, 0u8),
            FocusDir::Up => (SplitDir::Horizontal, 1u8),
            FocusDir::Down => (SplitDir::Horizontal, 0u8),
        };
        // Walk up the path looking for the first matching ancestor.
        for depth in (0..self.focused.len()).rev() {
            let last_step = self.focused[depth];
            if last_step != from_idx {
                continue;
            }
            let parent_path = &self.focused[..depth];
            let parent = self.root.walk(parent_path).expect("focus path stale");
            let PaneNode::Split { direction, .. } = parent else {
                continue;
            };
            if *direction != want_dir {
                continue;
            }
            // Found. Build new focus path: parent_path + sibling_idx + descend.
            let sibling = 1 - last_step;
            let mut new_focused = parent_path.to_vec();
            new_focused.push(sibling);
            let target = self.root.walk(&new_focused).expect("sibling path stale");
            target.descend_toward(dir, &mut new_focused);
            self.focused = new_focused;
            return true;
        }
        false
    }

    /// Reset every split ratio to 0.5.
    pub fn equalize(&mut self) {
        equalize_node(&mut self.root);
    }
}

fn collect_leaf_paths(node: &PaneNode) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut stack: Vec<u8> = Vec::new();
    walk_leaves(node, &mut stack, &mut out);
    out
}

fn walk_leaves(node: &PaneNode, path: &mut Vec<u8>, out: &mut Vec<Vec<u8>>) {
    match node {
        PaneNode::Leaf(_) => out.push(path.clone()),
        PaneNode::Split { first, second, .. } => {
            path.push(0);
            walk_leaves(first, path, out);
            path.pop();
            path.push(1);
            walk_leaves(second, path, out);
            path.pop();
        }
    }
}

fn equalize_node(node: &mut PaneNode) {
    match node {
        PaneNode::Leaf(_) => {}
        PaneNode::Split {
            ratio,
            first,
            second,
            ..
        } => {
            *ratio = 0.5;
            equalize_node(first);
            equalize_node(second);
        }
    }
}

#[cfg(test)]
mod tests {
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
            block_edit: None,
            block_draft: None,
            block_tabs: vec![BlockTab::empty()],
            block_tab_active: 0,
        }
    }

    #[test]
    fn snapshot_clone_preserves_cached_result_across_blocks() {
        // Build a pane whose Document has two HTTP blocks, populate
        // the second one's cached_result, then clone — the new pane
        // must keep the cached_result on the same block (split must
        // not wipe the response card).
        use crate::buffer::Segment;
        let md =
            "```http alias=a\nGET https://x.com\n```\n\n```http alias=b\nGET https://y.com\n```\n";
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
}
