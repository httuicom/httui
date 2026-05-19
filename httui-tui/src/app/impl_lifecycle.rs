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
