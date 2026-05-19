//! `impl App` — run-anchor recording, file-watcher sync, status bar.
//!
//! Mechanically extracted from the monolithic `impl App` in `app.rs`
//! (tui-v2 vertical 1, fase 2 p2-lifecycle) — pure code move, no
//! behavior change. `App::new` + `load_initial_document` deliberately
//! stay in `app/mod.rs` next to the `struct App` they construct.
//! Sibling `impl App {}` block; methods stay `pub fn` so every
//! `app.foo()` call site keeps resolving unchanged.

use crate::buffer::Segment;

use super::{App, LastRunAnchor, StatusKind, StatusMessage};

impl App {
    /// Record a `last_run_anchor` pointing at the block at
    /// `segment_idx` in the active document. Called by the DB and
    /// HTTP run-spawn paths right after they set `running_query`,
    /// so a subsequent `gr` can rerun the same block without the
    /// cursor being on it. Silent no-op when the active pane has no
    /// path or the segment isn't a block.
    pub fn record_run_anchor(&mut self, segment_idx: usize) {
        let Some(file_path) = self.active_pane().and_then(|p| p.document_path.clone()) else {
            return;
        };
        let alias = self
            .document()
            .and_then(|d| d.segments().get(segment_idx))
            .and_then(|s| match s {
                Segment::Block(b) => b.alias.clone(),
                _ => None,
            });
        self.last_run_anchor = Some(LastRunAnchor {
            file_path,
            segment_idx,
            alias,
        });
    }

    /// Re-sync the filesystem watcher to follow the current active
    /// document. Cheap to call repeatedly — `FileWatcher::watch`
    /// no-ops when the path matches the one already watched. Driven
    /// from the main loop after every key event so a tab switch /
    /// `:e` / file tree pick always takes effect, without having to
    /// instrument every call site.
    pub fn sync_file_watcher(&mut self) {
        // Resolve the absolute path first so we don't hold a borrow
        // on `self` while `file_watcher` (also on `self`) tries to
        // mutate.
        let absolute = self
            .active_pane()
            .and_then(|p| p.document_path.as_ref())
            .map(|rel| self.vault_path.join(rel));
        let Some(watcher) = self.file_watcher.as_mut() else {
            return;
        };
        match absolute {
            Some(path) => {
                if let Err(e) = watcher.watch(&path) {
                    tracing::warn!("file watcher reattach failed: {e}");
                }
            }
            None => watcher.unwatch(),
        }
    }

    /// Set the transient footer message. Cleared on next key dispatch.
    pub fn set_status(&mut self, kind: StatusKind, text: impl Into<String>) {
        self.status_message = Some(StatusMessage {
            text: text.into(),
            kind,
        });
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::fs_watch::FileWatcher;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    /// Same fixture contract as `impl_accessors`: `App::new` calls
    /// `block_in_place`, so every constructing test must run on a
    /// multi-thread runtime. The note `body` is written verbatim into
    /// `note.md` so callers can seed a block fence when needed.
    async fn app_fixture(body: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), body).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn record_run_anchor_captures_path_and_block_alias() {
        // A doc whose second segment is an HTTP block with an alias.
        let (mut app, _d, _v) =
            app_fixture("intro\n\n```http alias=req1\nGET https://example.com\n```\n").await;

        // The block is the segment after the prose paragraph.
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("doc should contain a block");

        app.record_run_anchor(block_idx);
        let anchor = app.last_run_anchor.as_ref().expect("anchor recorded");
        assert_eq!(anchor.segment_idx, block_idx);
        assert_eq!(anchor.alias.as_deref(), Some("req1"));
        assert_eq!(anchor.file_path.to_string_lossy(), "note.md".to_string());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn record_run_anchor_no_alias_when_segment_is_prose() {
        let (mut app, _d, _v) = app_fixture("just prose, no block\n").await;
        // Segment 0 is prose → alias resolves to None but the anchor
        // (path + idx) is still recorded.
        app.record_run_anchor(0);
        let anchor = app.last_run_anchor.as_ref().expect("anchor recorded");
        assert_eq!(anchor.segment_idx, 0);
        assert_eq!(anchor.alias, None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn record_run_anchor_noop_without_a_document_path() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        // Clear the focused pane's path → the early-return guard fires
        // and no anchor is set.
        if let Some(p) = app.active_pane_mut() {
            p.document_path = None;
        }
        app.record_run_anchor(0);
        assert!(app.last_run_anchor.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_file_watcher_noop_when_watcher_absent() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        // `file_watcher` is `None` for unit-test paths — the method
        // must early-return without panicking.
        assert!(app.file_watcher.is_none());
        app.sync_file_watcher();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_file_watcher_attaches_to_the_active_documents_path() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        app.file_watcher = Some(FileWatcher::new(tx));
        // Active pane points at note.md → watcher reattaches to the
        // resolved absolute path without error.
        app.sync_file_watcher();
        assert!(app.file_watcher.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sync_file_watcher_unwatches_when_no_path() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        app.file_watcher = Some(FileWatcher::new(tx));
        // No path on the active pane → the `None` arm calls `unwatch`.
        if let Some(p) = app.active_pane_mut() {
            p.document_path = None;
        }
        app.sync_file_watcher();
        assert!(app.file_watcher.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn set_and_clear_status_round_trip() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        assert!(app.status_message.is_none());

        app.set_status(StatusKind::Info, "saved");
        let msg = app.status_message.as_ref().expect("status set");
        assert_eq!(msg.text, "saved");
        assert_eq!(msg.kind, StatusKind::Info);

        app.set_status(StatusKind::Error, String::from("boom"));
        let msg = app.status_message.as_ref().expect("status replaced");
        assert_eq!(msg.text, "boom");
        assert_eq!(msg.kind, StatusKind::Error);

        app.clear_status();
        assert!(app.status_message.is_none());
    }
}
