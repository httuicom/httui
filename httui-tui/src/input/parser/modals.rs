// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
//! Modal / picker mode key decoders (DB row-detail, HTTP response
//! detail, connection / tab / template / environment / export pickers,
//! DB settings + confirm, content search, block history) plus the
//! shared `is_blocked_in_modal` guard. Mechanically moved out of
//! `vim/parser.rs` (tui-v2 vertical 1, fase 1 p3d) with no logic change.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;
use crate::input::parser::normal::parse_normal;
use crate::input::types::Operator;
use crate::vim::state::VimState;

/// Translate one key while the DB row-detail modal is open. The
/// modal is "the active buffer, but read-only" — `app.document_mut()`
/// redirects to its body doc, so we delegate parsing to
/// `parse_normal` and let the dispatch engine work normally. The
/// only exceptions are:
///
/// 1. modal-specific shortcuts (`Ctrl-C` closes, `Y` copies the row
///    as JSON). Note: `Esc` and `q` are NOT close shortcuts — they
///    keep their vim semantics (`Esc` clears a pending chord, `q`
///    starts macro recording — currently a no-op);
/// 2. actions that would mutate the buffer (insert, edit, paste,
///    undo, delete/change operators) — replaced with Noop so the
///    modal stays read-only;
/// 3. actions that would escape the modal's focus (window/tab/quit/
///    file-tree/quick-open/run-block) — also Noop, the modal owns
///    the keyboard until it's dismissed.
pub fn parse_db_row_detail(state: &mut VimState, key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        // Modal close is `Ctrl-C` only — `Esc` and `q` are reserved
        // for their normal vim semantics (cancelling a chord and
        // macro-recording, respectively). Closing on either felt
        // accidental once standard yank chords like `yi{` were in
        // play: a stray `Esc` to clear a pending count would
        // teleport-close the modal.
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => return Action::CloseDbRowDetail,
        // `Y` (uppercase) → copy the whole row as JSON. Distinct
        // from `y` so the standard yank chord family (`yy`, `y$`,
        // `yi{`, `yiw`, etc.) keeps working — those would otherwise
        // be eaten by a standalone `y` intercept the moment it
        // fires.
        (KeyModifiers::SHIFT, KeyCode::Char('Y'))
            if state.pending_count.is_none()
                && state.pending_operator.is_none()
                && !state.pending_window =>
        {
            return Action::CopyDbRowDetailJson;
        }
        _ => {}
    }
    let action = parse_normal(state, key);
    if is_blocked_in_modal(&action) {
        Action::Noop
    } else {
        action
    }
}

/// Decide whether an `Action` produced by `parse_normal` should be
/// suppressed inside the row-detail modal. Three categories:
///
/// - **Mutations**: the modal is read-only. Any action that would
///   change buffer contents (insert, delete, paste, undo, redo,
///   delete/change operators, `.` repeat) is dropped.
/// - **Mode transitions** (search, visual, ex): allowed in a normal
///   buffer, but they swap `app.vim.mode` away from `DbRowDetail`,
///   which breaks the modal's render path. Supporting them properly
///   needs a "return to modal mode after the transient mode exits"
///   plumbing — deferred. Until then, block.
/// - **Focus escapes**: the modal owns input until dismissed. Window
///   ops, tab nav, file-tree, quick-open, run-block, quit — none of
///   these should fire while the modal is up.
pub(crate) fn is_blocked_in_modal(action: &Action) -> bool {
    use Operator::{Change, Delete};
    matches!(
        action,
        // Mutations.
        Action::EnterInsert(_)
            | Action::ExitInsert
            | Action::InsertChar(_)
            | Action::InsertNewline
            | Action::DeleteBackward
            | Action::DeleteForward
            | Action::Paste(..)
            | Action::Undo
            | Action::Redo
            | Action::RepeatChange(_)
            | Action::OperatorMotion(Delete | Change, _, _)
            | Action::OperatorTextObject(Delete | Change, _, _)
            | Action::OperatorLinewise(Delete | Change, _)
            | Action::VisualOperator(Delete | Change)
            // Mode transitions that would break the modal's render
            // path. Search and ex are still blocked — supporting
            // them needs a "return to modal mode after the transient
            // mode exits" plumbing. Visual mode IS supported: the
            // modal renders whenever `app.db_row_detail` is Some,
            // independent of `app.vim.mode`, and the dispatch
            // restores `Mode::DbRowDetail` after the visual op.
            | Action::EnterSearch(_)
            | Action::SearchExecute
            | Action::SearchRepeat { .. }
            | Action::EnterCmdline
            // Focus escapes.
            | Action::Window(_)
            | Action::TabPrev
            | Action::TabNext
            | Action::TabGoto(_)
            | Action::FocusSwap
            | Action::TreeToggle
            | Action::EnterQuickOpen
            | Action::Quit
            | Action::RunBlock
            | Action::OpenDbRowDetail
            | Action::ExplainBlock
            | Action::OpenConnectionPicker
            | Action::OpenEnvironmentPicker
            | Action::OpenHelp
            | Action::OpenBlockTemplatePicker
            | Action::OpenTabPicker
    )
}

