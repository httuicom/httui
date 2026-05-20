//! Standard-mode Backspace applier with segment-boundary semantics
//! (tui-V2 / vertical 2 / cenário 4).
//!
//! Fresh module created by V2, NOT `coverage:exclude` (same policy as
//! V1's `standard_sel` / `standard_undo` and V2's `slash`). Owns its
//! own test surface.
//!
//! ## Why this exists
//!
//! `Document::delete_char_before_cursor` (in the legacy buffer module,
//! 1483 L and `coverage:exclude`-free but well over the 600-line size
//! gate so V2 must not touch it) returns a no-op when the cursor sits
//! at `offset == 0` of any segment. That makes the segment boundary
//! impenetrable: a user who Backspaces at the start of a block body
//! sees nothing happen.
//!
//! Owner decision recorded at /tui-start (2026-05-19): "tudo é texto,
//! vai ter algo antes" — the buffer should behave like a flat rope at
//! boundaries. Backspace at `offset == 0` walks into the previous
//! segment and deletes its last char from there.
//!
//! ## What the applier does
//!
//! Decoded by `input::standard::resolve` (the resolver rewrites the
//! generic `DeleteBackward` to `DeleteBackwardStandard` only on the
//! Standard profile; vim's path is unchanged). The flow:
//!
//! - **`InBlockResult`** — no-op. Result tables are read-only.
//! - **`offset > 0`** — delegate to the legacy in-segment delete via
//!   `Document::delete_char_before_cursor`. This is the common-case
//!   path; the boundary logic only kicks in at the segment edge.
//! - **`offset == 0` at start of document** — no-op (nothing before).
//! - **`offset == 0`, prev is empty Prose (padding)** — slide the
//!   cursor into the empty prose. No char to delete; the next
//!   Backspace from there walks past the padding to whatever lies
//!   beyond (or stops if it's start-of-doc).
//! - **`offset == 0`, prev is non-empty Prose** — move into the prev
//!   prose at its end, then delete one char (the legacy in-segment
//!   path takes it from here once cursor is at `offset > 0`).
//! - **`offset == 0`, prev is Block** — splice the block's `raw` rope
//!   (it includes the fence header, body, and closer): drop the last
//!   char, re-parse. If the re-parse still yields a single valid
//!   executable block, keep it as a `Block` and place the cursor at
//!   the new end of the raw. If the re-parse fails (the fence broke),
//!   demote the segment to `Prose` so the renderer shows the text
//!   instead of a stale block widget — matching the owner's
//!   "vai mostrar o texto ao invés de mostrar o bloco" expectation.
//!
//! ## Why undo / autosave taps live here too
//!
//! `route::route_standard` runs `maybe_snapshot` and the `last_edit`
//! clock for the original `DeleteBackward` variant. The new variant
//! `DeleteBackwardStandard` was added to `standard_undo::maybe_snapshot`'s
//! Delete arm so coalescing still works. But `route::route_standard`'s
//! own `last_edit` tap matches the original variant by name and would
//! force a touch of the 904-line `route.rs` to recognise the new one;
//! the applier sets `app.last_edit` inline instead, keeping the V2
//! change strictly additive (same trade-off `slash::apply_slash_key`
//! made).

use ropey::Rope;

use crate::app::App;
use crate::buffer::{Cursor, Segment};

