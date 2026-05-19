// size:exclude file — TUI app entrypoint, frozen scope per
// `feedback_notes_app_focus`. Sweep owner: (TUI parity).
// coverage:exclude file — same rationale (frozen scope; coverage
// gate not actionable until TUI parity wakes up). Audit-023.

use crossterm::event::KeyEvent;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info, warn};

use std::sync::Arc;

use httui_core::db::connections::PoolManager;
use sqlx::SqlitePool;

use crate::buffer::layout::layout_document;
use crate::buffer::{Document, Segment};
use crate::config::Config;
use crate::document_loader;
use crate::error::TuiResult;
use crate::event::{AppEvent, EventLoop};
use crate::pane::{Pane, PaneNode, TabState};
use crate::terminal;
use crate::tree::FileTree;
use crate::ui;
use crate::vault::ResolvedVault;
use crate::vim::{self, VimState};

// Mechanical split of the old monolithic `app.rs` (tui-v2 vertical 1,
// fase 2). Each submodule is a pure code move — no behavior change.
// The blanket `pub use` re-exports keep every `crate::app::*` call
// site resolving without edits.
mod modal_state;
mod picker_state;
mod result_tab;
mod running;
mod status;
mod tabbar;
mod viewport;
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

    /// Replace the focused pane's document with the file at
    /// `relative_path`. If that file is the focused leaf of another
    /// tab, switches to that tab instead. Refuses to clobber a dirty
    /// buffer unless `force` is true.
    pub fn open_document(&mut self, relative_path: PathBuf, force: bool) -> Result<String, String> {
        if let Some(idx) = self.tabs.find_focused(&relative_path) {
            self.tabs.active = idx;
            return Ok(format!("\"{}\"", file_name(&relative_path)));
        }
        if !force
            && self
                .active_pane()
                .and_then(|p| p.document.as_ref())
                .is_some_and(|d| d.is_dirty())
        {
            return Err("no write since last change (add ! to override)".into());
        }
        let doc = document_loader::load_document(&self.vault_path, &relative_path)
            .map_err(|e| format!("E484: Can't open file: {e}"))?;
        let name = file_name(&relative_path);
        // No tab yet (e.g. last close left us empty)? Open as new tab.
        if self.tabs.is_empty() {
            self.tabs
                .tabs
                .push(TabState::new(Pane::new(doc, relative_path)));
            self.tabs.active = 0;
            return Ok(format!("\"{name}\""));
        }
        // Replace the focused pane's document in-place.
        if let Some(p) = self.active_pane_mut() {
            p.document = Some(doc);
            p.document_path = Some(relative_path);
            p.viewport_top = 0;
        }
        Ok(format!("\"{name}\""))
    }

    /// Open `relative_path` in a brand-new tab. If already focused in
    /// another tab, switches to it instead.
    pub fn open_in_new_tab(&mut self, relative_path: PathBuf) -> Result<String, String> {
        if let Some(idx) = self.tabs.find_focused(&relative_path) {
            self.tabs.active = idx;
            return Ok(format!("\"{}\"", file_name(&relative_path)));
        }
        let doc = document_loader::load_document(&self.vault_path, &relative_path)
            .map_err(|e| format!("E484: Can't open file: {e}"))?;
        let name = file_name(&relative_path);
        let new_tab = TabState::new(Pane::new(doc, relative_path));
        self.tabs.tabs.push(new_tab);
        self.tabs.active = self.tabs.tabs.len() - 1;
        Ok(format!("\"{name}\""))
    }

    pub fn next_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.active = (self.tabs.active + 1) % self.tabs.len();
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.active = if self.tabs.active == 0 {
            self.tabs.len() - 1
        } else {
            self.tabs.active - 1
        };
    }

    /// Switch to the 1-indexed tab number `n`. Out-of-range no-ops.
    pub fn goto_tab(&mut self, n: usize) {
        if n == 0 || n > self.tabs.len() {
            return;
        }
        self.tabs.active = n - 1;
    }

    /// Close the active tab (drops every pane inside it). With dirty
    /// content in any pane and `force == false`, refuses.
    pub fn close_tab(&mut self, force: bool) -> Result<String, String> {
        if self.tabs.is_empty() {
            return Err("no tab to close".into());
        }
        let active = self.tabs.active;
        if !force && tab_has_dirty(&self.tabs.tabs[active]) {
            return Err("no write since last change (add ! to override)".into());
        }
        let removed = self.tabs.tabs.remove(active);
        let removed_path = removed
            .active_leaf()
            .document_path
            .clone()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(no name)".into());
        if self.tabs.tabs.is_empty() {
            self.tabs.active = 0;
            return Ok(format!("closed \"{removed_path}\""));
        }
        if active >= self.tabs.tabs.len() {
            self.tabs.active = self.tabs.tabs.len() - 1;
        }
        Ok(format!("closed \"{removed_path}\""))
    }

    // ----- file CRUD (vault-relative) ------------------------------------

    pub fn create_document(
        &mut self,
        relative_path: PathBuf,
        force: bool,
    ) -> Result<String, String> {
        if !force
            && self
                .active_pane()
                .and_then(|p| p.document.as_ref())
                .is_some_and(|d| d.is_dirty())
        {
            return Err("no write since last change (add ! to override)".into());
        }
        let vault = self.vault_path.to_string_lossy().into_owned();
        let path_str = relative_path.to_string_lossy().into_owned();
        httui_core::fs::create_note(&vault, &path_str)
            .map_err(|e| format!("create failed: {e}"))?;
        self.open_document(relative_path, true)
    }

    pub fn create_folder(&mut self, relative_path: PathBuf) -> Result<String, String> {
        let abs = self.vault_path.join(&relative_path);
        if abs.exists() {
            return Err(format!(
                "create folder failed: path already exists: {}",
                relative_path.display()
            ));
        }
        std::fs::create_dir_all(&abs).map_err(|e| format!("create folder failed: {e}"))?;
        Ok(format!("created folder \"{}\"", file_name(&relative_path)))
    }

    /// Rename a vault-relative path. With `src == None` the focused
    /// pane's path is used. Updates every pane (across all tabs) that
    /// currently shows the renamed path.
    pub fn rename_path(&mut self, src: Option<PathBuf>, dst: PathBuf) -> Result<String, String> {
        let src_rel = match src {
            Some(p) => p,
            None => self
                .document_path()
                .cloned()
                .ok_or_else(|| "no file name".to_string())?,
        };
        let vault = self.vault_path.clone();
        let src_abs = vault.join(&src_rel);
        let dst_abs = vault.join(&dst);
        if dst_abs.exists() {
            return Err(format!(
                "E13: File exists (add ! to override): {}",
                dst.display()
            ));
        }
        if let Some(parent) = dst_abs.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("rename failed: {e}"))?;
        }
        std::fs::rename(&src_abs, &dst_abs).map_err(|e| format!("rename failed: {e}"))?;
        // Update every pane referencing the old path.
        for tab in self.tabs.tabs.iter_mut() {
            for_each_leaf_mut(&mut tab.root, &mut |pane| {
                if pane.document_path.as_deref() == Some(src_rel.as_path()) {
                    pane.document_path = Some(dst.clone());
                }
            });
        }

        // Move the search-index row to the new path. Cheapest
        // correct thing: drop the old key + reinsert the file's
        // current content under the new key. We re-read from disk
        // (the move just happened) so the indexed body matches the
        // file even if the user renamed without saving first. Best-
        // effort — index can always be rebuilt via the next `<C-f>`
        // open in the worst case.
        if self.content_search_index_built
            && src_rel.extension().and_then(|s| s.to_str()) == Some("md")
        {
            let pool = self.pool_manager.app_pool().clone();
            let src_key = src_rel.to_string_lossy().to_string();
            let dst_key = dst.to_string_lossy().to_string();
            let dst_abs_for_read = dst_abs.clone();
            tokio::spawn(async move {
                if let Err(e) = httui_core::search::remove_search_entry(&pool, &src_key).await {
                    tracing::warn!("search index rename (drop old) failed: {e}");
                }
                let body = std::fs::read_to_string(&dst_abs_for_read).unwrap_or_default();
                if let Err(e) =
                    httui_core::search::update_search_entry(&pool, &dst_key, &body).await
                {
                    tracing::warn!("search index rename (insert new) failed: {e}");
                }
            });
        }

        let name = file_name(&dst);
        Ok(format!("renamed to \"{name}\""))
    }

    /// Delete a path under the vault. Panes pointing at the deleted
    /// path are emptied (document/path → None); a tab containing only
    /// empty leaves is collapsed to a single empty leaf.
    pub fn delete_path(&mut self, target: Option<PathBuf>, force: bool) -> Result<String, String> {
        let target_rel = match target {
            Some(p) => p,
            None => self
                .document_path()
                .cloned()
                .ok_or_else(|| "no file name".to_string())?,
        };
        let opens_current = self.document_path() == Some(&target_rel);
        if opens_current && !force && self.document().is_some_and(|d| d.is_dirty()) {
            return Err("no write since last change (add ! to override)".into());
        }
        let vault = self.vault_path.clone();
        let abs = vault.join(&target_rel);
        let metadata = std::fs::metadata(&abs).map_err(|e| format!("delete failed: {e}"))?;
        let was_dir = metadata.is_dir();
        if was_dir {
            std::fs::remove_dir_all(&abs).map_err(|e| format!("delete failed: {e}"))?;
        } else {
            std::fs::remove_file(&abs).map_err(|e| format!("delete failed: {e}"))?;
        }
        // Empty out any pane whose path matched the deleted target.
        for tab in self.tabs.tabs.iter_mut() {
            for_each_leaf_mut(&mut tab.root, &mut |pane| {
                if pane.document_path.as_deref() == Some(target_rel.as_path()) {
                    pane.document = None;
                    pane.document_path = None;
                    pane.viewport_top = 0;
                }
            });
        }

        // Drop search-index rows pointing at the deleted file (or any
        // file under the deleted directory). Without this, `<C-f>`
        // would surface a result that opens to a missing file. Only
        // runs after the index has been built — pre-build the rows
        // don't exist anyway.
        if self.content_search_index_built {
            let pool = self.pool_manager.app_pool().clone();
            let target_str = target_rel.to_string_lossy().to_string();
            tokio::spawn(async move {
                if was_dir {
                    let prefix = format!("{}/%", target_str.trim_end_matches('/'));
                    if let Err(e) = sqlx::query(
                        "DELETE FROM search_index WHERE file_path = ? OR file_path LIKE ?",
                    )
                    .bind(&target_str)
                    .bind(&prefix)
                    .execute(&pool)
                    .await
                    {
                        tracing::warn!("search index purge (dir) failed: {e}");
                    }
                } else if let Err(e) =
                    httui_core::search::remove_search_entry(&pool, &target_str).await
                {
                    tracing::warn!("search index purge (file) failed: {e}");
                }
            });
        }

        Ok(format!("deleted \"{}\"", file_name(&target_rel)))
    }
}

