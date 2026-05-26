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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Config;
    use crate::modal::Modal;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn bare_app() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_no_event_sender_errors() {
        let (mut app, _d, _v) = bare_app().await;
        let err = open_content_search(&mut app).unwrap_err();
        assert!(err.contains("event sender"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_kicks_off_rebuild_when_index_not_built() {
        let (mut app, _d, _v) = bare_app().await;
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        app.event_sender = Some(tx);
        open_content_search(&mut app).unwrap();
        let Some(Modal::ContentSearch(state)) = app.modal.as_ref() else {
            panic!("expected ContentSearch modal");
        };
        assert!(state.building, "rebuild kicked off → building flag true");
        assert!(matches!(app.vim.mode, Mode::ContentSearch));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_skips_rebuild_when_index_already_built() {
        let (mut app, _d, _v) = bare_app().await;
        app.content_search_index_built = true;
        open_content_search(&mut app).unwrap();
        let Some(Modal::ContentSearch(state)) = app.modal.as_ref() else {
            panic!()
        };
        assert!(!state.building, "no rebuild → building flag false");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_index_built_success_flips_flag_and_clears_banner() {
        let (mut app, _d, _v) = bare_app().await;
        app.modal = Some(Modal::ContentSearch(ContentSearchState::new()));
        if let Some(Modal::ContentSearch(s)) = app.modal.as_mut() {
            s.building = true;
        }
        handle_index_built(&mut app, Ok(()));
        assert!(app.content_search_index_built);
        let Some(Modal::ContentSearch(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert!(!s.building);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn handle_index_built_failure_closes_modal_and_sets_status() {
        let (mut app, _d, _v) = bare_app().await;
        app.modal = Some(Modal::ContentSearch(ContentSearchState::new()));
        handle_index_built(&mut app, Err("disk full".into()));
        assert!(app.modal.is_none());
        let s = app.status_message.as_ref().expect("status");
        assert!(s.text.contains("disk"), "got {:?}", s.text);
        assert!(matches!(app.vim.mode, Mode::Normal));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_content_search_clears_modal_and_returns_to_normal() {
        let (mut app, _d, _v) = bare_app().await;
        app.modal = Some(Modal::ContentSearch(ContentSearchState::new()));
        close_content_search(&mut app);
        assert!(app.modal.is_none());
        assert!(matches!(app.vim.mode, Mode::Normal));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_content_search_noop_other_modal() {
        let (mut app, _d, _v) = bare_app().await;
        app.modal = Some(Modal::Help);
        close_content_search(&mut app);
        assert!(matches!(app.modal, Some(Modal::Help)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn move_content_search_cursor_no_modal_is_noop() {
        let (mut app, _d, _v) = bare_app().await;
        move_content_search_cursor(&mut app, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn move_content_search_cursor_with_results_navigates() {
        let (mut app, _d, _v) = bare_app().await;
        let mut state = ContentSearchState::new();
        state.results = vec![
            httui_core::search::ContentSearchResult {
                file_path: "a.md".into(),
                snippet: String::new(),
            },
            httui_core::search::ContentSearchResult {
                file_path: "b.md".into(),
                snippet: String::new(),
            },
        ];
        app.modal = Some(Modal::ContentSearch(state));
        move_content_search_cursor(&mut app, 1);
        let Some(Modal::ContentSearch(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.selected, 1);
        move_content_search_cursor(&mut app, -1);
        let Some(Modal::ContentSearch(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.selected, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn content_search_char_appends_and_requeries() {
        let (mut app, _d, _v) = bare_app().await;
        app.modal = Some(Modal::ContentSearch(ContentSearchState::new()));
        // Mark as already built so requery actually runs (results empty against empty index).
        if let Some(Modal::ContentSearch(s)) = app.modal.as_mut() {
            s.building = false;
        }
        content_search_char(&mut app, 'h');
        content_search_char(&mut app, 'i');
        let Some(Modal::ContentSearch(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.query.as_str(), "hi");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn content_search_backspace_deletes_and_requeries() {
        let (mut app, _d, _v) = bare_app().await;
        let mut state = ContentSearchState::new();
        state.query = crate::vim::lineedit::LineEdit::from_str("abc");
        app.modal = Some(Modal::ContentSearch(state));
        content_search_backspace(&mut app);
        let Some(Modal::ContentSearch(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.query.as_str(), "ab");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn content_search_delete_after_cursor_and_requeries() {
        let (mut app, _d, _v) = bare_app().await;
        let mut state = ContentSearchState::new();
        let mut le = crate::vim::lineedit::LineEdit::from_str("ab");
        le.move_home();
        state.query = le;
        app.modal = Some(Modal::ContentSearch(state));
        content_search_delete(&mut app);
        let Some(Modal::ContentSearch(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.query.as_str(), "b");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn requery_while_building_is_noop() {
        let (mut app, _d, _v) = bare_app().await;
        let mut state = ContentSearchState::new();
        state.building = true;
        state.query = crate::vim::lineedit::LineEdit::from_str("anything");
        app.modal = Some(Modal::ContentSearch(state));
        // Should NOT touch results because building=true short-circuits.
        requery(&mut app);
        let Some(Modal::ContentSearch(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert!(s.results.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_content_search_with_no_results_closes_modal() {
        let (mut app, _d, _v) = bare_app().await;
        app.modal = Some(Modal::ContentSearch(ContentSearchState::new()));
        confirm_content_search(&mut app);
        assert!(app.modal.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_content_search_with_chosen_file_attempts_open() {
        let (mut app, _d, _v) = bare_app().await;
        let mut state = ContentSearchState::new();
        state.results = vec![httui_core::search::ContentSearchResult {
            file_path: "/nonexistent/file.md".into(),
            snippet: String::new(),
        }];
        app.modal = Some(Modal::ContentSearch(state));
        confirm_content_search(&mut app);
        assert!(app.modal.is_none(), "modal must close");
        // Status set with either ok or err — open_in_new_tab path executed.
        assert!(app.status_message.is_some());
    }
}
