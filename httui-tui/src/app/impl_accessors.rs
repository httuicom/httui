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
        if let Some(state) = self.db_row_detail() {
            return Some(&state.doc);
        }
        if let Some(state) = self.http_response_detail() {
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
        if self.db_row_detail().is_some() {
            return self.db_row_detail_mut().map(|s| &mut s.doc);
        }
        if self.http_response_detail().is_some() {
            return self.http_response_detail_mut().map(|s| &mut s.doc);
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
        if let Some(state) = self.db_row_detail() {
            return state.viewport_height;
        }
        if let Some(state) = self.http_response_detail() {
            return state.viewport_height;
        }
        self.active_pane().map(|p| p.viewport_height).unwrap_or(0)
    }

    /// Borrow the open `DbRowDetail` state if that modal is current.
    /// Returns `None` either when no modal is open or when another
    /// modal variant occupies the slot. Sugar over the
    /// `app.modal.as_ref().and_then(Modal::as_db_row_detail)` pattern.
    pub fn db_row_detail(&self) -> Option<&crate::app::DbRowDetailState> {
        self.modal.as_ref().and_then(|m| m.as_db_row_detail())
    }

    pub fn db_row_detail_mut(&mut self) -> Option<&mut crate::app::DbRowDetailState> {
        self.modal.as_mut().and_then(|m| m.as_db_row_detail_mut())
    }

    pub fn http_response_detail(&self) -> Option<&crate::app::HttpResponseDetailState> {
        self.modal.as_ref().and_then(|m| m.as_http_response_detail())
    }

    pub fn http_response_detail_mut(&mut self) -> Option<&mut crate::app::HttpResponseDetailState> {
        self.modal.as_mut().and_then(|m| m.as_http_response_detail_mut())
    }

    pub fn content_search(&self) -> Option<&crate::app::ContentSearchState> {
        self.modal.as_ref().and_then(|m| m.as_content_search())
    }

    pub fn content_search_mut(&mut self) -> Option<&mut crate::app::ContentSearchState> {
        self.modal.as_mut().and_then(|m| m.as_content_search_mut())
    }

    // ----- result tabs (per-block) ---------------------------------------

    /// Selected result tab for `block_id`. Missing entry → default
    /// (`ResultPanelTab::Result`). Used by every render path that
    /// paints a block's result panel.
    pub fn result_tab_for(&self, block_id: crate::buffer::block::BlockId) -> crate::app::ResultPanelTab {
        self.result_tabs.get(&block_id).copied().unwrap_or_default()
    }

    pub fn set_result_tab(
        &mut self,
        block_id: crate::buffer::block::BlockId,
        tab: crate::app::ResultPanelTab,
    ) {
        self.result_tabs.insert(block_id, tab);
    }

    /// Cycle the result tab of the block under the cursor. `dir >= 0`
    /// goes next, `dir < 0` goes prev. Works whenever the cursor is on
    /// a block — whether the user is editing the request (`InBlock`)
    /// or sitting in a result row (`InBlockResult`). No-op for `InProse`
    /// so Tab/Shift+Tab don't fire outside any block.
    pub fn cycle_result_tab_at_cursor(&mut self, dir: i32) {
        let Some(doc) = self.document() else { return };
        let segment_idx = match doc.cursor() {
            crate::buffer::Cursor::InBlock { segment_idx, .. }
            | crate::buffer::Cursor::InBlockResult { segment_idx, .. } => segment_idx,
            crate::buffer::Cursor::InProse { .. } => return,
        };
        let Some(block) = doc.block_at(segment_idx) else {
            return;
        };
        let block_id = block.id;
        let block_type = block.block_type.clone();
        let current = self.result_tab_for(block_id);
        let next = if dir >= 0 {
            current.next_for(&block_type)
        } else {
            current.prev_for(&block_type)
        };
        self.set_result_tab(block_id, next);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{DbRowDetailState, HttpResponseDetailState};
    use crate::buffer::Document;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    /// Build a real `App` rooted at a fresh tmpdir vault containing one
    /// markdown note, with an isolated (empty-schema) SQLite pool.
    ///
    /// `App::new` runs `load_connection_names` + `refresh_active_env_name`,
    /// both of which call `tokio::task::block_in_place` — that panics on a
    /// current-thread runtime, so every constructing test is
    /// `#[tokio::test(flavor = "multi_thread")]`. Returns the temp dirs
    /// too so they outlive the `App` (dropping them deletes the vault).
    async fn app_fixture(note: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), note).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    fn detail_doc(body: &str) -> Document {
        Document::from_markdown(body).unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn accessors_resolve_the_initial_documents_pane() {
        let (app, _d, _v) = app_fixture("# hello\n\nworld\n").await;

        // The single tab loaded by `App::new` is active …
        assert!(app.active_tab().is_some());
        assert!(app.active_pane().is_some());
        // … and its document + path are reachable through the accessors.
        assert!(app.document().is_some());
        assert_eq!(
            app.document_path().map(|p| p.to_string_lossy().to_string()),
            Some("note.md".to_string())
        );
        assert_eq!(app.viewport_height(), 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn mutable_accessors_yield_the_same_pane() {
        let (mut app, _d, _v) = app_fixture("body text\n").await;

        assert!(app.active_tab_mut().is_some());
        let pane = app.active_pane_mut().expect("active pane");
        pane.viewport_height = 42;
        pane.viewport_top = 3;
        // Re-read through the immutable path to confirm the write stuck.
        assert_eq!(app.viewport_height(), 42);
        assert!(app.document_mut().is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn document_redirects_to_db_row_detail_modal_while_open() {
        let (mut app, _d, _v) = app_fixture("editor body\n").await;
        app.modal = Some(crate::modal::Modal::DbRowDetail(DbRowDetailState {
            segment_idx: 0,
            row: 0,
            title: "row".into(),
            doc: detail_doc("MODAL BODY\n"),
            viewport_height: 7,
            viewport_top: 0,
        }));

        // While the modal is up the accessors return the modal doc, not
        // the editor's note, and report the modal's viewport height.
        // Compare against an independently-serialized reference doc so
        // the assertion is roundtrip-stable (markdown may normalize).
        let modal_md = detail_doc("MODAL BODY\n").to_markdown();
        let editor_md = detail_doc("editor body\n").to_markdown();
        assert_eq!(
            app.document().map(|d| d.to_markdown()),
            Some(modal_md.clone())
        );
        assert_ne!(app.document().map(|d| d.to_markdown()), Some(editor_md));
        assert_eq!(app.viewport_height(), 7);
        assert_eq!(app.document_mut().map(|d| d.to_markdown()), Some(modal_md));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn document_redirects_to_http_response_detail_modal_while_open() {
        let (mut app, _d, _v) = app_fixture("editor body\n").await;
        app.modal = Some(crate::modal::Modal::HttpResponseDetail(
            HttpResponseDetailState {
                segment_idx: 0,
                title: "resp".into(),
                doc: detail_doc("HTTP MODAL\n"),
                viewport_height: 9,
                viewport_top: 0,
            },
        ));

        let modal_md = detail_doc("HTTP MODAL\n").to_markdown();
        assert_eq!(
            app.document().map(|d| d.to_markdown()),
            Some(modal_md.clone())
        );
        assert_eq!(app.viewport_height(), 9);
        assert_eq!(app.document_mut().map(|d| d.to_markdown()), Some(modal_md));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cycle_result_tab_only_affects_the_focused_block() {
        let md = "```db-postgres alias=a connection=c\nSELECT 1\n```\n\
                  \n\
                  ```db-postgres alias=b connection=c\nSELECT 2\n```\n";
        let (mut app, _d, _v) = app_fixture(md).await;
        let (id_a, id_b) = {
            let doc = app.tabs.active_document_mut().unwrap();
            let mut ids = doc.block_ids();
            (ids.next().unwrap(), ids.next().unwrap())
        };
        app.set_result_tab(id_b, crate::app::ResultPanelTab::Stats);
        let seg_a = {
            let doc = app.tabs.active_document_mut().unwrap();
            doc.block_ids()
                .enumerate()
                .find(|(_, id)| *id == id_a)
                .map(|(i, _)| i)
                .unwrap()
                + doc
                    .segments()
                    .iter()
                    .take_while(|s| !matches!(s, crate::buffer::Segment::Block(_)))
                    .count()
        };
        if let Some(doc) = app.tabs.active_document_mut() {
            doc.set_cursor(crate::buffer::Cursor::InBlockResult {
                segment_idx: seg_a,
                row: 0,
            });
        }
        app.cycle_result_tab_at_cursor(1);
        assert_eq!(
            app.result_tab_for(id_a),
            crate::app::ResultPanelTab::Result.next_for("db-postgres"),
        );
        assert_eq!(app.result_tab_for(id_b), crate::app::ResultPanelTab::Stats);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cycle_result_tab_also_works_from_in_block_cursor() {
        let md = "```db-postgres alias=a connection=c\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_fixture(md).await;
        let id_a = app
            .tabs
            .active_document_mut()
            .unwrap()
            .block_ids()
            .next()
            .unwrap();
        let seg_a = app
            .tabs
            .active_document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .unwrap();
        if let Some(doc) = app.tabs.active_document_mut() {
            doc.set_cursor(crate::buffer::Cursor::InBlock {
                segment_idx: seg_a,
                offset: 0,
            });
        }
        app.cycle_result_tab_at_cursor(1);
        assert_eq!(
            app.result_tab_for(id_a),
            crate::app::ResultPanelTab::Result.next_for("db-postgres"),
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cycle_result_tab_noop_in_prose() {
        let md = "just prose, no block here\n";
        let (mut app, _d, _v) = app_fixture(md).await;
        app.cycle_result_tab_at_cursor(1);
        assert!(app.result_tabs.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refresh_viewport_for_cursor_clamps_top() {
        let (mut app, _d, _v) = app_fixture("# title\n\nbody line\n").await;
        // A non-zero viewport_top with cursor at doc start must be
        // clamped back so the cursor stays visible.
        {
            let pane = app.active_pane_mut().unwrap();
            pane.viewport_height = 10;
            pane.viewport_top = 50;
        }
        app.refresh_viewport_for_cursor();
        assert_eq!(app.active_pane().unwrap().viewport_top, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refresh_viewport_for_cursor_noop_without_document() {
        let (mut app, _d, _v) = app_fixture("x\n").await;
        // Empty the focused pane's document → the early-return path.
        if let Some(p) = app.active_pane_mut() {
            p.document = None;
        }
        app.refresh_viewport_for_cursor();
        assert!(app.active_pane().unwrap().document.is_none());
    }
}
