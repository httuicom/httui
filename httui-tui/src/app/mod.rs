// size:exclude file — TUI app entrypoint, frozen scope per
// `feedback_notes_app_focus`. Sweep owner: (TUI parity).
// coverage:exclude file — same rationale (frozen scope; coverage
// gate not actionable until TUI parity wakes up). Audit-023.

use crossterm::event::KeyEvent;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;
use tracing::warn;

use std::sync::Arc;

use httui_core::db::connections::PoolManager;
use sqlx::SqlitePool;

use crate::buffer::layout::layout_document;
use crate::buffer::{Document, Segment};
use crate::config::Config;
use crate::document_loader;
use crate::event::AppEvent;
use crate::pane::{Pane, TabState};
use crate::tree::FileTree;
use crate::vault::ResolvedVault;
use crate::vim::{self, VimState};

// Mechanical split of the old monolithic `app.rs` (tui-v2 vertical 1,
// fase 2). Each submodule is a pure code move — no behavior change.
// The blanket `pub use` re-exports keep every `crate::app::*` call
// site resolving without edits.
mod event_loop;
mod helpers;
mod impl_file_crud;
mod impl_file_tab;
mod modal_state;
mod picker_state;
mod result_tab;
mod running;
mod status;
mod tabbar;
mod viewport;
pub use event_loop::run;
pub(crate) use helpers::*;
pub use modal_state::*;
pub use picker_state::*;
pub use result_tab::*;
pub use running::*;
pub use status::*;
pub use tabbar::*;
pub(crate) use viewport::{clamp_viewport, cursor_y};