fn tab_has_dirty(tab: &TabState) -> bool {
    let mut dirty = false;
    for_each_leaf(&tab.root, &mut |pane| {
        if pane.document.as_ref().is_some_and(|d| d.is_dirty()) {
            dirty = true;
        }
    });
    dirty
}

fn for_each_leaf(node: &PaneNode, f: &mut impl FnMut(&Pane)) {
    match node {
        PaneNode::Leaf(p) => f(p),
        PaneNode::Split { first, second, .. } => {
            for_each_leaf(first, f);
            for_each_leaf(second, f);
        }
    }
}

fn for_each_leaf_mut(node: &mut PaneNode, f: &mut impl FnMut(&mut Pane)) {
    match node {
        PaneNode::Leaf(p) => f(p),
        PaneNode::Split { first, second, .. } => {
            for_each_leaf_mut(first, f);
            for_each_leaf_mut(second, f);
        }
    }
}

/// Snapshot the connection table into a `id → name` map so renderers
/// can stay sync. Falls back to an empty map on any error — the worst
/// case is footers showing the raw `connection=…` value from the fence.
fn load_connection_names(pool: &SqlitePool) -> std::collections::HashMap<String, String> {
    use httui_core::db::connections::list_connections;
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(list_connections(pool))
    });
    result
        .ok()
        .map(|conns| conns.into_iter().map(|c| (c.id, c.name)).collect())
        .unwrap_or_default()
}

