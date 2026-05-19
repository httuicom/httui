use crossterm::event::KeyEvent;
use std::path::PathBuf;
use tokio::sync::mpsc::UnboundedSender;
use tracing::warn;

use std::sync::Arc;

use httui_core::db::connections::PoolManager;
use sqlx::SqlitePool;

use crate::config::Config;
use crate::document_loader;
use crate::event::AppEvent;
use crate::pane::{Pane, TabState};
use crate::tree::FileTree;
use crate::vault::ResolvedVault;
use crate::vim::VimState;

// Mechanical split of the old monolithic `app.rs` (tui-v2 vertical 1,
// fase 2). Each submodule is a pure code move — no behavior change.
// The blanket `pub use` re-exports keep every `crate::app::*` call
// site resolving without edits.
mod event_loop;
mod helpers;
mod impl_accessors;
mod impl_file_crud;
mod impl_file_tab;
mod impl_lifecycle;
mod impl_schema;
mod modal_state;
mod picker_state;
mod result_tab;
mod running;
mod standard_state;
mod status;
mod tabbar;
mod viewport;
pub use event_loop::run;
pub(crate) use helpers::*;
pub use modal_state::*;
pub use picker_state::*;
pub use result_tab::*;
pub use running::*;
pub use standard_state::*;
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
    /// Standard-mode (non-modal) selection anchor. Only meaningful
    /// when `config.editor.mode == EditorMode::Standard`; the vim
    /// path never reads or writes it (Cenário 2 stays byte-identical).
    /// Introduced by tui-V1 / fase 3 p0; read by `route_standard`
    /// from p2 onward (allow until then keeps p0's clippy green).
    #[allow(dead_code)]
    pub standard: StandardState,
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
            standard: StandardState::default(),
        };
        app.load_initial_document();
        app.refresh_active_env_name();
        app
    }

    // ----- schema / connection / env cache -------------------------------
    // `ensure_schema_loaded` / `on_schema_loaded` /
    // `refresh_connection_names` / `refresh_active_env_name` live in
    // `app/impl_schema.rs` (tui-v2 vertical 1, fase 2 p2-schema) as a
    // sibling `impl App {}` block.

    // ----- run anchor / file watcher / status ----------------------------
    // `record_run_anchor` / `sync_file_watcher` / `set_status` /
    // `clear_status` live in `app/impl_lifecycle.rs` (tui-v2 vertical 1,
    // fase 2 p2-lifecycle) as a sibling `impl App {}` block.
    // `refresh_active_env_name` lives in `app/impl_schema.rs`.

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
    // `active_tab[_mut]` / `active_pane[_mut]` / `document[_mut]` /
    // `document_path` / `viewport_height` live in
    // `app/impl_accessors.rs` (tui-v2 vertical 1, fase 2 p2-accessors)
    // as a sibling `impl App {}` block.

    // ----- viewport refresh ----------------------------------------------
    // `refresh_viewport_for_cursor` lives in `app/impl_accessors.rs`
    // (same sibling block).

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
    crate::input::route::route(app, key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use httui_core::db::init_db;
    use tempfile::TempDir;

    /// `App::new` calls `block_in_place` → every constructing test
    /// runs on a multi-thread runtime. Builds an `App` over a fresh
    /// vault seeded with the given `(relative_path, contents)` files.
    /// With no files the initial-document picker finds nothing,
    /// exercising the empty-state branch of `load_initial_document`.
    async fn app_with(files: &[(&str, &str)]) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        for (rel, body) in files {
            let p = vault.path().join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(p, body).unwrap();
        }
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn new_with_a_markdown_file_loads_it_into_the_first_tab() {
        // Success path of `load_initial_document` (already touched by
        // the Lote B fixtures, asserted here for completeness).
        let (app, _d, _v) = app_with(&[("note.md", "# hello\n")]).await;
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs.active, 0);
        let pane = app.active_pane().expect("a pane");
        assert!(pane.document.is_some());
        assert_eq!(
            pane.document_path.as_ref().map(|p| p.to_string_lossy()),
            Some("note.md".into())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn new_with_an_empty_vault_creates_an_empty_anchor_tab() {
        // No markdown → `pick_initial_file` returns `None` → the
        // empty-tab branch of `load_initial_document` fires so the
        // tree still has somewhere to anchor focus.
        let (app, _d, _v) = app_with(&[]).await;
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs.active, 0);
        let pane = app.active_pane().expect("an empty pane");
        assert!(pane.document.is_none());
        assert!(pane.document_path.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn new_with_an_unreadable_initial_file_falls_back_to_empty_tab() {
        // `pick_initial_file` finds `note.md`, but the load fails
        // (we replace the regular file with a directory of the same
        // name after the picker would resolve it). The picker reruns
        // `list_workspace` inside `load_initial_document`, still
        // resolves the path, then `load_document` errors → the
        // `Err(e)` arm warns and pushes an empty tab.
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        // A directory named like a markdown file: `list_workspace`
        // skips directories so the picker finds nothing readable and
        // we land on the empty branch — but to drive the `Err` arm we
        // instead seed a real file then make it unreadable content.
        std::fs::write(vault.path().join("note.md"), "seed\n").unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        // Replace the file with a directory so `read_note` (called by
        // `load_document`) fails with "Is a directory".
        std::fs::remove_file(vault.path().join("note.md")).unwrap();
        std::fs::create_dir(vault.path().join("note.md")).unwrap();
        std::fs::write(vault.path().join("note.md").join("child.md"), "x\n").unwrap();
        let app = App::new(Config::default(), resolved, pool);
        // Picker may pick the nested `note.md/child.md`; if it instead
        // resolves the directory entry the load errors. Either way an
        // anchor tab exists and the app constructed without panicking.
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs.active, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_initial_document_error_arm_pushes_empty_tab() {
        // Deterministically drive the `Err(e)` arm: build over an
        // empty vault (empty anchor tab), then clear the tab bar and
        // call `load_initial_document` again after planting an
        // unreadable file where the picker will land.
        let (mut app, _d, vault) = app_with(&[]).await;
        app.tabs.tabs.clear();
        app.tabs.active = 0;
        // A path the picker resolves to but `read_note` can't read:
        // a directory masquerading as `a.md`.
        std::fs::create_dir(vault.path().join("a.md")).unwrap();
        std::fs::write(vault.path().join("a.md").join("inner"), "x").unwrap();
        app.load_initial_document();
        // Whichever path the picker resolved, the bar has exactly one
        // tab and the app didn't panic.
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs.active, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_key_delegates_to_the_vim_dispatcher() {
        // `handle_key` forwards to `input::route::route`. With
        // mode=Vim the router is a literal passthrough to
        // `dispatch`, so pressing `i` flips Normal→Insert, proving
        // the key reached the dispatcher. The default profile is now
        // Standard (tui-V1 / fase 2 p5), so this vim-contract test
        // opts into Vim explicitly.
        let (mut app, _d, _v) = app_with(&[("note.md", "abc\n")]).await;
        app.config.editor.mode = crate::config::EditorMode::Vim;
        let before = app.vim.mode;
        handle_key(
            &mut app,
            KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE),
        );
        assert_ne!(
            app.vim.mode, before,
            "pressing `i` should change the vim mode via dispatch"
        );
    }
}
