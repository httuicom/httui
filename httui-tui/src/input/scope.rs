//! Input focus stack — chain-of-responsibility key routing.
//!
//! `active_scopes(&App)` derives the stack from app state; the walker
//! goes top → bottom and each `handle_<scope>` returns `Consumed`,
//! `Forward`, or `Effect(Action)`. Default is `Consumed`, so an open
//! scope never leaks keys it didn't bind to the editor below.
//! `Forward` is opt-in for overlays (e.g. the SQL completion popup)
//! that filter instead of capture.

use crossterm::event::KeyEvent;

use crate::app::App;
use crate::input::action::Action;

/// One layer of the input stack. Order in `active_scopes` encodes
/// priority — the further down the `Vec`, the more recently it was
/// opened (and the closer to the user's focus). Stack is walked
/// top → bottom (`iter().rev()`) on dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Bottom of the stack. Vim or Standard engine handles the key
    /// based on `app.config.editor.mode`. Always present.
    Editor,
    /// `app.completion_popup.is_some()` — SQL completion overlay.
    /// Filters nav/accept/dismiss keys; everything else Forwards to
    /// the editor (the user types into the doc and the popup re-
    /// computes against the new prefix).
    CompletionPopup,
    /// `app.fence_edit.is_some()` — inline fence-edit prompt
    /// (alias/limit/timeout). Always consumes (`parse_fence_edit`
    /// is total).
    FenceEdit,
    /// `app.db_settings.is_some()` — DB block settings popup
    /// (limit/timeout). Always consumes.
    DbSettings,
    /// `app.content_search.is_some()` — full-text search panel.
    /// Always consumes.
    ContentSearch,
    /// `app.db_row_detail.is_some()` — DB row-detail modal.
    /// Always consumes.
    DbRowDetail,
    /// `app.http_response_detail.is_some()` — HTTP response-detail
    /// modal. Always consumes.
    HttpResponseDetail,
    /// `app.modal.is_some()` — every modal variant (forms, pickers,
    /// confirms). Consumes every key by default; unbound keys never
    /// leak to the editor.
    Modal,
    /// `app.running_query.is_some()` — catches the profile-specific
    /// query-cancel key (`Ctrl+C` in vim, `Esc` in standard). Sits
    /// at the top so cancel works from anywhere, including inside
    /// a modal.
    RunningQueryCatch,
}

#[derive(Debug)]
pub enum KeyOutcome {
    /// Key absorbed by this scope. No further routing.
    Consumed,
    /// Scope is open but doesn't bind this key — pass to next scope
    /// below. Used by overlays (e.g. SQL completion popup) that
    /// filter instead of capture. Reserved for future scopes; no
    /// scope returns this yet.
    #[allow(dead_code)]
    Forward,
    /// Apply this action then consume. Lets scopes describe what
    /// happens without coupling to the apply machinery.
    Effect(Action),
}

/// Derive the active input stack from `app` state. Order = priority
/// (top of stack = end of `Vec`). Editor is always at index 0; further
/// elements are pushed in the order they opened.
pub fn active_scopes(app: &App) -> Vec<ScopeKind> {
    let mut v = vec![ScopeKind::Editor];
    // Order below = stacking order (bottom → top). Top wins.
    if app.completion_popup.is_some() {
        v.push(ScopeKind::CompletionPopup);
    }
    if app.fence_edit.is_some() {
        v.push(ScopeKind::FenceEdit);
    }
    if app.db_settings.is_some() {
        v.push(ScopeKind::DbSettings);
    }
    if app.content_search.is_some() {
        v.push(ScopeKind::ContentSearch);
    }
    if app.db_row_detail.is_some() {
        v.push(ScopeKind::DbRowDetail);
    }
    if app.http_response_detail.is_some() {
        v.push(ScopeKind::HttpResponseDetail);
    }
    if app.modal.is_some() {
        v.push(ScopeKind::Modal);
    }
    // Running-query cancel sits at the very top so `Ctrl+C` / `Esc`
    // reach it before any modal / popup that might also bind those.
    if app.running_query.is_some() {
        v.push(ScopeKind::RunningQueryCatch);
    }
    v
}

/// Walk the active stack and dispatch one key. Editor-mode toggle is
/// meta (switches which engine Editor delegates to), so it short-
/// circuits the stack.
pub fn dispatch(app: &mut App, key: KeyEvent) {
    if crate::input::route::is_toggle_editor_mode(&app.config.editor.toggle_mode_key, key) {
        crate::input::route::toggle_editor_mode(app);
        return;
    }
    app.clear_status();
    let scopes = active_scopes(app);
    for kind in scopes.into_iter().rev() {
        match handle_scope(kind, app, key) {
            KeyOutcome::Consumed => return,
            KeyOutcome::Forward => continue,
            KeyOutcome::Effect(action) => {
                crate::input::dispatch::apply_action(app, action, false);
                return;
            }
        }
    }
}