/// Parser for `Mode::HttpResponseDetail`. Mirrors
/// [`parse_db_row_detail`]: read-only modal, motions are routed at the
/// modal's sub-`Document`, mutations + focus escapes are filtered out.
/// Two modal-specific shortcuts:
///
/// - `Ctrl-C` → close the modal.
/// - `Y` (uppercase) → copy the full response body to the clipboard.
///
/// `Esc` and `q` keep their normal vim semantics (cancel pending
/// chord and macro-record start, respectively) so a stray keystroke
/// during a `yi{` chord doesn't teleport-close the modal.
pub fn parse_http_response_detail(state: &mut VimState, key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => return Action::CloseHttpResponseDetail,
        (KeyModifiers::SHIFT, KeyCode::Char('Y'))
            if state.pending_count.is_none()
                && state.pending_operator.is_none()
                && !state.pending_window =>
        {
            return Action::CopyHttpResponseBody;
        }
        _ => {}
    }
    let action = parse_normal(state, key);
    if is_blocked_in_modal(&action) {
        Action::Noop
    } else {
        action
    }
}

/// Translate one key while the connection picker popup is open.
/// Tiny vocab: vertical-only navigation (`j`/`k` and the arrows),
/// `Enter` to apply, `Esc`/`Ctrl-C` to dismiss. Anything else is a
/// no-op so a stray keystroke can't leak through to the editor.
pub fn parse_connection_picker(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::CloseConnectionPicker,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::CloseConnectionPicker,
        (_, KeyCode::Enter) => Action::ConfirmConnectionPicker,
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            Action::MoveConnectionPickerCursor(1)
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            Action::MoveConnectionPickerCursor(-1)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => Action::MoveConnectionPickerCursor(1),
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Action::MoveConnectionPickerCursor(-1),
        // `D` (capital) deletes the highlighted connection. Lowercase
        // `d` would conflict with vim's `dd` linewise-delete reflex
        // and require pending state; capital is a single-press chord
        // and matches the picker's mode (no surrounding text edit).
        (mods, KeyCode::Char('D')) if !mods.contains(KeyModifiers::CONTROL) => {
            Action::DeleteConnectionInPicker
        }
        _ => Action::Noop,
    }
}

/// Translate one key while the tab picker is open. Same vocab as
/// the env / template pickers: vertical navigation, Enter, Esc.
pub fn parse_tab_picker(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::CloseTabPicker,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::CloseTabPicker,
        (_, KeyCode::Enter) => Action::ConfirmTabPicker,
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            Action::MoveTabPickerCursor(1)
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            Action::MoveTabPickerCursor(-1)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => Action::MoveTabPickerCursor(1),
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Action::MoveTabPickerCursor(-1),
        _ => Action::Noop,
    }
}

/// Translate one key while the block-template picker is open.
/// Same vocab as `parse_environment_picker`: vertical-only
/// navigation and Enter/Esc. No `D` (templates aren't deletable —
/// they're a static const).
pub fn parse_block_template_picker(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::CloseBlockTemplatePicker,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::CloseBlockTemplatePicker,
        (_, KeyCode::Enter) => Action::ConfirmBlockTemplatePicker,
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            Action::MoveBlockTemplatePickerCursor(1)
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            Action::MoveBlockTemplatePickerCursor(-1)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => Action::MoveBlockTemplatePickerCursor(1),
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Action::MoveBlockTemplatePickerCursor(-1),
        _ => Action::Noop,
    }
}

/// Translate one key while the environment picker is open. Same
/// vocab as `parse_connection_picker` minus `D` (no destructive
/// op for envs in V1 — they're configuration, not data, and one
/// missing env yields a clear "no active env" instead of a broken
/// block). Anything else is a no-op so stray keys don't leak.
pub fn parse_environment_picker(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::CloseEnvironmentPicker,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::CloseEnvironmentPicker,
        (_, KeyCode::Enter) => Action::ConfirmEnvironmentPicker,
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            Action::MoveEnvironmentPickerCursor(1)
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            Action::MoveEnvironmentPickerCursor(-1)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => Action::MoveEnvironmentPickerCursor(1),
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Action::MoveEnvironmentPickerCursor(-1),
        _ => Action::Noop,
    }
}

/// Translate one key while the export-format picker is open. Same
/// vocab as `parse_connection_picker` — vertical-only navigation
/// (`j`/`k`/arrows), `Enter` to copy, `Esc`/`Ctrl-C` to dismiss.
/// Anything else is a no-op so a stray keystroke can't leak through
/// to the editor underneath.
pub fn parse_db_export_picker(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::CloseDbExportPicker,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::CloseDbExportPicker,
        (_, KeyCode::Enter) => Action::ConfirmDbExportPicker,
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            Action::MoveDbExportPickerCursor(1)
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            Action::MoveDbExportPickerCursor(-1)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => Action::MoveDbExportPickerCursor(1),
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Action::MoveDbExportPickerCursor(-1),
        _ => Action::Noop,
    }
}

