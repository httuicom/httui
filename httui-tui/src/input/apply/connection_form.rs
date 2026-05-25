//! V3 P3 (2026-05-23): action handlers for the create-connection
//! form modal. Opened by `n` on the Connections page; submits via
//! `httui_core::vault_config::ConnectionsStore::create`.

use crate::app::{App, ConnectionFormFocus, ConnectionFormState, StatusKind, DRIVER_OPTIONS};
use crate::input::action::Action;
use crate::vim::lineedit::LineEdit;
use crate::vim::mode::Mode;

/// Open the form. Always starts on `Name` with empty fields and the
/// default driver (postgres).
pub(crate) fn apply_open_connection_form(app: &mut App) {
    app.modal = Some(crate::modal::Modal::ConnectionForm(
        ConnectionFormState::new(),
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
}

/// V3 P4.2: open the form pre-filled with the highlighted entry on
/// the Connections page. No-op if the page isn't open or the list
/// is empty. Sets `editing=Some(name)` so `apply_form_submit` routes
/// to `store.update`.
pub(crate) fn apply_open_connection_edit_form(app: &mut App) {
    let detail = match app.modal.as_ref() {
        Some(crate::modal::Modal::Connections(page)) => page.connections.get(page.selected).cloned(),
        _ => None,
    };
    let Some(detail) = detail else {
        return;
    };
    app.modal = Some(crate::modal::Modal::ConnectionForm(
        ConnectionFormState::for_edit(&detail),
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
}

/// Close the form. If the Connections page is still in `app.modal`
/// underneath conceptually, V3 P3 doesn't stack modals — the form
/// fully replaces the page. Closing returns to Normal mode so the
/// user can re-open the page via `gC` (cheap; the snapshot reloads).
pub(crate) fn apply_close_connection_form(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::ConnectionForm(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub(crate) fn apply_form_focus_next(app: &mut App) {
    if let Some(state) = form_state_mut(app) {
        state.focus = state.focus.next();
    }
}

pub(crate) fn apply_form_focus_prev(app: &mut App) {
    if let Some(state) = form_state_mut(app) {
        state.focus = state.focus.prev();
    }
}

pub(crate) fn apply_form_char(app: &mut App, c: char) {
    let Some(state) = form_state_mut(app) else {
        return;
    };
    if let Some(edit) = focused_lineedit(state) {
        edit.insert_char(c);
    }
}

pub(crate) fn apply_form_backspace(app: &mut App) {
    let Some(state) = form_state_mut(app) else {
        return;
    };
    if let Some(edit) = focused_lineedit(state) {
        edit.delete_before();
    }
}

pub(crate) fn apply_form_delete(app: &mut App) {
    let Some(state) = form_state_mut(app) else {
        return;
    };
    if let Some(edit) = focused_lineedit(state) {
        edit.delete_after();
    }
}

pub(crate) fn apply_form_cursor_left(app: &mut App) {
    if let Some(edit) = form_state_mut(app).and_then(focused_lineedit) {
        edit.move_left();
    }
}

pub(crate) fn apply_form_cursor_right(app: &mut App) {
    if let Some(edit) = form_state_mut(app).and_then(focused_lineedit) {
        edit.move_right();
    }
}

pub(crate) fn apply_form_cursor_home(app: &mut App) {
    if let Some(edit) = form_state_mut(app).and_then(focused_lineedit) {
        edit.move_home();
    }
}

pub(crate) fn apply_form_cursor_end(app: &mut App) {
    if let Some(edit) = form_state_mut(app).and_then(focused_lineedit) {
        edit.move_end();
    }
}

pub(crate) fn apply_form_cycle_driver(app: &mut App, delta: i32) {
    let Some(state) = form_state_mut(app) else {
        return;
    };
    let len = DRIVER_OPTIONS.len() as i32;
    let next = (state.driver_idx as i32 + delta).rem_euclid(len);
    state.driver_idx = next as usize;
}

pub(crate) fn apply_form_toggle_readonly(app: &mut App) {
    if let Some(state) = form_state_mut(app) {
        state.is_readonly = !state.is_readonly;
    }
}

/// Validate + call `store.create`. On success: reload the
/// Connections page list (if it was the prior screen) and close
/// the form. On failure: stash the message in `state.error` so the
/// render loop paints it.
pub(crate) fn apply_form_submit(app: &mut App) {
    use httui_core::vault_config::{CreateConnectionInput, UpdateConnectionInput};

    let Some(state) = form_state_ref(app) else {
        return;
    };

    if state.is_session_override {
        return submit_session_override(app);
    }

    let name = state.name.as_str().trim().to_string();
    if name.is_empty() {
        set_form_error(app, "name is required");
        return;
    }
    let driver = DRIVER_OPTIONS
        .get(state.driver_idx)
        .copied()
        .unwrap_or("postgres")
        .to_string();
    let host = trimmed_opt(&state.host);
    let port = parse_port_opt(state.port.as_str().trim());
    let database_name = trimmed_opt(&state.database_name);
    let username = trimmed_opt(&state.username);
    let password = trimmed_opt(&state.password);
    let description = trimmed_opt(&state.description);
    let is_readonly = Some(state.is_readonly);
    let editing = state.editing.clone();

    if !state.port.as_str().trim().is_empty() && port.is_none() {
        set_form_error(app, "port must be a number (1-65535)");
        return;
    }

    let store = app.connections_store.clone();

    let (result, verb): (Result<(), String>, &'static str) = if let Some(original_name) = editing {
        // V3 P4.2: edit mode. `UpdateConnectionInput` semantics:
        //   password: None → keep existing keychain entry
        //                    (we intentionally never pre-fill the
        //                    field, so an unchanged form keeps the
        //                    secret intact).
        //   password: Some(non-empty) → rewrite keychain entry.
        // Renaming (name field changed) is out of scope here — the
        // store has no rename op; would need delete+create. For V1
        // we just write under the original name and ignore the new
        // value, surfacing a status hint.
        if original_name != name {
            app.set_status(
                StatusKind::Info,
                format!(
                    "rename ({original_name} → {name}) not supported in v1; \
                     edited under original name"
                ),
            );
        }
        let input = UpdateConnectionInput {
            driver: Some(driver),
            host,
            port,
            database_name,
            username,
            password,
            ssl_mode: None,
            is_readonly,
            description,
        };
        let r = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(store.update(&original_name, input))
        })
        .map(|_| ());
        (r, "updated")
    } else {
        let input = CreateConnectionInput {
            name: name.clone(),
            driver,
            host,
            port,
            database_name,
            username,
            password,
            ssl_mode: None,
            is_readonly,
            description,
        };
        let r = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(store.create(input))
        })
        .map(|_| ());
        (r, "created")
    };

    match result {
        Ok(()) => {
            apply_close_connection_form(app);
            if let Err(msg) = crate::input::apply::pickers::open_connections_page(app) {
                app.set_status(StatusKind::Error, msg);
            }
            app.refresh_connection_names();
            app.set_status(StatusKind::Info, format!("{verb} connection \"{name}\""));
        }
        Err(e) => set_form_error(app, &e),
    }
}

/// Empty host AND port clears the entry.
fn submit_session_override(app: &mut App) {
    let Some(state) = form_state_ref(app) else {
        return;
    };
    let name = state
        .editing
        .clone()
        .unwrap_or_else(|| state.name.as_str().trim().to_string());
    if name.is_empty() {
        set_form_error(app, "no connection target for override");
        return;
    }
    let host = trimmed_opt(&state.host);
    let port_raw = state.port.as_str().trim().to_string();
    let port = parse_port_opt(&port_raw);
    if !port_raw.is_empty() && port.is_none() {
        set_form_error(app, "port must be a number (1-65535)");
        return;
    }
    let ov = crate::session_overrides::ConnectionOverride { host, port };
    let status_msg = if ov.is_empty() {
        app.session_overrides.clear(&name);
        format!("session override cleared · {name}")
    } else {
        let desc = format!(
            "{}{}",
            ov.host.as_deref().unwrap_or(""),
            ov.port.map(|p| format!(":{p}")).unwrap_or_default()
        );
        app.session_overrides.set(&name, ov);
        format!("session override · {name} → {desc}")
    };
    apply_close_connection_form(app);
    if let Err(msg) = crate::input::apply::pickers::open_connections_page(app) {
        app.set_status(StatusKind::Error, msg);
        return;
    }
    app.set_status(StatusKind::Info, status_msg);
}

// ---------- helpers ------------------------------------------------------

fn form_state_mut(app: &mut App) -> Option<&mut ConnectionFormState> {
    if let Some(crate::modal::Modal::ConnectionForm(state)) = app.modal.as_mut() {
        Some(state)
    } else {
        None
    }
}

fn form_state_ref(app: &App) -> Option<&ConnectionFormState> {
    if let Some(crate::modal::Modal::ConnectionForm(state)) = app.modal.as_ref() {
        Some(state)
    } else {
        None
    }
}

fn focused_lineedit(state: &mut ConnectionFormState) -> Option<&mut LineEdit> {
    match state.focus {
        ConnectionFormFocus::Name => Some(&mut state.name),
        ConnectionFormFocus::Host => Some(&mut state.host),
        ConnectionFormFocus::Port => Some(&mut state.port),
        ConnectionFormFocus::Database => Some(&mut state.database_name),
        ConnectionFormFocus::Username => Some(&mut state.username),
        ConnectionFormFocus::Password => Some(&mut state.password),
        ConnectionFormFocus::Description => Some(&mut state.description),
        ConnectionFormFocus::Driver | ConnectionFormFocus::Readonly => None,
    }
}

fn trimmed_opt(edit: &LineEdit) -> Option<String> {
    let t = edit.as_str().trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

fn parse_port_opt(s: &str) -> Option<u16> {
    if s.is_empty() {
        None
    } else {
        s.parse::<u16>().ok()
    }
}

fn set_form_error(app: &mut App, msg: &str) {
    if let Some(state) = form_state_mut(app) {
        state.error = Some(msg.to_string());
    }
}

// ---------- V3 P4: delete confirm ---------------------------------------

/// Open the delete-confirm modal for the highlighted entry on the
/// Connections page. No-op when the page isn't open or the list
/// is empty. Snapshots the name so the page state is free to be
/// rebuilt afterwards.
pub(crate) fn apply_open_connection_delete_confirm(app: &mut App) {
    let name = match app.modal.as_ref() {
        Some(crate::modal::Modal::Connections(page)) => {
            page.connections.get(page.selected).map(|c| c.name.clone())
        }
        _ => None,
    };
    let Some(name) = name else {
        return;
    };
    app.modal = Some(crate::modal::Modal::ConnectionDeleteConfirm(
        crate::app::ConnectionDeleteConfirmState { name },
    ));
    // Mode stays `Modal` (page was already in Modal); keep `vim`
    // untouched so the confirm sits on top of the page conceptually.
}

/// y/Enter — call `store.delete` and reload the Connections page.
pub(crate) fn apply_confirm_connection_delete(app: &mut App) {
    let name = match app.modal.as_ref() {
        Some(crate::modal::Modal::ConnectionDeleteConfirm(state)) => state.name.clone(),
        _ => return,
    };
    let store = app.connections_store.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.delete(&name))
    });
    match result {
        Ok(()) => {
            // Reopen the Connections page (refreshed list).
            if let Err(msg) = crate::input::apply::pickers::open_connections_page(app) {
                // List empty after delete → page open path returns
                // error; fall back to closing modal entirely.
                app.modal = None;
                app.vim.enter_normal();
                app.set_status(StatusKind::Info, format!("deleted \"{name}\" — {msg}"));
                app.refresh_connection_names();
                return;
            }
            app.refresh_connection_names();
            app.set_status(StatusKind::Info, format!("deleted \"{name}\""));
        }
        Err(e) => {
            // Reopen page so the user isn't stuck in the dead confirm
            // modal, and surface the error.
            let _ = crate::input::apply::pickers::open_connections_page(app);
            app.set_status(StatusKind::Error, format!("delete failed: {e}"));
        }
    }
}

