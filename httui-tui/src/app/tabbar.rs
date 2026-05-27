//! Open-tab registry.
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-tabbar) — pure code move, no behavior change. Re-exported from
//! `app/mod.rs` so `crate::app::TabBar` keeps resolving. Exercised by
//! the App-level integration tests (via `App.tabs`) and `ui::tabs`.

use std::path::{Path, PathBuf};

use crate::pane::TabState;

/// Open-tab registry. Each tab owns an independent [`TabState`] (a
/// binary tree of panes); the active tab is `tabs[active]`. Inactive
/// tabs keep their full pane state — there is no "stash" indirection
/// any more, since the data lives in the tree itself.
#[derive(Default)]
pub struct TabBar {
    pub tabs: Vec<TabState>,
    pub active: usize,
}

impl TabBar {
    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn active(&self) -> usize {
        self.active
    }

    /// Path shown by each tab's focused leaf, in display order. Used to
    /// render tab titles and to detect whether a path is already open.
    #[allow(dead_code)] // exposed for tooling / future API
    pub fn focused_paths(&self) -> Vec<Option<PathBuf>> {
        self.tabs
            .iter()
            .map(|t| t.active_leaf().document_path.clone())
            .collect()
    }

    /// Index of the tab whose *focused* pane shows `path`, if any.
    /// Non-focused panes inside other tabs are not searched — they're
    /// not addressable through the tab bar.
    pub fn find_focused(&self, path: &Path) -> Option<usize> {
        self.tabs.iter().position(|t| {
            t.active_leaf()
                .document_path
                .as_deref()
                .is_some_and(|p| p == path)
        })
    }

    /// Mutable borrow of the active tab's focused-leaf document. Lives
    /// on `TabBar` (rather than `App`) so callers can hold it
    /// alongside borrows of disjoint `App` fields like
    /// `app.vim.unnamed` — Rust permits split borrows across distinct
    /// fields, but only when the borrow doesn't pass through a method
    /// call on `App` itself.
    pub fn active_document_mut(&mut self) -> Option<&mut crate::buffer::Document> {
        let idx = self.active;
        self.tabs.get_mut(idx)?.active_leaf_mut().document.as_mut()
    }

    /// Immutable counterpart of [`Self::active_document_mut`]. Same
    /// split-borrow rationale.
    pub fn active_document(&self) -> Option<&crate::buffer::Document> {
        let idx = self.active;
        self.tabs.get(idx)?.active_leaf().document.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;
    use crate::pane::Pane;

    fn tab_with_path(name: &str) -> TabState {
        TabState::new(Pane {
            document: None,
            document_path: Some(PathBuf::from(name)),
            viewport_top: 0,
            viewport_height: 0,
            block_selected: None,
            block_region: 0,
        })
    }

    fn tab_with_doc(name: &str) -> TabState {
        TabState::new(Pane::new(
            Document::from_markdown("body\n").unwrap(),
            PathBuf::from(name),
        ))
    }

    fn tab_no_path() -> TabState {
        TabState::new(Pane::empty())
    }

    #[test]
    fn default_tabbar_is_empty() {
        let tb = TabBar::default();
        assert!(tb.is_empty());
        assert_eq!(tb.len(), 0);
        assert_eq!(tb.active(), 0);
    }

    #[test]
    fn len_and_is_empty_track_the_tab_vec() {
        let mut tb = TabBar::default();
        tb.tabs.push(tab_with_path("a.md"));
        tb.tabs.push(tab_with_path("b.md"));
        assert!(!tb.is_empty());
        assert_eq!(tb.len(), 2);
    }

    #[test]
    fn active_returns_the_active_index() {
        let mut tb = TabBar::default();
        tb.tabs.push(tab_with_path("a.md"));
        tb.tabs.push(tab_with_path("b.md"));
        tb.active = 1;
        assert_eq!(tb.active(), 1);
    }

    #[test]
    fn focused_paths_lists_each_tabs_focused_leaf_path() {
        let mut tb = TabBar::default();
        tb.tabs.push(tab_with_path("a.md"));
        tb.tabs.push(tab_no_path());
        let paths = tb.focused_paths();
        assert_eq!(paths, vec![Some(PathBuf::from("a.md")), None]);
    }

    #[test]
    fn find_focused_returns_index_of_matching_tab() {
        let mut tb = TabBar::default();
        tb.tabs.push(tab_with_path("a.md"));
        tb.tabs.push(tab_with_path("b.md"));
        assert_eq!(tb.find_focused(Path::new("b.md")), Some(1));
        assert_eq!(tb.find_focused(Path::new("a.md")), Some(0));
    }

    #[test]
    fn find_focused_none_when_no_tab_shows_the_path() {
        let mut tb = TabBar::default();
        tb.tabs.push(tab_with_path("a.md"));
        tb.tabs.push(tab_no_path());
        assert_eq!(tb.find_focused(Path::new("missing.md")), None);
    }

    #[test]
    fn active_document_mut_borrows_the_active_tabs_document() {
        let mut tb = TabBar::default();
        tb.tabs.push(tab_with_doc("a.md"));
        tb.tabs.push(tab_with_doc("b.md"));
        tb.active = 1;
        let doc = tb.active_document_mut().expect("active doc");
        // Mutate through the borrow to prove it's the live document.
        doc.mark_dirty();
        assert!(tb.tabs[1]
            .active_leaf()
            .document
            .as_ref()
            .unwrap()
            .is_dirty());
        assert!(!tb.tabs[0]
            .active_leaf()
            .document
            .as_ref()
            .unwrap()
            .is_dirty());
    }

    #[test]
    fn active_document_mut_none_when_active_index_out_of_range() {
        let mut tb = TabBar::default();
        tb.tabs.push(tab_with_doc("a.md"));
        tb.active = 9; // past the end
        assert!(tb.active_document_mut().is_none());
    }

    #[test]
    fn active_document_mut_none_when_focused_leaf_has_no_document() {
        let mut tb = TabBar::default();
        tb.tabs.push(tab_no_path());
        assert!(tb.active_document_mut().is_none());
    }
}
