use crossterm::event::KeyEvent;

use crate::app::{App, StatusKind};
use crate::buffer::Cursor;
// `Segment` has no production caller left in dispatch (the appliers
// that used it moved under `crate::input::apply` in fase 1 p5); the
// dispatch `mod tests` (`use super::*`) still references it, so it
// stays in scope behind `#[allow(unused_imports)]`.
#[allow(unused_imports)]
use crate::buffer::Segment;
use crate::input::block_swap::{action_needs_block_swap, InBlockSwap};
use crate::tree::{TreePrompt, TreePromptKind};
use crate::vim::change::ChangeRecord;
use crate::vim::ex::{self, ExResult};
use crate::vim::insert::{position_for_insert, recoil_after_exit};
use crate::vim::mode::Mode;
use crate::vim::motions;
use crate::vim::parser::{
    parse_block_history, parse_block_template_picker, parse_cmdline, parse_connection_picker,
    parse_content_search, parse_db_confirm_run, parse_db_export_picker, parse_db_row_detail,
    parse_db_settings_modal, parse_environment_picker, parse_fence_edit, parse_help,
    parse_http_response_detail, parse_insert, parse_normal, parse_quickopen, parse_search,
    parse_tab_picker, parse_tree, parse_tree_prompt, parse_visual, Action, InsertPos, Motion,
    Operator,
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

// completion-popup helpers moved to
// `crate::input::apply::completion` (fase 1 p5c). The popup-key
// appliers are now reached via the `apply_action` completion group
// (`apply_completion`, fase 1 p6b); only the three the `dispatch`
// router itself still calls stay re-exported here.
pub(crate) use crate::input::apply::completion::{
    force_open_completion_popup, parse_completion_popup_key, refresh_completion_popup,
};

/// Run an action against the app. `recording` toggles whether the
/// resulting change updates `last_change` — `.` replay sets it to
/// `false` so a `.` after a `.` doesn't trample its own record.
fn apply_action(app: &mut App, action: Action, recording: bool) {
    match action {
        Action::Noop => {}
        Action::Quit => {
            app.should_quit = true;
        }
        Action::Motion(m, count) => {
            // When the row-detail modal is open `app.document_mut()`
            // redirects to its body doc, so the motion engine drives
            // the modal's cursor automatically. Skip the editor-only
            // book-keeping (paginated-result prefetch, viewport
            // refresh) when the modal owns the focus.
            let in_modal = app.vim.mode == Mode::DbRowDetail;
            if !in_modal && matches!(m, Motion::Down) {
                maybe_prefetch_db_more_rows(app);
            }
            let viewport = app.viewport_height();
            if let Some(doc) = app.document_mut() {
                motions::apply(m, doc, count, viewport);
            }
            if m.is_find() {
                app.vim.last_find = Some(m);
            }
            if !in_modal {
                app.refresh_viewport_for_cursor();
            }
        }
        Action::EnterInsert(pos) => {
            if let Some(doc) = app.document_mut() {
                doc.snapshot();
                position_for_insert(doc, pos);
            }
            app.vim.enter_insert();
            app.vim.insert_session.start_plain(pos);
            app.refresh_viewport_for_cursor();
        }
        Action::EnterVisual => {
            if let Some(doc) = app.document() {
                let cur = doc.cursor();
                app.vim.enter_visual(cur);
            }
        }
        Action::EnterVisualLine => {
            if let Some(doc) = app.document() {
                let cur = doc.cursor();
                app.vim.enter_visual_line(cur);
            }
        }
        Action::ExitVisual => {
            return_from_visual(app);
        }
        Action::VisualSwap => {
            if let (Some(anchor), Some(doc)) = (app.vim.visual_anchor, app.document_mut()) {
                let cur = doc.cursor();
                doc.set_cursor(anchor);
                app.vim.visual_anchor = Some(cur);
                app.refresh_viewport_for_cursor();
            }
        }
        Action::VisualOperator(op) => apply_visual_operator(app, op, recording),
        Action::VisualSelectTextObject(textobj) => {
            apply_visual_select_textobject(app, textobj);
        }
        Action::RunBlock => crate::commands::db::apply_run_block(app),
        Action::OpenDbRowDetail => apply_open_result_detail(app),
        Action::CloseDbRowDetail => apply_close_db_row_detail(app),
        Action::CopyDbRowDetailJson => apply_copy_db_row_detail_json(app),
        Action::CloseHttpResponseDetail => apply_close_http_response_detail(app),
        Action::CopyHttpResponseBody => apply_copy_http_response_body(app),
        Action::OpenConnectionPicker => {
            if let Err(msg) = open_connection_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::ExplainBlock => crate::commands::db::run_explain(app),
        Action::CopyAsCurl => crate::commands::http::copy_as_curl(app),
        Action::CycleDisplayMode => crate::commands::db::cycle_display_mode(app),
        Action::CloseConnectionPicker => apply_close_connection_picker(app),
        Action::MoveConnectionPickerCursor(delta) => {
            apply_move_connection_picker_cursor(app, delta)
        }
        Action::ConfirmConnectionPicker => apply_confirm_connection_picker(app),
        Action::DeleteConnectionInPicker => apply_delete_connection_in_picker(app),
        Action::OpenDbExportPicker => {
            if let Err(msg) = crate::commands::db::open_export_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseDbExportPicker => crate::commands::db::close_export_picker(app),
        Action::MoveDbExportPickerCursor(delta) => {
            crate::commands::db::move_export_picker_cursor(app, delta)
        }
        Action::ConfirmDbExportPicker => crate::commands::db::confirm_export_picker(app),
        Action::OpenDbSettingsModal => {
            if let Err(msg) = crate::commands::db::open_db_settings_modal(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseDbSettingsModal => crate::commands::db::close_db_settings_modal(app),
        Action::ConfirmDbSettingsModal => crate::commands::db::confirm_db_settings_modal(app),
        Action::DbSettingsFocusNext => crate::commands::db::db_settings_focus_step(app, 1),
        Action::DbSettingsFocusPrev => crate::commands::db::db_settings_focus_step(app, -1),
        Action::DbSettingsChar(c) => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().insert_char(c);
            }
        }
        Action::DbSettingsBackspace => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().delete_before();
            }
        }
        Action::DbSettingsDelete => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().delete_after();
            }
        }
        Action::DbSettingsCursorLeft => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().move_left();
            }
        }
        Action::DbSettingsCursorRight => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().move_right();
            }
        }
        Action::DbSettingsCursorHome => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().move_home();
            }
        }
        Action::DbSettingsCursorEnd => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().move_end();
            }
        }
        Action::OpenBlockHistory => {
            if let Err(msg) = crate::commands::http::open_block_history(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseBlockHistory => crate::commands::http::close_block_history(app),
        Action::MoveBlockHistoryCursor(delta) => {
            crate::commands::http::move_block_history_cursor(app, delta)
        }
        Action::OpenContentSearch => {
            if let Err(msg) = crate::commands::search::open_content_search(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseContentSearch => crate::commands::search::close_content_search(app),
        Action::ConfirmContentSearch => crate::commands::search::confirm_content_search(app),
        Action::MoveContentSearchCursor(delta) => {
            crate::commands::search::move_content_search_cursor(app, delta)
        }
        Action::ContentSearchChar(c) => crate::commands::search::content_search_char(app, c),
        Action::ContentSearchBackspace => crate::commands::search::content_search_backspace(app),
        Action::ContentSearchDelete => crate::commands::search::content_search_delete(app),
        Action::ContentSearchCursorLeft => {
            if let Some(s) = app.content_search.as_mut() {
                s.query.move_left();
            }
        }
        Action::ContentSearchCursorRight => {
            if let Some(s) = app.content_search.as_mut() {
                s.query.move_right();
            }
        }
        Action::ContentSearchCursorHome => {
            if let Some(s) = app.content_search.as_mut() {
                s.query.move_home();
            }
        }
        Action::ContentSearchCursorEnd => {
            if let Some(s) = app.content_search.as_mut() {
                s.query.move_end();
            }
        }
        Action::OpenEnvironmentPicker => {
            if let Err(msg) = open_environment_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseEnvironmentPicker => apply_close_environment_picker(app),
        Action::MoveEnvironmentPickerCursor(delta) => {
            apply_move_environment_picker_cursor(app, delta)
        }
        Action::ConfirmEnvironmentPicker => apply_confirm_environment_picker(app),
        Action::OpenHelp => {
            app.help_visible = true;
            app.vim.mode = Mode::Help;
            app.vim.reset_pending();
        }
        Action::CloseHelp => {
            app.help_visible = false;
            app.vim.enter_normal();
        }
        Action::JumpNextBlock => apply_jump_block(app, JumpDir::Next),
        Action::JumpPrevBlock => apply_jump_block(app, JumpDir::Prev),
        Action::RerunLastBlock => apply_rerun_last_block(app),
        Action::WriteFile => {
            // `<C-s>` — same code path as `:w`, status reporting and
            // all. Routed through `ex::execute` (rather than the
            // string-based `ex::run`) to skip a redundant parse.
            match ex::execute(app, ex::ExCmd::Write) {
                ex::ExResult::Ok(msg) => app.set_status(StatusKind::Info, msg),
                ex::ExResult::Err(msg) => app.set_status(StatusKind::Error, msg),
                _ => {}
            }
        }
        Action::WriteAll => apply_write_all(app),
        Action::ReselectVisual => apply_reselect_visual(app),
        Action::ScrollCursorTo(pos) => apply_scroll_cursor_to(app, pos),
        Action::OpenBlockTemplatePicker => {
            app.block_template_picker = Some(crate::app::BlockTemplatePickerState::new());
            app.vim.mode = Mode::BlockTemplatePicker;
            app.vim.reset_pending();
        }
        Action::CloseBlockTemplatePicker => {
            app.block_template_picker = None;
            app.vim.enter_normal();
        }
        Action::MoveBlockTemplatePickerCursor(delta) => {
            apply_move_block_template_picker_cursor(app, delta)
        }
        Action::ConfirmBlockTemplatePicker => apply_confirm_block_template_picker(app),
        Action::OpenTabPicker => apply_open_tab_picker(app),
        Action::CloseTabPicker => {
            app.tab_picker = None;
            app.vim.enter_normal();
        }
        Action::MoveTabPickerCursor(delta) => apply_move_tab_picker_cursor(app, delta),
        Action::ConfirmTabPicker => apply_confirm_tab_picker(app),
        Action::CompletionNext
        | Action::CompletionPrev
        | Action::CompletionAccept
        | Action::CompletionDismiss
        | Action::ConfirmDbRun
        | Action::CancelDbRun => {
            crate::input::apply::completion::apply_completion(app, action, recording)
        }
        Action::ExitInsert => {
            // Recoil the cursor one column (vim's `<Esc>` semantics)
            // and flip the mode. The "did the user just finish a
            // fence?" reparse is handled at the dispatch top level —
            // running it here would fire while the in-block swap is
            // still pretending the block is a Prose segment, which
            // splices the synthetic prose back into the doc and
            // jumps the cursor out of the block.
            if let Some(doc) = app.document_mut() {
                recoil_after_exit(doc);
            }
            app.vim.enter_normal();
            if recording {
                if let Some(record) = app.vim.insert_session.finish() {
                    app.vim.last_change = Some(record);
                }
            } else {
                // Discard the in-flight session without overwriting the
                // existing `last_change` — replay path.
                let _ = app.vim.insert_session.finish();
            }
        }
        Action::InsertChar(c) => {
            if let Some(doc) = app.document_mut() {
                doc.insert_char_at_cursor(c);
            }
            app.vim.insert_session.push_char(c);
        }
        Action::InsertNewline => {
            if let Some(doc) = app.document_mut() {
                doc.insert_newline_at_cursor();
            }
            app.vim.insert_session.push_newline();
            app.refresh_viewport_for_cursor();
        }
        Action::DeleteBackward => {
            if let Some(doc) = app.document_mut() {
                doc.delete_char_before_cursor();
            }
            app.vim.insert_session.pop_char();
        }
        Action::DeleteForward => {
            if let Some(doc) = app.document_mut() {
                doc.delete_char_at_cursor();
            }
        }
        Action::EnterCmdline => {
            app.vim.enter_cmdline();
        }
        Action::CmdlineChar(c) => {
            app.vim.cmdline_push(c);
        }
        Action::CmdlineBackspace => {
            // Empty buffer + backspace exits the prompt — same as `<Esc>`.
            if !app.vim.cmdline_pop() {
                app.vim.enter_normal();
            }
        }
        Action::CmdlineDelete => {
            app.vim.cmdline.delete_after();
        }
        Action::CmdlineCursorLeft => app.vim.cmdline.move_left(),
        Action::CmdlineCursorRight => app.vim.cmdline.move_right(),
        Action::CmdlineCursorHome => app.vim.cmdline.move_home(),
        Action::CmdlineCursorEnd => app.vim.cmdline.move_end(),
        Action::CmdlineCancel => {
            app.vim.enter_normal();
        }
        Action::CmdlineExecute => {
            let buf = app.vim.cmdline.take();
            app.vim.enter_normal();
            match ex::run(app, &buf) {
                ExResult::Ok(msg) => app.set_status(StatusKind::Info, msg),
                ExResult::Err(msg) => app.set_status(StatusKind::Error, msg),
                ExResult::Unknown(s) => app.set_status(
                    StatusKind::Error,
                    format!("E492: not an editor command: {s}"),
                ),
                ExResult::Empty | ExResult::Quit => {}
            }
        }
        Action::OperatorMotion(op, motion, count) => {
            apply_op_motion(app, op, motion, count, recording);
        }
        Action::OperatorLinewise(op, count) => {
            apply_op_linewise(app, op, count, recording);
        }
        Action::OperatorTextObject(op, textobj, count) => {
            apply_op_textobject(app, op, textobj, count, recording);
        }
        Action::Paste(pos, count) => {
            apply_paste(app, pos, count, recording);
        }
        Action::Undo => {
            if let Some(doc) = app.document_mut() {
                if !doc.undo() {
                    app.set_status(StatusKind::Info, "already at oldest change");
                }
            }
            app.refresh_viewport_for_cursor();
        }
        Action::Redo => {
            if let Some(doc) = app.document_mut() {
                if !doc.redo() {
                    app.set_status(StatusKind::Info, "already at newest change");
                }
            }
            app.refresh_viewport_for_cursor();
        }
        Action::RepeatChange(count) => {
            replay_last_change(app, count.max(1));
        }
        Action::EnterSearch(forward) => {
            app.vim.enter_search(forward);
        }
        Action::SearchChar(c) => {
            app.vim.search_push(c);
        }
        Action::SearchBackspace => {
            if !app.vim.search_pop() {
                app.vim.enter_normal();
            }
        }
        Action::SearchDelete => {
            app.vim.search_buf.delete_after();
        }
        Action::SearchCursorLeft => app.vim.search_buf.move_left(),
        Action::SearchCursorRight => app.vim.search_buf.move_right(),
        Action::SearchCursorHome => app.vim.search_buf.move_home(),
        Action::SearchCursorEnd => app.vim.search_buf.move_end(),
        Action::SearchCancel => {
            app.vim.enter_normal();
        }
        Action::SearchExecute => {
            let pattern = app.vim.search_buf.take();
            let forward = app.vim.search_forward;
            app.vim.enter_normal();
            execute_search(app, &pattern, forward, /* save = */ true);
        }
        Action::SearchRepeat { reverse } => {
            let Some(pattern) = app.vim.last_search.clone() else {
                app.set_status(StatusKind::Error, "no previous search");
                return;
            };
            let forward = if reverse {
                !app.vim.last_search_forward
            } else {
                app.vim.last_search_forward
            };
            execute_search(app, &pattern, forward, /* save = */ false);
        }
        Action::EnterQuickOpen => {
            let files = list_vault_md_files(&app.vault_path.to_string_lossy());
            app.vim.enter_quickopen(files);
        }
        Action::QuickOpenChar(c) => {
            app.vim.quickopen.push_char(c);
        }
        Action::QuickOpenBackspace => {
            // Empty buffer + backspace closes the modal — same as `<Esc>`.
            if app.vim.quickopen.query.is_empty() {
                app.vim.enter_normal();
            } else {
                app.vim.quickopen.pop_char();
            }
        }
        Action::QuickOpenDelete => app.vim.quickopen.delete_after(),
        Action::QuickOpenCursorLeft => app.vim.quickopen.move_left(),
        Action::QuickOpenCursorRight => app.vim.quickopen.move_right(),
        Action::QuickOpenCursorHome => app.vim.quickopen.move_home(),
        Action::QuickOpenCursorEnd => app.vim.quickopen.move_end(),
        Action::QuickOpenSelectNext => {
            app.vim.quickopen.select_next();
        }
        Action::QuickOpenSelectPrev => {
            app.vim.quickopen.select_prev();
        }
        Action::QuickOpenCancel => {
            app.vim.enter_normal();
        }
        Action::QuickOpenExecute => {
            // Quick Open is the picker — always opens in a new tab (or
            // switches to the existing tab if already open). The vim
            // ex command `:e <path>` is the explicit "replace current"
            // path for users who want that.
            let chosen = app.vim.quickopen.chosen_path();
            app.vim.enter_normal();
            if let Some(path) = chosen {
                match app.open_in_new_tab(path) {
                    Ok(msg) => app.set_status(StatusKind::Info, msg),
                    Err(msg) => app.set_status(StatusKind::Error, msg),
                }
            }
        }
        Action::Window(_) => crate::input::apply::window::apply_window(app, action, recording),
        Action::TreeToggle => {
            if app.tree.visible {
                app.tree.visible = false;
                if app.vim.mode == Mode::Tree {
                    app.vim.enter_normal();
                }
            } else {
                app.tree.visible = true;
                app.tree.refresh(&app.vault_path);
                app.vim.mode = Mode::Tree;
            }
        }
        Action::FocusSwap => {
            if !app.tree.visible {
                return;
            }
            if app.vim.mode == Mode::Tree {
                app.vim.enter_normal();
            } else if app.vim.mode == Mode::Normal {
                app.vim.mode = Mode::Tree;
            }
        }
        Action::TreeSelectNext => app.tree.select_next(),
        Action::TreeSelectPrev => app.tree.select_prev(),
        Action::TreeSelectFirst => app.tree.select_first(),
        Action::TreeSelectLast => app.tree.select_last(),
        Action::TreeRefresh => {
            let vault = app.vault_path.clone();
            app.tree.refresh(&vault);
        }
        Action::TreeCollapse => {
            if app.tree.collapse_parent() {
                let vault = app.vault_path.clone();
                app.tree.refresh(&vault);
            }
        }
        Action::TreeActivate => {
            let Some(node) = app.tree.current().cloned() else {
                return;
            };
            if node.is_dir {
                if app.tree.toggle_expand() {
                    let vault = app.vault_path.clone();
                    app.tree.refresh(&vault);
                }
            } else {
                // Tree-driven open mirrors the modal: every Enter opens
                // a new tab (or switches to an existing one). Use `:e
                // <path>` if you want the vim-style replace behavior.
                let path = std::path::PathBuf::from(&node.path);
                match app.open_in_new_tab(path) {
                    Ok(msg) => {
                        app.set_status(StatusKind::Info, msg);
                        // Hand focus back to the editor on successful open —
                        // matches how netrw exits the tree after Enter.
                        app.vim.enter_normal();
                    }
                    Err(msg) => app.set_status(StatusKind::Error, msg),
                }
            }
        }
        Action::TabNext => {
            // When the cursor sits on a result row, `gt` cycles
            // the result-panel tab (Result → Messages → Plan →
            // Stats → Result) instead of switching editor tabs —
            // the editor-tab swap wouldn't be useful from inside a
            // table, and the result-panel needs *some* keyboard
            // affordance.
            if matches!(
                app.document().map(|d| d.cursor()),
                Some(Cursor::InBlockResult { .. })
            ) {
                app.db_result_tab = app.db_result_tab.next();
            } else {
                app.next_tab();
                app.refresh_viewport_for_cursor();
            }
        }
        Action::TabPrev => {
            if matches!(
                app.document().map(|d| d.cursor()),
                Some(Cursor::InBlockResult { .. })
            ) {
                app.db_result_tab = app.db_result_tab.prev();
            } else {
                app.prev_tab();
                app.refresh_viewport_for_cursor();
            }
        }
        Action::TabGoto(n) => {
            app.goto_tab(n);
            app.refresh_viewport_for_cursor();
        }
        Action::TreeCreate => {
            // Open the in-tree prompt anchored to the selected folder
            // (or the parent of the selected file). The user types
            // either a filename (e.g. `notes.md`) or a name with
            // trailing `/` (e.g. `subdir/`) to make a folder.
            let dir = match app.tree.current() {
                Some(node) if node.is_dir => node.path.clone(),
                Some(node) => match std::path::Path::new(&node.path).parent() {
                    Some(p) if !p.as_os_str().is_empty() => p.display().to_string(),
                    _ => String::new(),
                },
                None => String::new(),
            };
            app.tree.prompt = Some(TreePrompt::new(
                TreePromptKind::Create { dir },
                String::new(),
            ));
            app.vim.mode = Mode::TreePrompt;
        }
        Action::TreeRename => {
            let Some(node) = app.tree.current() else {
                return;
            };
            // Pre-fill the buffer with the source path so the user
            // edits the destination in place. Allowed for files and
            // folders alike — `rename_path` handles both.
            let path = node.path.clone();
            app.tree.prompt = Some(TreePrompt::new(
                TreePromptKind::Rename { from: path.clone() },
                path,
            ));
            app.vim.mode = Mode::TreePrompt;
        }
        Action::TreeDelete => {
            let Some(node) = app.tree.current() else {
                return;
            };
            app.tree.prompt = Some(TreePrompt::new(
                TreePromptKind::Delete {
                    target: node.path.clone(),
                },
                String::new(),
            ));
            app.vim.mode = Mode::TreePrompt;
        }
        Action::TreePromptChar(c) => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.insert_char(c);
            }
        }
        Action::TreePromptBackspace => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                if !prompt.input.delete_before() {
                    // Empty buffer + backspace acts like cancel.
                    app.tree.prompt = None;
                    app.vim.mode = Mode::Tree;
                }
            } else {
                app.vim.mode = Mode::Tree;
            }
        }
        Action::TreePromptDelete => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.delete_after();
            }
        }
        Action::TreePromptCursorLeft => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_left();
            }
        }
        Action::TreePromptCursorRight => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_right();
            }
        }
        Action::TreePromptCursorHome => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_home();
            }
        }
        Action::TreePromptCursorEnd => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_end();
            }
        }
        Action::TreePromptCancel => {
            app.tree.prompt = None;
            app.vim.mode = Mode::Tree;
        }
        Action::TreePromptExecute => {
            let Some(prompt) = app.tree.prompt.take() else {
                app.vim.mode = Mode::Tree;
                return;
            };
            app.vim.mode = Mode::Tree;
            run_tree_prompt(app, prompt);
        }
        Action::OpenFenceEditAlias => crate::commands::db::open_fence_edit_alias(app),
        Action::FenceEditChar(c) => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.insert_char(c);
            }
        }
        Action::FenceEditBackspace => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                // Backspace on an empty buffer cancels — same affordance
                // as the tree prompt; users can hold backspace to bail.
                if !prompt.input.delete_before() {
                    app.fence_edit = None;
                    app.vim.enter_normal();
                }
            } else {
                app.vim.enter_normal();
            }
        }
        Action::FenceEditDelete => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.delete_after();
            }
        }
        Action::FenceEditCursorLeft => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.move_left();
            }
        }
        Action::FenceEditCursorRight => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.move_right();
            }
        }
        Action::FenceEditCursorHome => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.move_home();
            }
        }
        Action::FenceEditCursorEnd => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.move_end();
            }
        }
        Action::FenceEditCancel => {
            app.fence_edit = None;
            app.vim.enter_normal();
            app.set_status(StatusKind::Info, "edit cancelled");
        }
        Action::FenceEditConfirm => crate::commands::db::confirm_fence_edit(app),
    }
}

