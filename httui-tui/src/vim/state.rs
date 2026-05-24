use crate::buffer::Cursor;
use crate::vim::change::{ChangeRecord, InsertSession};
use crate::vim::mode::Mode;
use crate::vim::parser::{Motion, Operator};
use crate::vim::register::Register;

/// Which find / till variant is waiting for its target char.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindKind {
    /// `f<c>` — forward, inclusive.
    F,
    /// `F<c>` — backward, inclusive.
    FBack,
    /// `t<c>` — forward, lands before the char.
    T,
    /// `T<c>` — backward, lands after the char.
    TBack,
}

/// Mutable bookkeeping the parser needs across keystrokes: the current
/// mode, the in-flight count, the "pending g" flag for `gg`, the
/// pending operator (`d`/`c`/`y` waiting for a motion), the command-line
/// buffer used while in [`Mode::CommandLine`], and the unnamed yank /
/// delete register (round 2 only — named registers land in round 3).
pub struct VimState {
    pub mode: Mode,
    pub pending_count: Option<usize>,
    pub pending_g: bool,
    pub pending_operator: Option<(Operator, usize)>,
    /// `Some(true)` when `i` was pressed with a pending operator (inner
    /// text object); `Some(false)` for `a` (around). Cleared as soon as
    /// the next keystroke resolves the target char.
    pub pending_textobj_inner: Option<bool>,
    /// `f`/`F`/`t`/`T` set this; the next keystroke supplies the target
    /// char and produces a [`Motion`] (or [`super::parser::Action::OperatorMotion`]
    /// when an operator is also pending).
    pub pending_find_kind: Option<FindKind>,
    /// Last find/till motion executed. `;` repeats it; `,` runs it
    /// reversed via [`Motion::reverse_find`].
    pub last_find: Option<Motion>,
    /// Last change command — replayed by `.` repeat. Persists across
    /// resets and across mode transitions.
    pub last_change: Option<ChangeRecord>,
    /// Live capture for the in-flight insert session. Picked up on
    /// `<Esc>` to finalize a [`ChangeRecord`].
    pub insert_session: InsertSession,
    /// Last executed search query, persisted for `n`/`N` repeat.
    pub last_search: Option<String>,
    /// Direction of the last executed search.
    pub last_search_forward: bool,
    /// Whether the search highlight is currently visible. `:noh` flips
    /// this off without losing `last_search` — so `n`/`N` keep working
    /// while the matches stop being painted on screen. Re-arms when a
    /// new search executes.
    pub search_highlight: bool,
    pub unnamed: Register,
    /// `Ctrl+W` was just seen and we're waiting for the window-command
    /// suffix (`v`/`s`/`h`/`j`/`k`/`l`/`c`/`w`/`=` …).
    pub pending_window: bool,
    /// `z` was just seen and we're waiting for the second key in a
    /// `zz` / `zt` / `zb` chord (cursor placement in viewport).
    pub pending_z: bool,
    /// Anchor cursor for [`Mode::Visual`] / [`Mode::VisualLine`] — the
    /// fixed end of the selection. The moving end is the document
    /// cursor itself. `None` outside visual modes.
    pub visual_anchor: Option<Cursor>,
    pub visual_origin_mode: Option<Mode>,
    /// Anchor + linewise flag of the last visual selection that was
    /// dismissed (Esc / operator completion / mode flip). Read by
    /// the `gv` chord to restore visual mode at that anchor.
    /// V1 only stores the anchor — the moving end isn't restored,
    /// so `gv` re-enters visual with the cursor wherever it
    /// currently is, and the user re-extends with motions. Good
    /// enough for "I want to redo something on that selection",
    /// short of full vim parity.
    pub last_visual: Option<LastVisual>,
}

/// Snapshot recorded on every exit from `Mode::Visual` / `Mode::VisualLine`
/// so the `gv` chord can put the user back in visual mode at the
/// same anchor.
#[derive(Debug, Clone, Copy)]
pub struct LastVisual {
    pub anchor: Cursor,
    pub linewise: bool,
}