/// Apply `Action::DeleteBackwardStandard`. See module doc for the
/// per-case behavior and the rationale.
pub fn apply_delete_backward_standard(app: &mut App) {
    let Some(doc) = app.document_mut() else {
        return;
    };

    let cursor = doc.cursor();
    let segment_idx = cursor.segment_idx();

    let offset = match cursor {
        Cursor::InProse { offset, .. } => offset,
        Cursor::InBlock { offset, .. } => offset,
        // Block-result rows are read-only — mirror
        // `delete_char_before_cursor`'s arm.
        Cursor::InBlockResult { .. } => return,
    };

    if offset > 0 {
        // Plain in-segment delete. The legacy path handles the
        // rope-edit + cursor-move + reparse-from-raw correctly. We
        // didn't need to clone it here.
        doc.delete_char_before_cursor();
        app.last_edit = Some(std::time::Instant::now());
        return;
    }

    // offset == 0 — the boundary case. If we're already at the very
    // first segment there's nothing earlier to consume; bail without
    // touching `last_edit` (no edit happened, so the autosave debounce
    // shouldn't reset).
    if segment_idx == 0 {
        return;
    }

    let prev_idx = segment_idx - 1;

    // Read enough of the previous segment to decide the case. The
    // borrow drops before we mutate so we can swap segments freely
    // below.
    let prev_kind = match doc.segments().get(prev_idx) {
        Some(Segment::Prose(rope)) => PrevKind::Prose {
            len: rope.len_chars(),
        },
        Some(Segment::Block(_)) => PrevKind::Block,
        _ => return,
    };

    match prev_kind {
        PrevKind::Prose { len: 0 } => {
            // Empty prose (typically padding). Slide cursor into it;
            // there's no char to delete here. Next Backspace will pop
            // back to whatever sits behind the padding (or bail if
            // it's start-of-doc).
            doc.set_cursor(Cursor::InProse {
                segment_idx: prev_idx,
                offset: 0,
            });
            // No actual delete → don't bump the clock. Matches "movement
            // doesn't push autosave" semantics from the V1 tap.
        }
        PrevKind::Prose { len } => {
            // Non-empty prose. Hand off to the legacy delete by
            // first parking the cursor at the end of the prev prose.
            // The in-segment arm of `delete_char_before_cursor` then
            // removes the last char and lands `offset = len - 1`.
            doc.set_cursor(Cursor::InProse {
                segment_idx: prev_idx,
                offset: len,
            });
            doc.delete_char_before_cursor();
            app.last_edit = Some(std::time::Instant::now());
        }
        PrevKind::Block => {
            // Block. Splice the last char out of `block.raw`, reparse,
            // then either keep it as Block (with cursor at new end of
            // raw) or demote to Prose when the fence stopped parsing.
            let demoted_text = {
                let Some(b) = doc.block_at_mut(prev_idx) else {
                    return;
                };
                let raw_len = b.raw.len_chars();
                if raw_len == 0 {
                    return;
                }
                b.raw.remove((raw_len - 1)..raw_len);
                let still_block = b.reparse_from_raw();
                // Snapshot the raw text now so we can drop the borrow
                // before calling back into Document for `replace_segment`.
                // Only needed in the demote case (`still_block == false`).
                if !still_block {
                    Some(b.raw.to_string())
                } else {
                    None
                }
            };

            if let Some(text) = demoted_text {
                doc.replace_segment(prev_idx, Segment::Prose(Rope::from_str(&text)));
            }

            // Place cursor at the new end of whatever the segment is
            // post-edit. The segment kind picks the matching cursor
            // variant: Prose if we demoted, Block otherwise.
            let new_cursor = match doc.segments().get(prev_idx) {
                Some(Segment::Prose(r)) => Cursor::InProse {
                    segment_idx: prev_idx,
                    offset: r.len_chars(),
                },
                Some(Segment::Block(b)) => Cursor::InBlock {
                    segment_idx: prev_idx,
                    offset: b.raw.len_chars(),
                },
                // Shouldn't happen (we just edited prev_idx) — keep the
                // cursor at the start as a defensive fallback.
                _ => Cursor::InProse {
                    segment_idx: prev_idx,
                    offset: 0,
                },
            };
            doc.set_cursor(new_cursor);
            doc.mark_dirty();
            app.last_edit = Some(std::time::Instant::now());
        }
    }
}

/// Discriminator returned by the read-only inspection of the previous
/// segment. Keeps the immutable borrow short.
enum PrevKind {
    Prose { len: usize },
    Block,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;
    use crate::config::{Config, EditorMode};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    /// Mirrors V1's standard_sel / V2's slash fixture — a real App on
    /// a tempdir vault with a single note pre-loaded by
    /// `load_initial_document`.
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

    fn block_segment_idx(doc: &Document) -> usize {
        doc.segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("fixture must contain a block")
    }