/// Global application state.
pub struct App {
    pub config: Config,
    pub vault_path: PathBuf,
    pub vim: VimState,
    pub tree: FileTree,
    pub tabs: TabBar,
    pub status_message: Option<StatusMessage>,
    pub should_quit: bool,
    /// Shared connection-pool registry. Built once at startup, holds
    /// pools per `connection_id` so the DB executor doesn't reconnect
    /// on every run.
    pub pool_manager: Arc<PoolManager>,
    /// `connection_id → human-readable name` lookup, populated at
    /// startup. The renderer uses this to show `connection: prod-db`
    /// in DB block footers instead of a raw UUID. Refreshed by
    /// `App::refresh_connection_names`.
    pub connection_names: std::collections::HashMap<String, String>,
    /// `Some` while the row-detail modal is open. Mode flips to
    /// `Mode::DbRowDetail` in lockstep so the dispatcher routes keys
    /// to the modal's parser.
    pub db_row_detail: Option<DbRowDetailState>,
    /// `Some` while the HTTP response-detail modal is open. Mode
    /// flips to `Mode::HttpResponseDetail` in lockstep. Same modal
    /// trick as `db_row_detail` — the modal's body lives in a
    /// sub-`Document` so motions/visual/yank work out of the box.
    pub http_response_detail: Option<HttpResponseDetailState>,
    /// Sender for the main loop's `AppEvent` channel — handed to
    /// spawned async tasks (currently the DB executor) so they can
    /// notify the loop when their work completes. Optional so unit
    /// tests can construct an `App` without an event loop; in
    /// production it's always populated by `App::wire_event_sender`
    /// before `main_loop` starts.
    pub event_sender: Option<UnboundedSender<AppEvent>>,
    /// Currently running async DB query, if any. Populated by
    /// `apply_run_block` / `load_more_db_block`; cleared by the
    /// main loop when the corresponding `DbBlockResult` arrives.
    /// Used by both the renderer (spinner) and the dispatcher
    /// (`Ctrl-C` to cancel).
    pub running_query: Option<RunningQuery>,
    /// Top row index of each DB block's result-table viewport,
    /// keyed by `segment_idx`. Persists across cursor moves so the
    /// scroll feels like an editor pane (cursor floats inside the
    /// window; window only slides when the cursor would scroll
    /// off-screen). Updated by the renderer in `ui::blocks`.
    pub result_viewport_top: std::collections::HashMap<usize, u16>,
    /// `Some` while the connection picker popup is open. Mode flips
    /// to `Mode::ConnectionPicker` so dispatch routes keys to the
    /// picker's parser. The popup renders independently of mode —
    /// any `Some` value paints it.
    pub connection_picker: Option<ConnectionPickerState>,
    /// In-memory introspection cache, fed by background tasks
    /// spawned from `ensure_schema_loaded`. Keyed by `connection_id`.
    /// The SQL completion engine (b) reads from here
    /// synchronously and falls back to "loading…" when the entry is
    /// absent. See `crate::schema` for the cache + dedup model.
    pub schema_cache: crate::schema::SchemaCache,
    /// `Some` while the SQL completion popup is open. Created by the
    /// dispatcher after a typing-relevant action lands in a DB block
    /// body; cleared on Accept/Dismiss or when the prefix becomes
    /// empty.
    pub completion_popup: Option<CompletionPopupState>,
    /// `Some` while the run-confirm modal is up. Set by
    /// `apply_run_block` when it detects an unscoped destructive
    /// query (UPDATE/DELETE without WHERE); the user answers `y`
    /// to run anyway or `n`/Esc/Ctrl-C to cancel.
    pub db_confirm_run: Option<DbConfirmRunState>,
    /// `Some` while the export-format picker is open. Mode flips to
    /// `Mode::DbExportPicker` so dispatch routes navigation/confirm
    /// keys to the picker. The popup renders independently of mode —
    /// any `Some` value paints it. See `commands::db::open_export_picker`.
    pub db_export_picker: Option<DbExportPickerState>,
    /// `Some` while the block-settings modal is open (`gs` chord).
    /// Mode flips to `Mode::DbSettings` so dispatch routes typing
    /// into the focused LineEdit. Renders independently of mode —
    /// any `Some` value paints it.
    pub db_settings: Option<DbSettingsState>,
    /// `Some` while the block-history modal is open (`gh` chord on
    /// an HTTP block). Mode flips to `Mode::BlockHistory`. The list
    /// is read once at open-time — re-running the underlying block
    /// while the modal is up doesn't refresh it.
    pub block_history: Option<BlockHistoryState>,
    /// Name of the currently-active environment, if any. Cached on
    /// startup (and after a future env switch) so the status bar can
    /// render the chip without an async hop on every redraw. `None`
    /// when no environment is set as active.
    pub active_env_name: Option<String>,
    /// `Some` while the content-search modal is open (`<C-f>`).
    /// Mode flips to `Mode::ContentSearch`. The query buffer + last
    /// FTS5 results live here; each keystroke re-queries.
    pub content_search: Option<ContentSearchState>,
    /// `Some` while the environment-picker modal is open (`gE`).
    /// Mode flips to `Mode::EnvironmentPicker` so dispatch routes
    /// navigation/confirm keys to the picker. Confirm calls
    /// `set_active_environment` + `refresh_active_env_name` so the
    /// status-bar chip updates in lockstep. Renders independently of
    /// mode — any `Some` value paints the popup.
    pub environment_picker: Option<EnvironmentPickerState>,
    /// `true` once the FTS5 search index has been (re)built this
    /// session. Set by `open_content_search` after the first lazy
    /// rebuild so subsequent opens skip the cost. Cleared by
    /// `:reindex` (when that lands) or a vault switch.
    pub content_search_index_built: bool,
    /// Selected tab in the DB result panel. Single global state —
    /// every block's result section uses the same selection. Cycled
    /// via `gt` / `gT` while the cursor is on a result row.
    pub db_result_tab: ResultPanelTab,
    /// `Some` while an inline fence-edit prompt is open (alias /
    /// limit / timeout). Mode flips to `Mode::FenceEdit` so dispatch
    /// routes typing into the prompt's `LineEdit`. Renders in the
    /// status bar like `TreePrompt` so the editor underneath stays
    /// visible. See `commands::db::open_fence_edit_*`.
    pub fence_edit: Option<FenceEditState>,
    /// Filesystem watcher for the active document. `None` until
    /// `wire_event_sender` runs (unit-test paths skip the watcher
    /// since there's no main loop to receive its events). Updated
    /// whenever the active document changes (load, tab switch,
    /// `:e <path>`).
    pub file_watcher: Option<crate::fs_watch::FileWatcher>,
    /// `true` while the keymap help modal is open (`g?`). Mode flips
    /// to `Mode::Help` so dispatch routes Esc/q to the closer. The
    /// modal is read-only and stateless — no scroll, no selection —
    /// so a flag is enough; a future iteration with a search field
    /// would graduate this to a struct.
    pub help_visible: bool,
    /// `Some` while the block-template picker is open (`gN`). Mode
    /// flips to `Mode::BlockTemplatePicker`. The template list is a
    /// static `&'static [BlockTemplate]` so the state only carries
    /// the selection cursor.
    pub block_template_picker: Option<BlockTemplatePickerState>,
    /// `Some` while the tab picker is open (`gb`). Mode flips to
    /// `Mode::TabPicker`. Snapshot of every tab's focused-leaf path and
    /// dirty flag is computed at open-time so the picker survives
    /// unrelated edits while it's up.
    pub tab_picker: Option<TabPickerState>,
    /// Coordinates of the most-recently run block, used by the
    /// `gr` chord to rerun without navigating back to the source.
    /// Recorded by `apply_run_block` (and the HTTP equivalent) at
    /// the moment the run is dispatched, so cancelled / failed runs
    /// still set the anchor — `gr` is "rerun whatever I just tried".
    /// `None` until the user has run at least one block this
    /// session.
    pub last_run_anchor: Option<LastRunAnchor>,
}

