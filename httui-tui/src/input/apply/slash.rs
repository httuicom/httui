//! `/` key applier — Standard-mode slash trigger (tui-V2 / vertical 2).
//!
//! Fresh module created by V2, NOT `coverage:exclude` (same policy as
//! `standard_sel` / `standard_undo` from V1). Owned by the vertical
//! that adds it, fully unit-covered.
//!
//! ## What it does
//!
//! Standard mode (the non-modal default since V1) decodes `/` to
//! `Action::SlashKey` in `input::standard::resolve`. The vim profile is
//! untouched — `/` there is still `EnterSearch(false)`. The applier
//! here is context-aware:
//!
//! - **Cursor in prose** — snapshot, insert `/` literally, then open
//!   the block-template picker on top. The slash stays in the buffer:
//!   if the user dismisses the picker (Esc) the `/` is what they
//!   would have typed, and if they confirm a template, the splice
//!   lands *after* the `/` (the confirm handler's normal placement
//!   rule). This mirrors the desktop CodeMirror slash-commands
//!   pattern conceptually, minus the inline-filter UX (deferred to
//!   tui-V11).
//! - **Cursor in a block** — insert `/` literally only. URLs and DB
//!   paths contain `/` constantly; opening a picker mid-fence would
//!   be hostile.
//! - **Cursor in a block result** — no-op (the result table is
//!   read-only; mirrors `insert_char_at_cursor`'s `InBlockResult`
//!   arm).
//!
//! ## Why the snapshot + last_edit live here
//!
//! `route::route_standard` runs `maybe_snapshot` and the `last_edit`
//! clock for `InsertChar`/`DeleteBackward`/… before dispatch. The
//! `SlashKey` arm is deliberately NOT in either of those lists — the
//! route file already crossed the 600-line size gate before V2
//! started, and threading a new variant through it would force a
//! mechanical split of a module that's outside V2's scope. Instead
//! the applier owns its own snapshot + edit-clock taps, keeping the
//! V2 change strictly additive.

use crate::app::{App, BlockTemplatePickerState};
use crate::buffer::Cursor;
use crate::vim::mode::Mode;