/// Translate one key while the DB block settings modal is open.
/// Mirrors `parse_fence_edit` but adds Tab/BackTab/Up/Down for
/// switching the focused field, and routes typing into whichever
/// LineEdit currently has focus. Same Esc/Enter contract as the
/// other modals.
pub fn parse_db_settings_modal(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::CloseDbSettingsModal,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::CloseDbSettingsModal,
        (_, KeyCode::Enter) => Action::ConfirmDbSettingsModal,
        // Tab / Down — focus next field. BackTab (Shift-Tab) /
        // Up — focus prev. We accept the arrow keys too because
        // the form has only two stacked inputs and j/k aren't
        // available (the LineEdit treats them as character input).
        (_, KeyCode::Tab) => Action::DbSettingsFocusNext,
        (_, KeyCode::Down) => Action::DbSettingsFocusNext,
        (_, KeyCode::BackTab) => Action::DbSettingsFocusPrev,
        (_, KeyCode::Up) => Action::DbSettingsFocusPrev,
        // LineEdit ops on the focused input.
        (_, KeyCode::Backspace) => Action::DbSettingsBackspace,
        (_, KeyCode::Delete) => Action::DbSettingsDelete,
        (_, KeyCode::Left) => Action::DbSettingsCursorLeft,
        (_, KeyCode::Right) => Action::DbSettingsCursorRight,
        (_, KeyCode::Home) => Action::DbSettingsCursorHome,
        (_, KeyCode::End) => Action::DbSettingsCursorEnd,
        // Plain printable char (no CONTROL) → insert into focused
        // input. We allow SHIFT for capitals; CONTROL is rejected
        // so terminal emulators that send `<C-x>` style chords
        // don't accidentally land in the buffer.
        (mods, KeyCode::Char(c)) if !mods.contains(KeyModifiers::CONTROL) => {
            Action::DbSettingsChar(c)
        }
        _ => Action::Noop,
    }
}

/// Translate one key while the content-search modal is open.
/// Hybrid of `parse_quickopen` (typing into a LineEdit) and the
/// list-picker pattern (j/k/arrows + Ctrl-n/p navigate selection).
/// `Up`/`Down` and `Ctrl-n`/`Ctrl-p` MOVE the highlight; plain
/// `j`/`k` go INTO the buffer (otherwise typing `j` after a `j`
/// motion would skip a character). Esc/Ctrl-C close.
pub fn parse_content_search(key: KeyEvent) -> Action {
    use crossterm::event::KeyCode::*;
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, Esc) => Action::CloseContentSearch,
        (KeyModifiers::CONTROL, Char('c')) => Action::CloseContentSearch,
        (_, Enter) => Action::ConfirmContentSearch,
        // Selection navigation — arrows + Ctrl-n/p only. j/k go
        // into the buffer like quick-open.
        (_, Up) => Action::MoveContentSearchCursor(-1),
        (_, Down) => Action::MoveContentSearchCursor(1),
        (KeyModifiers::CONTROL, Char('n')) => Action::MoveContentSearchCursor(1),
        (KeyModifiers::CONTROL, Char('p')) => Action::MoveContentSearchCursor(-1),
        // LineEdit ops on the query buffer.
        (_, Backspace) => Action::ContentSearchBackspace,
        (_, Delete) => Action::ContentSearchDelete,
        (_, Left) => Action::ContentSearchCursorLeft,
        (_, Right) => Action::ContentSearchCursorRight,
        (_, Home) => Action::ContentSearchCursorHome,
        (_, End) => Action::ContentSearchCursorEnd,
        // Plain printable char (no CONTROL) → into the buffer.
        // Ctrl-shifted chars stay rejected so terminal-emulator
        // chord sequences don't accidentally land in typing.
        (mods, Char(c)) if !mods.contains(KeyModifiers::CONTROL) => Action::ContentSearchChar(c),
        _ => Action::Noop,
    }
}

/// Translate one key while the block-history modal is open. Same
/// vocab as `parse_connection_picker` — vertical-only navigation
/// and Esc/Ctrl-C to dismiss. There's no Enter/confirm: the modal
/// is a read-only viewer (V1). Anything else is a no-op.
pub fn parse_block_history(key: KeyEvent) -> Action {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => Action::CloseBlockHistory,
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => Action::CloseBlockHistory,
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            Action::MoveBlockHistoryCursor(1)
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            Action::MoveBlockHistoryCursor(-1)
        }
        (KeyModifiers::CONTROL, KeyCode::Char('n')) => Action::MoveBlockHistoryCursor(1),
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => Action::MoveBlockHistoryCursor(-1),
        _ => Action::Noop,
    }
}

