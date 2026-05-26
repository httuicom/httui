//! Undo-group snapshot policy for the Standard (non-modal) editor
//! profile (tui-V1 / fase 4 p2).
//!
//! The vim engine takes a `doc.snapshot()` once per command (`i`,
//! `dw`, …) so one `u` undoes one logical edit. The Standard profile
//! has no commands — every printable char is its own
//! `Action::InsertChar`. Snapshotting on every keystroke would make
//! `Ctrl+Z` peel one character at a time, which nobody wants.
//!
//! `maybe_snapshot` runs once per keystroke (called from
//! `route_standard` before the action is dispatched) and decides
//! whether *this* edit opens a new undo group (→ take a snapshot) or
//! coalesces into the one in progress (→ skip the snapshot). The rule:
//!
//! - **Typing** coalesces run-of-the-mill characters into one group,
//!   but breaks the group at a word boundary (a whitespace char ends
//!   the current word's group) so undo rewinds word-by-word, the
//!   familiar editor feel.
//! - **Deleting** (Backspace / Delete) coalesces consecutive deletes
//!   into one group, separate from the typing group.
//! - **Newline** always its own snapshot (and resets the group) — a
//!   line break is a natural undo checkpoint.
//! - **Any non-textual action** (motion, selection, clipboard, save,
//!   undo/redo itself) resets the group without snapshotting, so the
//!   next edit starts a fresh group. Cut/Paste already take their own
//!   snapshot in `standard_sel` (fase 3) — this module deliberately
//!   does NOT snapshot for them (no double snapshot).
//!
//! ## Heuristic for the word boundary
//!
//! Tracking "the char before the caret" precisely would mean reaching
//! into the rope on every keystroke. We use the simpler, fully
//! deterministic rule documented above: while a typing group is open,
//! a `c.is_whitespace()` char closes that group (it snapshots, opening
//! the whitespace into a fresh group). So `"hello world"` undoes as
//! `" world"` then `"hello"`. Punctuation is treated as ordinary text
//! (kept in scope-small for V1).

use crate::app::App;
use crate::input::action::Action;

/// Which textual edit is being coalesced into the current undo group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditGroupKind {
    /// A run of inserted characters (broken at word boundaries).
    Insert,
    /// A run of Backspace / Delete.
    Delete,
}