// tree-prompt / search / DB-prefetch / block-jump / scroll /
// write-all appliers moved to `crate::input::apply::navigation`
// (fase 1 p5f). First group still called by the untouched
// `apply_action`; `should_prefetch` has no production caller left
// here but the dispatch `mod tests` references it.
#[allow(unused_imports)]
pub(crate) use crate::input::apply::navigation::should_prefetch;
pub(crate) use crate::input::apply::navigation::{
    apply_jump_block, apply_rerun_last_block, apply_reselect_visual, apply_scroll_cursor_to,
    apply_write_all, execute_search, list_vault_md_files, maybe_prefetch_db_more_rows,
    run_tree_prompt, JumpDir,
};

// operator / paste / visual-operator appliers moved to
// `crate::input::apply::operator` (fase 1 p5b). Re-exported so the
// untouched `apply_action` router + the `replay_*` helpers keep
// resolving them via their bare call sites.
pub(crate) use crate::input::apply::operator::{
    apply_op_linewise, apply_op_motion, apply_op_textobject, apply_paste, apply_visual_operator,
    apply_visual_select_textobject, return_from_visual,
};

// ───────────── block execution (`r` in normal) ─────────────
//
// `apply_run_block`, `run_db_block_inner`, `spawn_db_query`,
// `handle_db_block_result`, `cancel_running_query`, and
// `load_more_db_block` all moved to `crate::commands::db`. The
// vim-side action handlers (`apply_confirm_db_run`,
// `maybe_prefetch_db_more_rows`, the top-level dispatcher) call
// directly into that module.

