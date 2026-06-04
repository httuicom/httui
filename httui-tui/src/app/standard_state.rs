//! Standard-mode (non-modal) editor selection state.
//!
//! Introduced by tui-V1 / fase 3 p0. The conventional editor model
//! has no `Mode::Visual`; instead `Shift`+arrow extends a selection
//! anchored at the cursor's position when the first `Shift`+arrow was
//! pressed. That anchor lives here so it survives across keystrokes
//! independently of the vim engine's `visual_anchor` (which the
//! Standard path deliberately never touches — Cenário 2 / vim stays
//! byte-identical).
//!
//! `anchor == None` means "no active selection"; the next `Shift`+arrow
//! seeds it with `doc.cursor()`. A plain (non-Shift) arrow clears it.
//! Pure data — no `impl`, no `fn` — so the coverage gate treats this
//! file as structural-only (STRUCT auto-pass), like `app/status.rs`.

use crate::buffer::Cursor;
use crate::input::apply::standard_undo::EditGroupKind;

/// Selection anchor for the Standard (non-modal) editor profile. The
/// moving end of the selection is always the document cursor; the
/// anchor is the fixed end. `None` while nothing is selected.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StandardState {
    /// Where the current Shift-selection started. `None` ⇒ no active
    /// selection. Seeded on the first `Shift`+arrow, cleared by a
    /// plain arrow / `ClearSelection`.
    pub anchor: Option<Cursor>,
    /// Which kind of text edit is currently being coalesced into one
    /// undo group (tui-V1 / fase 4 p2). `None` ⇒ the next textual edit
    /// opens a fresh group (forces a snapshot). Reset by any
    /// non-textual action so undo granularity stays sane.
    pub edit_group: Option<EditGroupKind>,
    /// `true` after `Ctrl+W` was pressed and we're waiting for the
    /// next key to decode as a `WindowCmd` suffix (vim-style chord).
    /// Cleared after the next keystroke regardless of whether it
    /// decoded.
    pub pending_window_chord: bool,
}
