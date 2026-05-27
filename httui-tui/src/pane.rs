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
    /// Height of the editor area allocated to this pane on the most
    /// recent frame. Updated by the renderer; read by motion code that
    /// needs page-relative scroll amounts (e.g. `Ctrl+D`).
    pub viewport_height: u16,
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
}

impl Pane {
    pub fn empty() -> Self {
        Self {
            document: None,
            document_path: None,
            viewport_top: 0,
            viewport_height: 0,
            block_selected: None,
            block_region: 0,
            block_row: 0,
            block_col: 1,
            block_edit: None,
            block_draft: None,
        }
    }

    pub fn new(document: Document, path: PathBuf) -> Self {
        Self {
            document: Some(document),
            document_path: Some(path),
            viewport_top: 0,
            viewport_height: 0,
            block_selected: None,
            block_region: 0,
            block_row: 0,
            block_col: 1,
            block_edit: None,
            block_draft: None,
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
        let document = self
            .document
            .as_ref()
            .and_then(|d| Document::from_markdown(&d.to_markdown()).ok());
        Self {
            document,
            document_path: self.document_path.clone(),
            viewport_top: 0,
            viewport_height: 0,
            block_selected: self.block_selected,
            block_region: self.block_region,
            block_row: self.block_row,
            block_col: self.block_col,
            block_edit: None,
            block_draft: None,
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

    fn leaf_count(&self) -> usize {
        match self {
            PaneNode::Leaf(_) => 1,
            PaneNode::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
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
            target.descend_first(&mut new_focused);
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
            viewport_height: 0,
            block_selected: None,
            block_region: 0,
            block_row: 0,
            block_col: 1,
            block_edit: None,
            block_draft: None,
        }
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
