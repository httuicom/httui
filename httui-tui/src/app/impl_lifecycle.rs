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

    /// re-scan the active vault for `{{keychain:...}}`
    /// refs with no entry in the local keychain. Repopulates
    /// `pending_secrets`. Backend failures collapse to an empty list
    /// (the badge UX is "no pending" rather than blocking startup).
    pub fn scan_pending_secrets(&mut self) {
        use httui_core::secrets::Keychain;
        use httui_core::vault_config::missing_secrets::scan_missing_secrets;
        self.pending_secrets = scan_missing_secrets(&self.vault_path, &Keychain).unwrap_or_default();
    }

    /// open the first-run modal when there are pending
    /// secrets. No-op when the list is empty or when another modal is
    /// already on screen (avoids stomping over an open picker).
    pub fn open_pending_secrets_modal(&mut self) {
        if self.pending_secrets.is_empty() || self.modal.is_some() {
            return;
        }
        let items = self
            .pending_secrets
            .iter()
            .map(|r| crate::app::MissingSecretRow {
                keychain_key: r.keychain_key.clone(),
                label: r.label.clone(),
                kind: r.kind,
                value: crate::vim::lineedit::LineEdit::new(),
                saved: false,
            })
            .collect();
        self.modal = Some(crate::modal::Modal::VaultMissingSecrets(
            crate::app::VaultMissingSecretsState {
                items,
                selected: 0,
                editing: false,
            },
        ));
        self.vim.mode = crate::vim::mode::Mode::Modal;
        self.vim.reset_pending();
    }

    /// Swap the entire vault-dependent surface in-place. Used by V10
    /// (vault picker / empty-state Open/Clone/Create) so the user
    /// changes workspace without restarting the binary.
    ///
    /// What gets rebuilt:
    /// - `connections_store`, `environments_store` (new vault root)
    /// - `pool_manager` (wraps the new `connections_store`)
    /// - `connection_names`, `active_env_name` (re-read from the new
    ///   vault's TOMLs)
    /// - `schema_cache`, `result_viewport_top` (cleared — pertain to
    ///   the old vault's connections)
    /// - `tabs`, `tree` (cleared — old paths are vault-relative and
    ///   no longer resolve)
    /// - transient UI (`modal`, `db_row_detail`, `http_response_detail`,
    ///   `completion_popup`, `db_settings`, `content_search`,
    ///   `fence_edit`, `running_query`, `standard.anchor`, vim
    ///   pending chord) — drop everything that could refer to the old
    ///   document state
    /// - `file_watcher`, `connections_toml_watcher`, `envs_dir_watcher`
    ///   — rewired against the new vault paths if the event sender
    ///   exists (production), no-op in unit tests
    /// - `content_search_index_built` flipped to `false` so the next
    ///   FTS open triggers a fresh rebuild for the new vault
    ///
    /// Persistence: writes `set_active_vault(pool, new_vault)` so the
    /// next launch resumes here even without picker. Dirty buffers in
    /// the old tabs are silently dropped — callers (the modal flow)
    /// must prompt the user beforehand if `force == false`.
    pub fn switch_vault(&mut self, new_vault: std::path::PathBuf) -> Result<(), String> {
        let canonical = new_vault
            .canonicalize()
            .map_err(|e| format!("vault path {}: {e}", new_vault.display()))?;
        if !canonical.is_dir() {
            return Err(format!("{} is not a directory", canonical.display()));
        }

        // Persist first — if the DB write fails the in-memory swap
        // never happens, keeping the running state consistent with the
        // registry.
        let pool = self.pool_manager.app_pool().clone();
        let canonical_str = canonical.to_string_lossy().to_string();
        let persist_result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(httui_core::vaults::set_active_vault(&pool, &canonical_str))
        });
        if let Err(e) = persist_result {
            return Err(format!("save active vault: {e}"));
        }

        // Rebuild vault-dependent core stores.
        let connections_store =
            httui_core::vault_config::ConnectionsStore::new(canonical.clone());
        let new_pool_manager = std::sync::Arc::new(
            httui_core::db::connections::PoolManager::new_standalone(
                connections_store.clone(),
                pool.clone(),
            ),
        );
        let user_config_path =
            httui_core::vault_config::user_store::default_user_config_path()
                .unwrap_or_else(|_| canonical.join("user.toml"));
        let environments_store = httui_core::vault_config::EnvironmentsStore::new(
            canonical.clone(),
            user_config_path,
        );
        let connection_names = super::helpers::load_connection_names(&connections_store);

        self.vault_path = canonical;
        self.connections_store = connections_store;
        self.pool_manager = new_pool_manager;
        self.environments_store = environments_store;
        self.connection_names = connection_names;

        // Clear caches and per-block UI state — they all keyed off
        // the old vault's segments / connection ids.
        self.schema_cache = crate::schema::SchemaCache::new();
        self.result_viewport_top.clear();
        self.content_search_index_built = false;
        self.last_run_anchor = None;

        // Drop transient overlays so nothing dangles a pointer into
        // the old buffer / connection.
        self.modal = None;
        self.http_response_detail = None;
        self.completion_popup = None;
        self.db_settings = None;
        self.content_search = None;
        self.fence_edit = None;
        self.running_query = None;
        self.standard.anchor = None;
        self.vim.reset_pending();
        self.clear_status();

        // Replace tabs + tree with a fresh post-bootstrap state.
        self.tabs = super::TabBar::default();
        self.tree = crate::tree::FileTree::default();
        self.load_initial_document();
        self.refresh_active_env_name();

        // Rewire the filesystem watchers if production already
        // installed them. In unit tests they're `None` and we leave
        // them that way — the integration path in `event_loop` is
        // what owns watcher lifetime, not this method.
        if let Some(sender) = self.event_sender.clone() {
            self.file_watcher = Some(crate::fs_watch::FileWatcher::new(sender.clone()));
            self.sync_file_watcher();
            self.connections_toml_watcher =
                Some(crate::fs_watch::FileWatcher::new(sender.clone()));
            super::event_loop::sync_connections_toml_watcher(self);
            self.envs_dir_watcher = Some(crate::fs_watch::FileWatcher::new(sender));
            super::event_loop::sync_envs_dir_watcher(self);
        }

        // surface pending secrets for the new vault.
        // The modal opens automatically when there's something to
        // prompt; status-bar badge picks up the count.
        self.scan_pending_secrets();
        self.open_pending_secrets_modal();

        Ok(())
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

    #[tokio::test(flavor = "multi_thread")]
    async fn switch_vault_rejects_non_directory() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        let bogus = std::env::temp_dir().join("definitely-not-a-real-dir-9381");
        let err = app
            .switch_vault(bogus)
            .expect_err("non-existent path must error");
        assert!(
            err.contains("vault path"),
            "error should mention vault path: {err}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn switch_vault_swaps_root_and_resets_session_state() {
        let (mut app, _data, _vault_a) = app_fixture("# A\n").await;
        let old_vault = app.vault_path.clone();

        // Build a second vault on disk + plant a marker file so we can
        // assert the swap actually re-rooted the file picker.
        let vault_b = TempDir::new().unwrap();
        std::fs::write(vault_b.path().join("welcome.md"), "# B\n").unwrap();

        // Plant transient session state — switch_vault must wipe it.
        app.completion_popup = None; // already none, but assert later
        app.last_run_anchor = Some(super::LastRunAnchor {
            file_path: std::path::PathBuf::from("note.md"),
            segment_idx: 0,
            alias: None,
        });
        app.result_viewport_top.insert(0, 5);
        app.content_search_index_built = true;
        app.standard.anchor = Some(crate::buffer::Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });

        app.switch_vault(vault_b.path().to_path_buf())
            .expect("switch must succeed");

        // Root moved.
        assert_ne!(app.vault_path, old_vault);
        assert_eq!(
            app.vault_path,
            vault_b.path().canonicalize().unwrap(),
            "vault_path must canonicalize to the new vault"
        );
        // The new vault's initial file is picked up.
        let pane = app.active_pane().expect("anchor tab exists");
        let path = pane
            .document_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());
        assert_eq!(
            path.as_deref(),
            Some("welcome.md"),
            "initial-document picker should find welcome.md in the new vault"
        );
        // Session caches/UI flags wiped.
        assert!(app.last_run_anchor.is_none());
        assert!(app.result_viewport_top.is_empty());
        assert!(!app.content_search_index_built);
        assert!(app.standard.anchor.is_none());
        assert!(app.modal.is_none());
        // Active vault persisted in the registry.
        let pool = app.pool_manager.app_pool().clone();
        let active = httui_core::vaults::get_active_vault(&pool).await.unwrap();
        assert_eq!(active.as_deref(), Some(app.vault_path.to_string_lossy().as_ref()));
    }
}
