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
    /// `app.modal.is_some()` — every modal variant (forms, pickers,
    /// confirms, detail modals). The detail variants (`DbRowDetail`,
    /// `HttpResponseDetail`) host the vim engine over a read-only
    /// sub-doc and return [`crate::modal::ModalOutcome::Forward`] for
    /// keys that need to flow to the editor (standard profile +
    /// transient vim modes); every other variant consumes by default.
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
        ScopeKind::Modal => handle_modal(app, key),
        ScopeKind::RunningQueryCatch => handle_running_query_catch(app, key),
    }
}

/// Editor scope — routes to the configured engine. Cross-profile
/// global shortcuts (`is_editor_global_shortcut`) are bound only on
/// `standard_keymap`; vim's mode parsers don't know them, so we look
/// them up here before delegating to vim. Standard's `resolve` hits
/// the same table, so the lookup is skipped on that branch.
///
/// `Mode::Git` is the exception: the git panel owns its full key
/// surface (Ctrl+B branch, Ctrl+L log, Ctrl+R resolver, Ctrl+Y
/// share, …) and those chords overlap the global editor shortcuts
/// (`Ctrl+B` = tree). We dispatch through the per-mode parser
/// directly without the global pre-empt so the panel wins.
fn handle_editor(app: &mut App, key: KeyEvent) -> KeyOutcome {
    if app
        .blocks_workspace
        .as_ref()
        .is_some_and(|w| w.pane_picker.is_some())
    {
        use crossterm::event::{KeyCode, KeyModifiers};
        if key.code == KeyCode::Esc
            || (key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c'))
        {
            return KeyOutcome::Effect(crate::input::action::Action::BlocksPanePickerCancel);
        }
        if let KeyCode::Char(c) = key.code {
            let lower = c.to_ascii_lowercase();
            if lower.is_ascii_lowercase() {
                let n = (lower as u8 - b'a' + 1) as usize;
                let leaves = app.active_tab().map(|t| t.leaf_count()).unwrap_or(0);
                if n <= leaves {
                    return KeyOutcome::Effect(
                        crate::input::action::Action::BlocksPanePickerChoose(n),
                    );
                }
            }
        }
        return KeyOutcome::Consumed;
    }
    if app.vim.mode == crate::vim::mode::Mode::Git {
        crate::input::dispatch::dispatch(app, key);
        return KeyOutcome::Consumed;
    }
    if matches!(
        app.vim.mode,
        crate::vim::mode::Mode::Tree | crate::vim::mode::Mode::TreePrompt
    ) {
        use crate::input::action::Action;
        use crate::input::types::WindowCmd;
        use crossterm::event::{KeyCode, KeyModifiers};
        // Treat the sidebar as a navigable window for `Ctrl+W` chords.
        // Without this, `Ctrl+W l` from the tree was dropped because
        // `parse_tree` ignores Ctrl+W and the vim engine never sees
        // the chord either. Re-uses the standard mode's flag so the
        // existing apply path (`Action::Window(FocusLeft/...)`) takes
        // care of the actual movement (which already knows the tree↔
        // pane bridge via `apply::window::focus_dir`).
        if app.standard.pending_window_chord {
            app.standard.pending_window_chord = false;
            let cmd = match (key.modifiers, key.code) {
                (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('h')) => {
                    Some(WindowCmd::FocusLeft)
                }
                (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('l')) => {
                    Some(WindowCmd::FocusRight)
                }
                (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('j')) => {
                    Some(WindowCmd::FocusDown)
                }
                (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('k')) => {
                    Some(WindowCmd::FocusUp)
                }
                _ => None,
            };
            if let Some(c) = cmd {
                return KeyOutcome::Effect(Action::Window(c));
            }
            return KeyOutcome::Consumed;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('w')) {
            app.standard.pending_window_chord = true;
            return KeyOutcome::Consumed;
        }
        // BLOCKS-view sidebar chords (`n` create, `d`/`Delete`,
        // `Shift+arrow` reorder) take precedence over the generic
        // tree input. Restricted to `Mode::Tree` so they only fire
        // when the sidebar actually has focus — the vim engine in
        // the pane (NAV / EDIT) keeps `d`, `dw`, `dd` etc. for its
        // own operators.
        if matches!(app.view, crate::app::AppView::Blocks) {
            if let Some(action) = crate::input::apply::blocks_view::resolve_tree_key(app, key) {
                return KeyOutcome::Effect(action);
            }
        }
        if let Some(action) = crate::input::keymap::lookup(&app.standard_keymap, key) {
            if crate::input::keymap::is_editor_global_shortcut(action) {
                return KeyOutcome::Effect(action);
            }
        }
        crate::input::dispatch::dispatch(app, key);
        return KeyOutcome::Consumed;
    }
    if matches!(app.view, crate::app::AppView::Blocks) {
        if let Some(action) = crate::input::apply::blocks_view::resolve_pane_key(app, key) {
            return KeyOutcome::Effect(action);
        }
        // EDIT lifecycle chords (Esc, Ctrl+C, Ctrl+S) were claimed by
        // `resolve_pane_key` above. Anything else falls through to the
        // editor engine below — `App::document_mut` redirects it to
        // the field's sub-Document, so every vim motion / operator /
        // count / search / undo lands on the buffer transparently.
    }
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

/// Modal scope — wraps `Modal::handle_key_with_ctx`. Consumes by
/// default (`Continue` → `Consumed`), pops on `Close`, applies the
/// action via `Effect` on `Emit`, and lets detail modals push the key
/// down the stack via `Forward`. Net result: no key ever leaks past a
/// modal except where the modal explicitly delegates.
fn handle_modal(app: &mut App, key: KeyEvent) -> KeyOutcome {
    let editor_mode = app.config.editor.mode;
    let outcome = match app.modal.as_mut() {
        Some(m) => {
            let mut ctx = crate::modal::ModalKeyCtx {
                vim: &mut app.vim,
                editor_mode,
            };
            m.handle_key_with_ctx(key, &mut ctx)
        }
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
        crate::modal::ModalOutcome::Forward => KeyOutcome::Forward,
        crate::modal::ModalOutcome::CloseAndForward => {
            app.modal = None;
            KeyOutcome::Forward
        }
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
        crate::input::apply::envs_page::apply_envs(&mut app, Action::OpenEnvsPage);
        crate::input::apply::envs_page::apply_envs(&mut app, Action::OpenEnvForm);
        let scopes = active_scopes(&app);
        assert_eq!(scopes, vec![ScopeKind::Editor, ScopeKind::Modal]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn modal_swallows_unbound_key_in_standard() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Standard).await;
        crate::input::apply::envs_page::apply_envs(&mut app, Action::OpenEnvsPage);
        crate::input::apply::envs_page::apply_envs(&mut app, Action::OpenEnvForm);
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
        app.modal = Some(crate::modal::Modal::DbRowDetail(
            crate::app::DbRowDetailState {
                segment_idx: 0,
                row: 0,
                title: "test".into(),
                doc: crate::buffer::Document::from_markdown("alpha\nbeta\n").unwrap(),
                viewport_height: 4,
                viewport_top: 0,
            },
        ));
        app.vim.mode = crate::vim::mode::Mode::DbRowDetail;
        let before = app.document().unwrap().cursor();
        dispatch(&mut app, key(KeyCode::Down));
        let after = app.document().unwrap().cursor();
        assert_ne!(before, after, "Down must move modal cursor in standard");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn detail_modal_consumes_in_vim_profile() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Vim).await;
        app.modal = Some(crate::modal::Modal::DbRowDetail(
            crate::app::DbRowDetailState {
                segment_idx: 0,
                row: 0,
                title: "test".into(),
                doc: crate::buffer::Document::from_markdown("alpha\nbeta\n").unwrap(),
                viewport_height: 4,
                viewport_top: 0,
            },
        ));
        app.vim.mode = crate::vim::mode::Mode::DbRowDetail;
        let before = app.document().unwrap().cursor();
        dispatch(&mut app, key(KeyCode::Char('j')));
        let after = app.document().unwrap().cursor();
        assert_ne!(before, after, "j must move modal cursor in vim");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn http_detail_modal_forwards_in_standard_profile() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Standard).await;
        app.modal = Some(crate::modal::Modal::HttpResponseDetail(
            crate::app::HttpResponseDetailState {
                segment_idx: 0,
                title: "resp".into(),
                doc: crate::buffer::Document::from_markdown("line1\nline2\n").unwrap(),
                viewport_height: 4,
                viewport_top: 0,
            },
        ));
        app.vim.mode = crate::vim::mode::Mode::HttpResponseDetail;
        let before = app.document().unwrap().cursor();
        dispatch(&mut app, key(KeyCode::Down));
        let after = app.document().unwrap().cursor();
        assert_ne!(before, after);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn modal_blocks_universal_actions_through_stack() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Standard).await;
        crate::input::apply::envs_page::apply_envs(&mut app, Action::OpenEnvsPage);
        crate::input::apply::envs_page::apply_envs(&mut app, Action::OpenEnvForm);
        let before_tab = app.tabs.active;
        dispatch(&mut app, key(KeyCode::Tab));
        assert!(matches!(app.modal, Some(crate::modal::Modal::EnvForm(_))));
        assert_eq!(app.tabs.active, before_tab);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ctrl_b_in_git_mode_opens_branch_picker_not_tree() {
        // Regression: the `Ctrl+B` chord is bound globally to
        // `TreeToggle` in `standard_keymap`. When the git panel
        // is focused (`Mode::Git`), the panel's own `Ctrl+B`
        // (`OpenGitBranchPicker`) must win — the global pre-empt
        // would otherwise toggle the file tree.
        let (mut app, _d, vault) = app_fixture(EditorMode::Standard).await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["add", "."])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["commit", "-m", "seed"])
            .output()
            .unwrap();
        // Open the git panel (Mode::Git).
        crate::input::apply::git_panel::apply_git_panel(&mut app, Action::GitPanelToggle);
        assert_eq!(app.vim.mode, crate::vim::mode::Mode::Git);
        let tree_visible_before = app.tree.visible;
        // Press Ctrl+B through the scope dispatcher.
        dispatch(
            &mut app,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        );
        // Tree visibility unchanged.
        assert_eq!(
            app.tree.visible, tree_visible_before,
            "Ctrl+B in Mode::Git must not toggle tree"
        );
        // Branch picker modal opened.
        assert!(
            matches!(app.modal, Some(crate::modal::Modal::GitBranchPicker(_))),
            "Ctrl+B in Mode::Git must open the branch picker"
        );
    }

    /// `Ctrl+W` while focused on the sidebar (`Mode::Tree`) must set
    /// the pending-window-chord flag so the next `h/j/k/l` can route
    /// to the tree↔pane bridge in `apply::window::focus_dir`.
    #[tokio::test(flavor = "multi_thread")]
    async fn ctrl_w_in_tree_mode_sets_pending_chord_then_routes_focus() {
        let (mut app, _d, _v) = app_fixture(EditorMode::Standard).await;
        app.vim.mode = crate::vim::mode::Mode::Tree;
        // First Ctrl+W: arms the chord, no action yet.
        dispatch(
            &mut app,
            KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL),
        );
        assert!(
            app.standard.pending_window_chord,
            "Ctrl+W in Mode::Tree should arm the chord"
        );
        // Second key `l`: consumes chord + dispatches Window(FocusRight).
        dispatch(&mut app, key(KeyCode::Char('l')));
        assert!(
            !app.standard.pending_window_chord,
            "chord should be cleared after the follow-up key"
        );
    }
}