/// Decide whether `action` should open a new undo group (take a
/// `doc.snapshot()`) or coalesce into the group in progress. Mutates
/// `app.standard.edit_group` to reflect the new grouping state.
///
/// Call once per keystroke in `route_standard`, *before* the action is
/// dispatched, so the snapshot captures the pre-edit document.
pub(crate) fn maybe_snapshot(app: &mut App, action: &Action) {
    match action {
        Action::InsertChar(c) => {
            let group = app.standard.edit_group;
            // Open a fresh group when: not already inside an Insert
            // group, OR this char is whitespace and the previous one
            // belonged to a word (the word→space boundary). Either way
            // a whitespace char that *starts* the file/group also
            // snapshots — it's a new Insert group.
            let boundary = group == Some(EditGroupKind::Insert) && c.is_whitespace();
            if group != Some(EditGroupKind::Insert) || boundary {
                if let Some(doc) = app.document_mut() {
                    doc.snapshot();
                }
                app.standard.edit_group = Some(EditGroupKind::Insert);
            }
            // else: coalesce — same group, no snapshot.
        }
        Action::DeleteBackward | Action::DeleteBackwardStandard | Action::DeleteForward => {
            // tui-V2 vertical 2 / cenário 4: `DeleteBackwardStandard`
            // shares the Delete undo group with the legacy variants so
            // a run of Backspaces (including ones that cross segment
            // boundaries via `apply::standard_delete`) coalesces into
            // one undo step.
            if app.standard.edit_group != Some(EditGroupKind::Delete) {
                if let Some(doc) = app.document_mut() {
                    doc.snapshot();
                }
                app.standard.edit_group = Some(EditGroupKind::Delete);
            }
        }
        Action::InsertNewline => {
            if let Some(doc) = app.document_mut() {
                doc.snapshot();
            }
            app.standard.edit_group = None;
        }
        // Any non-textual action ends the current group without
        // snapshotting. Cut/Paste already snapshot in standard_sel —
        // not snapshotting here avoids a duplicate. Undo/Redo must
        // also reset so the next edit opens a clean group.
        _ => {
            app.standard.edit_group = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, EditorMode};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_with_note() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), "abc\n").unwrap();
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
    async fn first_insert_opens_a_group_and_snapshots() {
        let (mut app, _d, _v) = app_with_note().await;
        assert_eq!(app.standard.edit_group, None);
        maybe_snapshot(&mut app, &Action::InsertChar('a'));
        assert_eq!(app.standard.edit_group, Some(EditGroupKind::Insert));
        assert!(app.document().unwrap().can_undo(), "first insert snapshots");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn consecutive_word_chars_coalesce_into_one_snapshot() {
        let (mut app, _d, _v) = app_with_note().await;
        maybe_snapshot(&mut app, &Action::InsertChar('a'));
        // No real doc edit here; snapshot count is what matters. Take
        // a baseline then keep "typing" word chars — no new snapshot.
        let after_first = app.document().unwrap().can_undo();
        assert!(after_first);
        maybe_snapshot(&mut app, &Action::InsertChar('b'));
        maybe_snapshot(&mut app, &Action::InsertChar('c'));
        assert_eq!(app.standard.edit_group, Some(EditGroupKind::Insert));
        // Undo once → back to the single captured snapshot (empty
        // redo-less stack means a second undo is a no-op).
        assert!(app.document_mut().unwrap().undo());
        assert!(
            !app.document().unwrap().can_undo(),
            "only one snapshot was taken for the whole word"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn whitespace_breaks_the_insert_group() {
        let (mut app, _d, _v) = app_with_note().await;
        maybe_snapshot(&mut app, &Action::InsertChar('h'));
        maybe_snapshot(&mut app, &Action::InsertChar('i'));
        // Space at a word boundary → new snapshot, fresh group.
        maybe_snapshot(&mut app, &Action::InsertChar(' '));
        assert_eq!(app.standard.edit_group, Some(EditGroupKind::Insert));
        // Two snapshots now (the word, then the space onward).
        assert!(app.document_mut().unwrap().undo());
        assert!(
            app.document().unwrap().can_undo(),
            "the boundary added a second snapshot"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_run_coalesces_separately_from_insert() {
        let (mut app, _d, _v) = app_with_note().await;
        maybe_snapshot(&mut app, &Action::InsertChar('x'));
        maybe_snapshot(&mut app, &Action::DeleteBackward);
        assert_eq!(app.standard.edit_group, Some(EditGroupKind::Delete));
        maybe_snapshot(&mut app, &Action::DeleteForward);
        // Still one delete group → exactly two snapshots total
        // (insert group + delete group).
        assert!(app.document_mut().unwrap().undo());
        assert!(app.document_mut().unwrap().undo());
        assert!(
            !app.document().unwrap().can_undo(),
            "insert + delete = exactly two groups"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn newline_always_snapshots_and_resets_group() {
        let (mut app, _d, _v) = app_with_note().await;
        maybe_snapshot(&mut app, &Action::InsertChar('a'));
        maybe_snapshot(&mut app, &Action::InsertNewline);
        assert_eq!(
            app.standard.edit_group, None,
            "newline resets the group so the next edit starts fresh"
        );
        assert!(app.document_mut().unwrap().undo());
        assert!(
            app.document().unwrap().can_undo(),
            "newline added its own snapshot on top of the insert one"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn non_textual_action_resets_group_without_snapshot() {
        let (mut app, _d, _v) = app_with_note().await;
        maybe_snapshot(&mut app, &Action::InsertChar('a'));
        let could_undo = app.document().unwrap().can_undo();
        assert!(could_undo);
        // A motion ends the group but takes no snapshot.
        maybe_snapshot(
            &mut app,
            &Action::Motion(crate::input::types::Motion::Right, 1),
        );
        assert_eq!(app.standard.edit_group, None);
        assert!(app.document_mut().unwrap().undo());
        assert!(
            !app.document().unwrap().can_undo(),
            "motion added no extra snapshot"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cut_paste_actions_do_not_double_snapshot() {
        let (mut app, _d, _v) = app_with_note().await;
        // Cut/Paste already snapshot in standard_sel — maybe_snapshot
        // must only reset the group, never add a snapshot.
        maybe_snapshot(&mut app, &Action::Cut);
        assert_eq!(app.standard.edit_group, None);
        assert!(
            !app.document().unwrap().can_undo(),
            "Cut takes no snapshot here (standard_sel owns it)"
        );
        maybe_snapshot(&mut app, &Action::PasteSystem);
        assert!(!app.document().unwrap().can_undo());
        maybe_snapshot(&mut app, &Action::Undo);
        assert_eq!(app.standard.edit_group, None);
        assert!(!app.document().unwrap().can_undo());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_then_insert_opens_a_new_group() {
        let (mut app, _d, _v) = app_with_note().await;
        maybe_snapshot(&mut app, &Action::DeleteBackward);
        assert_eq!(app.standard.edit_group, Some(EditGroupKind::Delete));
        maybe_snapshot(&mut app, &Action::InsertChar('z'));
        assert_eq!(
            app.standard.edit_group,
            Some(EditGroupKind::Insert),
            "switching delete→insert opens a fresh group"
        );
        // Two snapshots: the delete group and the new insert group.
        assert!(app.document_mut().unwrap().undo());
        assert!(app.document_mut().unwrap().undo());
        assert!(!app.document().unwrap().can_undo());
    }
}
