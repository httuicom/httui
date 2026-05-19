//! Block-as-prose swap — RAII guard that temporarily promotes the
//! cursor's executable block to a Prose segment so the operator engine
//! can edit the fence as plain text, then rebuilds it on `exit`.
//! Mechanically moved out of `vim/dispatch.rs` (tui-v2 vertical 1,
//! fase 1 p4) with no logic change (contiguous RAII block — fields and
//! Drop-equivalent `exit` ordering preserved).

use ropey::Rope;

use crate::app::App;
use crate::buffer::block::BlockNode;
use crate::buffer::{Cursor, Segment};
use crate::input::action::Action;
use crate::input::types::Motion;

/// Decide whether an action should run with the block-as-prose swap
/// active. Buffer-touching actions (motions, operators, edits, paste,
/// undo) need the swap so they see the SQL as a normal rope; mode
/// transitions and tab/window plumbing don't care.
///
/// Vertical motions (`j`/`k`) are deliberately excluded: they need to
/// see `Cursor::InBlock` so they can hop into the result table at the
/// SQL boundary. Inside the SQL, the same branches in `motions::apply_*`
/// already handle line-by-line navigation — no swap required.
pub(crate) fn action_needs_block_swap(action: &Action) -> bool {
    if let Action::Motion(motion, _) = action {
        if matches!(motion, Motion::Down | Motion::Up) {
            return false;
        }
    }
    matches!(
        action,
        Action::Motion(..)
            | Action::OperatorMotion(..)
            | Action::OperatorLinewise(..)
            | Action::OperatorTextObject(..)
            | Action::VisualOperator(_)
            | Action::VisualSwap
            | Action::Paste(..)
            | Action::Undo
            | Action::Redo
            | Action::RepeatChange(_)
            | Action::InsertChar(_)
            | Action::InsertNewline
            | Action::DeleteBackward
            | Action::DeleteForward
            | Action::EnterInsert(_)
            | Action::ExitInsert
            | Action::EnterVisual
            | Action::EnterVisualLine
            | Action::SearchExecute
            | Action::SearchRepeat { .. }
    )
}

/// While alive, the active document's `segment_idx`-th block is
/// pretending to be a prose run with the SQL as its content.
/// `exit` puts the block back together with whatever the action ended
/// up writing into the prose, and converts the cursor back to
/// `InBlock` if it's still pointing into the swapped slot.
pub(crate) struct InBlockSwap {
    // `pub(crate)`: `apply_cross_segment_visual` in `vim::dispatch`
    // reads `swap.segment_idx` across the module boundary the p4 move
    // introduced. Field order preserved (RAII contract).
    pub(crate) segment_idx: usize,
    original_block: BlockNode,
    original_query: String,
}

impl InBlockSwap {
    pub(crate) fn maybe_enter(app: &mut App) -> Option<Self> {
        let cursor = app.document()?.cursor();
        let Cursor::InBlock {
            segment_idx,
            offset: raw_offset,
        } = cursor
        else {
            return None;
        };
        let doc = app.tabs.active_document_mut()?;
        let block = match doc.segments().get(segment_idx)? {
            Segment::Block(b) => b.clone(),
            _ => return None,
        };
        // Promote the entire raw rope (header + body + closer) to a
        // Prose segment so the operator engine treats every row as
        // editable text. After the operator runs, `exit` parses the
        // mutated rope back into a block (or keeps last-good fields
        // when the fence is broken mid-edit). This is what unlocks
        // d / y / c on the fence header — `alias=foo` deletions, etc.
        let raw_text = block.raw.to_string();
        doc.replace_segment(segment_idx, Segment::Prose(Rope::from_str(&raw_text)));
        let abs = raw_offset.min(raw_text.chars().count());
        doc.set_cursor(Cursor::InProse {
            segment_idx,
            offset: abs,
        });
        Some(Self {
            segment_idx,
            original_block: block,
            original_query: raw_text,
        })
    }

    pub(crate) fn exit(self, app: &mut App) {
        let Some(doc) = app.tabs.active_document_mut() else {
            return;
        };
        let new_raw = match doc.segments().get(self.segment_idx) {
            Some(Segment::Prose(rope)) => rope.to_string(),
            _ => self.original_query.clone(),
        };
        let cursor_after = doc.cursor();
        // Rebuild the block from the mutated raw rope. We try to
        // reparse — clean parses update derived fields; broken
        // parses keep last-good fields so the user can keep editing.
        // Either way, id / state / cached_result survive (Phase 3
        // contract).
        let mut new_block = self.original_block.clone();
        new_block.raw = Rope::from_str(&new_raw);
        new_block.reparse_from_raw();
        doc.replace_segment(self.segment_idx, Segment::Block(new_block));
        // If the cursor still points at the swapped segment, convert
        // back to InBlock at the equivalent raw offset. The prose-mode
        // operator may have moved it past the new rope's length —
        // clamp.
        if let Cursor::InProse {
            segment_idx,
            offset: abs,
        } = cursor_after
        {
            if segment_idx == self.segment_idx {
                let new_len = new_raw.chars().count();
                let clamped = abs.min(new_len);
                doc.set_cursor(Cursor::InBlock {
                    segment_idx,
                    offset: clamped,
                });
            }
        }
    }
}
