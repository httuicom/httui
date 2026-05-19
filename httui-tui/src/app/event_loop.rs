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
