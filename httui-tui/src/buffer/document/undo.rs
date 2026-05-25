//! Undo / redo wiring on top of `vim::undo::UndoStack`. Snapshots
//! capture `(segments, cursor, next_block_id)`; `restore` flags the
//! document dirty conservatively because diffing against the
//! last-saved snapshot would require tracking that separately.

use crate::vim::undo::Snapshot;

use super::Document;

impl Document {
    /// Capture the current state onto the undo past stack. Called by
    /// the dispatch layer immediately before any undoable command —
    /// `i`/`a`/`o`/`O`, operators that modify (`d`/`c`), paste.
    pub fn snapshot(&mut self) {
        self.undo.push(self.snapshot_of_self());
    }

    /// Restore the most recent past snapshot. Returns `false` when the
    /// stack is empty (nothing to undo).
    pub fn undo(&mut self) -> bool {
        let Some(snap) = self.undo.pop_undo() else {
            return false;
        };
        let current = self.snapshot_of_self();
        self.undo.push_redo(current);
        self.restore(snap);
        true
    }

    /// Pop a redo snapshot (set up by a prior `undo`) and apply it.
    /// Returns `false` if the redo stack is empty.
    pub fn redo(&mut self) -> bool {
        let Some(snap) = self.undo.pop_redo() else {
            return false;
        };
        let current = self.snapshot_of_self();
        self.undo.push_past(current);
        self.restore(snap);
        true
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    pub(super) fn snapshot_of_self(&self) -> Snapshot {
        Snapshot {
            segments: self.segments.clone(),
            cursor: self.cursor,
            next_block_id: self.next_block_id,
        }
    }

    pub(super) fn restore(&mut self, snap: Snapshot) {
        self.segments = snap.segments;
        self.cursor = snap.cursor;
        self.next_block_id = snap.next_block_id;
        // Conservatively flag dirty after any history move — proving
        // the restored state matches disk would require tracking the
        // last-saved snapshot.
        self.dirty = true;
    }
}