impl App {
    pub fn new(config: Config, resolved: ResolvedVault, app_pool: SqlitePool) -> Self {
        // Lookup pulls connection records from the vault's
        // `connections.toml`;
        // legacy SQLite-only lookup is no longer used.
        let conn_lookup = httui_core::vault_config::ConnectionsStore::new(resolved.vault.clone());
        let pool_manager = Arc::new(PoolManager::new_standalone(conn_lookup, app_pool));
        let connection_names = load_connection_names(pool_manager.app_pool());
        let mut app = Self {
            config,
            vault_path: resolved.vault,
            vim: VimState::new(),
            tree: FileTree::default(),
            tabs: TabBar::default(),
            status_message: None,
            should_quit: false,
            pool_manager,
            connection_names,
            db_row_detail: None,
            http_response_detail: None,
            event_sender: None,
            running_query: None,
            result_viewport_top: std::collections::HashMap::new(),
            connection_picker: None,
            schema_cache: crate::schema::SchemaCache::new(),
            completion_popup: None,
            db_confirm_run: None,
            db_export_picker: None,
            db_settings: None,
            block_history: None,
            active_env_name: None,
            content_search: None,
            content_search_index_built: false,
            db_result_tab: ResultPanelTab::default(),
            fence_edit: None,
            environment_picker: None,
            file_watcher: None,
            help_visible: false,
            block_template_picker: None,
            last_run_anchor: None,
            tab_picker: None,
        };
        app.load_initial_document();
        app.refresh_active_env_name();
        app
    }

    /// Kick off a background introspection of `connection_id` if one
    /// isn't already pending and the cache is empty. Cheap to call
    /// repeatedly — the dedup gate makes the second/third call a
    /// no-op. Result lands as `AppEvent::SchemaLoaded`.
    pub fn ensure_schema_loaded(&mut self, connection_id: &str) {
        if self.schema_cache.get(connection_id).is_some() {
            return;
        }
        if self.schema_cache.is_pending(connection_id) {
            return;
        }
        let Some(sender) = self.event_sender.clone() else {
            // No event loop wired yet (unit-test-only path). Skip
            // silently — the test that constructed the App didn't
            // need async cache resolution.
            return;
        };
        self.schema_cache.mark_pending(connection_id);
        let pool_mgr = self.pool_manager.clone();
        let app_pool = self.pool_manager.app_pool().clone();
        let conn_id = connection_id.to_string();
        tokio::spawn(async move {
            // SQLite cache (TTL 300s) is the fast path; introspection
            // hits the actual driver only on miss / expired entries.
            // Mirrors `useSchemaCacheStore.ensureLoaded` on desktop.
            let result =
                match httui_core::db::schema_cache::get_cached_schema(&app_pool, &conn_id, 300)
                    .await
                {
                    Ok(Some(entries)) if !entries.is_empty() => Ok(entries),
                    _ => {
                        httui_core::db::schema_cache::introspect_schema(
                            &pool_mgr, &app_pool, &conn_id,
                        )
                        .await
                    }
                };
            let _ = sender.send(crate::event::AppEvent::SchemaLoaded {
                connection_id: conn_id,
                result,
            });
        });
    }