// connection-picker appliers moved to `crate::input::apply::pickers`
// (fase 1 p5d). Re-exported so the untouched `apply_action` resolves them.
pub(crate) use crate::input::apply::pickers::{
    apply_close_connection_picker, apply_confirm_connection_picker,
    apply_delete_connection_in_picker, apply_move_connection_picker_cursor, open_connection_picker,
};

// tab / template / environment picker appliers moved to
// `crate::input::apply::pickers` (fase 1 p5d). Re-exported so the
// untouched `apply_action` keeps resolving them.
pub(crate) use crate::input::apply::pickers::{
    apply_close_environment_picker, apply_confirm_block_template_picker,
    apply_confirm_environment_picker, apply_confirm_tab_picker,
    apply_move_block_template_picker_cursor, apply_move_environment_picker_cursor,
    apply_move_tab_picker_cursor, apply_open_tab_picker, open_environment_picker,
};

// result-detail modal appliers moved to
// `crate::input::apply::modal_detail` (fase 1 p5e). The first group is
// still called by the untouched `apply_action`; the second group has
// no production caller left here but the dispatch `mod tests`
// (`use super::*`) references it, so it stays re-exported behind
// `#[allow(unused_imports)]`.
pub(crate) use crate::input::apply::modal_detail::{
    apply_close_db_row_detail, apply_close_http_response_detail, apply_copy_db_row_detail_json,
    apply_copy_http_response_body, apply_open_result_detail,
};
#[allow(unused_imports)]
pub(crate) use crate::input::apply::modal_detail::{
    build_http_response_body_text, build_http_response_modal_title, format_size,
};

