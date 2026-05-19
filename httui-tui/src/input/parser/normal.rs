// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
//! Normal-mode key decoder. Mechanically moved out of `vim/parser.rs`
//! (tui-v2 vertical 1, fase 1 p3a) with no logic change.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;
use crate::input::parser::{
    doubled_for, find_kind_to_motion, key_to_find_kind, key_to_operator, try_motion,
};
use crate::input::types::{
    build_textobject, InsertPos, Motion, Operator, PastePos, ScrollPos, WindowCmd,
};
use crate::vim::state::VimState;

/// Translate one key in Normal mode to an [`Action`]. Mutates the
/// parser state to handle multi-key prefixes (counts, `gg`, operators).
pub fn parse_normal(state: &mut VimState, key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;

    if code == KeyCode::Esc {
        state.reset_pending();
        return Action::Noop;
    }

    // `<C-s>` — universal save (VSCode / JetBrains / Sublime
    // convention). Bound here in normal mode so it works without
    // having to type `:w<CR>`.
    if modifiers == KeyModifiers::CONTROL && matches!(code, KeyCode::Char('s')) {
        state.reset_pending();
        return Action::WriteFile;
    }

    // Resolve a pending `z` chord (`zz`, `zt`, `zb`).
    if state.pending_z {
        state.pending_z = false;
        if let KeyCode::Char(c) = code {
            return match c {
                'z' => Action::ScrollCursorTo(ScrollPos::Center),
                't' => Action::ScrollCursorTo(ScrollPos::Top),
                'b' => Action::ScrollCursorTo(ScrollPos::Bottom),
                _ => Action::Noop,
            };
        }
        return Action::Noop;
    }

    // Resolve a pending `Ctrl+W` window-prefix — the next keystroke
    // becomes a [`WindowCmd`]. Anything we don't recognize cancels the
    // prefix silently.
    if state.pending_window {
        state.pending_window = false;
        let cmd = match (modifiers, code) {
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('v')) => {
                Some(WindowCmd::SplitVertical)
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('s')) => {
                Some(WindowCmd::SplitHorizontal)
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('h')) => {
                Some(WindowCmd::FocusLeft)
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('l')) => {
                Some(WindowCmd::FocusRight)
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('k')) => {
                Some(WindowCmd::FocusUp)
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('j')) => {
                Some(WindowCmd::FocusDown)
            }
            // `<C-w>w` and `<C-w><C-w>` both cycle.
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('w'))
            | (KeyModifiers::CONTROL, KeyCode::Char('w')) => Some(WindowCmd::Cycle),
            // `<C-w>c` and `<C-w>q` both close the focused window.
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('c'))
            | (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('q')) => {
                Some(WindowCmd::Close)
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('=')) => {
                Some(WindowCmd::Equalize)
            }
            _ => None,
        };
        return cmd.map(Action::Window).unwrap_or(Action::Noop);
    }

    // Resolve a pending find/till — `f` `t` `F` `T` waiting for a target
    // char. Falls back to a no-op when the keystroke isn't a printable char.
    if let Some(kind) = state.pending_find_kind {
        let target = match code {
            KeyCode::Char(c) => c,
            _ => {
                state.pending_find_kind = None;
                state.pending_operator = None;
                return Action::Noop;
            }
        };
        state.pending_find_kind = None;
        let motion = find_kind_to_motion(kind, target);
        let count = state.take_count();
        if let Some((op, op_count)) = state.pending_operator.take() {
            return Action::OperatorMotion(op, motion, op_count.max(1) * count.max(1));
        }
        return Action::Motion(motion, count);
    }

    // Resolve a pending text object first — `d` `i` `w` arriving here
    // with `pending_textobj_inner = Some(true)` and `pending_operator`
    // still set, expecting `w`/`"`/`(`/etc. to complete the trigram.
    if let Some(inner) = state.pending_textobj_inner {
        let target = match code {
            KeyCode::Char(c) => c,
            _ => {
                // Anything non-char cancels.
                state.pending_operator = None;
                state.pending_textobj_inner = None;
                return Action::Noop;
            }
        };
        let around = !inner;
        if let Some(textobj) = build_textobject(around, target) {
            let (op, op_count) = state
                .pending_operator
                .take()
                .unwrap_or((Operator::Delete, 1));
            state.pending_textobj_inner = None;
            return Action::OperatorTextObject(op, textobj, op_count.max(1));
        }
        // Unknown target — abort the whole operator chain silently.
        state.pending_operator = None;
        state.pending_textobj_inner = None;
        return Action::Noop;
    }

    // Digit accumulation. `0` is special: with no pending count it is
    // the LineStart motion (which may compose with a pending operator).
    if let KeyCode::Char(c) = code {
        if c.is_ascii_digit() {
            let d = c.to_digit(10).unwrap() as usize;
            if d == 0 && state.pending_count.is_none() {
                if let Some((op, op_count)) = state.pending_operator.take() {
                    return Action::OperatorMotion(op, Motion::LineStart, op_count.max(1));
                }
                return Action::Motion(Motion::LineStart, 1);
            }
            state.push_digit(d);
            return Action::Noop;
        }
    }

    // `z` (no modifier) starts the scroll-positioning chord.
    // Counts before `z` aren't supported (vim's behavior here is
    // niche — `<n>zz` sets scrolloff to N — out of scope V1).
    if modifiers == KeyModifiers::NONE
        && matches!(code, KeyCode::Char('z'))
        && state.pending_operator.is_none()
        && !state.pending_g
    {
        state.pending_z = true;
        return Action::Noop;
    }

    // Resolve `gg` (the second `g`).
    if state.pending_g {
        state.pending_g = false;
        if let KeyCode::Char('g') = code {
            let count = state.take_count();
            if let Some((op, op_count)) = state.pending_operator.take() {
                let motion = if count > 1 {
                    Motion::GotoLine(count)
                } else {
                    Motion::DocStart
                };
                return Action::OperatorMotion(op, motion, op_count.max(1));
            }
            return if count > 1 {
                Action::Motion(Motion::GotoLine(count), 1)
            } else {
                Action::Motion(Motion::DocStart, 1)
            };
        }
        // `gt` / `gT` — tab navigation. `<n>gt` jumps to tab n.
        if let KeyCode::Char('t') = code {
            let count = state.take_count();
            return if count > 1 {
                Action::TabGoto(count)
            } else {
                Action::TabNext
            };
        }
        if let KeyCode::Char('T') = code {
            state.take_count();
            return Action::TabPrev;
        }
        // `gd` — cycle the focused block's display mode. Doesn't
        // consume the leading count (mode-cycle is per-press, not
        // per-N), but we still drain `pending_count` so a stale
        // count doesn't leak into the next keystroke.
        if let KeyCode::Char('d') = code {
            state.take_count();
            return Action::CycleDisplayMode;
        }
        // `ga` — open the inline alias-edit popup over the focused
        // block. Same count-drain rule as `gd`: counts are
        // meaningless for a metadata edit. Picked over `<C-a>` so
        // tmux users (whose default-alt prefix is `<C-a>`) don't
        // have the chord intercepted before it reaches us.
        if let KeyCode::Char('a') = code {
            state.take_count();
            return Action::OpenFenceEditAlias;
        }
        // `gx` — open the export-format picker over the focused DB
        // block. `x` for "eXport" sidesteps `e` (word-end motion)
        // and `y` (yank chord prefix). Counts dropped same as
        // siblings. Dispatch validates cursor + result before opening.
        if let KeyCode::Char('x') = code {
            state.take_count();
            return Action::OpenDbExportPicker;
        }
        // `gs` — open the block settings modal (limit + timeout in a
        // single popup). Memory `project_tui_block_settings_modal.md`
        // pinned the UX: one modal with Tab-navigation, NOT chords
        // per field. Counts dropped, dispatch validates DB-only.
        if let KeyCode::Char('s') = code {
            state.take_count();
            return Action::OpenDbSettingsModal;
        }
        // `gh` — open the run-history modal for the focused HTTP
        // block. Read-only listing of `block_run_history` rows for
        // `(file_path, alias)`. Dispatch validates HTTP-only +
        // aliased; anonymous blocks have no history (no key to
        // group runs under).
        if let KeyCode::Char('h') = code {
            state.take_count();
            return Action::OpenBlockHistory;
        }
        // `gE` — open the environment picker (capital E to dodge
        // `ge` motion). Lists every env from SQLite; Enter activates
        // and refreshes the status-bar chip. Counts are meaningless
        // for a global registry switch.
        if let KeyCode::Char('E') = code {
            state.take_count();
            return Action::OpenEnvironmentPicker;
        }
        // `g?` — open the keymap help modal. Bare `?` is taken by
        // search-backwards, so the help lookup lives behind the
        // `g` prefix family.
        if let KeyCode::Char('?') = code {
            state.take_count();
            return Action::OpenHelp;
        }
        // `g]` / `g[` — jump to next / previous block in document
        // order. Mnemonic: bracket = "structure boundary". Counts
        // are dropped (single-step navigation per press).
        if let KeyCode::Char(']') = code {
            state.take_count();
            return Action::JumpNextBlock;
        }
        if let KeyCode::Char('[') = code {
            state.take_count();
            return Action::JumpPrevBlock;
        }
        // `gN` — open the block-template picker. Capital N to dodge
        // vim's `gn` (find next match) motion.
        if let KeyCode::Char('N') = code {
            state.take_count();
            return Action::OpenBlockTemplatePicker;
        }
        // `gr` — rerun the last block dispatched this session.
        // Vim's bare `gr` (replace-with-virtual-edit) isn't wired
        // here, so the chord is free for our use.
        if let KeyCode::Char('r') = code {
            state.take_count();
            return Action::RerunLastBlock;
        }
        // `gb` — open the tab picker. `b` for "buffer" (vim
        // terminology — files in vim are buffers). Counts dropped.
        if let KeyCode::Char('b') = code {
            state.take_count();
            return Action::OpenTabPicker;
        }
        // `gW` — write every dirty tab. Capital W to dodge vim's
        // `gw` (format text) motion. Counts are meaningless for a
        // multi-file save.
        if let KeyCode::Char('W') = code {
            state.take_count();
            return Action::WriteAll;
        }
        // `gv` — reselect the last visual region. Vim convention.
        if let KeyCode::Char('v') = code {
            state.take_count();
            return Action::ReselectVisual;
        }
        // Drop the prefix and continue parsing.
    }

    let count = state.take_count();

    // Operator-pending branch. `d`/`c`/`y` set state.pending_operator;
    // the next keystroke either doubles (linewise) or supplies a motion.
    if let Some((op, op_count)) = state.pending_operator {
        // `dd`, `cc`, `yy` — linewise shortcut.
        if doubled_for(op, code) {
            state.pending_operator = None;
            return Action::OperatorLinewise(op, op_count.max(1) * count.max(1));
        }
        // `dgg`, `cgg`, `ygg` — defer to the next keystroke.
        if let KeyCode::Char('g') = code {
            state.pending_count = if count > 1 { Some(count) } else { None };
            state.pending_g = true;
            return Action::Noop;
        }
        if let KeyCode::Char('G') = code {
            state.pending_operator = None;
            let motion = if count > 1 {
                Motion::GotoLine(count)
            } else {
                Motion::DocEnd
            };
            return Action::OperatorMotion(op, motion, op_count.max(1));
        }
        // Plain motion.
        if let Some(m) = try_motion(key) {
            state.pending_operator = None;
            return Action::OperatorMotion(op, m, op_count.max(1) * count.max(1));
        }
        // Find/till prefix — `df<c>`, `dt<c>`, etc. Stash the combined
        // count and let the pending-find resolver at the top of
        // `parse_normal` produce the OperatorMotion next tick.
        if let Some(kind) = key_to_find_kind(modifiers, code) {
            state.pending_find_kind = Some(kind);
            state.pending_operator = Some((op, op_count.max(1) * count.max(1)));
            return Action::Noop;
        }
        // Repeat last find — `d;` / `d,`.
        if let KeyCode::Char(';') = code {
            if let Some(m) = state.last_find {
                state.pending_operator = None;
                return Action::OperatorMotion(op, m, op_count.max(1) * count.max(1));
            }
            state.pending_operator = None;
            return Action::Noop;
        }
        if let KeyCode::Char(',') = code {
            if let Some(m) = state.last_find.and_then(Motion::reverse_find) {
                state.pending_operator = None;
                return Action::OperatorMotion(op, m, op_count.max(1) * count.max(1));
            }
            state.pending_operator = None;
            return Action::Noop;
        }
        // Text-object prefix. `i` or `a` starts the trigram; the next
        // keystroke (handled at the top of `parse_normal`) supplies the
        // target char and produces an [`Action::OperatorTextObject`].
        match (modifiers, code) {
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('i')) => {
                state.pending_textobj_inner = Some(true);
                // Stash the count back so it's available when we resolve.
                state.pending_operator = Some((op, op_count.max(1) * count.max(1)));
                return Action::Noop;
            }
            (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('a')) => {
                state.pending_textobj_inner = Some(false);
                state.pending_operator = Some((op, op_count.max(1) * count.max(1)));
                return Action::Noop;
            }
            _ => {}
        }
        // Unrecognized → cancel the operator silently.
        state.pending_operator = None;
        return Action::Noop;
    }

    // No operator pending — interpret the keystroke as a fresh command.

    // Operators (entry).
    if let Some(op) = key_to_operator(modifiers, code) {
        state.pending_operator = Some((op, count));
        return Action::Noop;
    }

    // Find/till entry.
    if let Some(kind) = key_to_find_kind(modifiers, code) {
        state.pending_find_kind = Some(kind);
        // Stash count so the resolver consumes it.
        state.pending_count = if count > 1 { Some(count) } else { None };
        return Action::Noop;
    }

    // Repeat last find — `;` / `,`.
    if let KeyCode::Char(';') = code {
        if let Some(m) = state.last_find {
            return Action::Motion(m, count);
        }
        return Action::Noop;
    }
    if let KeyCode::Char(',') = code {
        if let Some(m) = state.last_find.and_then(Motion::reverse_find) {
            return Action::Motion(m, count);
        }
        return Action::Noop;
    }

    // Plain motion?
    if let Some(m) = try_motion(key) {
        return Action::Motion(m, count);
    }

    // App-level shortcuts (non-vim) — centralised in
    // `vim::keybindings` so they're easy to find and remap. Each
    // helper wraps a `KeyChord` constant; check them before the
    // big match below so the literal branches stay focused on
    // genuine vim primitives.
    use crate::vim::keybindings as kb;
    if kb::matches_run_block(&key) {
        return Action::RunBlock;
    }
    if kb::matches_open_db_row_detail(&key) {
        return Action::OpenDbRowDetail;
    }
    if kb::matches_quick_open(&key) {
        return Action::EnterQuickOpen;
    }
    if kb::matches_tree_toggle(&key) {
        return Action::TreeToggle;
    }
    if kb::matches_focus_swap(&key) {
        return Action::FocusSwap;
    }
    if kb::matches_open_connection_picker(&key) {
        return Action::OpenConnectionPicker;
    }
    if kb::matches_explain_block(&key) {
        return Action::ExplainBlock;
    }
    if kb::matches_copy_as_curl(&key) {
        return Action::CopyAsCurl;
    }
    if kb::matches_content_search(&key) {
        return Action::OpenContentSearch;
    }

    match (modifiers, code) {
        // gg / G with optional count — these need state.
        (_, KeyCode::Char('g')) => {
            state.pending_count = if count > 1 { Some(count) } else { None };
            state.pending_g = true;
            Action::Noop
        }
        (_, KeyCode::Char('G')) => {
            if count > 1 {
                Action::Motion(Motion::GotoLine(count), 1)
            } else {
                Action::Motion(Motion::DocEnd, 1)
            }
        }

        // Insert variants.
        (_, KeyCode::Char('i')) => Action::EnterInsert(InsertPos::Current),
        (_, KeyCode::Char('a')) => Action::EnterInsert(InsertPos::After),
        (_, KeyCode::Char('I')) => Action::EnterInsert(InsertPos::LineStart),
        (_, KeyCode::Char('A')) => Action::EnterInsert(InsertPos::LineEnd),
        (_, KeyCode::Char('o')) => Action::EnterInsert(InsertPos::LineBelow),
        (_, KeyCode::Char('O')) => Action::EnterInsert(InsertPos::LineAbove),

        // Operator shortcuts (uppercase). All of these decompose into
        // `<op><motion>` or `<op><op>` so the operator engine handles them.
        (_, KeyCode::Char('D')) => Action::OperatorMotion(Operator::Delete, Motion::LineEnd, count),
        (_, KeyCode::Char('C')) => Action::OperatorMotion(Operator::Change, Motion::LineEnd, count),
        (_, KeyCode::Char('Y')) => Action::OperatorLinewise(Operator::Yank, count),
        (_, KeyCode::Char('x')) => Action::OperatorMotion(Operator::Delete, Motion::Right, count),
        (_, KeyCode::Char('X')) => Action::OperatorMotion(Operator::Delete, Motion::Left, count),
        (_, KeyCode::Char('s')) => Action::OperatorMotion(Operator::Change, Motion::Right, count),
        (_, KeyCode::Char('S')) => Action::OperatorLinewise(Operator::Change, count),

        // Visual mode entry — `v` charwise, `V` linewise. The dispatch
        // layer captures the current cursor as the anchor.
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('v')) => Action::EnterVisual,
        (_, KeyCode::Char('V')) => Action::EnterVisualLine,

        // Paste. Excluding Ctrl so `Ctrl+P` (quick-open) reaches the
        // dedicated arm below. (`r` / `<CR>` / `<C-p>` / `<C-e>` /
        // `Tab` are app-level shortcuts handled by the keybindings
        // pre-match block above.)
        (KeyModifiers::NONE, KeyCode::Char('p')) => Action::Paste(PastePos::After, count),
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('P')) => {
            Action::Paste(PastePos::Before, count)
        }

        // History.
        (KeyModifiers::NONE, KeyCode::Char('u')) => Action::Undo,
        (KeyModifiers::CONTROL, KeyCode::Char('r')) => Action::Redo,
        (KeyModifiers::NONE, KeyCode::Char('.')) => Action::RepeatChange(count),

        // `Ctrl+W` — vim window prefix. Sets `state.pending_window`
        // so the next keystroke is interpreted as a [`WindowCmd`] by
        // the prefix-resolution branch at the top of `parse_normal`.
        (KeyModifiers::CONTROL, KeyCode::Char('w')) => {
            state.pending_window = true;
            Action::Noop
        }

        // Search entry + repeat.
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('/')) => Action::EnterSearch(true),
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char('?')) => {
            Action::EnterSearch(false)
        }
        (KeyModifiers::NONE, KeyCode::Char('n')) => Action::SearchRepeat { reverse: false },
        (_, KeyCode::Char('N')) => Action::SearchRepeat { reverse: true },

        // Command-line entry.
        (KeyModifiers::NONE | KeyModifiers::SHIFT, KeyCode::Char(':')) => Action::EnterCmdline,

        // Emergency quit.
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::Quit,

        _ => Action::Noop,
    }
}