    // ───── In-segment deletes (offset > 0) ─────

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_in_prose_offset_gt_zero_removes_one_char() {
        // Plain Backspace away from any boundary: deletes one char,
        // updates last_edit. The applier delegates to the legacy
        // in-segment path; this test guards that the delegation works.
        let (mut app, _d, _v) = app_with("hello\n").await;
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 5,
        });

        apply_delete_backward_standard(&mut app);

        let serialized = app.document().unwrap().to_markdown();
        assert!(
            serialized.starts_with("hell"),
            "one char must be removed; got {serialized:?}"
        );
        assert!(app.last_edit.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_in_block_offset_gt_zero_removes_one_char() {
        // Backspace inside a block body — also delegates to the legacy
        // in-segment path. The block reparses on each edit (legacy
        // behavior); we just verify a char left the raw rope.
        let md = "```http alias=req1\nGET /api\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        let block_idx = block_segment_idx(app.document().unwrap());
        let raw_len = match app.document().unwrap().segments().get(block_idx).unwrap() {
            Segment::Block(b) => b.raw.len_chars(),
            _ => unreachable!(),
        };
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: raw_len.saturating_sub(2),
        });

        apply_delete_backward_standard(&mut app);

        // Raw shrank by one. We probe the doc's segments to avoid
        // re-asserting against the canonical serializer (which may
        // reformat).
        let new_len = match app.document().unwrap().segments().get(block_idx).unwrap() {
            Segment::Block(b) => b.raw.len_chars(),
            _ => unreachable!(),
        };
        assert_eq!(new_len, raw_len - 1, "block raw must shrink by one char");
        assert!(app.last_edit.is_some());
    }

    // ───── Boundary cases (offset == 0) ─────

    #[tokio::test(flavor = "multi_thread")]
    async fn boundary_at_start_of_doc_is_a_noop() {
        // Cursor at segment 0, offset 0, with no segment before. The
        // applier must bail without panicking and without touching
        // last_edit (no edit happened).
        let (mut app, _d, _v) = app_with("hello\n").await;
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });

        let before = app.document().unwrap().to_markdown();
        apply_delete_backward_standard(&mut app);
        let after = app.document().unwrap().to_markdown();

        assert_eq!(before, after, "start-of-doc Backspace must not mutate");
        assert!(
            app.last_edit.is_none(),
            "no edit → no clock; movement-only no-op"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn boundary_into_empty_prose_slides_cursor_without_deleting() {
        // Block at idx=1, empty prose padding at idx=0. Cursor in the
        // block at offset 0 (its very beginning, before the opening
        // fence char). Backspace lands the cursor in the empty prose
        // padding; no char is removed.
        let md = "```http alias=req1\nGET /api\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        let block_idx = block_segment_idx(app.document().unwrap());
        assert!(block_idx >= 1, "fixture must have a leading prose padding");
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: 0,
        });

        let before = app.document().unwrap().to_markdown();
        apply_delete_backward_standard(&mut app);
        let after = app.document().unwrap().to_markdown();

        assert_eq!(
            before, after,
            "boundary-into-empty-prose must be cursor-only"
        );
        assert!(
            matches!(
                app.document().unwrap().cursor(),
                Cursor::InProse { segment_idx, offset }
                if segment_idx == block_idx - 1 && offset == 0
            ),
            "cursor must have slid into the empty prose at idx-1"
        );
        assert!(
            app.last_edit.is_none(),
            "no actual delete → don't reset autosave clock"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn boundary_into_nonempty_prose_deletes_last_char() {
        // Doc with prose BETWEEN two blocks places a non-empty Prose
        // segment immediately before a Block. Cursor at offset 0 of
        // the second block exercises the
        // "InBlock offset == 0, prev = Prose(non-empty)" arm: applier
        // parks the cursor at the end of the prev prose, then the
        // legacy in-segment delete strips one char.
        let md = "```http alias=a\nGET /a\n```\nhello\n```http alias=b\nGET /b\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        let block_indices: Vec<usize> = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
            .collect();
        assert_eq!(block_indices.len(), 2, "fixture must have two blocks");
        let second_block_idx = block_indices[1];
        let prev_idx = second_block_idx - 1;
        let prose_text_before = match app.document().unwrap().segments().get(prev_idx).unwrap() {
            Segment::Prose(r) => r.to_string(),
            _ => panic!("prev of second block must be the inter-block prose"),
        };
        assert!(
            !prose_text_before.is_empty(),
            "the between-blocks prose must hold the 'hello' text"
        );
        let prose_len_before = prose_text_before.chars().count();

        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: second_block_idx,
            offset: 0,
        });

        apply_delete_backward_standard(&mut app);

        let prose_text_after = match app.document().unwrap().segments().get(prev_idx).unwrap() {
            Segment::Prose(r) => r.to_string(),
            _ => panic!("prev segment must still be Prose"),
        };
        assert_eq!(
            prose_text_after.chars().count(),
            prose_len_before - 1,
            "prev prose must shrink by one char"
        );
        assert!(
            matches!(
                app.document().unwrap().cursor(),
                Cursor::InProse { segment_idx, offset }
                if segment_idx == prev_idx && offset == prose_len_before - 1
            ),
            "cursor must land at the new end of the prev prose"
        );
        assert!(app.last_edit.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn boundary_into_block_with_valid_fence_keeps_block() {
        // Prose before a block, cursor at offset 0 of the (empty
        // padding) prose AFTER the block. Backspace consumes the
        // trailing `\n` of the block's raw. The fence is permissive
        // enough that the reparse still produces one block — so the
        // segment stays Block, just shorter by one char.
        let md = "```http alias=req1\nGET /api\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        let block_idx = block_segment_idx(app.document().unwrap());
        let after_block_idx = block_idx + 1;
        // Sanity: the trailing prose padding sits right after the block.
        assert!(
            matches!(
                app.document().unwrap().segments().get(after_block_idx),
                Some(Segment::Prose(_))
            ),
            "fixture must have a trailing prose padding"
        );
        let raw_len_before = match app.document().unwrap().segments().get(block_idx).unwrap() {
            Segment::Block(b) => b.raw.len_chars(),
            _ => unreachable!(),
        };
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: after_block_idx,
            offset: 0,
        });

        apply_delete_backward_standard(&mut app);

        // The block must still be a Block, raw len down by 1.
        let raw_len_after = match app.document().unwrap().segments().get(block_idx).unwrap() {
            Segment::Block(b) => b.raw.len_chars(),
            _ => panic!("block must survive — fence is still valid after one trailing-char delete"),
        };
        assert_eq!(raw_len_after, raw_len_before - 1);
        // Cursor should have hopped into the block at its new end.
        assert!(
            matches!(
                app.document().unwrap().cursor(),
                Cursor::InBlock { segment_idx, offset }
                if segment_idx == block_idx && offset == raw_len_after
            ),
            "cursor must land at the new end of the block raw"
        );
        assert!(app.last_edit.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn boundary_into_block_that_breaks_fence_demotes_to_prose() {
        // Manually corrupt the block raw to be one char away from a
        // broken fence, then Backspace from the trailing prose. The
        // resulting raw should fail `parse_blocks`, and the applier
        // must demote the Block segment to a Prose so the renderer
        // shows the text. Confirms the user's "vira prosa quando
        // quebra" expectation.
        let md = "```http alias=req1\nGET /api\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        let block_idx = block_segment_idx(app.document().unwrap());
        let after_block_idx = block_idx + 1;

        // Trim the block's raw down to just `"`abc` "` — three
        // backticks followed by `abc` — by deleting most of it. Use
        // `block_at_mut` to splice the raw directly.
        {
            let doc = app.document_mut().unwrap();
            let block = doc.block_at_mut(block_idx).unwrap();
            let new_raw = Rope::from_str("```a");
            block.raw = new_raw;
            block.reparse_from_raw();
        }
        // Now Backspace from the trailing prose padding. The applier
        // strips the last char from `"```a"` → `"```"`, parser sees
        // no info string after the fence, no block_type matches
        // EXECUTABLE_TYPES → parsed.len() == 0 → demote.
        app.document_mut().unwrap().set_cursor(Cursor::InProse {
            segment_idx: after_block_idx,
            offset: 0,
        });

        apply_delete_backward_standard(&mut app);

        // Segment at `block_idx` must now be Prose holding the
        // residual text.
        match app.document().unwrap().segments().get(block_idx).unwrap() {
            Segment::Prose(r) => {
                let text = r.to_string();
                assert_eq!(text, "```", "demoted prose must hold the residual raw text");
            }
            Segment::Block(_) => panic!("the broken-fence block must demote to Prose"),
        }
        assert!(app.last_edit.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_in_block_result_is_a_noop() {
        // Mirror the legacy applier's read-only handling of
        // InBlockResult: no edit, no clock tap.
        let md = "```http alias=req1\nGET /api\n```\n";
        let (mut app, _d, _v) = app_with(md).await;
        let block_idx = block_segment_idx(app.document().unwrap());
        app.document_mut()
            .unwrap()
            .set_cursor(Cursor::InBlockResult {
                segment_idx: block_idx,
                row: 0,
            });

        let before = app.document().unwrap().to_markdown();
        apply_delete_backward_standard(&mut app);
        let after = app.document().unwrap().to_markdown();

        assert_eq!(before, after);
        assert!(app.last_edit.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn no_active_document_is_inert() {
        // Vault with no `.md` — App::new opens an empty tab. The
        // applier must bail without panicking and must not touch the
        // edit clock.
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut cfg = Config::default();
        cfg.editor.mode = EditorMode::Standard;
        let mut app = App::new(cfg, resolved, pool);

        apply_delete_backward_standard(&mut app);
        assert!(app.last_edit.is_none());
    }
}
