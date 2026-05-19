//! Standard-mode (non-modal) selection + system-clipboard handlers.
//!
//! Introduced by tui-V1 / fase 3. The conventional editor profile has
//! no `Mode::Visual`; `Shift`+arrow extends a selection anchored at
//! `App.standard.anchor`, and `Ctrl+C/X/V` copy / cut / paste through
//! the OS clipboard. None of this touches the vim engine — the vim
//! path never decodes into these `Action`s, so Cenário 2 stays
//! byte-identical. All logic lives here (a fresh, non-`coverage:
//! exclude` module) instead of the legacy `apply/operator.rs` /
//! `apply/misc.rs`, so TD7 (≥80% on new code, no exclude) holds.
//!
//! Scope: **same prose segment only**. A selection whose anchor and
//! cursor land in different segments (prose ↔ block, etc.) is a
//! graceful no-op for the cut/copy/replace path — exactly the way the
//! vim operator engine refuses cross-segment ranges. Cross-segment
//! selection is explicitly out of fase-3 scope.

use crate::app::App;
use crate::buffer::Cursor;
use crate::clipboard::SystemClipboard;
use crate::input::action::Action;

/// The active Standard selection as a same-segment prose char range,
/// or `None` when there's no selection / it's cross-segment / not
/// prose. Returned as `(segment_idx, lo, hi)` with `lo <= hi` — a
/// half-open `[lo, hi)` range over the segment's chars (conventional
/// editor semantics: the anchor and the caret bound the run, the
/// char *under* the caret is not included).
fn selection_range(app: &App, anchor: Cursor) -> Option<(usize, usize, usize)> {
    let cursor = app.document()?.cursor();
    let (
        Cursor::InProse {
            segment_idx: a_seg,
            offset: a_off,
        },
        Cursor::InProse {
            segment_idx: c_seg,
            offset: c_off,
        },
    ) = (anchor, cursor)
    else {
        // Block / result cursors, or anchor in one kind & cursor in
        // another → refuse (no-op), like the vim engine does.
        return None;
    };
    if a_seg != c_seg || a_off == c_off {
        // Cross-segment, or an empty selection (anchor == caret).
        return None;
    }
    let (lo, hi) = (a_off.min(c_off), a_off.max(c_off));
    Some((a_seg, lo, hi))
}

/// Entry point routed from both `route_standard` (the live path) and
/// the exhaustive `apply_action` match (formal closure — these
/// variants are intercepted by `route_standard` before `apply_action`
/// in production, so the router arm only exists to keep the match
/// total). `clip` is the injected clipboard seam so unit tests run
/// without a display server.
pub(crate) fn apply_standard_sel(app: &mut App, action: Action, clip: &mut impl SystemClipboard) {
    match action {
        Action::SelectExtend(m) => extend_selection(app, m),
        Action::ClearSelection => app.standard.anchor = None,
        Action::Copy => copy_selection(app, clip),
        Action::Cut => cut_selection(app, clip),
        Action::PasteSystem => paste_system(app, clip),
        _ => unreachable!("apply_standard_sel: variante fora do grupo"),
    }
}

/// `Shift`+<motion>: seed the anchor from the current caret on the
/// first one, then move the caret by the motion (the moving end of
/// the selection). The actual cursor move reuses the existing
/// `Motion` apply path so motion semantics stay identical to a plain
/// arrow.
fn extend_selection(app: &mut App, motion: crate::input::types::Motion) {
    if app.standard.anchor.is_none() {
        if let Some(doc) = app.document() {
            app.standard.anchor = Some(doc.cursor());
        }
    }
    // Delegate the actual caret move to the shared Motion path so the
    // moving end behaves exactly like a bare arrow would.
    crate::input::dispatch::apply_action(
        app,
        Action::Motion(motion, 1),
        /* recording = */ true,
    );
}

/// `Ctrl+C`: copy the selection to the OS clipboard. The document is
/// never mutated. No-op (and no clipboard write) when there's no
/// usable selection.
fn copy_selection(app: &mut App, clip: &mut impl SystemClipboard) {
    let Some(anchor) = app.standard.anchor else {
        return;
    };
    let Some((seg, lo, hi)) = selection_range(app, anchor) else {
        return;
    };
    let text = match app.document() {
        Some(doc) => doc.text_in_segment_range(seg, lo, hi),
        None => return,
    };
    if let Err(msg) = clip.set(&text) {
        app.set_status(crate::app::StatusKind::Error, &msg);
    }
}

