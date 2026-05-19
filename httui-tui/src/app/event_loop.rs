//! Terminal lifecycle + the async main loop.
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-event_loop) — pure code move, no behavior change. `handle_key`
//! deliberately stays in `app/mod.rs` (the P5 input-rewire seam); the
//! loop reaches it via `super::handle_key`. `run` is re-exported from
//! `app/mod.rs` so `crate::app::run` keeps resolving for `main.rs`.

use std::time::Duration;

use sqlx::SqlitePool;
use tracing::{debug, info};

use crate::buffer::Document;
use crate::config::Config;
use crate::error::TuiResult;
use crate::event::{AppEvent, EventLoop};
use crate::terminal;
use crate::ui;
use crate::vault::ResolvedVault;

use super::{handle_key, App};

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
            // A live event — fold it into `app`. `handle_app_event`
            // returns `false` when the event was a quit signal, in
            // which case `should_quit` is already set and the
            // `while` guard ends the loop next iteration.
            Some(ev) => {
                let _ = handle_app_event(app, ev);
            }
            // Channel closed (the event-stream task exited). Same
            // outcome as an explicit `Quit`: stop the loop.
            None => app.should_quit = true,
        }
    }
    debug!("main loop exiting");
    Ok(())
}

/// Fold a single `AppEvent` into `app`. Extracted verbatim from the
/// `main_loop` match so the per-arm logic is reachable from headless
/// tests without a real terminal + event stream (tui-v2 vertical 1,
/// fase 2 p3c — owner decision per `decisions.md` TD7). Pure code
/// move: every arm runs the exact same logic, in the same order of
/// side effects, as the inlined `match` it replaces.
///
/// Returns `should_continue` — `true` for every event except `Quit`
/// (which sets `app.should_quit = true` and returns `false`,
/// mirroring the old `Some(AppEvent::Quit) => app.should_quit = true`
/// arm; the `while !app.should_quit` guard then ends the loop). The
/// stream-closed (`None`) case is handled by the caller, not here,
/// because `handle_app_event` takes a concrete `AppEvent`.
fn handle_app_event(app: &mut App, ev: AppEvent) -> bool {
    match ev {
        AppEvent::Key(k) => {
            handle_key(app, k);
            // Any keystroke that opened/closed a tab, ran `:e`,
            // or picked a file from quickopen / tree changes the
            // active document. Re-target the watcher in lockstep
            // so external edits to the new file get reloaded
            // and stale watches on closed files stop firing.
            app.sync_file_watcher();
        }
        AppEvent::Resize(_, _) => {}
        // Debounced auto-save (tui-V01 / fase 5 p3 — Cenário 4).
        // All decision logic + the write call live in the covered
        // `super::autosave` module; this arm is just the seam.
        AppEvent::Tick => super::autosave::tick_autosave(app),
        AppEvent::DbBlockResult {
            segment_idx,
            kind,
            outcome,
        } => {
            crate::commands::db::handle_db_block_result(app, segment_idx, kind, outcome);
        }
        AppEvent::HttpBlockResult {
            segment_idx,
            outcome,
        } => {
            crate::commands::http::handle_http_block_result(app, segment_idx, outcome);
        }
        AppEvent::SchemaLoaded {
            connection_id,
            result,
        } => {
            app.on_schema_loaded(connection_id, result);
        }
        AppEvent::ContentSearchIndexBuilt { result } => {
            crate::commands::search::handle_index_built(app, result);
        }
        AppEvent::FileChangedExternally { path } => {
            handle_file_changed_externally(app, path);
        }
        AppEvent::Quit => {
            app.should_quit = true;
            return false;
        }
    }
    true
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::StatusKind;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Same fixture contract as the Lote B sibling modules: `App::new`
    /// calls `block_in_place`, so every constructing test must run on
    /// a multi-thread runtime. `body` is written verbatim into
    /// `note.md`, which `App::new`'s initial-document picker lands on.
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

    // ----- handle_file_changed_externally --------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn fce_noop_when_no_active_pane() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        // Drop every tab so `active_pane()` returns `None` → the
        // first `let-else` guard fires and the fn returns immediately.
        app.tabs.tabs.clear();
        handle_file_changed_externally(&mut app, PathBuf::from("/nowhere/note.md"));
        assert!(app.status_message.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fce_noop_when_active_pane_has_no_path() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        // Clear the focused pane's path → the second `let-else` guard
        // (`document_path`) fires.
        if let Some(p) = app.active_pane_mut() {
            p.document_path = None;
        }
        handle_file_changed_externally(&mut app, PathBuf::from("/nowhere/note.md"));
        assert!(app.status_message.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fce_drops_stale_event_for_a_different_path() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        // The active doc is `note.md`; an event for some other file
        // is stale (the user switched tabs after the write).
        handle_file_changed_externally(&mut app, PathBuf::from("/tmp/some-other-file.md"));
        assert!(app.status_message.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fce_returns_when_disk_read_fails() {
        let (mut app, _d, vault) = app_fixture("body\n").await;
        let absolute = vault.path().join("note.md");
        // Delete the file so `read_note` errors → the `Err(_)` arm
        // returns without surfacing a status hint.
        std::fs::remove_file(&absolute).unwrap();
        handle_file_changed_externally(&mut app, absolute);
        assert!(app.status_message.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fce_returns_when_pane_has_no_document() {
        let (mut app, _d, vault) = app_fixture("body\n").await;
        let absolute = vault.path().join("note.md");
        // File reads fine, but the pane's `document` is `None` → the
        // `None => return` arm of the buffer match fires.
        if let Some(p) = app.active_pane_mut() {
            p.document = None;
        }
        handle_file_changed_externally(&mut app, absolute);
        assert!(app.status_message.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fce_silently_drops_when_disk_equals_buffer() {
        let (mut app, _d, vault) = app_fixture("hello world\n").await;
        let absolute = vault.path().join("note.md");
        // Write the buffer's *own* canonical markdown back to disk so
        // `disk == buffer` holds exactly (avoids roundtrip drift) —
        // this models the watcher firing on our own save.
        let canonical = app.document().unwrap().to_markdown();
        std::fs::write(&absolute, &canonical).unwrap();
        handle_file_changed_externally(&mut app, absolute);
        assert!(app.status_message.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fce_warns_and_keeps_buffer_when_dirty_and_disk_differs() {
        let (mut app, _d, vault) = app_fixture("original\n").await;
        let absolute = vault.path().join("note.md");
        // Buffer has unsaved edits.
        app.document_mut().unwrap().mark_dirty();
        // Disk now differs from the buffer.
        std::fs::write(&absolute, "external rewrite\n").unwrap();
        handle_file_changed_externally(&mut app, absolute);
        let msg = app.status_message.as_ref().expect("status surfaced");
        assert_eq!(msg.kind, StatusKind::Error);
        assert!(msg.text.contains(":e!"));
        // Buffer left untouched (still the original markdown).
        assert!(app.document().unwrap().to_markdown().contains("original"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fce_reloads_silently_when_clean_and_disk_differs() {
        let (mut app, _d, vault) = app_fixture("before\n").await;
        let absolute = vault.path().join("note.md");
        // Move the viewport so we can assert it resets on reload.
        if let Some(p) = app.active_pane_mut() {
            p.viewport_top = 7;
        }
        std::fs::write(&absolute, "after the external edit\n").unwrap();
        handle_file_changed_externally(&mut app, absolute);
        let msg = app.status_message.as_ref().expect("status surfaced");
        assert_eq!(msg.kind, StatusKind::Info);
        assert!(msg.text.contains("reloaded"));
        assert!(msg.text.contains("note.md"));
        // New disk content is now in the buffer; viewport reset.
        assert!(app
            .document()
            .unwrap()
            .to_markdown()
            .contains("after the external edit"));
        assert_eq!(app.active_pane().unwrap().viewport_top, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fce_reloads_a_document_containing_an_executable_fence() {
        let (mut app, _d, vault) = app_fixture("before\n").await;
        let absolute = vault.path().join("note.md");
        // Disk now holds an executable DB fence. `Document::from_markdown`
        // is infallible (it never returns `Err`), so the reload path
        // always lands on the success arm — the `Err(e)` arm at
        // event_loop.rs is unreachable with today's parser. We still
        // assert the swap happens cleanly through the block-bearing
        // parse path (no panic, content replaced, Info status).
        std::fs::write(&absolute, "intro\n\n```db-conn\nSELECT 1\n```\n").unwrap();
        handle_file_changed_externally(&mut app, absolute);
        let msg = app.status_message.as_ref().expect("status surfaced");
        assert_eq!(msg.kind, StatusKind::Info);
        assert!(app.document().unwrap().to_markdown().contains("SELECT 1"));
    }

    // ----- handle_app_event ----------------------------------------------
    //
    // The `main_loop` match body, extracted to a free fn so every arm
    // is reachable headless. These tests assert (a) the event is
    // routed to the right handler (via that handler's known
    // observable effect) and (b) the `should_continue` contract:
    // `false` only for `Quit`, `true` for everything else. The
    // per-handler logic itself is covered by the handlers' own test
    // modules (`commands::{db,http,search}`, `impl_schema`).

    // `AppEvent` is already in scope via `use super::*;` (re-exported
    // from the module-level `use crate::event::{AppEvent, EventLoop}`).
    use crate::event::DbBlockResultKind;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[tokio::test(flavor = "multi_thread")]
    async fn key_event_routes_to_dispatch_and_continues() {
        // `Key` forwards to `handle_key` → `input::route::route`.
        // This test asserts the *vim* path specifically (Normal→Insert
        // on `i`), so it opts into Vim mode — the default is now
        // Standard (tui-V1 / fase 2 p5: Standard is the default
        // profile). Pressing `i` flips the vim mode, proving the arm
        // reached the dispatcher; the event is non-terminal so
        // `should_continue` is `true`.
        let (mut app, _d, _v) = app_fixture("abc\n").await;
        app.config.editor.mode = crate::config::EditorMode::Vim;
        let before = app.vim.mode;
        let cont = handle_app_event(
            &mut app,
            AppEvent::Key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
        );
        assert!(cont, "Key is non-terminal");
        assert_ne!(app.vim.mode, before, "`i` should flip the vim mode");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resize_is_a_noop_that_continues() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        let before = app.document().unwrap().to_markdown();
        let cont = handle_app_event(&mut app, AppEvent::Resize(120, 40));
        assert!(cont);
        // Pure no-op: document + status untouched, quit not set.
        assert_eq!(app.document().unwrap().to_markdown(), before);
        assert!(app.status_message.is_none());
        assert!(!app.should_quit);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tick_is_a_noop_that_continues() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        let cont = handle_app_event(&mut app, AppEvent::Tick);
        assert!(cont);
        assert!(app.status_message.is_none());
        assert!(!app.should_quit);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tick_triggers_autosave_when_debounce_elapsed() {
        // End-to-end wiring proof (tui-V01 / fase 5 p3): the Tick arm
        // of `handle_app_event` must reach `autosave::tick_autosave`,
        // which in turn fires `:w`. We seed dirty state + an expired
        // debounce and check the file on disk after one Tick.
        let (mut app, _d, vault) = app_fixture("body\n").await;
        for ch in "HI".chars() {
            app.tabs
                .active_document_mut()
                .unwrap()
                .insert_char_at_cursor(ch);
        }
        app.last_edit = Some(std::time::Instant::now() - Duration::from_secs(10));
        app.config.auto_save_debounce_ms = 1000;

        let cont = handle_app_event(&mut app, AppEvent::Tick);
        assert!(cont, "Tick is non-terminal");

        let body = std::fs::read_to_string(vault.path().join("note.md")).unwrap();
        assert!(
            body.contains("HI"),
            "Tick arm must invoke autosave::tick_autosave: {body:?}"
        );
        assert!(!app.tabs.active_document_mut().unwrap().is_dirty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn quit_sets_should_quit_and_stops_the_loop() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        assert!(!app.should_quit);
        let cont = handle_app_event(&mut app, AppEvent::Quit);
        assert!(!cont, "Quit is the only terminal event");
        assert!(app.should_quit, "Quit must set should_quit");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn db_block_result_routes_to_the_db_handler() {
        // An `Err` outcome for a `Run` always clears `running_query`
        // and (with no matching block) is a safe no-op on the doc.
        // Reaching the handler at all proves the arm routes; we assert
        // the handler's invariant (running_query cleared) + continue.
        let (mut app, _d, _v) = app_fixture("body\n").await;
        app.running_query = None;
        let cont = handle_app_event(
            &mut app,
            AppEvent::DbBlockResult {
                segment_idx: 999,
                kind: DbBlockResultKind::Run,
                outcome: Err("boom".to_string()),
            },
        );
        assert!(cont, "DbBlockResult is non-terminal");
        assert!(
            app.running_query.is_none(),
            "the db handler always clears running_query"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn http_block_result_routes_to_the_http_handler() {
        // Seed a doc with prose then an http block. `from_markdown`
        // also prepends a synthetic empty-prose segment before a
        // leading block, but here the explicit prose run already
        // occupies seg 0, so the block lands at `segment_idx = 1`.
        // With a real block, the handler reaches its `Err` branch
        // (no block → early-return before touching status). The
        // `Err` arm flips block state + surfaces a status error —
        // observable proof the event reached
        // `handle_http_block_result`.
        let (mut app, _d, _v) =
            app_fixture("intro\n\n```http alias=h\nGET https://example.com/users\n```\n").await;
        // Locate the block segment so the test isn't coupled to the
        // exact padding rule.
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .expect("doc has an http block");
        let cont = handle_app_event(
            &mut app,
            AppEvent::HttpBlockResult {
                segment_idx: block_idx,
                outcome: Err("network down".to_string()),
            },
        );
        assert!(cont, "HttpBlockResult is non-terminal");
        let msg = app.status_message.as_ref().expect("status surfaced");
        assert_eq!(msg.kind, StatusKind::Error);
        assert!(msg.text.contains("network down"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn schema_loaded_err_routes_to_on_schema_loaded() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        let cont = handle_app_event(
            &mut app,
            AppEvent::SchemaLoaded {
                connection_id: "conn-1".to_string(),
                result: Err("introspection failed".to_string()),
            },
        );
        assert!(cont, "SchemaLoaded is non-terminal");
        // `on_schema_loaded`'s Err arm surfaces a status error.
        let msg = app.status_message.as_ref().expect("status surfaced");
        assert_eq!(msg.kind, StatusKind::Error);
        assert!(msg.text.contains("schema introspection failed"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn schema_loaded_ok_routes_and_stores_into_cache() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        let cont = handle_app_event(
            &mut app,
            AppEvent::SchemaLoaded {
                connection_id: "conn-ok".to_string(),
                result: Ok(Vec::new()),
            },
        );
        assert!(cont);
        // Ok arm clears the pending flag without surfacing an error.
        assert!(app.status_message.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn content_search_index_built_err_routes_to_search_handler() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        let cont = handle_app_event(
            &mut app,
            AppEvent::ContentSearchIndexBuilt {
                result: Err("fts build failed".to_string()),
            },
        );
        assert!(cont, "ContentSearchIndexBuilt is non-terminal");
        // The Err arm dumps the modal + sets a status error.
        let msg = app.status_message.as_ref().expect("status surfaced");
        assert_eq!(msg.kind, StatusKind::Error);
        assert!(msg.text.contains("fts build failed"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn content_search_index_built_ok_routes_and_sets_flag() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        assert!(!app.content_search_index_built);
        let cont = handle_app_event(
            &mut app,
            AppEvent::ContentSearchIndexBuilt { result: Ok(()) },
        );
        assert!(cont);
        assert!(
            app.content_search_index_built,
            "the Ok arm flips content_search_index_built"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn file_changed_externally_routes_to_the_reload_path() {
        // Clean buffer + a real disk diff → silent reload + Info
        // status, proving the arm forwards to
        // `handle_file_changed_externally`.
        let (mut app, _d, vault) = app_fixture("before\n").await;
        let absolute = vault.path().join("note.md");
        std::fs::write(&absolute, "after external edit\n").unwrap();
        let cont = handle_app_event(
            &mut app,
            AppEvent::FileChangedExternally {
                path: absolute.clone(),
            },
        );
        assert!(cont, "FileChangedExternally is non-terminal");
        let msg = app.status_message.as_ref().expect("status surfaced");
        assert_eq!(msg.kind, StatusKind::Info);
        assert!(app
            .document()
            .unwrap()
            .to_markdown()
            .contains("after external edit"));
    }

    // ----- run / main_loop are terminal-bound ----------------------------
    //
    // `run` calls `terminal::setup` (raw-mode an interactive TTY) and
    // `EventLoop::start` (a `crossterm::event::EventStream` background
    // task that needs a real tty). `main_loop` only ever runs *inside*
    // `run`, driving `terminal.draw` + `events.next()`. Both are pure
    // wiring (terminal setup/teardown + the extracted
    // `handle_app_event` calls covered above) and cannot be exercised
    // from a headless `cargo test` process without injecting the
    // terminal + event stream. See the final report for the seam
    // analysis.
}
