// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
//! Catch-all `apply_action` arms that don't belong to a focused
//! domain module: editing (insert/delete), cmdline, search, quick
//! open, tree navigation + prompts, tab switching, DB-settings /
//! export / block-history / content-search modal plumbing, fence
//! edit, help, undo/redo, and the simple one-liners (`Noop`, `Quit`,
//! `Motion`, …).
//!
//! Mechanically split out of the `apply_action` router in
//! `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p6g) — every arm body
//! is copied verbatim. The outer router routes only this group's
//! variants here, so the `unreachable!` is a compile-time-backed
//! invariant.

use crate::app::{App, StatusKind};
use crate::input::action::Action;
use crate::input::apply::navigation::{
    execute_search, list_vault_md_files, maybe_prefetch_db_more_rows,
};
use crate::input::types::Motion;
use crate::vim::ex::{self, ExResult};
use crate::vim::insert::{position_for_insert, recoil_after_exit};
use crate::vim::mode::Mode;
use crate::vim::motions;

pub(crate) fn apply_misc(app: &mut App, action: Action, recording: bool) {
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
        Action::RunBlock => crate::commands::db::apply_run_block(app),
        Action::ExplainBlock => crate::commands::db::run_explain(app),
        Action::CopyAsCurl => crate::commands::http::copy_as_curl(app),
        Action::CycleDisplayMode => crate::commands::db::cycle_display_mode(app),
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
        Action::OpenHelp => {
            app.modal = Some(crate::modal::Modal::Help);
            app.vim.mode = Mode::Modal;
            app.vim.reset_pending();
        }
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
            crate::input::apply::replay::replay_last_change(app, count.max(1));
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
        Action::OpenFenceEditAlias => crate::commands::db::open_fence_edit_alias(app),
        Action::FenceEditChar(c) => {
            if let Some((_, le)) = app.modal.as_mut().and_then(|m| m.as_prompt_mut()) {
                le.insert_char(c);
            }
        }
        Action::FenceEditBackspace => {
            // Backspace on an empty buffer cancels — same affordance
            // as the tree prompt; users can hold backspace to bail.
            let close = match app.modal.as_mut().and_then(|m| m.as_prompt_mut()) {
                Some((_, le)) => !le.delete_before(),
                None => true,
            };
            if close {
                app.modal = None;
                app.vim.enter_normal();
            }
        }
        Action::FenceEditDelete => {
            if let Some((_, le)) = app.modal.as_mut().and_then(|m| m.as_prompt_mut()) {
                le.delete_after();
            }
        }
        Action::FenceEditCursorLeft => {
            if let Some((_, le)) = app.modal.as_mut().and_then(|m| m.as_prompt_mut()) {
                le.move_left();
            }
        }
        Action::FenceEditCursorRight => {
            if let Some((_, le)) = app.modal.as_mut().and_then(|m| m.as_prompt_mut()) {
                le.move_right();
            }
        }
        Action::FenceEditCursorHome => {
            if let Some((_, le)) = app.modal.as_mut().and_then(|m| m.as_prompt_mut()) {
                le.move_home();
            }
        }
        Action::FenceEditCursorEnd => {
            if let Some((_, le)) = app.modal.as_mut().and_then(|m| m.as_prompt_mut()) {
                le.move_end();
            }
        }
        Action::FenceEditCancel => {
            app.modal = None;
            app.vim.enter_normal();
            app.set_status(StatusKind::Info, "edit cancelled");
        }
        Action::FenceEditConfirm => crate::commands::db::confirm_fence_edit(app),
        _ => unreachable!("apply_misc: variante fora do grupo"),
    }
}
