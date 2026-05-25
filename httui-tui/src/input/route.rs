//! Mode-aware input router. The app's `handle_key` delegates here.
//!
//! `route` is the single seam that splits keystrokes by the
//! configured editor profile (`app.config.editor.mode`):
//!
//! - **Vim** — literal passthrough to `crate::input::dispatch::dispatch`,
//!   byte-identical to what the old `handle_key` did. Zero behaviour
//!   change for vim users; the whole modal engine runs unchanged.
//! - **Standard** — the conventional non-modal model. A small set of
//!   pre-filters (mirrored from the top of `dispatch`) runs first so
//!   shared infrastructure (transient status, query-cancel, the SQL
//!   completion popup) keeps working, then `standard::resolve` decodes
//!   the key and `apply_action` runs it.
//!
//! Introduced by tui-V1 / fase 2 p5.

use crossterm::event::KeyEvent;

use crate::app::App;
use crate::config::EditorMode;

/// Route one keystroke through the input focus stack.
pub fn route(app: &mut App, key: KeyEvent) {
    crate::input::scope::dispatch(app, key);
}

/// `true` when `key` is the configured vim↔standard toggle chord.
/// A malformed config value parses to `None` → the toggle is simply
/// unbound (the user fixes the string in `config.toml`) rather than
/// panicking. Pure helper so route-tests can drive it directly.
pub(crate) fn is_toggle_editor_mode(toggle_key: &str, key: KeyEvent) -> bool {
    crate::input::keychord::parse_key_chord(toggle_key)
        .map(|chord| chord.matches(key))
        .unwrap_or(false)
}

/// Flip `config.editor.mode` and clear transient input state in BOTH
/// profiles so the new profile starts from a clean slate:
///
/// - Vim → Standard: `VimState::enter_normal` drops pending operators /
///   counts / cmdline / search buffer / visual anchor.
/// - Standard → Vim: `StandardState::default` drops the selection
///   anchor (the only Standard-owned transient).
///
/// Modal popups (`completion_popup`, `db_settings`, `block_history`,
/// pickers, the help modal, `running_query`) are NOT touched — they
/// own their own dismissal flow and the next routed key reaches them
/// through the new profile's path.
pub(crate) fn toggle_editor_mode(app: &mut App) {
    app.config.editor.mode = match app.config.editor.mode {
        EditorMode::Standard => EditorMode::Vim,
        EditorMode::Vim => EditorMode::Standard,
    };
    app.vim.enter_normal();
    app.standard = crate::app::StandardState::default();
    if let Some(path) = app.config_path.as_ref() {
        if let Err(e) = crate::config::save_config(path, &app.config) {
            tracing::warn!("persist editor mode toggle failed: {e}");
        }
    }
}

