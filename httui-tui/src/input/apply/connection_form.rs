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
    use httui_core::vault_config::CreateConnectionInput;

    let Some(state) = form_state_ref(app) else {
        return;
    };

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

    if !state.port.as_str().trim().is_empty() && port.is_none() {
        set_form_error(app, "port must be a number (1-65535)");
        return;
    }

    let store = app.connections_store.clone();
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

    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.create(input))
    });

    match result {
        Ok(_) => {
            apply_close_connection_form(app);
            // Reopen the Connections page so the new entry is visible
            // immediately. `open_connections_page` reloads from the
            // store; cheap and idempotent.
            if let Err(msg) = crate::input::apply::pickers::open_connections_page(app) {
                app.set_status(StatusKind::Error, msg);
            }
            app.refresh_connection_names();
            app.set_status(StatusKind::Info, format!("created connection \"{name}\""));
        }
        Err(e) => set_form_error(app, &e),
    }
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

pub(crate) fn apply_connection_form(app: &mut App, action: Action) {
    match action {
        Action::OpenConnectionForm => apply_open_connection_form(app),
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
        _ => unreachable!("apply_connection_form: variante fora do grupo"),
    }
}