fn handle_scope(kind: ScopeKind, app: &mut App, key: KeyEvent) -> KeyOutcome {
    match kind {
        ScopeKind::Editor => handle_editor(app, key),
        ScopeKind::CompletionPopup => handle_completion_popup(key),
        ScopeKind::FenceEdit => handle_fence_edit(key),
        ScopeKind::DbSettings => handle_db_settings(key),
        ScopeKind::ContentSearch => handle_content_search(key),
        ScopeKind::DbRowDetail => handle_db_row_detail(app, key),
        ScopeKind::HttpResponseDetail => handle_http_response_detail(app, key),
        ScopeKind::Modal => handle_modal(app, key),
        ScopeKind::RunningQueryCatch => handle_running_query_catch(app, key),
    }
}

/// Editor scope — routes to the configured engine. Cross-profile
/// global shortcuts (`is_editor_global_shortcut`) are bound only on
/// `standard_keymap`; vim's mode parsers don't know them, so we look
/// them up here before delegating to vim. Standard's `resolve` hits
/// the same table, so the lookup is skipped on that branch.
fn handle_editor(app: &mut App, key: KeyEvent) -> KeyOutcome {
    match app.config.editor.mode {
        crate::config::EditorMode::Vim => {
            if let Some(action) = crate::input::keymap::lookup(&app.standard_keymap, key) {
                if crate::input::keymap::is_editor_global_shortcut(action) {
                    return KeyOutcome::Effect(action);
                }
            }
            crate::input::dispatch::dispatch(app, key);
        }
        crate::config::EditorMode::Standard => crate::input::route::route_standard(app, key),
    }
    KeyOutcome::Consumed
}

/// Completion popup — overlay filter. Returns `Effect` for the
/// nav/accept/dismiss keys it cares about, `Forward` for everything
/// else (so the user keeps typing into the editor and the post-
/// action hook refreshes the popup against the new prefix).
fn handle_completion_popup(key: KeyEvent) -> KeyOutcome {
    match crate::input::apply::completion::parse_completion_popup_key(key) {
        Some(action) => KeyOutcome::Effect(action),
        None => KeyOutcome::Forward,
    }
}

/// Fence-edit prompt (alias/limit/timeout inline editing). Parser is
/// total — every key maps to an action — so this scope always
/// consumes via `Effect`.
fn handle_fence_edit(key: KeyEvent) -> KeyOutcome {
    KeyOutcome::Effect(crate::input::parser::lineedit::parse_fence_edit(key))
}

/// DB block settings popup (limit/timeout). Same shape as fence
/// edit — parser is total.
fn handle_db_settings(key: KeyEvent) -> KeyOutcome {
    KeyOutcome::Effect(crate::input::parser::modals::parse_db_settings_modal(key))
}

/// Full-text content search panel. Parser is total.
fn handle_content_search(key: KeyEvent) -> KeyOutcome {
    KeyOutcome::Effect(crate::input::parser::modals::parse_content_search(key))
}

/// DB row-detail modal — vim motions over a read-only doc. The parser
/// is a vim parser, so in Standard profile we `Forward` and let
/// `route_standard` decode arrows/etc. against the modal doc via the
/// `document_mut` redirect.
fn handle_db_row_detail(app: &mut App, key: KeyEvent) -> KeyOutcome {
    if app.config.editor.mode == crate::config::EditorMode::Standard {
        return KeyOutcome::Forward;
    }
    let action = crate::input::parser::modals::parse_db_row_detail(&mut app.vim, key);
    KeyOutcome::Effect(action)
}

/// HTTP response-detail modal — same shape as DbRowDetail.
fn handle_http_response_detail(app: &mut App, key: KeyEvent) -> KeyOutcome {
    if app.config.editor.mode == crate::config::EditorMode::Standard {
        return KeyOutcome::Forward;
    }
    let action = crate::input::parser::modals::parse_http_response_detail(&mut app.vim, key);
    KeyOutcome::Effect(action)
}

/// Running-query cancel — catches the profile-specific cancel chord.
/// `Ctrl+C` in vim, `Esc` in standard. Everything else `Forward`s so
/// the rest of the stack still works while a query streams.
fn handle_running_query_catch(app: &mut App, key: KeyEvent) -> KeyOutcome {
    use crossterm::event::{KeyCode, KeyModifiers};
    let cancel = match app.config.editor.mode {
        crate::config::EditorMode::Vim => {
            key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c')
        }
        crate::config::EditorMode::Standard => key.code == KeyCode::Esc,
    };
    if cancel {
        crate::commands::db::cancel_running_query(app);
        KeyOutcome::Consumed
    } else {
        KeyOutcome::Forward
    }
}