// window / split commands moved to `crate::input::apply::window`
// (fase 1 p5a). The `apply_action` window group routes to
// `crate::input::apply::window::apply_window` directly (fase 1 p6a),
// so no facade re-export is needed here anymore.

// ───────────── . repeat ─────────────

fn replay_last_change(app: &mut App, count: usize) {
    let Some(record) = app.vim.last_change.clone() else {
        return;
    };
    for _ in 0..count {
        replay_once(app, record.clone());
    }
}

fn replay_once(app: &mut App, record: ChangeRecord) {
    match record {
        ChangeRecord::OperatorMotion(op, motion, c) => {
            apply_op_motion(app, op, motion, c, false);
        }
        ChangeRecord::OperatorLinewise(op, c) => {
            apply_op_linewise(app, op, c, false);
        }
        ChangeRecord::OperatorTextObject(op, t, c) => {
            apply_op_textobject(app, op, t, c, false);
        }
        ChangeRecord::Paste(pos, c) => {
            apply_paste(app, pos, c, false);
        }
        ChangeRecord::Insert { pos, typed } => {
            replay_insert_session(app, Some(pos), None, &typed);
        }
        ChangeRecord::ChangeMotion {
            motion,
            op_count,
            typed,
        } => {
            apply_op_motion(app, Operator::Change, motion, op_count, false);
            replay_typed(app, &typed);
            // Replay's ExitInsert fires through dispatch only via real
            // keystrokes; here we exit synthetically.
            apply_action(app, Action::ExitInsert, false);
        }
        ChangeRecord::ChangeLinewise { op_count, typed } => {
            apply_op_linewise(app, Operator::Change, op_count, false);
            replay_typed(app, &typed);
            apply_action(app, Action::ExitInsert, false);
        }
        ChangeRecord::ChangeTextObject {
            textobj,
            op_count,
            typed,
        } => {
            apply_op_textobject(app, Operator::Change, textobj, op_count, false);
            replay_typed(app, &typed);
            apply_action(app, Action::ExitInsert, false);
        }
    }
}