/// V3 P4.3: `t` on the Connections page — open a pool for the
/// highlighted entry and run the dialect's `test` query. Surface
/// ok+latency or err on the status bar. No-op without an open page.
pub(crate) fn apply_test_selected_connection(app: &mut App) {
    let name = match app.modal.as_ref() {
        Some(crate::modal::Modal::Connections(page)) => {
            page.connections.get(page.selected).map(|c| c.name.clone())
        }
        _ => None,
    };
    let Some(name) = name else {
        return;
    };
    app.set_status(StatusKind::Info, format!("testing {name}…"));
    let pool_mgr = app.pool_manager.clone();
    let started = std::time::Instant::now();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(pool_mgr.test_connection(&name))
    });
    let elapsed_ms = started.elapsed().as_millis();
    match result {
        Ok(()) => app.set_status(
            StatusKind::Info,
            format!("{name} · ok ({elapsed_ms}ms)"),
        ),
        Err(e) => app.set_status(StatusKind::Error, format!("{name} · {e}")),
    }
}

/// n/Esc — close confirm and reopen the page unchanged.
pub(crate) fn apply_cancel_connection_delete(app: &mut App) {
    if !matches!(
        app.modal,
        Some(crate::modal::Modal::ConnectionDeleteConfirm(_))
    ) {
        return;
    }
    // Reopen the page (cheap reload). If reload fails, fall back to
    // closing the modal so the user isn't trapped.
    if crate::input::apply::pickers::open_connections_page(app).is_err() {
        app.modal = None;
        app.vim.enter_normal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_fixture() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    fn open_form(app: &mut App) {
        apply_open_connection_form(app);
    }

    fn current_state(app: &App) -> &ConnectionFormState {
        form_state_ref(app).expect("form should be open")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_seeds_default_state() {
        let (mut app, _d, _v) = app_fixture().await;
        apply_open_connection_form(&mut app);
        let state = current_state(&app);
        assert_eq!(state.focus, ConnectionFormFocus::Name);
        assert_eq!(state.driver_idx, 0);
        assert!(state.name.as_str().is_empty());
        assert!(!state.is_readonly);
        assert_eq!(app.vim.mode, Mode::Modal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_clears_modal_and_returns_normal() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        apply_close_connection_form(&mut app);
        assert!(form_state_ref(&app).is_none());
        assert_eq!(app.vim.mode, Mode::Normal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_is_noop_when_other_modal_active() {
        let (mut app, _d, _v) = app_fixture().await;
        // Different modal open — close_form must not blow it away.
        app.modal = Some(crate::modal::Modal::Help);
        apply_close_connection_form(&mut app);
        assert!(matches!(app.modal, Some(crate::modal::Modal::Help)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn focus_next_cycles_through_fields() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        let order = ConnectionFormFocus::ORDER;
        for expected in order.iter().skip(1) {
            apply_form_focus_next(&mut app);
            assert_eq!(current_state(&app).focus, *expected);
        }
        // Wrap.
        apply_form_focus_next(&mut app);
        assert_eq!(current_state(&app).focus, order[0]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn focus_prev_wraps_backwards() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        apply_form_focus_prev(&mut app);
        assert_eq!(
            current_state(&app).focus,
            *ConnectionFormFocus::ORDER.last().unwrap()
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn char_inserts_into_focused_field() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        apply_form_char(&mut app, 'a');
        apply_form_char(&mut app, 'b');
        assert_eq!(current_state(&app).name.as_str(), "ab");
        // Tab over to host and type — name must stay put.
        apply_form_focus_next(&mut app); // Driver
        apply_form_focus_next(&mut app); // Host
        apply_form_char(&mut app, 'x');
        let state = current_state(&app);
        assert_eq!(state.name.as_str(), "ab");
        assert_eq!(state.host.as_str(), "x");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn char_on_driver_or_readonly_is_inert() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        // Move focus to Driver.
        apply_form_focus_next(&mut app);
        apply_form_char(&mut app, 'X');
        // Nothing typed anywhere.
        assert!(current_state(&app).name.as_str().is_empty());
        assert!(current_state(&app).host.as_str().is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn backspace_delete_cursor_ops_target_focused_input() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        apply_form_char(&mut app, 'a');
        apply_form_char(&mut app, 'b');
        apply_form_char(&mut app, 'c');
        apply_form_backspace(&mut app);
        assert_eq!(current_state(&app).name.as_str(), "ab");
        apply_form_cursor_home(&mut app);
        apply_form_delete(&mut app);
        assert_eq!(current_state(&app).name.as_str(), "b");
        apply_form_cursor_end(&mut app);
        apply_form_char(&mut app, 'z');
        assert_eq!(current_state(&app).name.as_str(), "bz");
        apply_form_cursor_left(&mut app);
        apply_form_char(&mut app, '.');
        assert_eq!(current_state(&app).name.as_str(), "b.z");
        apply_form_cursor_right(&mut app);
        apply_form_char(&mut app, '!');
        assert_eq!(current_state(&app).name.as_str(), "b.z!");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cycle_driver_advances_and_wraps() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        apply_form_cycle_driver(&mut app, 1);
        assert_eq!(current_state(&app).driver_idx, 1);
        apply_form_cycle_driver(&mut app, 1);
        assert_eq!(current_state(&app).driver_idx, 2);
        apply_form_cycle_driver(&mut app, 1);
        assert_eq!(current_state(&app).driver_idx, 0); // wrap
        apply_form_cycle_driver(&mut app, -1);
        assert_eq!(current_state(&app).driver_idx, 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_readonly_flips() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        assert!(!current_state(&app).is_readonly);
        apply_form_toggle_readonly(&mut app);
        assert!(current_state(&app).is_readonly);
        apply_form_toggle_readonly(&mut app);
        assert!(!current_state(&app).is_readonly);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn submit_rejects_empty_name_with_error_inline() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        // driver=sqlite so the username-required postgres validation
        // doesn't fire — we want the name check to surface first.
        apply_form_cycle_driver(&mut app, 2);
        apply_form_submit(&mut app);
        let state = current_state(&app);
        assert_eq!(state.error.as_deref(), Some("name is required"));
        // Modal still open.
        assert!(form_state_ref(&app).is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn submit_rejects_non_numeric_port() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        // name=ok, driver=postgres (default), port=abc.
        for c in "ok".chars() {
            apply_form_char(&mut app, c);
        }
        // Navigate to Host then Port and type a non-number.
        apply_form_focus_next(&mut app); // Driver
        apply_form_focus_next(&mut app); // Host
        apply_form_focus_next(&mut app); // Port
        apply_form_char(&mut app, 'a');
        apply_form_submit(&mut app);
        let state = current_state(&app);
        assert!(state
            .error
            .as_deref()
            .unwrap_or("")
            .contains("port must be a number"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn submit_sqlite_success_closes_form_and_persists() {
        let (mut app, _d, vault) = app_fixture().await;
        open_form(&mut app);
        // Name = "local".
        for c in "local".chars() {
            apply_form_char(&mut app, c);
        }
        // Move to Driver and select sqlite (idx 2).
        apply_form_focus_next(&mut app);
        apply_form_cycle_driver(&mut app, 2);
        // Move to Database and type a path (required for sqlite).
        for _ in 0..3 {
            apply_form_focus_next(&mut app);
        } // Driver → Host → Port → Database
        for c in "/tmp/x.db".chars() {
            apply_form_char(&mut app, c);
        }
        apply_form_submit(&mut app);
        // Modal closed; Connections page re-opened with the entry.
        assert!(
            matches!(app.modal, Some(crate::modal::Modal::Connections(_))),
            "submit should reopen the page; modal was: {:?}",
            app.modal
        );
        // TOML was written.
        let toml_path = vault.path().join("connections.toml");
        let contents = std::fs::read_to_string(toml_path).unwrap();
        assert!(contents.contains("[connections.local]"));
        assert!(contents.contains("type = \"sqlite\""));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn submit_postgres_missing_required_field_surfaces_store_error() {
        let (mut app, _d, _v) = app_fixture().await;
        open_form(&mut app);
        // name=p1, driver=postgres (default), but no host/username/db.
        // The store's `require` helper rejects with "X is required for postgres".
        for c in "p1".chars() {
            apply_form_char(&mut app, c);
        }
        apply_form_submit(&mut app);
        let state = current_state(&app);
        let err = state.error.as_deref().unwrap_or("");
        assert!(
            err.contains("required for postgres"),
            "expected a required-field error, got: {err:?}"
        );
    }

    // ---------- V3 P4: delete confirm ----------

    async fn seed_one(app: &App, name: &str) {
        use httui_core::vault_config::CreateConnectionInput;
        app.connections_store
            .create(CreateConnectionInput {
                name: name.into(),
                driver: "sqlite".into(),
                host: None,
                port: None,
                database_name: Some("/tmp/x.db".into()),
                username: None,
                password: None,
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_delete_confirm_snapshots_highlighted_name() {
        let (mut app, _d, _v) = app_fixture().await;
        seed_one(&app, "to-go").await;
        crate::input::apply::pickers::open_connections_page(&mut app).unwrap();
        apply_open_connection_delete_confirm(&mut app);
        match &app.modal {
            Some(crate::modal::Modal::ConnectionDeleteConfirm(state)) => {
                assert_eq!(state.name, "to-go");
            }
            other => panic!("expected ConnectionDeleteConfirm, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_delete_confirm_is_noop_when_page_not_open() {
        let (mut app, _d, _v) = app_fixture().await;
        apply_open_connection_delete_confirm(&mut app);
        assert!(app.modal.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_delete_removes_entry_and_reopens_page() {
        let (mut app, _d, vault) = app_fixture().await;
        seed_one(&app, "kill-me").await;
        seed_one(&app, "keeper").await;
        crate::input::apply::pickers::open_connections_page(&mut app).unwrap();
        // Snapshot the highlighted name (TOML order is BTreeMap-
        // alphabetical, not insertion order), then delete it.
        let target = match &app.modal {
            Some(crate::modal::Modal::Connections(page)) => {
                page.connections[page.selected].name.clone()
            }
            _ => panic!("page should be open"),
        };
        apply_open_connection_delete_confirm(&mut app);
        apply_confirm_connection_delete(&mut app);
        // Modal is back to the (refreshed) Connections page; deleted
        // entry is gone, the other one remains.
        match &app.modal {
            Some(crate::modal::Modal::Connections(page)) => {
                let names: Vec<&str> =
                    page.connections.iter().map(|c| c.name.as_str()).collect();
                assert!(!names.contains(&target.as_str()), "{target} should be gone");
                assert_eq!(names.len(), 1);
            }
            other => panic!("expected Connections, got {other:?}"),
        }
        // TOML reflects the delete.
        let contents = std::fs::read_to_string(vault.path().join("connections.toml")).unwrap();
        assert!(!contents.contains(&format!("[connections.{target}]")));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_delete_last_entry_keeps_empty_page() {
        let (mut app, _d, _v) = app_fixture().await;
        seed_one(&app, "only-one").await;
        crate::input::apply::pickers::open_connections_page(&mut app).unwrap();
        apply_open_connection_delete_confirm(&mut app);
        apply_confirm_connection_delete(&mut app);
        // Page stays open but with an empty list — render path shows
        // the "press n to add" hint. Empty-list-closes-modal would
        // surprise the user mid-confirm; better to keep them on the
        // page so they can immediately create another one.
        match &app.modal {
            Some(crate::modal::Modal::Connections(page)) => {
                assert!(page.connections.is_empty());
            }
            other => panic!("expected empty Connections page, got {other:?}"),
        }
    }

    // ---------- V3 P4.2: edit form ----------

    #[tokio::test(flavor = "multi_thread")]
    async fn open_edit_form_prefills_state_with_selected_entry() {
        let (mut app, _d, _v) = app_fixture().await;
        seed_one(&app, "to-edit").await;
        crate::input::apply::pickers::open_connections_page(&mut app).unwrap();
        apply_open_connection_edit_form(&mut app);
        match &app.modal {
            Some(crate::modal::Modal::ConnectionForm(state)) => {
                assert_eq!(state.name.as_str(), "to-edit");
                assert_eq!(state.editing.as_deref(), Some("to-edit"));
                assert_eq!(state.driver_idx, 2); // sqlite
                assert_eq!(state.database_name.as_str(), "/tmp/x.db");
            }
            other => panic!("expected ConnectionForm, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_edit_form_noop_when_page_not_open() {
        let (mut app, _d, _v) = app_fixture().await;
        apply_open_connection_edit_form(&mut app);
        assert!(app.modal.is_none());
    }

    // ---------- V3 P4.3: test connection ----------

    #[tokio::test(flavor = "multi_thread")]
    async fn test_selected_connection_noop_when_page_not_open() {
        let (mut app, _d, _v) = app_fixture().await;
        apply_test_selected_connection(&mut app);
        // No modal, no panic — status may or may not change.
        assert!(app.modal.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_selected_connection_sqlite_succeeds_and_reports_latency() {
        let (mut app, _d, vault) = app_fixture().await;
        // Seed a sqlite connection pointing at a tempfile that does
        // exist; sqlite "ping" via SELECT 1 always succeeds.
        let db_path = vault.path().join("ping.db");
        // touch — sqlite create_pool opens with create_if_missing.
        std::fs::write(&db_path, b"").unwrap();
        use httui_core::vault_config::CreateConnectionInput;
        app.connections_store
            .create(CreateConnectionInput {
                name: "ping-me".into(),
                driver: "sqlite".into(),
                host: None,
                port: None,
                database_name: Some(db_path.display().to_string()),
                username: None,
                password: None,
                ssl_mode: None,
                is_readonly: None,
                description: None,
            })
            .await
            .unwrap();
        crate::input::apply::pickers::open_connections_page(&mut app).unwrap();
        apply_test_selected_connection(&mut app);
        let msg = app.status_message.as_ref().expect("status set");
        assert_eq!(msg.kind, StatusKind::Info);
        assert!(
            msg.text.contains("ping-me · ok"),
            "expected ok status, got: {:?}",
            msg.text
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn submit_in_edit_mode_calls_update_not_create() {
        let (mut app, _d, vault) = app_fixture().await;
        seed_one(&app, "edit-me").await;
        crate::input::apply::pickers::open_connections_page(&mut app).unwrap();
        apply_open_connection_edit_form(&mut app);
        // Navigate to Description and append a marker so we can prove
        // the row was overwritten with the new value.
        for _ in 0..8 {
            apply_form_focus_next(&mut app);
        } // Name → Driver → Host → Port → Database → Username → Password → Readonly → Description
        for c in "edited".chars() {
            apply_form_char(&mut app, c);
        }
        apply_form_submit(&mut app);
        // Modal back to page; entry updated.
        match &app.modal {
            Some(crate::modal::Modal::Connections(page)) => {
                let entry = &page.connections[0];
                assert_eq!(entry.name, "edit-me");
                assert_eq!(entry.description.as_deref(), Some("edited"));
            }
            other => panic!("expected Connections, got {other:?}"),
        }
        let contents = std::fs::read_to_string(vault.path().join("connections.toml")).unwrap();
        assert!(contents.contains("description = \"edited\""));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_delete_reopens_page_unchanged() {
        let (mut app, _d, vault) = app_fixture().await;
        seed_one(&app, "stay").await;
        crate::input::apply::pickers::open_connections_page(&mut app).unwrap();
        apply_open_connection_delete_confirm(&mut app);
        apply_cancel_connection_delete(&mut app);
        match &app.modal {
            Some(crate::modal::Modal::Connections(page)) => {
                assert_eq!(page.connections.len(), 1);
                assert_eq!(page.connections[0].name, "stay");
            }
            other => panic!("expected Connections, got {other:?}"),
        }
        // TOML untouched.
        let contents = std::fs::read_to_string(vault.path().join("connections.toml")).unwrap();
        assert!(contents.contains("[connections.stay]"));
    }

}

pub(crate) fn apply_connection_form(app: &mut App, action: Action) {
    match action {
        Action::OpenConnectionForm => apply_open_connection_form(app),
        Action::OpenConnectionEditForm => apply_open_connection_edit_form(app),
        Action::CloseConnectionForm => apply_close_connection_form(app),
        Action::ConnectionFormFocusNext => apply_form_focus_next(app),
        Action::ConnectionFormFocusPrev => apply_form_focus_prev(app),
        Action::ConnectionFormChar(c) => apply_form_char(app, c),
        Action::ConnectionFormBackspace => apply_form_backspace(app),
        Action::ConnectionFormDelete => apply_form_delete(app),
        Action::ConnectionFormCursorLeft => apply_form_cursor_left(app),
        Action::ConnectionFormCursorRight => apply_form_cursor_right(app),
        Action::ConnectionFormCursorHome => apply_form_cursor_home(app),
        Action::ConnectionFormCursorEnd => apply_form_cursor_end(app),
        Action::ConnectionFormCycleDriver(delta) => apply_form_cycle_driver(app, delta),
        Action::ConnectionFormToggleReadonly => apply_form_toggle_readonly(app),
        Action::ConnectionFormSubmit => apply_form_submit(app),
        Action::OpenConnectionDeleteConfirm => apply_open_connection_delete_confirm(app),
        Action::ConfirmConnectionDelete => apply_confirm_connection_delete(app),
        Action::CancelConnectionDelete => apply_cancel_connection_delete(app),
        Action::TestSelectedConnection => apply_test_selected_connection(app),
        _ => unreachable!("apply_connection_form: variante fora do grupo"),
    }
}