    /// Fold a `SchemaLoaded` event into `schema_cache`. Called from
    /// the main loop. Errors surface in the status bar but don't
    /// poison the cache so a retry can succeed.
    pub fn on_schema_loaded(
        &mut self,
        connection_id: String,
        result: Result<Vec<httui_core::db::schema_cache::SchemaEntry>, String>,
    ) {
        self.schema_cache.clear_pending(&connection_id);
        match result {
            Ok(entries) => {
                let tables = crate::schema::group_entries(entries);
                self.schema_cache.store(&connection_id, tables);
            }
            Err(msg) => {
                self.set_status(
                    StatusKind::Error,
                    format!("schema introspection failed: {msg}"),
                );
            }
        }
    }

    /// Refresh the connection_id → name cache from SQLite. Call
    /// after creating / renaming / deleting a connection so block
    /// footers update without restarting the TUI.
    #[allow(dead_code)] // wired up by the upcoming connection picker.
    pub fn refresh_connection_names(&mut self) {
        self.connection_names = load_connection_names(self.pool_manager.app_pool());
    }

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

    /// Re-resolve the active environment's display name from the
    /// SQLite registry and stash it on `active_env_name`. Cheap to
    /// call (single async query under `block_in_place`) — invoke
    /// after a hypothetical env-switch so the status bar chip
    /// updates without a TUI restart. Today no UI mutates the
    /// active env, so this only runs at startup.
    pub fn refresh_active_env_name(&mut self) {
        let pool = self.pool_manager.app_pool().clone();
        self.active_env_name = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async move {
                let id = httui_core::db::environments::get_active_environment_id(&pool).await?;
                let envs = httui_core::db::environments::list_environments(&pool)
                    .await
                    .ok()?;
                envs.into_iter().find(|e| e.id == id).map(|e| e.name)
            })
        });
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

    fn load_initial_document(&mut self) {
        let Some(file) = document_loader::pick_initial_file(&self.vault_path) else {
            // No file → still create an empty tab so the tree has
            // somewhere to anchor focus when files appear later.
            self.tabs.tabs.push(TabState::new(Pane::empty()));
            self.tabs.active = 0;
            return;
        };
        match document_loader::load_document(&self.vault_path, &file) {
            Ok(doc) => {
                self.tabs.tabs.push(TabState::new(Pane::new(doc, file)));
                self.tabs.active = 0;
            }
            Err(e) => {
                warn!(?e, "failed to load initial document");
                self.tabs.tabs.push(TabState::new(Pane::empty()));
                self.tabs.active = 0;
            }
        }
    }

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

    // ----- file open / tab management ------------------------------------
    // `open_document` / `open_in_new_tab` / `next_tab` / `prev_tab` /
    // `goto_tab` / `close_tab` live in `app/impl_file_tab.rs` (tui-v2
    // vertical 1, fase 2 p2-file_tab) as a sibling `impl App {}` block.

    // ----- file CRUD (vault-relative) ------------------------------------
    // `create_document` / `create_folder` / `rename_path` /
    // `delete_path` live in `app/impl_file_crud.rs` (tui-v2 vertical 1,
    // fase 2 p2-file_crud) as a sibling `impl App {}` block.
}

// `run` + `main_loop` + `handle_file_changed_externally` live in
// `app/event_loop.rs` (tui-v2 vertical 1, fase 2 p1-event_loop).
// `run` is re-exported from this module so `crate::app::run` keeps
// resolving for `main.rs`. `handle_key` deliberately STAYS here — it
// is the P5 input-rewire seam; `event_loop::main_loop` reaches it via
// `super::handle_key`.
pub(crate) fn handle_key(app: &mut App, key: KeyEvent) {
    vim::dispatch(app, key);
}
