//! BLOCKS-view tab strip: per-pane stack of `BlockTab` snapshots and
//! the swap/push/close helpers that keep the pane mirror fields in
//! sync with the active slot.
//!
//! Why a sibling module: the active tab's truth lives in the pane's
//! direct fields (`document`, `block_selected`, …) so the 200+ existing
//! call sites that read/write them stay unchanged. The inactive tabs
//! live in `pane.block_tabs`; switching tabs MOVES the mirror into the
//! previously-active slot and pulls the target slot's contents back
//! into the mirror. The slot for `block_tab_active` itself is left as
//! an empty placeholder between swaps — `inactive_tab(idx)` returns
//! `None` for that index to make the invariant explicit at the
//! reader.
//!
//! Moves (not clones) are used throughout: `Document` and the boxed
//! `RegionEdit` / `BlockDraft` aren't `Clone`, and the swap semantics
//! are inherently transfer-of-ownership.

use std::path::PathBuf;

use crate::app::{BlockDraft, BlockRef, RegionEdit};
use crate::buffer::Document;
use crate::pane::{CloseResult, Pane};

/// One slot in the BLOCKS-view tab strip. Snapshots the nine pane
/// fields that change per-tab. Cross-file by design — each tab can
/// point at a different `.md` and carry its own document + cursor +
/// edit state.
pub struct BlockTab {
    pub document: Option<Document>,
    pub document_path: Option<PathBuf>,
    pub viewport_top: u16,
    pub block_selected: Option<BlockRef>,
    pub block_region: usize,
    pub block_row: usize,
    pub block_col: usize,
    pub block_edit: Option<Box<RegionEdit>>,
    pub block_draft: Option<Box<BlockDraft>>,
}

impl BlockTab {
    pub fn empty() -> Self {
        Self {
            document: None,
            document_path: None,
            viewport_top: 0,
            block_selected: None,
            block_region: 0,
            block_row: 0,
            block_col: 1,
            block_edit: None,
            block_draft: None,
        }
    }
}

impl Pane {
    /// Number of BLOCKS-view tabs in this pane. Always `>= 1`.
    pub fn tab_count(&self) -> usize {
        self.block_tabs.len()
    }

    /// Inactive tab snapshot read directly from the strip. Returns
    /// `None` for the active index — that slot is conceptually empty
    /// because the truth lives in the pane mirror fields. Callers
    /// rendering the tab bar must read `pane.block_selected` /
    /// `pane.block_draft` for the active tab.
    pub fn inactive_tab(&self, idx: usize) -> Option<&BlockTab> {
        if idx == self.block_tab_active {
            None
        } else {
            self.block_tabs.get(idx)
        }
    }

    fn drain_mirror(&mut self) -> BlockTab {
        BlockTab {
            document: self.document.take(),
            document_path: self.document_path.take(),
            viewport_top: self.viewport_top,
            block_selected: self.block_selected.take(),
            block_region: self.block_region,
            block_row: self.block_row,
            block_col: self.block_col,
            block_edit: self.block_edit.take(),
            block_draft: self.block_draft.take(),
        }
    }

    fn restore_mirror_from(&mut self, tab: BlockTab) {
        self.document = tab.document;
        self.document_path = tab.document_path;
        self.viewport_top = tab.viewport_top;
        self.block_selected = tab.block_selected;
        self.block_region = tab.block_region;
        self.block_row = tab.block_row;
        self.block_col = tab.block_col;
        self.block_edit = tab.block_edit;
        self.block_draft = tab.block_draft;
    }

    /// Activate `idx`. No-op when `idx` is already active or out of
    /// range. Moves the current mirror INTO the previously-active slot
    /// (so the strip stays consistent) and pulls the target slot INTO
    /// the mirror.
    pub fn swap_to_tab(&mut self, idx: usize) -> bool {
        if idx >= self.block_tabs.len() || idx == self.block_tab_active {
            return false;
        }
        let snapshot = self.drain_mirror();
        self.block_tabs[self.block_tab_active] = snapshot;
        let target = std::mem::replace(&mut self.block_tabs[idx], BlockTab::empty());
        self.restore_mirror_from(target);
        self.block_tab_active = idx;
        true
    }

    /// Push an empty tab to the end of the strip and activate it.
    /// Used by `Ctrl+T` — the greeter populates the new tab afterwards.
    pub fn push_blank_tab(&mut self) -> usize {
        let snapshot = self.drain_mirror();
        self.block_tabs[self.block_tab_active] = snapshot;
        self.block_tabs.push(BlockTab::empty());
        self.block_tab_active = self.block_tabs.len() - 1;
        // mirror is already empty after `drain_mirror`; nothing to restore.
        self.block_tab_active
    }