/// Modal scope — wraps `Modal::handle_key`. Consumes by default
/// (`ModalOutcome::Continue` → `Consumed`), pops on `Close`, applies
/// the action via `Effect` on `Emit`. Net result: no key ever leaks
/// past a modal, including keys the modal didn't bind.
fn handle_modal(app: &mut App, key: KeyEvent) -> KeyOutcome {
    let outcome = match app.modal.as_mut() {
        Some(m) => m.handle_key(key),
        // Stack derivation guarantees we only enter here when modal is
        // open, but the early-return keeps the function total.
        None => return KeyOutcome::Forward,
    };
    match outcome {
        crate::modal::ModalOutcome::Continue => KeyOutcome::Consumed,
        crate::modal::ModalOutcome::Close => {
            app.modal = None;
            app.vim.enter_normal();
            KeyOutcome::Consumed
        }
        crate::modal::ModalOutcome::Emit(action) => KeyOutcome::Effect(action),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, EditorMode};
    use crate::vault::ResolvedVault;
    use crossterm::event::{KeyCode, KeyModifiers};
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_fixture(mode: EditorMode) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), "abc\n").unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut cfg = Config::default();
        cfg.editor.mode = mode;
        (App::new(cfg, resolved, pool), data, vault)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stack_has_only_editor_when_idle() {
        let (app, _d, _v) = app_fixture(EditorMode::Standard).await;
        let scopes = active_scopes(&app);
        assert_eq!(scopes, vec![ScopeKind::Editor]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stack_pushes_modal_when_open() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Standard).await;
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            Action::OpenEnvsPage,
        );
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            Action::OpenEnvForm,
        );
        let scopes = active_scopes(&app);
        assert_eq!(scopes, vec![ScopeKind::Editor, ScopeKind::Modal]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn modal_swallows_unbound_key_in_standard() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Standard).await;
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            Action::OpenEnvsPage,
        );
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            Action::OpenEnvForm,
        );
        let before_doc = app.document().unwrap().to_markdown();
        let before_tab = app.tabs.active;
        dispatch(&mut app, key(KeyCode::F(11)));
        assert!(matches!(app.modal, Some(crate::modal::Modal::EnvForm(_))));
        assert_eq!(app.document().unwrap().to_markdown(), before_doc);
        assert_eq!(app.tabs.active, before_tab);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn detail_modal_forwards_in_standard_profile() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Standard).await;
        app.db_row_detail = Some(crate::app::DbRowDetailState {
            segment_idx: 0,
            row: 0,
            title: "test".into(),
            doc: crate::buffer::Document::from_markdown("alpha\nbeta\n").unwrap(),
            viewport_height: 4,
            viewport_top: 0,
        });
        app.vim.mode = crate::vim::mode::Mode::DbRowDetail;
        let before = app.document().unwrap().cursor();
        dispatch(&mut app, key(KeyCode::Down));
        let after = app.document().unwrap().cursor();
        assert_ne!(before, after, "Down must move modal cursor in standard");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn detail_modal_consumes_in_vim_profile() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Vim).await;
        app.db_row_detail = Some(crate::app::DbRowDetailState {
            segment_idx: 0,
            row: 0,
            title: "test".into(),
            doc: crate::buffer::Document::from_markdown("alpha\nbeta\n").unwrap(),
            viewport_height: 4,
            viewport_top: 0,
        });
        app.vim.mode = crate::vim::mode::Mode::DbRowDetail;
        let before = app.document().unwrap().cursor();
        dispatch(&mut app, key(KeyCode::Char('j')));
        let after = app.document().unwrap().cursor();
        assert_ne!(before, after, "j must move modal cursor in vim");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn http_detail_modal_forwards_in_standard_profile() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Standard).await;
        app.http_response_detail = Some(crate::app::HttpResponseDetailState {
            segment_idx: 0,
            title: "resp".into(),
            doc: crate::buffer::Document::from_markdown("line1\nline2\n").unwrap(),
            viewport_height: 4,
            viewport_top: 0,
        });
        app.vim.mode = crate::vim::mode::Mode::HttpResponseDetail;
        let before = app.document().unwrap().cursor();
        dispatch(&mut app, key(KeyCode::Down));
        let after = app.document().unwrap().cursor();
        assert_ne!(before, after);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn modal_blocks_universal_actions_through_stack() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Standard).await;
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            Action::OpenEnvsPage,
        );
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            Action::OpenEnvForm,
        );
        let before_tab = app.tabs.active;
        dispatch(&mut app, key(KeyCode::Tab));
        assert!(matches!(app.modal, Some(crate::modal::Modal::EnvForm(_))));
        assert_eq!(app.tabs.active, before_tab);
    }
}
