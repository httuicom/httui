//! Content-search modal commands. Wires `<C-f>` → lazy FTS5 index
//! rebuild → per-keystroke `search_content` queries → file open on
//! confirm.
//!
//! V1 is sync end-to-end: `tokio::block_in_place` calls into the
//! async core APIs from the dispatch thread. Acceptable on small-
//! to-medium vaults (sub-millisecond FTS5 lookups). When this
//! starts to feel slow, debounce on a tokio task with a
//! `latest_query_id` guard so out-of-order results don't flicker.

use crate::app::{App, ContentSearchState, StatusKind};
use crate::vim::mode::Mode;

/// `<C-f>` from normal mode — open the modal. On the first open in
/// a session we kick off the FTS5 index rebuild *asynchronously*
/// (the modal renders an "indexing…" banner until the
/// `ContentSearchIndexBuilt` event lands; per-keystroke search is
/// gated on the build completing). Subsequent opens skip the
/// rebuild because `app.content_search_index_built` is already true.
pub fn open_content_search(app: &mut App) -> Result<(), String> {
    let mut state = ContentSearchState::new();

    if !app.content_search_index_built {
        let Some(sender) = app.event_sender.clone() else {
            return Err("internal: event sender missing".into());
        };
        let pool = app.pool_manager.app_pool().clone();
        let vault_path = app.vault_path.to_string_lossy().to_string();
        state.building = true;
        tokio::spawn(async move {
            let result = httui_core::search::rebuild_search_index(&pool, &vault_path)
                .await
                .map_err(|e| format!("index rebuild failed: {e}"));
            let _ = sender.send(crate::event::AppEvent::ContentSearchIndexBuilt { result });
        });
    }

    app.modal = Some(crate::modal::Modal::ContentSearch(state));
    app.vim.mode = Mode::ContentSearch;
    app.vim.reset_pending();
    Ok(())
}

/// Main-loop callback when the async rebuild finishes. Flips the
/// global flag, clears the modal's `building` banner, and runs the
/// current query (the user may have typed while the build was
/// running — those keystrokes accumulated in the buffer but
/// `requery` was a no-op). Failures dump the modal and show the
/// error in the status line.
pub fn handle_index_built(app: &mut App, result: Result<(), String>) {
    match result {
        Ok(()) => {
            app.content_search_index_built = true;
            if let Some(state) = app.content_search_mut() {
                state.building = false;
            }
            // Run the query against the now-populated index. If the
            // buffer is empty `requery` short-circuits to clearing
            // results.
            requery(app);
        }
        Err(msg) => {
            if matches!(app.modal, Some(crate::modal::Modal::ContentSearch(_))) {
        app.modal = None;
    }
            app.vim.enter_normal();
            app.set_status(StatusKind::Error, msg);
        }
    }
}

pub fn close_content_search(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::ContentSearch(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub fn move_content_search_cursor(app: &mut App, delta: i32) {
    let Some(state) = app.content_search_mut() else {
        return;
    };
    if delta >= 0 {
        state.select_next();
    } else {
        state.select_prev();
    }
}

/// Insert a character into the query buffer + re-run the search.
/// `requery` is split out so backspace/delete/cursor-edit paths can
/// share the same FTS5 round-trip without copy-paste.
pub fn content_search_char(app: &mut App, c: char) {
    if let Some(state) = app.content_search_mut() {
        state.query.insert_char(c);
    }
    requery(app);
}

pub fn content_search_backspace(app: &mut App) {
    if let Some(state) = app.content_search_mut() {
        state.query.delete_before();
    }
    requery(app);
}

pub fn content_search_delete(app: &mut App) {
    if let Some(state) = app.content_search_mut() {
        state.query.delete_after();
    }
    requery(app);
}

/// Run the FTS5 search synchronously against the current query.
/// Empty query → empty result list (matches the JS-side behavior).
/// Skips entirely while the index is still building — querying a
/// half-populated index would surface partial / wrong results.
fn requery(app: &mut App) {
    let Some(state) = app.content_search() else {
        return;
    };
    if state.building {
        return;
    }
    let query = state.query.as_str().to_string();
    let pool = app.pool_manager.app_pool().clone();
    let results = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(async move { httui_core::search::search_content(&pool, &query).await })
    });
    if let Some(state) = app.content_search_mut() {
        match results {
            Ok(list) => {
                state.results = list;
                state.selected = 0;
            }
            Err(_) => {
                // Common case: malformed FTS5 query mid-typing
                // (e.g. unmatched quote). Silently empty the
                // results — pestering the user with a status on
                // every keystroke is noise.
                state.results.clear();
                state.selected = 0;
            }
        }
    }
}

/// Enter — open the highlighted result's file in a new tab. Closes
/// the modal on success; on failure (file moved/deleted since the
/// index was built) keeps the modal open with a status hint so the
/// user can pick another result.
pub fn confirm_content_search(app: &mut App) {
    let chosen_path = app
        .content_search()
        .and_then(|s| s.chosen())
        .map(|r| r.file_path.clone());

    let Some(file_path) = chosen_path else {
        // Empty results / nothing selected — Esc-equivalent.
        close_content_search(app);
        return;
    };

    // Close the modal before delegating so the file opens cleanly.
    close_content_search(app);

    match app.open_in_new_tab(std::path::PathBuf::from(file_path)) {
        Ok(msg) => app.set_status(StatusKind::Info, msg),
        Err(msg) => app.set_status(StatusKind::Error, msg),
    }
}