fn replay_insert_session(app: &mut App, pos: Option<InsertPos>, _origin: Option<()>, typed: &str) {
    if let Some(p) = pos {
        apply_action(app, Action::EnterInsert(p), false);
    }
    replay_typed(app, typed);
    apply_action(app, Action::ExitInsert, false);
}

fn replay_typed(app: &mut App, typed: &str) {
    for c in typed.chars() {
        if c == '\n' {
            apply_action(app, Action::InsertNewline, false);
        } else {
            apply_action(app, Action::InsertChar(c), false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_prefetch_skips_when_backend_says_done() {
        // Cursor near the bottom but the server already exhausted
        // pages — no further fetch should fire.
        assert!(!should_prefetch(95, 100, false, 5));
        assert!(!should_prefetch(99, 100, false, 5));
    }

    #[test]
    fn should_prefetch_waits_for_threshold_when_more_pages_exist() {
        // Plenty of headroom: don't trigger.
        assert!(!should_prefetch(0, 100, true, 5));
        assert!(!should_prefetch(94, 100, true, 5));
        // Within the threshold band: trigger.
        assert!(should_prefetch(95, 100, true, 5));
        assert!(should_prefetch(99, 100, true, 5));
        // Past the loaded set (the engine reaches this momentarily
        // when motion overshoots before append finishes).
        assert!(should_prefetch(100, 100, true, 5));
    }

    #[test]
    fn should_prefetch_fires_immediately_for_small_initial_pages() {
        // 3 rows back, threshold 5, has_more true → trigger from
        // row 0 (page is smaller than the prefetch window).
        assert!(should_prefetch(0, 3, true, 5));
        assert!(should_prefetch(2, 3, true, 5));
    }

    #[test]
    fn should_prefetch_handles_empty_set() {
        // Defensive: no rows + has_more shouldn't crash on the
        // arithmetic and shouldn't fire (cursor can't be in an
        // empty result anyway).
        assert!(should_prefetch(0, 0, true, 5));
        assert!(!should_prefetch(0, 0, false, 5));
    }

    #[test]
    fn format_size_picks_unit_by_magnitude() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 kB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
    }

    #[test]
    fn build_http_response_body_text_lays_out_status_headers_and_body() {
        // Synthesize a fenced HTTP block + cached_result mimicking the
        // shape `http_response_to_json` emits, then check the rendered
        // modal body has the section headings, status line and an
        // indented body line.
        let block_md = "```http alias=req1\nGET https://api.example.com/users\n```\n";
        let doc = crate::buffer::Document::from_markdown(block_md).unwrap();
        let mut block = doc
            .segments()
            .iter()
            .find_map(|s| match s {
                Segment::Block(b) => Some(b.clone()),
                _ => None,
            })
            .expect("block segment in parsed doc");
        block.cached_result = Some(serde_json::json!({
            "status": 200,
            "status_text": "OK",
            "headers": [
                {"key": "content-type", "value": "application/json"},
                {"key": "x-trace", "value": "abc"},
            ],
            "cookies": [],
            "body": serde_json::json!({"ok": true}),
            "size_bytes": 42,
            "timing": {"total_ms": 142, "ttfb_ms": 30},
        }));

        let text = build_http_response_body_text(&block).expect("body text");
        // Status line is line zero with code and human label.
        assert!(text.starts_with("200 OK"), "got: {text}");
        assert!(text.contains("142 ms"));
        assert!(text.contains("42 B"));
        // Section heading + a header pair.
        assert!(text.contains("\nHeaders\n"));
        assert!(text.contains("  content-type: application/json"));
        // Body section + pretty JSON line.
        assert!(text.contains("\nBody\n"));
        assert!(text.contains("  \"ok\": true"));
    }

    #[test]
    fn build_http_response_modal_title_includes_status_size_alias() {
        let block_md = "```http alias=login\nPOST https://example.com/login\n```\n";
        let doc = crate::buffer::Document::from_markdown(block_md).unwrap();
        let mut block = doc
            .segments()
            .iter()
            .find_map(|s| match s {
                Segment::Block(b) => Some(b.clone()),
                _ => None,
            })
            .expect("block segment in parsed doc");
        block.cached_result = Some(serde_json::json!({
            "status": 201,
            "size_bytes": 1500,
        }));
        let title = build_http_response_modal_title(&block);
        // Padded with spaces (it's a window title, ratatui paints it
        // with a space-buffer so it doesn't kiss the corner).
        assert!(title.starts_with(' '));
        assert!(title.ends_with(' '));
        assert!(title.contains("201"));
        assert!(title.contains("1.5 kB"));
        assert!(title.contains("login"));
    }

    /// Build a Document with prose / block / prose / block / prose
    /// to exercise `apply_jump_block` against the segment iterator.
    fn doc_with_two_blocks() -> crate::buffer::Document {
        let md = "intro\n\n```http alias=a\nGET https://a.test\n```\n\nmid\n\n```http alias=b\nGET https://b.test\n```\n\nend\n";
        crate::buffer::Document::from_markdown(md).unwrap()
    }

    fn block_indices(doc: &crate::buffer::Document) -> Vec<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
            .collect()
    }

    /// Stand-alone reimplementation of the navigator inside
    /// `apply_jump_block` — same predicate (skip current, no wrap),
    /// no `App` plumbing. Lets the test cover the search rule
    /// without spinning up a full `App`.
    fn next_block(doc: &crate::buffer::Document, cur: usize) -> Option<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .skip(cur + 1)
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
    }

    fn prev_block(doc: &crate::buffer::Document, cur: usize) -> Option<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .take(cur)
            .rev()
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
    }

    #[test]
    fn jump_next_block_walks_forward_no_wrap() {
        let doc = doc_with_two_blocks();
        let blocks = block_indices(&doc);
        assert_eq!(blocks.len(), 2);
        // From the first prose segment (idx 0) → first block.
        assert_eq!(next_block(&doc, 0), Some(blocks[0]));
        // From the first block → second block.
        assert_eq!(next_block(&doc, blocks[0]), Some(blocks[1]));
        // From the second block → no more (no wrap).
        assert_eq!(next_block(&doc, blocks[1]), None);
    }

    #[test]
    fn jump_prev_block_walks_backward_no_wrap() {
        let doc = doc_with_two_blocks();
        let blocks = block_indices(&doc);
        // From the second block → first block.
        assert_eq!(prev_block(&doc, blocks[1]), Some(blocks[0]));
        // From the first block → no earlier block.
        assert_eq!(prev_block(&doc, blocks[0]), None);
        // From the last prose segment → previous block.
        let last = doc.segments().len() - 1;
        assert_eq!(prev_block(&doc, last), Some(blocks[1]));
    }

    #[test]
    fn jump_block_no_blocks_yields_none() {
        let md = "just prose\n\nno blocks at all\n";
        let doc = crate::buffer::Document::from_markdown(md).unwrap();
        assert_eq!(next_block(&doc, 0), None);
        assert_eq!(prev_block(&doc, 0), None);
    }
}