/// `Ctrl+X`: copy the selection out, delete it from the doc, collapse
/// the caret + anchor to the range start. Snapshots first so a single
/// `undo` (vim) / future Ctrl+Z restores it.
fn cut_selection(app: &mut App, clip: &mut impl SystemClipboard) {
    let Some(anchor) = app.standard.anchor else {
        return;
    };
    let Some((seg, lo, hi)) = selection_range(app, anchor) else {
        return;
    };
    let text = match app.document() {
        Some(doc) => doc.text_in_segment_range(seg, lo, hi),
        None => return,
    };
    if let Err(msg) = clip.set(&text) {
        // Clipboard failed — don't destroy the user's text just
        // because the OS clipboard is unavailable.
        app.set_status(crate::app::StatusKind::Error, &msg);
        return;
    }
    if let Some(doc) = app.document_mut() {
        doc.snapshot();
        doc.delete_range_in_segment(seg, lo, hi);
        doc.set_cursor(Cursor::InProse {
            segment_idx: seg,
            offset: lo,
        });
    }
    app.standard.anchor = None;
    app.refresh_viewport_for_cursor();
}

/// `Ctrl+V`: paste the OS clipboard at the caret. If a selection is
/// active it's replaced (delete range, then insert at the range
/// start). Snapshots first. Empty clipboard → no-op.
fn paste_system(app: &mut App, clip: &mut impl SystemClipboard) {
    let text = match clip.get() {
        Ok(t) => t,
        Err(msg) => {
            app.set_status(crate::app::StatusKind::Error, &msg);
            return;
        }
    };
    if text.is_empty() {
        return;
    }

    // Replace an active same-segment selection, else insert at caret.
    let range = app.standard.anchor.and_then(|a| selection_range(app, a));

    if let Some((seg, lo, hi)) = range {
        if let Some(doc) = app.document_mut() {
            doc.snapshot();
            doc.delete_range_in_segment(seg, lo, hi);
            let n = doc.insert_text_in_segment(seg, lo, &text);
            doc.set_cursor(Cursor::InProse {
                segment_idx: seg,
                offset: lo + n,
            });
        }
        app.standard.anchor = None;
        app.refresh_viewport_for_cursor();
        return;
    }

    // No selection — insert at the caret (prose only; block / result
    // carets are a graceful no-op, matching the rest of fase 3).
    let Some(Cursor::InProse {
        segment_idx,
        offset,
    }) = app.document().map(|d| d.cursor())
    else {
        return;
    };
    if let Some(doc) = app.document_mut() {
        doc.snapshot();
        let n = doc.insert_text_in_segment(segment_idx, offset, &text);
        doc.set_cursor(Cursor::InProse {
            segment_idx,
            offset: offset + n,
        });
    }
    app.refresh_viewport_for_cursor();
}

