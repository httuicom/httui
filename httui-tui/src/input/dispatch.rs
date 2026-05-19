//! Top-level input dispatcher + the exhaustive `Action` router.
//! Mechanically moved out of `vim/dispatch.rs` (tui-v2 vertical 1,
//! fase 1 p6-router) with no logic change. `vim::dispatch` is now a
//! thin facade re-exporting `dispatch` (and `apply_action` for the
//! `replay` helpers). The ~16 external `crate::vim::dispatch::*` call
//! sites keep resolving through that facade unchanged.

use crossterm::event::KeyEvent;

use crate::app::{App, StatusKind};
use crate::buffer::Cursor;
use crate::input::action::Action;
use crate::input::block_swap::{action_needs_block_swap, InBlockSwap};
use crate::vim::mode::Mode;
use crate::vim::parser::{
    parse_block_history, parse_block_template_picker, parse_cmdline, parse_connection_picker,
    parse_content_search, parse_db_confirm_run, parse_db_export_picker, parse_db_row_detail,
    parse_db_settings_modal, parse_environment_picker, parse_fence_edit, parse_help,
    parse_http_response_detail, parse_insert, parse_normal, parse_quickopen, parse_search,
    parse_tab_picker, parse_tree, parse_tree_prompt, parse_visual,
};

// completion-popup helpers moved to
// `crate::input::apply::completion` (fase 1 p5c). The popup-key
// appliers are now reached via the `apply_action` completion group
// (`apply_completion`, fase 1 p6b); only the three the `dispatch`
// router itself still calls stay re-exported here.
pub(crate) use crate::input::apply::completion::{
    force_open_completion_popup, parse_completion_popup_key, refresh_completion_popup,
};

/// Top-level vim key dispatcher. The app's `handle_key` delegates here.
pub fn dispatch(app: &mut App, key: KeyEvent) {
    // Any keystroke clears the previous transient status message,
    // matching vim's "press a key to dismiss" feel.
    app.clear_status();

    // `Ctrl-C` while a query is running cancels it — runs before
    // mode parsing so it works from anywhere (Normal, Modal, the
    // middle of a chord). Other modes that bind Ctrl-C (modal close
    // etc.) lose to it; the next key after the cancel completes
    // returns control.
    use crossterm::event::{KeyCode, KeyModifiers};
    if app.running_query.is_some()
        && key.modifiers == KeyModifiers::CONTROL
        && key.code == KeyCode::Char('c')
    {
        crate::commands::db::cancel_running_query(app);
        return;
    }

    // `Ctrl+Space` in insert mode — manual trigger for the SQL
    // completion popup. Lets the user browse the full dialect
    // listing right after a space (where the auto-trigger has no
    // prefix to chew on) or force-reopen a popup they just dismissed.
    //
    // Terminal quirk: most terminals report Ctrl+Space as KeyCode::
    // Char(' ') with the CONTROL modifier set, but some emit the
    // legacy NUL byte form (`Char('\0')`). Accept both.
    if app.vim.mode == Mode::Insert
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(' ') | KeyCode::Char('\0'))
    {
        force_open_completion_popup(app);
        return;
    }

    // Completion popup keys are intercepted before mode parsing so a
    // user mid-typing (Mode::Insert) can navigate / accept / dismiss
    // without leaving insert. Any unmatched key falls through to the
    // mode parser; that re-filter happens in the trigger below.
    if app.completion_popup.is_some() {
        if let Some(action) = parse_completion_popup_key(key) {
            apply_action(app, action, false);
            return;
        }
    }

    let action = match app.vim.mode {
        Mode::Normal => parse_normal(&mut app.vim, key),
        Mode::Insert => parse_insert(key),
        Mode::CommandLine => parse_cmdline(key),
        Mode::Search => parse_search(key),
        Mode::QuickOpen => parse_quickopen(key),
        Mode::Tree => parse_tree(key),
        Mode::TreePrompt => parse_tree_prompt(key),
        Mode::Visual | Mode::VisualLine => parse_visual(&mut app.vim, key),
        Mode::DbRowDetail => parse_db_row_detail(&mut app.vim, key),
        Mode::HttpResponseDetail => parse_http_response_detail(&mut app.vim, key),
        Mode::ConnectionPicker => parse_connection_picker(key),
        Mode::DbConfirmRun => parse_db_confirm_run(key),
        Mode::DbExportPicker => parse_db_export_picker(key),
        Mode::FenceEdit => parse_fence_edit(key),
        Mode::DbSettings => parse_db_settings_modal(key),
        Mode::BlockHistory => parse_block_history(key),
        Mode::ContentSearch => parse_content_search(key),
        Mode::EnvironmentPicker => parse_environment_picker(key),
        Mode::Help => parse_help(key),
        Mode::BlockTemplatePicker => parse_block_template_picker(key),
        Mode::TabPicker => parse_tab_picker(key),
    };

    // Snapshot the pre-swap cursor so the post-action "reparse on
    // ExitInsert" hook can tell whether the user was genuinely in
    // prose (closing-fence-completion case) or inside a block via
    // the swap (then `swap.exit` already rebuilds the block, and
    // reparse would corrupt the cursor).
    let cursor_before_swap = app.document().map(|d| d.cursor());
    let was_exit_insert = matches!(action, Action::ExitInsert);

    // When the cursor is parked inside a block's editable body, swap
    // the block segment for a synthetic prose segment so the entire
    // motion/operator engine — built around `Cursor::InProse` — can
    // run unchanged. The reverse swap happens after the action so the
    // file on disk still serializes back to a fence.
    let swap = if action_needs_block_swap(&action) {
        InBlockSwap::maybe_enter(app)
    } else {
        None
    };
    apply_action(app, action, /* recording = */ true);
    if let Some(s) = swap {
        s.exit(app);
    }

    // ExitInsert from genuine prose: the user might have just typed a
    // closing fence, so re-scan the prose for newly complete blocks
    // and splice them in. This *must* run after the swap is fully
    // unwound — running it while the swap is active would splice the
    // synthetic prose (= the original block's raw) back into the doc
    // and teleport the cursor out of the block.
    let was_in_prose = matches!(cursor_before_swap, Some(Cursor::InProse { .. }));
    if was_exit_insert && was_in_prose {
        if let Some(Cursor::InProse { segment_idx, .. }) = app.document().map(|d| d.cursor()) {
            if let Some(doc) = app.document_mut() {
                if doc.reparse_prose_at(segment_idx) {
                    app.set_status(StatusKind::Info, "block parsed");
                }
            }
        }
    }
    // After a typing-relevant action lands in a DB block, refresh
    // the completion popup against the new prefix. `InsertChar` and
    // `DeleteBackward` are the two paths that shift the prefix at
    // the cursor; everything else is a no-op for the popup.
    if matches!(action, Action::InsertChar(_) | Action::DeleteBackward) {
        refresh_completion_popup(app);
    }
}