/// Apply `Action::SlashKey`. See module doc for the context-aware
/// rules and the rationale for the inline snapshot / `last_edit`
/// taps.
pub fn apply_slash_key(app: &mut App) {
    // Sample the cursor *before* the insert advances it — we need to
    // know whether the keystroke landed in prose to decide whether to
    // open the picker. `Cursor` is `Copy` so this is cheap and lets
    // the immutable borrow drop before the `document_mut` below.
    let cursor_pre = app.document().map(|d| d.cursor());

    let inserted = if let Some(doc) = app.document_mut() {
        // `/` is its own undo group: a snapshot here means `Ctrl+Z`
        // after dismissing the picker rewinds exactly to the
        // pre-slash document. Without this, the slash would coalesce
        // into whatever Insert group was open before, which is
        // surprising when the picker UX implies a discrete action.
        doc.snapshot();
        doc.insert_char_at_cursor('/');
        true
    } else {
        false
    };

    // No document → nothing to do; the picker would have nowhere to
    // splice into. Bail before touching mode / clock state.
    if !inserted {
        return;
    }

    // Open the picker only when the keystroke landed in prose. In a
    // block, the `/` is just a literal character. In a block result,
    // the insert was a no-op (read-only) — same treatment.
    if matches!(cursor_pre, Some(Cursor::InProse { .. })) {
        app.modal = Some(crate::modal::Modal::BlockTemplatePicker(
            BlockTemplatePickerState::new(),
        ));
        app.vim.mode = Mode::Modal;
        app.vim.reset_pending();
    }

    // Auto-save edit-clock tap (same shape as the matches in
    // `route::route_standard` for InsertChar / DeleteBackward / …).
    // The Tick branch in `event_loop` debounces against this.
    app.last_edit = Some(std::time::Instant::now());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::{Config, EditorMode};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    /// Build an `App` opened on a single-note vault with the given
    /// markdown content. Mirrors the fixture in `standard_sel` /
    /// `standard_undo` (both `coverage:exclude`-free V1 modules) so
    /// the applier under test runs against a real `Document` parsed
    /// by `from_markdown` and exercised via the public `App` surface.
    async fn app_with(text: &str) -> (App, TempDir, TempDir) {
        // `App::new` calls `load_initial_document`, which opens the
        // first `.md` it finds in the vault. Writing `note.md` before
        // constructing the App is enough — no extra "open" call.
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), text).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut cfg = Config::default();
        cfg.editor.mode = EditorMode::Standard;
        let app = App::new(cfg, resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn slash_in_prose_inserts_char_and_opens_picker() {
        // Cenário 1 happy path: cursor in prose, the slash is typed,
        // the picker pops up. Both effects MUST happen — the slash
        // stays in the buffer (no magic eat) and the picker is the
        // visible side effect that lets the user confirm a template.
        let (mut app, _d, _v) = app_with("hello\n").await;
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 5,
        });

        apply_slash_key(&mut app);

        // Picker must be visible with a fresh state.
        assert!(
            matches!(app.modal, Some(crate::modal::Modal::BlockTemplatePicker(_))),
            "slash in prose must open the block-template picker"
        );
        assert_eq!(app.vim.mode, Mode::Modal);

        // `/` must have landed in the rope.
        let serialized = app.document().unwrap().to_markdown();
        assert!(
            serialized.starts_with("hello/"),
            "slash must be inserted literally; got {serialized:?}"
        );

        // last_edit must be set so the autosave debounce can see it.
        assert!(
            app.last_edit.is_some(),
            "slash must record last_edit so autosave fires"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn slash_in_block_inserts_char_without_opening_picker() {
        // A `/` inside a block (e.g. typing a URL path) must be a
        // literal insert. Opening the picker here would be hostile —
        // every URL has slashes.
        let md = "```http alias=req1\nGET https://example.com\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        // `pad_with_prose` inserts an empty Prose before the first
        // block, so the block lives at segment index 1, not 0. Offset
        // 25 is inside the body line "GET https://example.com".
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: 1,
            offset: 25,
        });
        let before = app.document().unwrap().to_markdown();
        let slashes_before = before.matches('/').count();

        apply_slash_key(&mut app);

        assert!(
            !matches!(app.modal, Some(crate::modal::Modal::BlockTemplatePicker(_))),
            "slash inside a block must NOT open the picker"
        );
        // Insertion still happened — the block's raw rope grew by one
        // char.
        let after = app.document().unwrap().to_markdown();
        let slashes_after = after.matches('/').count();
        assert_eq!(
            slashes_after,
            slashes_before + 1,
            "exactly one extra `/` must land in the block body; before={before:?} after={after:?}"
        );
        assert!(app.last_edit.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn slash_in_block_result_does_not_open_picker() {
        // Block results are read-only (`insert_char_at_cursor`
        // returns silently). The slash applier must mirror that — no
        // picker open. `last_edit` *does* get set (the applier is
        // uniform; the no-op is at the buffer layer).
        let md = "```http alias=req1\nGET https://example.com\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InBlockResult {
                segment_idx: 1,
                row: 0,
            });

        let before = app.document().unwrap().to_markdown();
        apply_slash_key(&mut app);
        let after = app.document().unwrap().to_markdown();

        assert_eq!(
            before, after,
            "slash in block result must not mutate the document"
        );
        assert!(
            !matches!(app.modal, Some(crate::modal::Modal::BlockTemplatePicker(_))),
            "slash in block result must NOT open the picker"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn slash_in_prose_snapshots_for_undo() {
        // The snapshot taken in `apply_slash_key` must let undo rewind
        // past the slash exactly. Confidence test: place cursor, fire
        // slash, then undo and check the doc matches the pre-slash
        // state.
        let (mut app, _d, _v) = app_with("hello\n").await;
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 5,
        });
        let before = app.document().unwrap().to_markdown();

        apply_slash_key(&mut app);
        let after_slash = app.document().unwrap().to_markdown();
        assert_ne!(before, after_slash, "slash must change the buffer");

        // Close the picker so undo isn't gated by modal state.
        app.modal = None;
        app.vim.enter_normal();

        // Undo via the same public path `Action::Undo` uses.
        let did_undo = app.document_mut().unwrap().undo();
        assert!(did_undo, "the snapshot must produce a usable undo step");
        let after_undo = app.document().unwrap().to_markdown();
        assert_eq!(
            after_undo, before,
            "undo after slash must restore the pre-slash document"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn slash_with_no_active_document_is_inert() {
        // App without an open tab — no document. The applier must
        // not panic, must not set last_edit (no edit happened), and
        // must not open the picker (nothing to splice into).
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut cfg = Config::default();
        cfg.editor.mode = EditorMode::Standard;
        let mut app = App::new(cfg, resolved, pool);
        // Intentionally do NOT open any document.

        apply_slash_key(&mut app);
        assert!(!matches!(app.modal, Some(crate::modal::Modal::BlockTemplatePicker(_))));
        assert!(
            app.last_edit.is_none(),
            "no doc → no edit clock; the applier must bail before setting last_edit"
        );
    }
}