fn file_name(p: &std::path::Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

pub async fn run(config: Config, resolved: ResolvedVault, app_pool: SqlitePool) -> TuiResult<()> {
    info!(vault = %resolved.vault.display(), "starting notes-tui");

    terminal::install_panic_hook();
    let mut terminal = terminal::setup(config.mouse_enabled)?;
    let mut events = EventLoop::start(Duration::from_millis(250))?;
    let mut app = App::new(config, resolved, app_pool);
    // Spawned async tasks (currently the DB executor in
    // `vim::dispatch`) push their results back through this sender;
    // the main loop folds them into the app via `AppEvent` matches.
    let sender = events.sender();
    app.event_sender = Some(sender.clone());
    // Wire the filesystem watcher now that the event sender exists.
    // `sync_file_watcher` reads the active pane's path and starts
    // watching — covers the initial document loaded by `App::new`.
    app.file_watcher = Some(crate::fs_watch::FileWatcher::new(sender));
    app.sync_file_watcher();

    let result = main_loop(&mut terminal, &mut app, &mut events).await;

    let _ = terminal::teardown(&mut terminal);
    result
}

async fn main_loop(
    terminal: &mut terminal::Tui,
    app: &mut App,
    events: &mut EventLoop,
) -> TuiResult<()> {
    while !app.should_quit {
        terminal
            .draw(|f| {
                ui::render(f, app);
            })
            .map_err(|e| crate::error::TuiError::Terminal(format!("draw: {e}")))?;

        match events.next().await {
            Some(AppEvent::Key(k)) => {
                handle_key(app, k);
                // Any keystroke that opened/closed a tab, ran `:e`,
                // or picked a file from quickopen / tree changes the
                // active document. Re-target the watcher in lockstep
                // so external edits to the new file get reloaded
                // and stale watches on closed files stop firing.
                app.sync_file_watcher();
            }
            Some(AppEvent::Resize(_, _)) => {}
            Some(AppEvent::Tick) => {}
            Some(AppEvent::DbBlockResult {
                segment_idx,
                kind,
                outcome,
            }) => {
                crate::commands::db::handle_db_block_result(app, segment_idx, kind, outcome);
            }
            Some(AppEvent::HttpBlockResult {
                segment_idx,
                outcome,
            }) => {
                crate::commands::http::handle_http_block_result(app, segment_idx, outcome);
            }
            Some(AppEvent::SchemaLoaded {
                connection_id,
                result,
            }) => {
                app.on_schema_loaded(connection_id, result);
            }
            Some(AppEvent::ContentSearchIndexBuilt { result }) => {
                crate::commands::search::handle_index_built(app, result);
            }
            Some(AppEvent::FileChangedExternally { path }) => {
                handle_file_changed_externally(app, path);
            }
            Some(AppEvent::Quit) | None => app.should_quit = true,
        }
    }
    debug!("main loop exiting");
    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) {
    vim::dispatch(app, key);
}

/// Fold a `FileChangedExternally` event into the active pane.
/// Three outcomes:
///
/// 1. Disk content equals the buffer's serialized markdown — drop
///    silently. The watcher fires for our own writes too, and we
///    can't tell those apart at the OS level. Comparing content is
///    cheap and unambiguous.
/// 2. Disk differs and the buffer is clean — replace the document
///    in place. Cursor is reset to doc start (preserving the
///    previous cursor across an unrelated rewrite is brittle —
///    rewrites can shorten/restructure the doc).
/// 3. Disk differs and the buffer is dirty — leave the buffer
///    alone, surface a status warning. The user keeps their work;
///    a future iteration could open a conflict-resolution UI.
///
/// The event's `path` is checked against the active pane's path;
/// stale events (user switched tabs after the write) are dropped
/// rather than reloading the wrong file.
fn handle_file_changed_externally(app: &mut App, path: std::path::PathBuf) {
    let Some(pane) = app.active_pane() else {
        return;
    };
    let Some(rel) = pane.document_path.clone() else {
        return;
    };
    let absolute = app.vault_path.join(&rel);
    if absolute != path {
        // Stale event for a previously-watched file; ignore.
        return;
    }
    let disk = match httui_core::fs::read_note(
        &app.vault_path.to_string_lossy(),
        &rel.to_string_lossy(),
    ) {
        Ok(s) => s,
        Err(_) => {
            // Read failed (file deleted, permission flip). Don't
            // surface a status hint here — `notify` events fire on
            // the unlink that precedes a rename too, so noise is
            // expected.
            return;
        }
    };
    let buffer = match pane.document.as_ref() {
        Some(d) => d.to_markdown(),
        None => return,
    };
    if disk == buffer {
        return;
    }
    let dirty = pane.document.as_ref().is_some_and(|d| d.is_dirty());
    if dirty {
        app.set_status(
            crate::app::StatusKind::Error,
            "file changed on disk; buffer has unsaved edits — use `:e!` to discard and reload",
        );
        return;
    }
    // Clean buffer + disk diff → reload silently.
    let new_doc = match Document::from_markdown(&disk) {
        Ok(d) => d,
        Err(e) => {
            app.set_status(crate::app::StatusKind::Error, format!("reload failed: {e}"));
            return;
        }
    };
    if let Some(pane) = app.active_pane_mut() {
        pane.document = Some(new_doc);
        pane.viewport_top = 0;
    }
    app.set_status(
        crate::app::StatusKind::Info,
        format!("reloaded {} from disk", rel.display()),
    );
}