impl VimState {
    pub fn new() -> Self {
        Self {
            mode: Mode::Normal,
            pending_count: None,
            pending_g: false,
            pending_operator: None,
            pending_textobj_inner: None,
            pending_find_kind: None,
            last_find: None,
            last_change: None,
            insert_session: InsertSession::default(),
            last_search: None,
            last_search_forward: true,
            search_highlight: true,
            unnamed: Register::empty(),
            pending_window: false,
            pending_z: false,
            visual_anchor: None,
            visual_origin_mode: None,
            last_visual: None,
        }
    }

    pub fn enter_insert(&mut self) {
        self.snapshot_visual_for_reselect();
        self.mode = Mode::Insert;
        self.reset_pending();
        self.visual_anchor = None;
    }

    pub fn enter_normal(&mut self) {
        self.snapshot_visual_for_reselect();
        self.mode = Mode::Normal;
        self.reset_pending();
        self.visual_anchor = None;
    }

    /// Capture the current visual anchor + linewise flag into
    /// `last_visual` if we're about to leave a visual mode. Called
    /// from every mode-flip path so `gv` works regardless of which
    /// chord caused the exit (Esc, operator completion, write-quit,
    /// any other mode transition).
    fn snapshot_visual_for_reselect(&mut self) {
        if let (Some(anchor), true) = (
            self.visual_anchor,
            matches!(self.mode, Mode::Visual | Mode::VisualLine),
        ) {
            self.last_visual = Some(LastVisual {
                anchor,
                linewise: self.mode == Mode::VisualLine,
            });
        }
    }

    /// Enter charwise visual mode anchored at `at`.
    pub fn enter_visual(&mut self, at: Cursor) {
        self.visual_origin_mode = Some(self.mode);
        self.mode = Mode::Visual;
        self.reset_pending();
        self.visual_anchor = Some(at);
    }

    /// Enter linewise visual mode anchored at `at`.
    pub fn enter_visual_line(&mut self, at: Cursor) {
        self.visual_origin_mode = Some(self.mode);
        self.mode = Mode::VisualLine;
        self.reset_pending();
        self.visual_anchor = Some(at);
    }

    pub fn push_digit(&mut self, d: usize) {
        let next = self.pending_count.unwrap_or(0).saturating_mul(10) + d;
        self.pending_count = Some(next);
    }

    pub fn take_count(&mut self) -> usize {
        self.pending_count.take().unwrap_or(1)
    }

    pub fn reset_pending(&mut self) {
        self.pending_count = None;
        self.pending_g = false;
        self.pending_operator = None;
        self.pending_textobj_inner = None;
        self.pending_find_kind = None;
        self.pending_window = false;
        self.pending_z = false;
        // `last_find` intentionally persists across resets — `;` and `,`
        // can repeat a find from any subsequent normal-mode state.
    }
}

impl Default for VimState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_digit_accumulates() {
        let mut s = VimState::new();
        s.push_digit(1);
        s.push_digit(2);
        s.push_digit(3);
        assert_eq!(s.pending_count, Some(123));
    }

    #[test]
    fn take_count_consumes_and_defaults_to_one() {
        let mut s = VimState::new();
        s.push_digit(5);
        assert_eq!(s.take_count(), 5);
        assert_eq!(s.pending_count, None);
        // Sem count pendente, default = 1
        assert_eq!(s.take_count(), 1);
    }

    #[test]
    fn enter_visual_captures_origin_mode() {
        let cur = Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        };
        for origin in [Mode::Normal, Mode::HttpResponseDetail, Mode::DbRowDetail] {
            let mut s = VimState::new();
            s.mode = origin;
            s.enter_visual(cur);
            assert_eq!(s.visual_origin_mode, Some(origin));
            assert_eq!(s.mode, Mode::Visual);
        }
    }

    #[test]
    fn enter_visual_line_captures_origin_mode() {
        let cur = Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        };
        let mut s = VimState::new();
        s.mode = Mode::HttpResponseDetail;
        s.enter_visual_line(cur);
        assert_eq!(s.visual_origin_mode, Some(Mode::HttpResponseDetail));
        assert_eq!(s.mode, Mode::VisualLine);
    }

    #[test]
    fn enter_insert_clears_pending() {
        let mut s = VimState::new();
        s.push_digit(7);
        s.pending_g = true;
        s.enter_insert();
        assert_eq!(s.mode, Mode::Insert);
        assert_eq!(s.pending_count, None);
        assert!(!s.pending_g);
    }

}
