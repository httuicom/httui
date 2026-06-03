// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
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
use crate::input::parser::git_panel::parse_git_panel;
use crate::modal::ModalOutcome;
use crate::vim::mode::Mode;
use crate::vim::parser::{
    parse_cmdline, parse_content_search, parse_db_row_detail, parse_db_settings_modal,
    parse_fence_edit, parse_http_response_detail, parse_insert, parse_normal, parse_quickopen,
    parse_search, parse_tree, parse_tree_prompt, parse_visual,
};

// Test-only imports: the in-file `mod tests` below uses `use
// super::*`, so the symbols its cases reference must resolve in this
// module's scope. They have no production caller here (the appliers
// they exercise live in `crate::input::apply::*`), hence
// `#[allow(unused_imports)]`.
#[cfg(test)]
#[allow(unused_imports)]
use crate::buffer::Segment;
#[cfg(test)]
#[allow(unused_imports)]
use crate::input::apply::modal_detail::{
    build_http_response_body_text, build_http_response_modal_title, format_size,
};
#[cfg(test)]
#[allow(unused_imports)]
use crate::input::apply::navigation::should_prefetch;

// completion-popup helpers moved to
// `crate::input::apply::completion` (fase 1 p5c). The popup-key
// appliers are now reached via the `apply_action` completion group
// (`apply_completion`, fase 1 p6b); only the three the `dispatch`
// router itself still calls stay re-exported here.
pub(crate) use crate::input::apply::completion::{
    force_open_completion_popup, refresh_completion_popup,
};

