//! `impl App` — pane / document accessors + viewport refresh.
//!
//! Mechanically extracted from the monolithic `impl App` in `app.rs`
//! (tui-v2 vertical 1, fase 2 p2-accessors) — pure code move, no
//! behavior change. Sibling `impl App {}` block; methods stay `pub fn`
//! so every `app.foo()` call site keeps resolving unchanged.

use std::path::PathBuf;

use crate::buffer::layout::layout_document;
use crate::buffer::Document;
use crate::pane::{Pane, TabState};

use super::{clamp_viewport, cursor_y, App};

impl App {
    // ----- pane accessors --------------------------------------------------

    pub fn active_tab(&self) -> Option<&TabState> {
        self.tabs.tabs.get(self.tabs.active)
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut TabState> {
        self.tabs.tabs.get_mut(self.tabs.active)
    }

    pub fn active_pane(&self) -> Option<&Pane> {
        self.active_tab().map(|t| t.active_leaf())
    }

    pub fn active_pane_mut(&mut self) -> Option<&mut Pane> {
        self.active_tab_mut().map(|t| t.active_leaf_mut())
    }

    /// The document the vim engine should operate on right now. When
    /// the row-detail modal is open this returns the modal's body
    /// `Document`, so motions, search, visual, yank — every read
    /// pathway in the dispatch — see the modal as the active buffer.
    /// File-save / status-bar code that needs the editor's note
    /// specifically should reach for `tabs.active_document()` instead.
    pub fn document(&self) -> Option<&Document> {
        if let Some(state) = self.db_row_detail.as_ref() {
            return Some(&state.doc);
        }
        if let Some(state) = self.http_response_detail.as_ref() {
            return Some(&state.doc);
        }
        self.active_pane().and_then(|p| p.document.as_ref())
    }

    /// Mutable counterpart of [`Self::document`]. Same redirect: the
    /// modal's body doc wins while the modal is up. Mutating the
    /// modal doc is fine — the parser filter blocks every action
    /// that would actually change its contents (insert / edit /
    /// paste / undo), so this stays "read-only" from the user's
    /// perspective.
    pub fn document_mut(&mut self) -> Option<&mut Document> {
        // Two-step access keeps the borrow checker happy: probe
        // `is_some` immutably (drops at end of `if`), then take a
        // fresh mut borrow only on the modal branch.
        if self.db_row_detail.is_some() {
            return self.db_row_detail.as_mut().map(|s| &mut s.doc);
        }
        if self.http_response_detail.is_some() {
            return self.http_response_detail.as_mut().map(|s| &mut s.doc);
        }
        self.active_pane_mut().and_then(|p| p.document.as_mut())
    }

    pub fn document_path(&self) -> Option<&PathBuf> {
        self.active_pane().and_then(|p| p.document_path.as_ref())
    }

    /// Height the vim engine should use for half-page motions. While
    /// the modal is open this returns the modal's body height — same
    /// reasoning as [`Self::document`].
    pub fn viewport_height(&self) -> u16 {
        if let Some(state) = self.db_row_detail.as_ref() {
            return state.viewport_height;
        }
        if let Some(state) = self.http_response_detail.as_ref() {
            return state.viewport_height;
        }
        self.active_pane().map(|p| p.viewport_height).unwrap_or(0)
    }

    // ----- viewport refresh ----------------------------------------------

    /// Re-anchor the viewport so the cursor stays visible after a
    /// motion or edit. Public so the vim dispatcher can call it.
    pub fn refresh_viewport_for_cursor(&mut self) {
        let Some(pane) = self.active_pane_mut() else {
            return;
        };
        let Some(doc) = pane.document.as_ref() else {
            return;
        };
        let layouts = layout_document(doc, 80);
        let cursor_y = cursor_y(doc, &layouts);
        pane.viewport_top = clamp_viewport(pane.viewport_top, pane.viewport_height, cursor_y);
    }
}
