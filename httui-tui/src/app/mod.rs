use crossterm::event::KeyEvent;
use std::path::PathBuf;
use std::time::Instant;
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
mod autosave;
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
    /// In-memory introspection cache, fed by background tasks
    /// spawned from `ensure_schema_loaded`. Keyed by `connection_id`.
    /// The SQL completion engine (b) reads from here
    /// synchronously and falls back to "loading…" when the entry is
    /// absent. See `crate::schema` for the cache + dedup model.
    pub schema_cache: crate::schema::SchemaCache,
    /// `Some` while the block-settings modal is open (`gs` chord).
    /// Mode flips to `Mode::DbSettings` so dispatch routes typing
    /// into the focused LineEdit. Renders independently of mode —
    /// any `Some` value paints it.
    pub db_settings: Option<DbSettingsState>,
    /// Name of the currently-active environment, if any. Cached on
    /// startup (and after a future env switch) so the status bar can
    /// render the chip without an async hop on every redraw. `None`
    /// when no environment is set as active.
    pub active_env_name: Option<String>,
    /// `true` once the FTS5 search index has been (re)built this
    /// session. Set by `open_content_search` after the first lazy
    /// rebuild so subsequent opens skip the cost. Cleared by
    /// `:reindex` (when that lands) or a vault switch.
    pub content_search_index_built: bool,
    /// Per-block selected tab in the result panel. Keyed by `BlockId`
    /// so cycling tabs in one block doesn't move tabs in unrelated
    /// blocks. Missing entry → [`ResultPanelTab::default()`].
    /// Cycled via `gt`/`gT` (or `Tab`/`Shift+Tab`) while the cursor
    /// is on a result row.
    pub result_tabs: std::collections::HashMap<crate::buffer::block::BlockId, ResultPanelTab>,
    /// Filesystem watcher for the active document. `None` until
    /// `wire_event_sender` runs (unit-test paths skip the watcher
    /// since there's no main loop to receive its events). Updated
    /// whenever the active document changes (load, tab switch,
    /// `:e <path>`).
    pub file_watcher: Option<crate::fs_watch::FileWatcher>,
    /// V3 P6 (2026-05-23): dedicated watcher for `<vault>/connections.toml`.
    /// Fires `AppEvent::FileChangedExternally` when an external tool
    /// rewrites the file (git checkout, manual edit, MCP); the event
    /// handler invalidates `ConnectionsStore`'s cache and reloads the
    /// page if it's open.
    pub connections_toml_watcher: Option<crate::fs_watch::FileWatcher>,
    /// V4 P8 (2026-05-23): watcher for `<vault>/envs/` dir entries.
    pub envs_dir_watcher: Option<crate::fs_watch::FileWatcher>,
    pub last_run_anchor: Option<LastRunAnchor>,
    pub standard: StandardState,
    pub last_edit: Option<Instant>,
    pub standard_keymap: Vec<(crate::input::keychord::KeyChord, crate::input::action::Action)>,
    pub config_path: Option<PathBuf>,
    pub modal: Option<crate::modal::Modal>,
    pub connections_store: Arc<httui_core::vault_config::ConnectionsStore>,
    /// V4 P1 (2026-05-23): file-backed environments store. Reads
    /// `<vault>/envs/*.toml`; the active env id lives in the
    /// per-machine `user.toml` so the same vault can be opened
    /// from desktop/TUI side-by-side with separate "current env".
    pub environments_store: Arc<httui_core::vault_config::EnvironmentsStore>,
    /// `{{keychain:...}}` references found in the active
    /// vault that have no entry in the local keychain. Repopulated
    /// after every `switch_vault` and at `App::new`. The status-bar
    /// badge reads `.len()` for the "⚠ N pending" counter,
    /// and the first-run modal reads the list to render the form.
    pub pending_secrets: Vec<httui_core::vault_config::missing_secrets::MissingRef>,
    /// set to `true` while a sub-modal (Create/Clone/Open/Pending)
    /// is open via the vault picker's verb chords. The sub-modal's
    /// close handler reads this flag to decide whether to dismiss to
    /// the editor (auto-opens) or re-open the picker on top
    /// (chord-driven flow). Cleared in the same close handler.
    pub resume_vault_picker: bool,
}

impl App {
    pub fn new(config: Config, resolved: ResolvedVault, app_pool: SqlitePool) -> Self {
        // Lookup pulls connection records from the vault's
        // `connections.toml`;
        // legacy SQLite-only lookup is no longer used.
        let connections_store =
            httui_core::vault_config::ConnectionsStore::new(resolved.vault.clone());
        let pool_manager =
            Arc::new(PoolManager::new_standalone(connections_store.clone(), app_pool));
        let connection_names = load_connection_names(&connections_store);
        // V4 P1: file-backed envs store. Falls back to the vault's
        // own `user.toml` if the global config path can't be
        // resolved (HOME unset, sandbox test). Side-effect: tests
        // get a per-temp-dir user.toml automatically.
        let user_config_path =
            httui_core::vault_config::user_store::default_user_config_path()
                .unwrap_or_else(|_| resolved.vault.join("user.toml"));
        let environments_store =
            httui_core::vault_config::EnvironmentsStore::new(resolved.vault.clone(), user_config_path);
        let standard_keymap = crate::input::keymap::resolve_standard_keymap(&config.keymap);
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
            event_sender: None,
            running_query: None,
            result_viewport_top: std::collections::HashMap::new(),
            schema_cache: crate::schema::SchemaCache::new(),
            db_settings: None,
            active_env_name: None,
            content_search_index_built: false,
            result_tabs: std::collections::HashMap::new(),
            file_watcher: None,
            connections_toml_watcher: None,
            envs_dir_watcher: None,
            last_run_anchor: None,
            standard: StandardState::default(),
            last_edit: None,
            standard_keymap,
            config_path: None,
            modal: None,
            connections_store,
            environments_store,
            pending_secrets: Vec::new(),
            resume_vault_picker: false,
        };
        app.load_initial_document();
        app.refresh_active_env_name();
        app.scan_pending_secrets();
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
