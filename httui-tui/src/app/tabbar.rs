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
}