/// Top-level vim key dispatcher. The app's `handle_key` delegates here.
pub fn dispatch(app: &mut App, key: KeyEvent) {
    use crossterm::event::{KeyCode, KeyModifiers};

    // `Ctrl+Space` in insert mode opens the SQL completion popup.
    // Terminal quirk: most terminals report Ctrl+Space as
    // `KeyCode::Char(' ')` with CONTROL set, some emit the legacy
    // NUL byte form (`Char('\0')`). Accept both.
    if app.vim.mode == Mode::Insert
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(' ') | KeyCode::Char('\0'))
    {
        force_open_completion_popup(app);
        return;
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
        Mode::FenceEdit => parse_fence_edit(key),
        Mode::DbSettings => parse_db_settings_modal(key),
        Mode::ContentSearch => parse_content_search(key),
        Mode::Git => parse_git_panel(key),
        Mode::Modal => {
            handle_modal_key(app, key);
            return;
        }
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

fn handle_modal_key(app: &mut App, key: KeyEvent) {
    let editor_mode = app.config.editor.mode;
    let outcome = match app.modal.as_mut() {
        Some(m) => {
            let mut ctx = crate::modal::ModalKeyCtx {
                vim: &mut app.vim,
                editor_mode,
            };
            m.handle_key_with_ctx(key, &mut ctx)
        }
        None => {
            app.vim.enter_normal();
            return;
        }
    };
    match outcome {
        ModalOutcome::Continue | ModalOutcome::Forward => {}
        ModalOutcome::Close => {
            app.modal = None;
            app.vim.enter_normal();
        }
        ModalOutcome::Emit(action) => {
            apply_action(app, action, false);
        }
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
        | Action::VisualPaste
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
        | Action::ActivateEnvByIndex(_)
        | Action::OpenConnectionsPage
        | Action::CloseConnectionsPage
        | Action::MoveConnectionsPageCursor(_)
        | Action::OpenSessionOverrideForm
        | Action::ClearSessionOverride
        | Action::OpenBlockTemplatePicker
        | Action::CloseBlockTemplatePicker
        | Action::MoveBlockTemplatePickerCursor(_)
        | Action::ConfirmBlockTemplatePicker
        | Action::OpenTabPicker
        | Action::CloseTabPicker
        | Action::MoveTabPickerCursor(_)
        | Action::ConfirmTabPicker
        | Action::OpenVaultPicker
        | Action::CloseVaultPicker
        | Action::MoveVaultPickerCursor(_)
        | Action::ConfirmVaultPicker
        | Action::OpenVaultCreateForm
        | Action::CloseVaultCreateForm
        | Action::VaultCreateFormFocusNext
        | Action::VaultCreateFormFocusPrev
        | Action::VaultCreateFormChar(_)
        | Action::VaultCreateFormBackspace
        | Action::VaultCreateFormSubmit
        | Action::OpenVaultCloneForm
        | Action::CloseVaultCloneForm
        | Action::VaultCloneFormFocusNext
        | Action::VaultCloneFormFocusPrev
        | Action::VaultCloneFormChar(_)
        | Action::VaultCloneFormBackspace
        | Action::VaultCloneFormSubmit
        | Action::OpenVaultOpenPicker
        | Action::CloseVaultOpenPicker
        | Action::MoveVaultOpenPickerCursor(_)
        | Action::VaultOpenPickerEnter
        | Action::VaultOpenPickerOpenAsVault
        | Action::VaultOpenPickerUp
        | Action::CloseVaultMissingSecrets
        | Action::MoveVaultMissingSecretsCursor(_)
        | Action::VaultMissingSecretsEnterEdit
        | Action::VaultMissingSecretsCancelEdit
        | Action::VaultMissingSecretsChar(_)
        | Action::VaultMissingSecretsBackspace
        | Action::VaultMissingSecretsSave
        | Action::VaultMissingSecretsSkip => {
            crate::input::apply::pickers::apply_pickers(app, action, recording)
        }
        Action::OpenConnectionForm
        | Action::OpenConnectionEditForm
        | Action::CloseConnectionForm
        | Action::OpenConnectionDeleteConfirm
        | Action::ConfirmConnectionDelete
        | Action::CancelConnectionDelete
        | Action::ConnectionFormFocusNext
        | Action::ConnectionFormFocusPrev
        | Action::ConnectionFormChar(_)
        | Action::ConnectionFormBackspace
        | Action::ConnectionFormDelete
        | Action::ConnectionFormCursorLeft
        | Action::ConnectionFormCursorRight
        | Action::ConnectionFormCursorHome
        | Action::ConnectionFormCursorEnd
        | Action::ConnectionFormCycleDriver(_)
        | Action::ConnectionFormToggleReadonly
        | Action::ConnectionFormSubmit
        | Action::TestSelectedConnection => {
            crate::input::apply::connection_form::apply_connection_form(app, action)
        }
        Action::OpenEnvsPage
        | Action::CloseEnvsPage
        | Action::EnvsPageFocusToggle
        | Action::EnvsPageFocusEnvs
        | Action::EnvsPageFocusVars
        | Action::EnvsPageMoveEnvCursor(_)
        | Action::EnvsPageMoveVarCursor(_)
        | Action::EnvsPageActivateEnv
        | Action::OpenEnvForm
        | Action::OpenEnvEditForm
        | Action::CloseEnvForm
        | Action::EnvFormChar(_)
        | Action::EnvFormBackspace
        | Action::EnvFormDelete
        | Action::EnvFormCursorLeft
        | Action::EnvFormCursorRight
        | Action::EnvFormHome
        | Action::EnvFormEnd
        | Action::EnvFormSubmit
        | Action::OpenVarForm
        | Action::OpenVarEditForm
        | Action::CloseVarForm
        | Action::VarFormChar(_)
        | Action::VarFormBackspace
        | Action::VarFormDelete
        | Action::VarFormCursorLeft
        | Action::VarFormCursorRight
        | Action::VarFormHome
        | Action::VarFormEnd
        | Action::VarFormFocusNext
        | Action::VarFormFocusPrev
        | Action::VarFormToggleSecret
        | Action::VarFormSubmit
        | Action::OpenEnvDeleteConfirm
        | Action::OpenVarDeleteConfirm
        | Action::ConfirmEnvOrVarDelete
        | Action::CancelEnvOrVarDelete
        | Action::OpenEnvCloneForm
        | Action::CloseEnvCloneForm
        | Action::EnvCloneFormChar(_)
        | Action::EnvCloneFormBackspace
        | Action::EnvCloneFormFocusToggle
        | Action::EnvCloneFormMoveVarCursor(_)
        | Action::EnvCloneFormToggleVar
        | Action::EnvCloneFormToggleAll
        | Action::EnvCloneFormSubmit => crate::input::apply::envs_page::apply_envs(app, action),
        Action::OpenSettings
        | Action::CloseSettingsPage
        | Action::SettingsNextSection
        | Action::SettingsPrevSection
        | Action::SettingsMoveCursor(_)
        | Action::SettingsActivateRow
        | Action::SettingsCancelCapture
        | Action::SettingsCommitCapture(_)
        | Action::SettingsResetBinding => {
            crate::input::apply::settings_page::apply_settings_page(app, action)
        }
        Action::ToggleAppView
        | Action::BlocksPaneNextRegion
        | Action::BlocksPanePrevRegion
        | Action::BlocksPaneJumpRegion(_)
        | Action::BlocksPanePickerChoose(_)
        | Action::BlocksPanePickerCancel
        | Action::BlocksPaneRowUp
        | Action::BlocksPaneRowDown
        | Action::BlocksPaneColLeft
        | Action::BlocksPaneColRight
        | Action::BlocksRegionEnterEdit
        | Action::BlocksRegionEnterEditInsert
        | Action::BlocksRegionCommitEdit
        | Action::BlocksRegionCancelEdit
        | Action::BlocksSaveDraft
        | Action::BlocksNextBlockMotion
        | Action::BlocksPrevBlockMotion
        | Action::BlocksRunFocused
        | Action::BlocksCancelRun
        | Action::BlocksHeaderInsertRow
        | Action::BlocksHeaderDeleteRow
        | Action::BlocksHeaderToggleEnabled
        | Action::BlocksHeaderDeleteConfirm
        | Action::BlocksHeaderDeleteCancel
        | Action::BlocksFieldAdvanceNext
        | Action::BlocksFieldOpenBelow
        | Action::BlocksFieldOpenAbove
        | Action::BlocksResponseNextTab
        | Action::BlocksResponsePrevTab
        | Action::BlocksTreeNewBlock
        | Action::BlocksTreeReorderUp
        | Action::BlocksTreeReorderDown
        | Action::BlocksTreeDeleteBlock
        | Action::BlocksTreeOpenSplitVertical
        | Action::BlocksTreeOpenSplitHorizontal
        | Action::BlocksUnsavedPromptSave
        | Action::BlocksUnsavedPromptDiscard
        | Action::BlocksUnsavedPromptCancel
        | Action::BlocksTabNew
        | Action::BlocksTabClose
        | Action::BlocksTabNext
        | Action::BlocksTabPrev => {
            crate::input::apply::blocks_view::apply_blocks_view(app, action)
        }
        Action::JumpNextBlock
        | Action::JumpPrevBlock
        | Action::RerunLastBlock
        | Action::WriteAll
        | Action::ReselectVisual
        | Action::ScrollCursorTo(_) => {
            crate::input::apply::navigation::apply_navigation(app, action, recording)
        }
        Action::SelectExtend(_)
        | Action::ClearSelection
        | Action::Copy
        | Action::Cut
        | Action::PasteSystem => {
            // Standard-mode-only family. `route_standard` intercepts
            // these *before* `apply_action` in production (so it can
            // inject a clipboard + manage the anchor), so this arm is
            // the formal closure that keeps the match exhaustive — it
            // routes through the real clipboard for completeness.
            let mut clip = crate::clipboard::ArboardClipboard;
            crate::input::apply::standard_sel::apply_standard_sel(app, action, &mut clip)
        }
        Action::Window(_) => crate::input::apply::window::apply_window(app, action, recording),
        Action::SlashKey => crate::input::apply::slash::apply_slash_key(app),
        Action::DeleteBackwardStandard => {
            crate::input::apply::standard_delete::apply_delete_backward_standard(app)
        }
        Action::FocusSwap
        | Action::TabGoto(..)
        | Action::TabNext
        | Action::TabPrev
        | Action::TreeActivate
        | Action::TreeActivateNewTab
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
        Action::GitPanelToggle
        | Action::GitPanelChar(..)
        | Action::GitPanelBackspace
        | Action::GitPanelDelete
        | Action::GitPanelCursorLeft
        | Action::GitPanelCursorRight
        | Action::GitPanelCursorHome
        | Action::GitPanelCursorEnd
        | Action::GitPanelCommit
        | Action::GitPanelCancel
        | Action::GitPanelSync
        | Action::GitConfirmSetUpstream
        | Action::GitCancelSetUpstream
        | Action::OpenGitBranchPicker
        | Action::CloseGitBranchPicker
        | Action::MoveGitBranchPickerCursor(..)
        | Action::ConfirmGitBranchPicker
        | Action::OpenGitLogPage
        | Action::CloseGitLogPage
        | Action::MoveGitLogPageCursor(..)
        | Action::ScrollGitLogDiff(..)
        | Action::OpenGitConflictResolver
        | Action::CloseGitConflictResolver
        | Action::MoveGitConflictResolverFile(..)
        | Action::ResolveGitConflict(..)
        | Action::GitPanelShare
        | Action::GitPanelToggleAmend => {
            crate::input::apply::git_panel::apply_git_panel(app, action)
        }
        Action::CloseBlockHistory
        | Action::CloseContentSearch
        | Action::CloseDbExportPicker
        | Action::CloseDbSettingsModal
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
        | Action::ToggleEditorMode
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