/// Run an action against the app. `recording` toggles whether the
/// resulting change updates `last_change` — `.` replay sets it to
/// `false` so a `.` after a `.` doesn't trample its own record.
pub(crate) fn apply_action(app: &mut App, action: Action, recording: bool) {
    // Mechanically partitioned (tui-v2 vertical 1, fase 1 p6):
    // every variant routes to its domain's `apply_<group>` in
    // `crate::input::apply::*`. The match stays EXHAUSTIVE — the
    // compiler proves every `Action` is routed; each group's own
    // `unreachable!` proves nothing is mis-routed into it.
    match action {
        Action::EnterVisual
        | Action::EnterVisualLine
        | Action::ExitVisual
        | Action::VisualSwap
        | Action::VisualOperator(_)
        | Action::VisualSelectTextObject(_)
        | Action::OperatorMotion(..)
        | Action::OperatorLinewise(..)
        | Action::OperatorTextObject(..)
        | Action::Paste(..) => {
            crate::input::apply::operator::apply_operator(app, action, recording)
        }
        Action::CompletionNext
        | Action::CompletionPrev
        | Action::CompletionAccept
        | Action::CompletionDismiss
        | Action::ConfirmDbRun
        | Action::CancelDbRun => {
            crate::input::apply::completion::apply_completion(app, action, recording)
        }
        Action::OpenDbRowDetail
        | Action::CloseDbRowDetail
        | Action::CopyDbRowDetailJson
        | Action::CloseHttpResponseDetail
        | Action::CopyHttpResponseBody => {
            crate::input::apply::modal_detail::apply_modal_detail(app, action, recording)
        }
        Action::OpenConnectionPicker
        | Action::CloseConnectionPicker
        | Action::MoveConnectionPickerCursor(_)
        | Action::ConfirmConnectionPicker
        | Action::DeleteConnectionInPicker
        | Action::OpenEnvironmentPicker
        | Action::CloseEnvironmentPicker
        | Action::MoveEnvironmentPickerCursor(_)
        | Action::ConfirmEnvironmentPicker
        | Action::OpenBlockTemplatePicker
        | Action::CloseBlockTemplatePicker
        | Action::MoveBlockTemplatePickerCursor(_)
        | Action::ConfirmBlockTemplatePicker
        | Action::OpenTabPicker
        | Action::CloseTabPicker
        | Action::MoveTabPickerCursor(_)
        | Action::ConfirmTabPicker => {
            crate::input::apply::pickers::apply_pickers(app, action, recording)
        }
        Action::JumpNextBlock
        | Action::JumpPrevBlock
        | Action::RerunLastBlock
        | Action::WriteAll
        | Action::ReselectVisual
        | Action::ScrollCursorTo(_) => {
            crate::input::apply::navigation::apply_navigation(app, action, recording)
        }
        Action::Window(_) => crate::input::apply::window::apply_window(app, action, recording),
        Action::FocusSwap
        | Action::TabGoto(..)
        | Action::TabNext
        | Action::TabPrev
        | Action::TreeActivate
        | Action::TreeCollapse
        | Action::TreeCreate
        | Action::TreeDelete
        | Action::TreePromptBackspace
        | Action::TreePromptCancel
        | Action::TreePromptChar(..)
        | Action::TreePromptCursorEnd
        | Action::TreePromptCursorHome
        | Action::TreePromptCursorLeft
        | Action::TreePromptCursorRight
        | Action::TreePromptDelete
        | Action::TreePromptExecute
        | Action::TreeRefresh
        | Action::TreeRename
        | Action::TreeSelectFirst
        | Action::TreeSelectLast
        | Action::TreeSelectNext
        | Action::TreeSelectPrev
        | Action::TreeToggle => {
            crate::input::apply::tree_nav::apply_tree_nav(app, action, recording)
        }
        Action::CloseBlockHistory
        | Action::CloseContentSearch
        | Action::CloseDbExportPicker
        | Action::CloseDbSettingsModal
        | Action::CloseHelp
        | Action::CmdlineBackspace
        | Action::CmdlineCancel
        | Action::CmdlineChar(..)
        | Action::CmdlineCursorEnd
        | Action::CmdlineCursorHome
        | Action::CmdlineCursorLeft
        | Action::CmdlineCursorRight
        | Action::CmdlineDelete
        | Action::CmdlineExecute
        | Action::ConfirmContentSearch
        | Action::ConfirmDbExportPicker
        | Action::ConfirmDbSettingsModal
        | Action::ContentSearchBackspace
        | Action::ContentSearchChar(..)
        | Action::ContentSearchCursorEnd
        | Action::ContentSearchCursorHome
        | Action::ContentSearchCursorLeft
        | Action::ContentSearchCursorRight
        | Action::ContentSearchDelete
        | Action::CopyAsCurl
        | Action::CycleDisplayMode
        | Action::DbSettingsBackspace
        | Action::DbSettingsChar(..)
        | Action::DbSettingsCursorEnd
        | Action::DbSettingsCursorHome
        | Action::DbSettingsCursorLeft
        | Action::DbSettingsCursorRight
        | Action::DbSettingsDelete
        | Action::DbSettingsFocusNext
        | Action::DbSettingsFocusPrev
        | Action::DeleteBackward
        | Action::DeleteForward
        | Action::EnterCmdline
        | Action::EnterInsert(..)
        | Action::EnterQuickOpen
        | Action::EnterSearch(..)
        | Action::ExitInsert
        | Action::ExplainBlock
        | Action::FenceEditBackspace
        | Action::FenceEditCancel
        | Action::FenceEditChar(..)
        | Action::FenceEditConfirm
        | Action::FenceEditCursorEnd
        | Action::FenceEditCursorHome
        | Action::FenceEditCursorLeft
        | Action::FenceEditCursorRight
        | Action::FenceEditDelete
        | Action::InsertChar(..)
        | Action::InsertNewline
        | Action::Motion(..)
        | Action::MoveBlockHistoryCursor(..)
        | Action::MoveContentSearchCursor(..)
        | Action::MoveDbExportPickerCursor(..)
        | Action::Noop
        | Action::OpenBlockHistory
        | Action::OpenContentSearch
        | Action::OpenDbExportPicker
        | Action::OpenDbSettingsModal
        | Action::OpenFenceEditAlias
        | Action::OpenHelp
        | Action::QuickOpenBackspace
        | Action::QuickOpenCancel
        | Action::QuickOpenChar(..)
        | Action::QuickOpenCursorEnd
        | Action::QuickOpenCursorHome
        | Action::QuickOpenCursorLeft
        | Action::QuickOpenCursorRight
        | Action::QuickOpenDelete
        | Action::QuickOpenExecute
        | Action::QuickOpenSelectNext
        | Action::QuickOpenSelectPrev
        | Action::Quit
        | Action::Redo
        | Action::RepeatChange(..)
        | Action::RunBlock
        | Action::SearchBackspace
        | Action::SearchCancel
        | Action::SearchChar(..)
        | Action::SearchCursorEnd
        | Action::SearchCursorHome
        | Action::SearchCursorLeft
        | Action::SearchCursorRight
        | Action::SearchDelete
        | Action::SearchExecute
        | Action::SearchRepeat { .. }
        | Action::Undo
        | Action::WriteFile => crate::input::apply::misc::apply_misc(app, action, recording),
    }
}
