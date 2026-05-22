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

use crossterm::event::{KeyCode, KeyEvent};

use crate::app::App;
use crate::config::EditorMode;

/// Route one keystroke by the configured editor profile.
pub fn route(app: &mut App, key: KeyEvent) {
    // Hot toggle (Action::ToggleEditorMode): the configured chord
    // (`EditorConfig::toggle_mode_key`, default `f2`) flips
    // `config.editor.mode` and resets transient input state in BOTH
    // directions. Intercepted BEFORE the per-profile branch so the
    // chord works regardless of the currently-active profile —
    // `Vim::dispatch` would otherwise swallow it inside any modal
    // state (e.g. Insert/Visual). The default is an F-key, not
    // `Ctrl+Shift+M`: that chord is unreachable on terminals without
    // the kitty keyboard protocol (tui-V03 / cenário 0).
    if is_toggle_editor_mode(&app.config.editor.toggle_mode_key, key) {
        toggle_editor_mode(app);
        return;
    }

    match app.config.editor.mode {
        // Literal passthrough — exactly the call the old
        // `handle_key` made. Vim behaviour stays byte-identical; we
        // deliberately do NOT branch inside `dispatch`.
        EditorMode::Vim => crate::input::dispatch::dispatch(app, key),
        EditorMode::Standard => route_standard(app, key),
    }
}