/// The Standard-mode selection anchor to paint, or `None` when no
/// selection should be drawn. Pure + owned by the selection module
/// (kept out of the `ui/mod.rs` legacy render monolith on purpose —
/// that file is a pre-existing size/coverage debt and fase 3 must
/// not grow it). The selection is always charwise in the non-modal
/// model, so the renderer pairs this anchor with `linewise = false`.
/// The moving end is the doc cursor, exactly like the vim overlay.
///
/// Wired into `ui/render_root.rs` (one call site) as the fallback arm
/// of the visual-overlay match, alongside the existing vim-overlay
/// arms; the vim arms are deliberately NOT folded in here so Cenário 2
/// stays byte-identical and this helper keeps a single responsibility.
pub(crate) fn standard_overlay_anchor(
    editor_mode: crate::config::EditorMode,
    standard_anchor: Option<Cursor>,
) -> Option<Cursor> {
    if editor_mode == crate::config::EditorMode::Standard {
        standard_anchor
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::FakeClipboard;
    use crate::config::{Config, EditorMode};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_with(text: &str) -> (App, TempDir, TempDir) {
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

    fn prose(app: &App) -> (usize, usize) {
        match app.document().unwrap().cursor() {
            Cursor::InProse {
                segment_idx,
                offset,
            } => (segment_idx, offset),
            other => panic!("expected prose cursor, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn extend_selection_seeds_anchor_then_moves_caret() {
        let (mut app, _d, _v) = app_with("abcdef\n").await;
        let (seg, off0) = prose(&app);
        assert!(app.standard.anchor.is_none());
        extend_selection(&mut app, crate::input::types::Motion::Right);
        assert_eq!(
            app.standard.anchor,
            Some(Cursor::InProse {
                segment_idx: seg,
                offset: off0
            }),
            "first Shift+arrow seeds the anchor at the pre-move caret"
        );
        let (_, off1) = prose(&app);
        assert_ne!(off0, off1, "caret (moving end) advanced");
        // A second extend keeps the original anchor.
        extend_selection(&mut app, crate::input::types::Motion::Right);
        assert_eq!(
            app.standard.anchor,
            Some(Cursor::InProse {
                segment_idx: seg,
                offset: off0
            }),
            "anchor is sticky across further extends"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn clear_selection_drops_anchor() {
        let (mut app, _d, _v) = app_with("abc\n").await;
        app.standard.anchor = Some(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        let mut clip = FakeClipboard::default();
        apply_standard_sel(&mut app, Action::ClearSelection, &mut clip);
        assert!(app.standard.anchor.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copy_puts_selection_on_clipboard_without_mutating_doc() {
        let (mut app, _d, _v) = app_with("hello world\n").await;
        let (seg, _) = prose(&app);
        // Select "hello" (offsets 0..5).
        app.standard.anchor = Some(Cursor::InProse {
            segment_idx: seg,
            offset: 0,
        });
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: seg,
            offset: 5,
        });
        let before = app.document().unwrap().to_markdown();
        let mut clip = FakeClipboard::default();
        apply_standard_sel(&mut app, Action::Copy, &mut clip);
        assert_eq!(clip.get(), Ok("hello".to_string()));
        assert_eq!(
            app.document().unwrap().to_markdown(),
            before,
            "copy must not mutate the doc"
        );
        assert!(
            app.standard.anchor.is_some(),
            "copy keeps the selection (matches every editor)"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copy_without_selection_is_a_noop() {
        let (mut app, _d, _v) = app_with("abc\n").await;
        let mut clip = FakeClipboard::with("sentinel");
        apply_standard_sel(&mut app, Action::Copy, &mut clip);
        assert_eq!(
            clip.get(),
            Ok("sentinel".to_string()),
            "no selection → clipboard untouched"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cut_removes_selection_and_fills_clipboard() {
        let (mut app, _d, _v) = app_with("hello world\n").await;
        let (seg, _) = prose(&app);
        app.standard.anchor = Some(Cursor::InProse {
            segment_idx: seg,
            offset: 0,
        });
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: seg,
            offset: 6,
        });
        let mut clip = FakeClipboard::default();
        apply_standard_sel(&mut app, Action::Cut, &mut clip);
        assert_eq!(clip.get(), Ok("hello ".to_string()));
        assert!(
            app.document().unwrap().to_markdown().starts_with("world"),
            "cut removed the range from the doc: {:?}",
            app.document().unwrap().to_markdown()
        );
        assert!(app.standard.anchor.is_none(), "cut collapses the anchor");
        assert_eq!(prose(&app), (seg, 0), "caret collapses to the range start");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cut_without_selection_is_a_noop() {
        let (mut app, _d, _v) = app_with("abc\n").await;
        let before = app.document().unwrap().to_markdown();
        let mut clip = FakeClipboard::default();
        apply_standard_sel(&mut app, Action::Cut, &mut clip);
        assert_eq!(app.document().unwrap().to_markdown(), before);
        assert_eq!(clip.get(), Ok(String::new()));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn paste_at_caret_inserts_clipboard_text() {
        let (mut app, _d, _v) = app_with("abc\n").await;
        let (seg, _) = prose(&app);
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: seg,
            offset: 1,
        });
        let mut clip = FakeClipboard::with("XY");
        apply_standard_sel(&mut app, Action::PasteSystem, &mut clip);
        assert!(
            app.document().unwrap().to_markdown().starts_with("aXYbc"),
            "paste inserted at the caret: {:?}",
            app.document().unwrap().to_markdown()
        );
        assert_eq!(prose(&app), (seg, 3), "caret lands after the paste");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn paste_over_selection_replaces_it() {
        let (mut app, _d, _v) = app_with("hello world\n").await;
        let (seg, _) = prose(&app);
        // Select "hello".
        app.standard.anchor = Some(Cursor::InProse {
            segment_idx: seg,
            offset: 0,
        });
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: seg,
            offset: 5,
        });
        let mut clip = FakeClipboard::with("HI");
        apply_standard_sel(&mut app, Action::PasteSystem, &mut clip);
        assert!(
            app.document()
                .unwrap()
                .to_markdown()
                .starts_with("HI world"),
            "selection replaced by the paste: {:?}",
            app.document().unwrap().to_markdown()
        );
        assert!(
            app.standard.anchor.is_none(),
            "anchor collapses after replace"
        );
        assert_eq!(prose(&app), (seg, 2), "caret after the inserted text");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn paste_empty_clipboard_is_a_noop() {
        let (mut app, _d, _v) = app_with("abc\n").await;
        let before = app.document().unwrap().to_markdown();
        let mut clip = FakeClipboard::default();
        apply_standard_sel(&mut app, Action::PasteSystem, &mut clip);
        assert_eq!(app.document().unwrap().to_markdown(), before);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn copy_cut_cross_segment_is_a_graceful_noop() {
        // anchor in segment 0, caret forced to a different segment
        // index → selection_range refuses → no clipboard write, no
        // mutation. (We fake the cross-segment by pointing the anchor
        // at a non-existent second segment idx; selection_range only
        // checks the indices differ.)
        let (mut app, _d, _v) = app_with("abc\n").await;
        app.standard.anchor = Some(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: 9,
            offset: 1,
        });
        let before = app.document().unwrap().to_markdown();
        let mut clip = FakeClipboard::with("keep");
        apply_standard_sel(&mut app, Action::Copy, &mut clip);
        apply_standard_sel(&mut app, Action::Cut, &mut clip);
        assert_eq!(clip.get(), Ok("keep".to_string()));
        assert_eq!(app.document().unwrap().to_markdown(), before);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn roteiro_mirror_copy_move_paste_reproduces_text() {
        // Cenário 1 passos 5-6: Shift-select → Copy → move caret →
        // Paste reproduces the copied run.
        let (mut app, _d, _v) = app_with("abcdef\n").await;
        let (seg, _) = prose(&app);
        // Shift-select "abc" via two extends from offset 0.
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: seg,
            offset: 0,
        });
        let mut clip = FakeClipboard::default();
        apply_standard_sel(
            &mut app,
            Action::SelectExtend(crate::input::types::Motion::Right),
            &mut clip,
        );
        apply_standard_sel(
            &mut app,
            Action::SelectExtend(crate::input::types::Motion::Right),
            &mut clip,
        );
        apply_standard_sel(
            &mut app,
            Action::SelectExtend(crate::input::types::Motion::Right),
            &mut clip,
        );
        apply_standard_sel(&mut app, Action::Copy, &mut clip);
        assert_eq!(clip.get(), Ok("abc".to_string()));
        // Move caret to end of line, paste.
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: seg,
            offset: 6,
        });
        app.standard.anchor = None;
        apply_standard_sel(&mut app, Action::PasteSystem, &mut clip);
        assert!(
            app.document()
                .unwrap()
                .to_markdown()
                .starts_with("abcdefabc"),
            "paste reproduced the copied run: {:?}",
            app.document().unwrap().to_markdown()
        );
    }

    #[test]
    fn standard_overlay_anchor_only_paints_in_standard_with_anchor() {
        use crate::config::EditorMode;
        let a = Cursor::InProse {
            segment_idx: 0,
            offset: 4,
        };
        // Standard + anchor → paint it.
        assert_eq!(
            standard_overlay_anchor(EditorMode::Standard, Some(a)),
            Some(a)
        );
        // Standard, no anchor → nothing.
        assert_eq!(standard_overlay_anchor(EditorMode::Standard, None), None);
        // Vim profile → never paint a Standard anchor (the vim path
        // owns the overlay; Cenário 2 stays byte-identical).
        assert_eq!(standard_overlay_anchor(EditorMode::Vim, Some(a)), None);
        assert_eq!(standard_overlay_anchor(EditorMode::Vim, None), None);
    }
}