    /// Push a pre-populated tab to the end of the strip and activate it.
    /// Used by `Ctrl+Enter` from the tree — open a block as a new tab
    /// without replacing the current one.
    pub fn push_block_tab(&mut self, tab: BlockTab) -> usize {
        let snapshot = self.drain_mirror();
        self.block_tabs[self.block_tab_active] = snapshot;
        self.block_tabs.push(BlockTab::empty());
        self.block_tab_active = self.block_tabs.len() - 1;
        self.restore_mirror_from(tab);
        self.block_tab_active
    }

    /// Replace the active tab's state with `tab`. Originally wired to
    /// tree `Enter`; Smart Enter inlines the mirror assignment now,
    /// so this stays as a typed helper for future surfaces
    /// (template paste, etc) that want the same swap-in-place
    /// semantics without juggling the mirror by hand.
    #[allow(dead_code)]
    pub fn replace_active_tab(&mut self, tab: BlockTab) {
        let _ = self.drain_mirror();
        self.restore_mirror_from(tab);
    }

    /// Close the active tab. Returns:
    /// - [`CloseResult::Closed`] when a sibling tab remains and was
    ///   activated. New focus prefers the slot to the right (next),
    ///   or falls back to the slot to the left when the closed tab
    ///   was last.
    /// - [`CloseResult::LastLeaf`] when the active tab was the only
    ///   one left — the caller (the dispatcher) collapses the
    ///   surrounding `PaneNode` via `TabState::close_focused`.
    pub fn close_active_tab(&mut self) -> CloseResult {
        if self.block_tabs.len() <= 1 {
            return CloseResult::LastLeaf;
        }
        let _ = self.drain_mirror();
        self.block_tabs.remove(self.block_tab_active);
        if self.block_tab_active >= self.block_tabs.len() {
            self.block_tab_active = self.block_tabs.len() - 1;
        }
        let target = std::mem::replace(
            &mut self.block_tabs[self.block_tab_active],
            BlockTab::empty(),
        );
        self.restore_mirror_from(target);
        CloseResult::Closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::Pane;

    fn pane_with_block(name: &str, alias: &str) -> Pane {
        let mut p = Pane::empty();
        p.document_path = Some(PathBuf::from(name));
        p.block_selected = Some(BlockRef {
            file_idx: 0,
            block_idx: alias.bytes().next().unwrap_or(b'a') as usize,
        });
        p.block_region = 2;
        p.block_row = 3;
        p.block_col = 1;
        p
    }

    #[test]
    fn new_pane_has_one_tab() {
        let p = Pane::new(
            Document::from_markdown("x\n").unwrap(),
            PathBuf::from("a.md"),
        );
        assert_eq!(p.tab_count(), 1);
        assert_eq!(p.block_tab_active, 0);
        assert!(
            p.inactive_tab(0).is_none(),
            "active slot reads through the mirror, not the strip",
        );
    }

    #[test]
    fn push_blank_tab_activates_a_clean_empty_tab() {
        let mut p = pane_with_block("a.md", "a");
        assert_eq!(p.tab_count(), 1);
        let new_idx = p.push_blank_tab();
        assert_eq!(new_idx, 1);
        assert_eq!(p.tab_count(), 2);
        assert_eq!(p.block_tab_active, 1);
        // Mirror cleared.
        assert!(p.document.is_none());
        assert!(p.block_selected.is_none());
        // The previous tab's state landed in the inactive slot.
        let prev = p.inactive_tab(0).expect("prev tab in strip");
        assert_eq!(prev.document_path.as_ref().unwrap(), &PathBuf::from("a.md"));
        assert!(prev.block_selected.is_some());
    }

    #[test]
    fn swap_to_tab_roundtrips_state() {
        let mut p = pane_with_block("a.md", "a");
        p.push_blank_tab();
        // On the blank tab now — populate it.
        p.document_path = Some(PathBuf::from("b.md"));
        p.block_region = 0;
        p.block_row = 0;
        // Swap back to the first tab.
        assert!(p.swap_to_tab(0));
        assert_eq!(p.block_tab_active, 0);
        assert_eq!(p.document_path.as_ref().unwrap(), &PathBuf::from("a.md"));
        assert_eq!(p.block_region, 2);
        assert_eq!(p.block_row, 3);
        // And tab 1 is preserved.
        let t1 = p.inactive_tab(1).expect("tab 1 still in strip");
        assert_eq!(t1.document_path.as_ref().unwrap(), &PathBuf::from("b.md"));
        assert_eq!(t1.block_region, 0);
    }

    #[test]
    fn swap_to_self_or_oob_is_noop() {
        let mut p = pane_with_block("a.md", "a");
        assert!(!p.swap_to_tab(0), "self-swap returns false");
        assert!(!p.swap_to_tab(99), "out-of-range returns false");
        assert_eq!(p.tab_count(), 1);
        assert_eq!(p.block_tab_active, 0);
    }

    #[test]
    fn push_block_tab_activates_the_payload() {
        let mut p = pane_with_block("a.md", "a");
        let payload = BlockTab {
            document: None,
            document_path: Some(PathBuf::from("c.md")),
            viewport_top: 7,
            block_selected: Some(BlockRef {
                file_idx: 42,
                block_idx: 99,
            }),
            block_region: 1,
            block_row: 4,
            block_col: 0,
            block_edit: None,
            block_draft: None,
        };
        let new_idx = p.push_block_tab(payload);
        assert_eq!(new_idx, 1);
        assert_eq!(p.tab_count(), 2);
        assert_eq!(p.block_tab_active, 1);
        // Mirror reflects the pushed payload.
        assert_eq!(p.document_path.as_ref().unwrap(), &PathBuf::from("c.md"));
        assert_eq!(p.viewport_top, 7);
        assert_eq!(p.block_selected.as_ref().unwrap().file_idx, 42);
        assert_eq!(p.block_region, 1);
    }

    #[test]
    fn replace_active_tab_overwrites_in_place() {
        let mut p = pane_with_block("a.md", "a");
        let payload = BlockTab {
            document: None,
            document_path: Some(PathBuf::from("z.md")),
            viewport_top: 0,
            block_selected: None,
            block_region: 5,
            block_row: 0,
            block_col: 0,
            block_edit: None,
            block_draft: None,
        };
        p.replace_active_tab(payload);
        assert_eq!(p.tab_count(), 1, "strip length unchanged");
        assert_eq!(p.block_tab_active, 0);
        assert_eq!(p.document_path.as_ref().unwrap(), &PathBuf::from("z.md"));
        assert_eq!(p.block_region, 5);
    }

    #[test]
    fn close_active_tab_on_single_tab_returns_last_leaf() {
        let mut p = pane_with_block("a.md", "a");
        assert_eq!(p.close_active_tab(), CloseResult::LastLeaf);
        // No mutation.
        assert_eq!(p.tab_count(), 1);
        assert_eq!(p.document_path.as_ref().unwrap(), &PathBuf::from("a.md"));
    }

    #[test]
    fn close_active_tab_drops_active_and_activates_prev() {
        let mut p = pane_with_block("a.md", "a");
        p.push_blank_tab();
        p.document_path = Some(PathBuf::from("b.md"));
        p.block_region = 0;
        // Closes tab 1 (the blank one we just populated). Active falls
        // back to tab 0 (the original).
        assert_eq!(p.close_active_tab(), CloseResult::Closed);
        assert_eq!(p.tab_count(), 1);
        assert_eq!(p.block_tab_active, 0);
        assert_eq!(p.document_path.as_ref().unwrap(), &PathBuf::from("a.md"));
        assert_eq!(p.block_region, 2, "tab 0's region preserved");
    }

    #[test]
    fn close_middle_tab_keeps_active_pointing_at_a_real_slot() {
        let mut p = pane_with_block("a.md", "a");
        p.push_blank_tab();
        p.document_path = Some(PathBuf::from("b.md"));
        p.push_blank_tab();
        p.document_path = Some(PathBuf::from("c.md"));
        // Strip: [a, b, c], active = 2. Swap to middle (index 1).
        assert!(p.swap_to_tab(1));
        assert_eq!(p.block_tab_active, 1);
        // Close the middle tab. Strip shrinks; active stays in range.
        assert_eq!(p.close_active_tab(), CloseResult::Closed);
        assert_eq!(p.tab_count(), 2);
        assert!(p.block_tab_active < p.tab_count());
        let paths: Vec<PathBuf> = (0..p.tab_count())
            .map(|i| {
                if i == p.block_tab_active {
                    p.document_path.clone().unwrap()
                } else {
                    p.inactive_tab(i).unwrap().document_path.clone().unwrap()
                }
            })
            .collect();
        assert!(paths.contains(&PathBuf::from("a.md")));
        assert!(paths.contains(&PathBuf::from("c.md")));
        assert!(!paths.contains(&PathBuf::from("b.md")));
    }
}