/// `true` when `key` is the configured vim↔standard toggle chord.
/// A malformed config value parses to `None` → the toggle is simply
/// unbound (the user fixes the string in `config.toml`) rather than
/// panicking. Pure helper so route-tests can drive it directly.
fn is_toggle_editor_mode(toggle_key: &str, key: KeyEvent) -> bool {
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
/// through the new profile's path. `app.last_edit` is preserved
/// because the toggle is a meta-action, not an edit: a pending
/// auto-save debounce should still flush.
fn toggle_editor_mode(app: &mut App) {
    app.config.editor.mode = match app.config.editor.mode {
        EditorMode::Standard => EditorMode::Vim,
        EditorMode::Vim => EditorMode::Standard,
    };
    app.vim.enter_normal();
    app.standard = crate::app::StandardState::default();
}

/// Standard-mode path: minimal pre-filters mirrored from the top of
/// `dispatch`, then decode + apply via the pure `standard::resolve`.
fn route_standard(app: &mut App, key: KeyEvent) {
    // Any keystroke clears the previous transient status message —
    // same "press a key to dismiss" feel as the vim path.
    app.clear_status();

    // `Esc` while a query is running cancels it. fase 3 p3 moved
    // query-cancel off `Ctrl+C` onto `Esc`; `Ctrl+C` in Standard is
    // now contextual — copy an active selection, else quit the TUI
    // (see the decode tail below). The vim path is untouched — it
    // still cancels on `Ctrl+C` inside `dispatch` (Cenário 2
    // byte-identical). `Esc` without a running query stays a no-op
    // here: `standard::resolve` returns `None` for it, so the
    // decode tail below does nothing.
    if app.running_query.is_some() && key.code == KeyCode::Esc {
        crate::commands::db::cancel_running_query(app);
        return;
    }

    // The SQL completion popup intercepts navigation / accept /
    // dismiss keys before the regular decode, exactly as `dispatch`
    // does. Unmatched keys fall through to the standard decoder.
    if app.completion_popup.is_some() {
        if let Some(action) = crate::input::dispatch::parse_completion_popup_key(key) {
            crate::input::dispatch::apply_action(app, action, false);
            return;
        }
    }

    let Some(action) = crate::input::standard::resolve(key) else {
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

    // Auto-save edit-clock tap (tui-V01 / fase 5 p2). Set
    // `app.last_edit` for any action that *mutates* the buffer; the
    // Tick branch in `event_loop` debounces against it. The list
    // mirrors `standard_undo::maybe_snapshot`'s mutating set, plus
    // Cut/Paste (which snapshot in `standard_sel`). Motion / Copy /
    // ClearSelection / SelectExtend are pure cursor moves and must
    // NOT reset the clock — otherwise just navigating after an edit
    // would push the debounce indefinitely. Ortogonal to
    // `edit_group`/`maybe_snapshot` (no shared state).
    if matches!(
        action,
        Action::InsertChar(_)
            | Action::InsertNewline
            | Action::DeleteBackward
            | Action::DeleteForward
            | Action::Cut
            | Action::PasteSystem
    ) {
        app.last_edit = Some(std::time::Instant::now());
    }

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
    // `Action` is already in scope from the auto-save tap above.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use crossterm::event::KeyModifiers;
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

    fn ctrl_shift(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL | KeyModifiers::SHIFT)
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
    async fn standard_redo_restores_via_ctrl_y_and_ctrl_shift_z() {
        // Type → undo → redo round-trips for both redo chords. The
        // fixture seeds "abc\n", so type a string that is NOT a
        // substring of the seed to tell typed-vs-undone apart.
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
        // And again via Ctrl+Shift+Z.
        route(&mut app, ctrl(KeyCode::Char('z')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            original,
            "undo rewinds again"
        );
        route(&mut app, ctrl_shift(KeyCode::Char('z')));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            typed,
            "Ctrl+Shift+Z redoes"
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
        assert_eq!(
            crate::input::standard::resolve(ctrl(KeyCode::Char('v'))),
            Some(crate::input::action::Action::PasteSystem)
        );
    }

    // ----- fase 5 p2: last_edit clock --------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn insert_char_sets_last_edit_clock() {
        // Standard-mode `a` decodes to InsertChar('a'); the auto-save
        // tap must record `last_edit`. Proves the textual-action arm
        // of the matches!() in route_standard.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        assert!(app.last_edit.is_none(), "fresh App: no edit clock yet");
        route(&mut app, key(KeyCode::Char('a')));
        assert!(
            app.last_edit.is_some(),
            "InsertChar must set last_edit (auto-save debounce input)"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn motion_does_not_set_last_edit_clock() {
        // A pure cursor move (Left arrow → Action::Motion) must NOT
        // reset the clock — otherwise just navigating after an edit
        // would push the debounce window indefinitely.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        route(&mut app, key(KeyCode::Right));
        assert!(
            app.last_edit.is_none(),
            "Motion must NOT set last_edit (only mutating actions do)"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_backward_sets_last_edit_clock() {
        // Backspace decodes to DeleteBackward — also part of the
        // mutating set the tap watches.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        // Move cursor into the doc so backspace has something to bite.
        route(&mut app, key(KeyCode::Right));
        assert!(app.last_edit.is_none());
        route(&mut app, key(KeyCode::Backspace));
        assert!(app.last_edit.is_some(), "DeleteBackward must set last_edit");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn insert_newline_sets_last_edit_clock() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        route(&mut app, key(KeyCode::Enter));
        assert!(app.last_edit.is_some(), "InsertNewline must set last_edit");
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
    // `Config::default()`, so the toggle key is `f2` in every test
    // below.

    #[test]
    fn is_toggle_chord_matches_the_configured_f_key() {
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
        assert!(!super::is_toggle_editor_mode("f2", ctrl(KeyCode::Char('s'))));
    }

    #[test]
    fn is_toggle_chord_unbound_when_config_is_malformed() {
        // A garbage / empty config string parses to None → the toggle
        // is simply unbound, never panics.
        assert!(!super::is_toggle_editor_mode("", key(KeyCode::F(2))));
        assert!(!super::is_toggle_editor_mode("nonsense", key(KeyCode::F(2))));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_flips_standard_to_vim() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        route(&mut app, key(KeyCode::F(2)));
        assert_eq!(app.config.editor.mode, EditorMode::Vim);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_flips_vim_to_standard() {
        let (mut app, _d, _v) = app_with_note(EditorMode::Vim).await;
        route(&mut app, key(KeyCode::F(2)));
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
        route(&mut app, key(KeyCode::F(2)));
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
        route(&mut app, key(KeyCode::F(2)));
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
        route(&mut app, key(KeyCode::F(2)));
        let after = app.document().unwrap().to_markdown();
        assert_eq!(
            before, after,
            "toggle chord must NOT be typed as text in vim Insert"
        );
        assert_eq!(app.config.editor.mode, EditorMode::Standard);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_preserves_last_edit_clock() {
        // The toggle is a meta-action, not an edit. A pending
        // auto-save debounce (driven by `last_edit`) should NOT be
        // cleared by the profile flip — otherwise a user editing then
        // toggling would lose the pending flush.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        route(&mut app, key(KeyCode::Char('z')));
        let clock_before = app.last_edit;
        assert!(clock_before.is_some());
        route(&mut app, key(KeyCode::F(2)));
        assert_eq!(
            app.last_edit, clock_before,
            "toggle must NOT reset the auto-save edit clock"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_is_round_trip_clean() {
        // Symmetry: Standard → Vim → Standard returns to a usable
        // Standard with no residual state on either side. A plain
        // arrow after the round-trip must move the cursor (proves the
        // Standard route is alive) without re-seeding the anchor.
        let (mut app, _d, _v) = app_with_note(EditorMode::Standard).await;
        route(&mut app, key(KeyCode::F(2)));
        assert_eq!(app.config.editor.mode, EditorMode::Vim);
        route(&mut app, key(KeyCode::F(2)));
        assert_eq!(app.config.editor.mode, EditorMode::Standard);
        assert!(app.standard.anchor.is_none());
        // Sanity: a typed char still inserts.
        route(&mut app, key(KeyCode::Char('Q')));
        assert!(app.document().unwrap().to_markdown().contains('Q'));
    }
}