/// Standard-mode editor body: decode + apply via `standard::resolve`.
/// Invoked by the `Editor` scope handler in `input::scope`; preempting
/// scopes (modal, popup, query cancel, etc.) already had their chance.
pub(crate) fn route_standard(app: &mut App, key: KeyEvent) {
    let Some(action) = crate::input::standard::resolve(&app.standard_keymap, key) else {
        return;
    };
    use crate::input::action::Action;

    // Ctrl+C is contextual in Standard mode: with an active selection
    // it copies; with no selection it quits the TUI (Windows-Terminal
    // style). `resolve` always decodes Ctrl+C to `Copy` — the pure
    // decoder can't see selection state — so the no-selection → Quit
    // rewrite lands here, where `app.standard` is reachable. This is
    // Standard's only quit affordance (`:q` needs vim mode).
    let action = if matches!(action, Action::Copy) && app.standard.anchor.is_none() {
        Action::Quit
    } else {
        action
    };

    // Grid is read-only so InsertNewline would be a no-op; intercept
    // it to open row detail. Same remap for error blocks — the error
    // panel has no rows for the cursor to land on, so we accept Enter
    // from inside the SQL body too.
    let action = if matches!(action, Action::InsertNewline) {
        if cursor_in_db_block_result_or_error(app) {
            Action::OpenDbRowDetail
        } else {
            action
        }
    } else {
        action
    };

    // Undo-group snapshot policy (tui-V1 / fase 4 p2). Runs once per
    // keystroke BEFORE any dispatch so the snapshot captures the
    // pre-edit document; it covers every path below (incl. the
    // anchor-collapse branch). It only ever resets the group for
    // Cut/Paste (those snapshot themselves in `standard_sel`), so
    // there's no double snapshot. The vim path never reaches here.
    crate::input::apply::standard_undo::maybe_snapshot(app, &action);

    // Standard-mode selection / clipboard family is handled by the
    // dedicated (fully-covered) `standard_sel` module, with a real
    // clipboard injected here. The vim path never reaches this — it
    // never decodes into these `Action`s — so Cenário 2 is untouched.
    match action {
        Action::SelectExtend(_)
        | Action::ClearSelection
        | Action::Copy
        | Action::Cut
        | Action::PasteSystem => {
            let mut clip = crate::clipboard::ArboardClipboard;
            crate::input::apply::standard_sel::apply_standard_sel(app, action, &mut clip);
        }
        Action::Motion(..) if app.standard.anchor.is_some() => {
            // A plain (non-Shift) arrow while a selection is active
            // collapses it first, then moves normally — conventional
            // editor behaviour. Collapse routes through `standard_sel`
            // so anchor ownership stays in one covered place.
            let mut clip = crate::clipboard::ArboardClipboard;
            crate::input::apply::standard_sel::apply_standard_sel(
                app,
                Action::ClearSelection,
                &mut clip,
            );
            crate::input::dispatch::apply_action(app, action, /* recording = */ true);
        }
        _ => crate::input::dispatch::apply_action(app, action, /* recording = */ true),
    }
}

/// True for a real result row, or anywhere inside a DB block whose
/// cached result is an error (error panel has no rows to land on).
fn cursor_in_db_block_result_or_error(app: &App) -> bool {
    use crate::buffer::{Cursor, Segment};
    let Some(doc) = app.document() else { return false };
    match doc.cursor() {
        Cursor::InBlockResult { .. } => true,
        Cursor::InBlock { segment_idx, .. } => {
            let Some(Segment::Block(block)) = doc.segments().get(segment_idx) else {
                return false;
            };
            if !block.is_db() {
                return false;
            }
            matches!(
                block
                    .cached_result
                    .as_ref()
                    .and_then(|v| v.get("results"))
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|r| r.get("kind"))
                    .and_then(|v| v.as_str()),
                Some("error")
            )
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use crossterm::event::{KeyCode, KeyModifiers};
    use httui_core::db::init_db;
    use tempfile::TempDir;

    /// Build an `App` over a fresh vault seeded with one note open in
    /// the first tab. Mirrors the `app::tests` fixture so route tests
    /// exercise the real `handle_key` plumbing.
    async fn app_with_note(mode: EditorMode) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), "abc\n").unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut cfg = Config::default();
        cfg.editor.mode = mode;
        let app = App::new(cfg, resolved, pool);
        (app, data, vault)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    fn alt(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::ALT)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vim_branch_is_a_literal_passthrough_to_dispatch() {
        // Route with mode=Vim must do exactly what calling
        // `dispatch` directly does: pressing `i` flips Normal→Insert.
        let (mut app, _d, _v) = app_with_note(EditorMode::Vim).await;
        let before = app.vim.mode;
        route(&mut app, key(KeyCode::Char('i')));
        assert_ne!(
            app.vim.mode, before,
            "Vim passthrough should reach dispatch and flip the mode"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vim_route_matches_calling_dispatch_directly() {
        // Same key, two apps in Vim mode: one driven through `route`,
        // one through `dispatch` directly. The resulting doc + cursor
        // + mode must be identical (byte-identical passthrough).
        let (mut a, _da, _va) = app_with_note(EditorMode::Vim).await;
        let (mut b, _db, _vb) = app_with_note(EditorMode::Vim).await;
        for ev in [
            key(KeyCode::Char('i')),
            key(KeyCode::Char('x')),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        ] {
            route(&mut a, ev);
            crate::input::dispatch::dispatch(&mut b, ev);
        }
        assert_eq!(a.vim.mode, b.vim.mode);
        let da = a.document().unwrap().to_markdown();
        let db = b.document().unwrap().to_markdown();
        assert_eq!(da, db, "route(Vim) must equal dispatch() byte-for-byte");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn modal_open_blocks_universal_actions_too() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            crate::input::action::Action::OpenEnvsPage,
        );
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            crate::input::action::Action::OpenEnvForm,
        );
        let tab_count_before = app.tabs.tabs.len();
        let tab_active_before = app.tabs.active;
        route(&mut app, key(KeyCode::Tab));
        route(&mut app, ctrl(KeyCode::PageDown));
        assert_eq!(app.tabs.tabs.len(), tab_count_before);
        assert_eq!(app.tabs.active, tab_active_before);
        assert!(matches!(app.modal, Some(crate::modal::Modal::EnvForm(_))));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn modal_open_in_standard_swallows_unbound_keys() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            crate::input::action::Action::OpenEnvsPage,
        );
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            crate::input::action::Action::OpenEnvForm,
        );
        assert!(matches!(
            app.modal,
            Some(crate::modal::Modal::EnvForm(_))
        ));
        let before = app.document().unwrap().to_markdown();
        route(&mut app, key(KeyCode::F(11)));
        let after = app.document().unwrap().to_markdown();
        assert_eq!(before, after, "F11 must not mutate the editor doc");
        assert!(
            matches!(app.modal, Some(crate::modal::Modal::EnvForm(_))),
            "modal must stay open"
        );

        let tab_count_before = app.tabs.tabs.len();
        let tab_active_before = app.tabs.active;
        route(&mut app, key(KeyCode::Tab));
        assert_eq!(app.tabs.tabs.len(), tab_count_before);
        assert_eq!(app.tabs.active, tab_active_before, "Tab leaked to editor");
        assert!(
            matches!(app.modal, Some(crate::modal::Modal::EnvForm(_))),
            "modal must stay open after Tab"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_types_without_pressing_i() {
        // The whole point of Standard: a default App types text with
        // no `i` first. Press `X` → the doc gains an `X`, no mode
        // dance.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let before = app.document().unwrap().to_markdown();
        route(&mut app, key(KeyCode::Char('X')));
        let after = app.document().unwrap().to_markdown();
        assert_ne!(before, after, "Standard should insert the char directly");
        assert!(
            after.contains('X'),
            "typed char should be in the doc, got: {after:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_default_is_standard_no_i_needed() {
        // Config::default() is Standard — prove the default profile
        // routes through the standard decoder (not vim) by typing
        // straight away from a freshly constructed default App.
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), "abc\n").unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        route(&mut app, key(KeyCode::Char('Z')));
        assert!(app.document().unwrap().to_markdown().contains('Z'));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_arrow_moves_cursor() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let before = app.document().unwrap().cursor();
        route(&mut app, key(KeyCode::Right));
        let after = app.document().unwrap().cursor();
        assert_ne!(before, after, "Right arrow should move the cursor");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_enter_inserts_newline() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let before_lines = app.document().unwrap().to_markdown().lines().count();
        route(&mut app, key(KeyCode::Enter));
        let after_lines = app.document().unwrap().to_markdown().lines().count();
        assert!(
            after_lines > before_lines,
            "Enter should add a line ({before_lines} -> {after_lines})"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_unbound_key_is_a_noop() {
        // Esc has no Standard binding → `resolve` returns None → the
        // doc is untouched and nothing panics.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let before = app.document().unwrap().to_markdown();
        route(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(before, app.document().unwrap().to_markdown());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_ctrl_s_saves_the_file() {
        // Type a char (marks the doc dirty), then Ctrl-S → the on-disk
        // file reflects the edit and the doc is no longer dirty.
        let (mut app, _d, vault) = app_with_note(EditorMode::Standard).await;
        route(&mut app, key(KeyCode::Char('Q')));
        assert!(app.document().unwrap().is_dirty());
        route(&mut app, ctrl(KeyCode::Char('s')));
        assert!(
            !app.document().unwrap().is_dirty(),
            "Ctrl-S should clear the dirty flag"
        );
        let on_disk = std::fs::read_to_string(vault.path().join("note.md")).unwrap();
        assert!(
            on_disk.contains('Q'),
            "saved file should contain the typed char, got: {on_disk:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_ctrl_s_resets_undo_group_so_post_save_undoes_alone() {
        // tui-V1 / fase 4 p3 — harden Ctrl+S: WriteFile is a
        // non-textual action, so `maybe_snapshot` resets edit_group.
        // Type, save, type again → one Ctrl+Z undoes ONLY the
        // post-save run (the save split the undo group); the file is
        // saved (dirty cleared) in between.
        let (mut app, _d, vault) = app_with_note(EditorMode::Standard).await;
        type_str(&mut app, "PRE");
        let after_pre = app.document().unwrap().to_markdown();
        route(&mut app, ctrl(KeyCode::Char('s')));
        assert!(!app.document().unwrap().is_dirty(), "Ctrl+S clears dirty");
        let on_disk = std::fs::read_to_string(vault.path().join("note.md")).unwrap();
        assert!(on_disk.contains("PRE"), "saved file has the pre-save run");
        type_str(&mut app, "POST");
        assert!(app.document().unwrap().to_markdown().contains("POST"));
        route(&mut app, ctrl(KeyCode::Char('z')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            after_pre,
            "one undo reverses only the post-save typing (WriteFile reset the group)"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_clears_transient_status_on_keystroke() {
        // Mirrors the vim path's "press a key to dismiss" feel.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        app.set_status(crate::app::StatusKind::Info, "hi");
        assert!(app.status_message.is_some());
        route(&mut app, key(KeyCode::Right));
        assert!(
            app.status_message.is_none(),
            "any standard keystroke should clear the transient status"
        );
    }

    fn shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::SHIFT)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_shift_arrow_starts_a_selection() {
        // fase 3 p2: first Shift+arrow seeds the anchor at the
        // pre-move caret and advances the caret (moving end).
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let before = app.document().unwrap().cursor();
        assert!(app.standard.anchor.is_none());
        route(&mut app, shift(KeyCode::Right));
        assert_eq!(
            app.standard.anchor,
            Some(before),
            "anchor seeded at the caret before the move"
        );
        assert_ne!(
            app.document().unwrap().cursor(),
            before,
            "caret (moving end) advanced"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_plain_arrow_collapses_active_selection() {
        // A bare arrow while a selection is active drops the anchor
        // and still moves the caret.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        route(&mut app, shift(KeyCode::Right));
        assert!(app.standard.anchor.is_some(), "precondition: selecting");
        let mid = app.document().unwrap().cursor();
        route(&mut app, key(KeyCode::Right));
        assert!(
            app.standard.anchor.is_none(),
            "plain arrow collapses the selection"
        );
        assert_ne!(
            app.document().unwrap().cursor(),
            mid,
            "plain arrow still moves the caret after collapsing"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_plain_arrow_without_selection_is_unchanged() {
        // No anchor → behaves exactly like before fase 3 (no-op on
        // the anchor, normal move).
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let before = app.document().unwrap().cursor();
        route(&mut app, key(KeyCode::Right));
        assert!(app.standard.anchor.is_none());
        assert_ne!(app.document().unwrap().cursor(), before);
    }

    fn fake_running_query() -> crate::app::RunningQuery {
        crate::app::RunningQuery {
            segment_idx: 0,
            cancel: tokio_util::sync::CancellationToken::new(),
            started_at: std::time::Instant::now(),
            kind: crate::app::RunningKind::Run,
            cache_key: None,
            bytes_received: 0,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_esc_cancels_a_running_query() {
        // fase 3 p3: query-cancel moved off Ctrl+C onto Esc.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let rq = fake_running_query();
        let token = rq.cancel.clone();
        app.running_query = Some(rq);
        route(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(token.is_cancelled(), "Esc cancels the in-flight query");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_esc_without_query_is_a_noop() {
        // No regression: Esc with nothing running does nothing
        // (`resolve` returns None for Esc — the decode tail no-ops).
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let before = app.document().unwrap().to_markdown();
        assert!(app.running_query.is_none());
        route(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.document().unwrap().to_markdown(), before);
        assert!(app.running_query.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_ctrl_c_without_selection_quits_not_cancels() {
        // Ctrl+C with no selection quits the TUI (contextual rewrite —
        // Standard's only quit affordance). It must NOT cancel the
        // running query — that's Esc's job.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let rq = fake_running_query();
        let token = rq.cancel.clone();
        app.running_query = Some(rq);
        assert!(app.standard.anchor.is_none(), "precondition: no selection");
        route(&mut app, ctrl(KeyCode::Char('c')));
        assert!(app.should_quit, "Ctrl+C with no selection quits");
        assert!(
            !token.is_cancelled(),
            "Ctrl+C must not cancel the query (Esc owns cancel)"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_ctrl_c_copies_an_active_selection() {
        // End-to-end through `route`: Shift-select then Ctrl+C — the
        // selection lands on the system clipboard (via the real
        // ArboardClipboard; in headless CI the set may error but the
        // doc is never mutated and nothing panics).
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let before = app.document().unwrap().to_markdown();
        route(&mut app, shift(KeyCode::Right));
        route(&mut app, shift(KeyCode::Right));
        assert!(app.standard.anchor.is_some());
        route(&mut app, ctrl(KeyCode::Char('c')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            before,
            "Copy never mutates the doc"
        );
        assert!(
            app.standard.anchor.is_some(),
            "Copy keeps the selection alive"
        );
        assert!(
            !app.should_quit,
            "Ctrl+C with a selection copies — it must not quit"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_ctrl_x_cuts_an_active_selection() {
        // Cut removes the selected run and collapses the anchor. The
        // clipboard write may fail headless, in which case cut is a
        // safe no-op (text preserved) — assert the invariant either
        // way: anchor never survives a *successful* cut, doc only
        // shrinks if the clipboard accepted the text.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        route(&mut app, shift(KeyCode::Right));
        route(&mut app, shift(KeyCode::Right));
        let had_anchor = app.standard.anchor.is_some();
        assert!(had_anchor);
        route(&mut app, ctrl(KeyCode::Char('x')));
        // Either the cut succeeded (anchor collapsed, doc shrank) or
        // the clipboard was unavailable (doc + anchor preserved). No
        // panic, no half-state.
        let md = app.document().unwrap().to_markdown();
        if app.standard.anchor.is_none() {
            assert!(md.len() < 4, "successful cut shrank the doc: {md:?}");
        } else {
            assert_eq!(md, "abc\n", "failed cut preserved the doc");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_ctrl_v_routes_to_paste_without_panicking() {
        // fase 3 p4: Ctrl+V decodes to PasteSystem and reaches the
        // standard_sel paste path. Through `route` the real clipboard
        // is used (headless CI → empty/err → no-op). Deterministic
        // invariant: routing works, nothing panics, the doc is only
        // ever grown by real clipboard content (never corrupted).
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let before = app.document().unwrap().to_markdown();
        route(&mut app, ctrl(KeyCode::Char('v')));
        let after = app.document().unwrap().to_markdown();
        // Either unchanged (no/empty clipboard) or the clipboard text
        // was inserted at the caret — never a panic / partial write.
        assert!(
            after == before || after.len() >= before.len(),
            "paste must not shrink/corrupt the doc: {before:?} -> {after:?}"
        );
    }

    fn type_str(app: &mut App, s: &str) {
        for c in s.chars() {
            route(app, key(KeyCode::Char(c)));
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_undo_rewinds_a_whole_typed_word_not_one_char() {
        // Cenário 1 passo 6: typing "abc" then Ctrl+Z must restore the
        // ORIGINAL doc (one undo group for the whole run), not peel a
        // single char.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let original = app.document().unwrap().to_markdown();
        type_str(&mut app, "abc");
        assert!(app.document().unwrap().to_markdown().contains("abc"));
        route(&mut app, ctrl(KeyCode::Char('z')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            original,
            "one Ctrl+Z undoes the whole typed word"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_motion_splits_undo_groups() {
        // "ab" + Right + "cd" → Ctrl+Z undoes only "cd", a second
        // Ctrl+Z undoes "ab".
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let original = app.document().unwrap().to_markdown();
        type_str(&mut app, "ab");
        let after_ab = app.document().unwrap().to_markdown();
        route(&mut app, key(KeyCode::Right));
        type_str(&mut app, "cd");
        route(&mut app, ctrl(KeyCode::Char('z')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            after_ab,
            "first undo removes only the post-motion 'cd'"
        );
        route(&mut app, ctrl(KeyCode::Char('z')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            original,
            "second undo removes 'ab'"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_word_boundary_splits_undo_at_whitespace() {
        // "hello world" → Ctrl+Z undoes " world", Ctrl+Z undoes
        // "hello" (the word→space boundary opened a 2nd group).
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let original = app.document().unwrap().to_markdown();
        type_str(&mut app, "hello world");
        assert!(app
            .document()
            .unwrap()
            .to_markdown()
            .contains("hello world"));
        route(&mut app, ctrl(KeyCode::Char('z')));
        let mid = app.document().unwrap().to_markdown();
        assert!(
            mid.contains("hello") && !mid.contains("hello world"),
            "first undo peels ' world', leaving 'hello': {mid:?}"
        );
        route(&mut app, ctrl(KeyCode::Char('z')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            original,
            "second undo peels 'hello'"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_redo_restores_via_ctrl_y() {
        // Type → undo → redo round-trip. The fixture seeds "abc\n", so
        // type a string that is NOT a substring of the seed to tell
        // typed-vs-undone apart. Redo is `Ctrl+Y` — the old
        // `Ctrl+Shift+Z` alias was dropped in tui-V03: a chord matcher
        // ignores Shift on letter keys, so `ctrl+shift+z` is
        // indistinguishable from `ctrl+z` (undo).
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let original = app.document().unwrap().to_markdown();
        type_str(&mut app, "ZZZ");
        let typed = app.document().unwrap().to_markdown();
        assert!(typed.contains("ZZZ"));
        route(&mut app, ctrl(KeyCode::Char('z')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            original,
            "undo rewinds the typed run"
        );
        route(&mut app, ctrl(KeyCode::Char('y')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            typed,
            "Ctrl+Y redoes"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_cut_then_type_undoes_independently_no_double_snapshot() {
        // Shift-select + Cut (snapshot owned by standard_sel) then
        // type → Ctrl+Z undoes the typing, Ctrl+Z undoes the cut.
        // Proves maybe_snapshot does NOT double-snapshot Cut.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let original = app.document().unwrap().to_markdown();
        route(&mut app, shift(KeyCode::Right));
        route(&mut app, ctrl(KeyCode::Char('x')));
        let after_cut = app.document().unwrap().to_markdown();
        // Headless clipboard may make cut a no-op; only assert the
        // undo-grouping invariant when the cut actually mutated.
        if after_cut != original {
            type_str(&mut app, "Z");
            route(&mut app, ctrl(KeyCode::Char('z')));
            assert_eq!(
                app.document().unwrap().to_markdown(),
                after_cut,
                "first undo removes only the typed 'Z'"
            );
            route(&mut app, ctrl(KeyCode::Char('z')));
            assert_eq!(
                app.document().unwrap().to_markdown(),
                original,
                "second undo reverses the cut (no double snapshot)"
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_ctrl_z_with_nothing_to_undo_sets_status_no_panic() {
        // Ctrl+Z on a pristine doc → "already at oldest change", no
        // panic, doc untouched.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let original = app.document().unwrap().to_markdown();
        route(&mut app, ctrl(KeyCode::Char('z')));
        assert_eq!(app.document().unwrap().to_markdown(), original);
        let msg = app
            .status_message
            .as_ref()
            .map(|m| m.text.clone())
            .unwrap_or_default();
        assert!(
            msg.contains("oldest change"),
            "expected the 'already at oldest change' status, got: {msg:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_ctrl_v_decodes_to_paste_system() {
        // Pin the decode: the router must hand PasteSystem to
        // standard_sel (the behavioural mirror of the roteiro —
        // Shift-sel→Copy→move→Paste — is proven deterministically in
        // `apply::standard_sel::tests::roteiro_mirror_copy_move_paste`
        // with an injected FakeClipboard; here we only assert the
        // route seam decodes Ctrl+V correctly).
        let keymap = crate::input::keymap::resolve_standard_keymap(
            &crate::config::KeymapConfig::default(),
        );
        assert_eq!(
            crate::input::standard::resolve(&keymap, ctrl(KeyCode::Char('v'))),
            Some(crate::input::action::Action::PasteSystem)
        );
    }

    // ---------------------------------------------------------------
    // Hot toggle — the configured chord (`EditorConfig::toggle_mode_key`,
    // default `f2`) flips `config.editor.mode` from BOTH profiles, in
    // EVERY mode, and resets transient input state in the receiving
    // profile.
    // ---------------------------------------------------------------

    // The chord grammar itself is covered by `crate::input::keychord`
    // tests; here we only exercise the route helper's wiring and its
    // malformed-config fallback. `app_with_note` builds the app from
    // `Config::default()`, so the toggle key is `alt+m` in every test
    // below — the runtime default since tui-V03 keymap fase 4 (the
    // earlier `f2` was rejected on UX grounds).

    #[test]
    fn is_toggle_chord_matches_the_configured_chord() {
        // The configured-chord parameter is what the function honours —
        // pass a chord string and verify it matches its own key event.
        // Use the runtime default (`alt+m`) plus a legacy `f2` to prove
        // both still parse correctly.
        assert!(super::is_toggle_editor_mode(
            "alt+m",
            alt(KeyCode::Char('m'))
        ));
        assert!(!super::is_toggle_editor_mode(
            "alt+m",
            alt(KeyCode::Char('n'))
        ));
        assert!(super::is_toggle_editor_mode("f2", key(KeyCode::F(2))));
        assert!(!super::is_toggle_editor_mode("f2", key(KeyCode::F(3))));
    }

    #[test]
    fn is_toggle_chord_matches_a_configured_ctrl_letter() {
        assert!(super::is_toggle_editor_mode(
            "ctrl+e",
            ctrl(KeyCode::Char('e'))
        ));
    }

    #[test]
    fn is_toggle_chord_rejects_unrelated_key() {
        assert!(!super::is_toggle_editor_mode(
            "alt+m",
            ctrl(KeyCode::Char('s'))
        ));
    }

    #[test]
    fn is_toggle_chord_unbound_when_config_is_malformed() {
        // A garbage / empty config string parses to None → the toggle
        // is simply unbound, never panics.
        assert!(!super::is_toggle_editor_mode("", alt(KeyCode::Char('m'))));
        assert!(!super::is_toggle_editor_mode(
            "nonsense",
            alt(KeyCode::Char('m'))
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_flips_standard_to_vim() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        route(&mut app, alt(KeyCode::Char('m')));
        assert_eq!(app.config.editor.mode, EditorMode::Vim);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_flips_vim_to_standard() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Vim).await;
        route(&mut app, alt(KeyCode::Char('m')));
        assert_eq!(app.config.editor.mode, EditorMode::Standard);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_clears_vim_transient_state_on_vim_to_standard() {
        // Park vim in Insert with a pending count: toggle must drop
        // both (call `enter_normal`) so re-toggling back into Vim
        // doesn't resume mid-operation.
        let (mut app, _d, _v) = app_with_note(EditorMode::Vim).await;
        route(&mut app, key(KeyCode::Char('i')));
        assert_eq!(app.vim.mode, crate::vim::mode::Mode::Insert);
        app.vim.pending_count = Some(7);
        route(&mut app, alt(KeyCode::Char('m')));
        // Profile flipped AND vim state was reset.
        assert_eq!(app.config.editor.mode, EditorMode::Standard);
        assert_eq!(app.vim.mode, crate::vim::mode::Mode::Normal);
        assert_eq!(app.vim.pending_count, None);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_clears_standard_anchor_on_standard_to_vim() {
        // Seed a Standard selection (Shift+Right twice) then toggle:
        // the anchor must be dropped so Vim's visual-mode logic
        // doesn't see leftover Standard state.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let shift_right = KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT);
        route(&mut app, shift_right);
        route(&mut app, shift_right);
        assert!(
            app.standard.anchor.is_some(),
            "Shift+arrow should have seeded the Standard anchor"
        );
        route(&mut app, alt(KeyCode::Char('m')));
        assert_eq!(app.config.editor.mode, EditorMode::Vim);
        assert!(
            app.standard.anchor.is_none(),
            "toggle must drop the Standard anchor before handing off to Vim"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_works_from_vim_insert_mode() {
        // Critical: the chord must be intercepted BEFORE the per-profile
        // branch. Otherwise vim's Insert-mode handler would type the
        // character. Enter Insert mode first, then toggle — the
        // document must NOT gain an 'm' (or 'M') and the profile must
        // flip.
        let (mut app, _d, _v) = app_with_note(EditorMode::Vim).await;
        route(&mut app, key(KeyCode::Char('i')));
        assert_eq!(app.vim.mode, crate::vim::mode::Mode::Insert);
        let before = app.document().unwrap().to_markdown();
        route(&mut app, alt(KeyCode::Char('m')));
        let after = app.document().unwrap().to_markdown();
        assert_eq!(
            before, after,
            "toggle chord must NOT be typed as text in vim Insert"
        );
        assert_eq!(app.config.editor.mode, EditorMode::Standard);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_persists_mode_to_config_file() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        let cfg_dir = TempDir::new().unwrap();
        let cfg_path = cfg_dir.path().join("config.toml");
        crate::config::save_config(&cfg_path, &app.config).unwrap();
        app.config_path = Some(cfg_path.clone());
        route(&mut app, alt(KeyCode::Char('m')));
        let on_disk: Config = toml::from_str(&std::fs::read_to_string(&cfg_path).unwrap()).unwrap();
        assert_eq!(on_disk.editor.mode, EditorMode::Vim);
        route(&mut app, alt(KeyCode::Char('m')));
        let on_disk: Config = toml::from_str(&std::fs::read_to_string(&cfg_path).unwrap()).unwrap();
        assert_eq!(on_disk.editor.mode, EditorMode::Standard);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_without_config_path_is_silent() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        assert!(app.config_path.is_none());
        route(&mut app, alt(KeyCode::Char('m')));
        assert_eq!(app.config.editor.mode, EditorMode::Vim);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_is_round_trip_clean() {
        // Symmetry: Standard → Vim → Standard returns to a usable
        // Standard with no residual state on either side. A plain
        // arrow after the round-trip must move the cursor (proves the
        // Standard route is alive) without re-seeding the anchor.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        route(&mut app, alt(KeyCode::Char('m')));
        assert_eq!(app.config.editor.mode, EditorMode::Vim);
        route(&mut app, alt(KeyCode::Char('m')));
        assert_eq!(app.config.editor.mode, EditorMode::Standard);
        assert!(app.standard.anchor.is_none());
        // Sanity: a typed char still inserts.
        route(&mut app, key(KeyCode::Char('Q')));
        assert!(app.document().unwrap().to_markdown().contains('Q'));
    }
}
