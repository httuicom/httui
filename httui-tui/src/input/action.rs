//! Keypress-decoded `Action` set. Input profiles (standard, vim)
//! decode raw keystrokes into this enum; `vim::dispatch` interprets
//! it against the app.

use crate::input::types::{
    InsertPos, Motion, Operator, PastePos, ScrollPos, TextObject, WindowCmd,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Motion(Motion, usize),
    EnterInsert(InsertPos),
    InsertChar(char),
    InsertNewline,
    DeleteBackward,
    DeleteForward,
    ExitInsert,
    EnterCmdline,
    CmdlineChar(char),
    CmdlineBackspace,
    CmdlineDelete,
    CmdlineCursorLeft,
    CmdlineCursorRight,
    CmdlineCursorHome,
    CmdlineCursorEnd,
    CmdlineExecute,
    CmdlineCancel,
    OperatorMotion(Operator, Motion, usize),
    OperatorLinewise(Operator, usize),
    OperatorTextObject(Operator, TextObject, usize),
    Paste(PastePos, usize),
    Undo,
    Redo,
    RepeatChange(usize),
    EnterSearch(bool),
    SearchChar(char),
    SearchBackspace,
    SearchDelete,
    SearchCursorLeft,
    SearchCursorRight,
    SearchCursorHome,
    SearchCursorEnd,
    SearchExecute,
    SearchCancel,
    SearchRepeat {
        reverse: bool,
    },
    EnterQuickOpen,
    QuickOpenChar(char),
    QuickOpenBackspace,
    QuickOpenDelete,
    QuickOpenCursorLeft,
    QuickOpenCursorRight,
    QuickOpenCursorHome,
    QuickOpenCursorEnd,
    QuickOpenSelectNext,
    QuickOpenSelectPrev,
    QuickOpenExecute,
    QuickOpenCancel,
    TreeToggle,
    GitPanelToggle,
    GitPanelChar(char),
    GitPanelBackspace,
    GitPanelDelete,
    GitPanelCursorLeft,
    GitPanelCursorRight,
    GitPanelCursorHome,
    GitPanelCursorEnd,
    /// Empty draft → prefill via [`crate::git::template::commit_template`].
    GitPanelCommit,
    GitPanelCancel,
    /// Push without upstream opens the [`Action::GitConfirmSetUpstream`] modal.
    GitPanelSync,
    GitConfirmSetUpstream,
    GitCancelSetUpstream,
    OpenGitBranchPicker,
    CloseGitBranchPicker,
    MoveGitBranchPickerCursor(i32),
    ConfirmGitBranchPicker,
    OpenGitLogPage,
    CloseGitLogPage,
    MoveGitLogPageCursor(i32),
    /// Scrolls the diff pane without changing the selected commit.
    ScrollGitLogDiff(i32),
    OpenGitConflictResolver,
    CloseGitConflictResolver,
    MoveGitConflictResolverFile(i32),
    ResolveGitConflict(crate::git::ConflictVersion),
    GitPanelShare,
    /// Auto-resets after a successful commit.
    GitPanelToggleAmend,
    FocusSwap,
    TreeSelectNext,
    TreeSelectPrev,
    TreeSelectFirst,
    TreeSelectLast,
    TreeActivate,
    TreeCollapse,
    TreeRefresh,
    TabNext,
    TabPrev,
    TabGoto(usize),
    Window(WindowCmd),
    TreeCreate,
    TreeRename,
    TreeDelete,
    TreePromptChar(char),
    TreePromptBackspace,
    TreePromptDelete,
    TreePromptCursorLeft,
    TreePromptCursorRight,
    TreePromptCursorHome,
    TreePromptCursorEnd,
    TreePromptExecute,
    TreePromptCancel,
    Quit,
    EnterVisual,
    EnterVisualLine,
    VisualOperator(Operator),
    VisualSwap,
    VisualSelectTextObject(TextObject),
    ExitVisual,
    VisualPaste,
    RunBlock,
    OpenDbRowDetail,
    CloseDbRowDetail,
    CopyDbRowDetailJson,
    CloseHttpResponseDetail,
    CopyHttpResponseBody,
    OpenConnectionPicker,
    ExplainBlock,
    CopyAsCurl,
    /// Persists via `display=` in the fence on the next save.
    CycleDisplayMode,
    OpenFenceEditAlias,
    FenceEditChar(char),
    FenceEditBackspace,
    FenceEditDelete,
    FenceEditCursorLeft,
    FenceEditCursorRight,
    FenceEditCursorHome,
    FenceEditCursorEnd,
    FenceEditConfirm,
    FenceEditCancel,
    CloseConnectionPicker,
    MoveConnectionPickerCursor(i32),
    ConfirmConnectionPicker,
    /// If the deleted connection was the block's current `connection=`,
    /// the block keeps its stale id (re-runs surface the missing-connection
    /// error). No confirm step — connection rows are configuration, not data.
    DeleteConnectionInPicker,
    CompletionNext,
    CompletionPrev,
    CompletionAccept,
    CompletionDismiss,
    ConfirmDbRun,
    CancelDbRun,
    OpenDbExportPicker,
    CloseDbExportPicker,
    MoveDbExportPickerCursor(i32),
    ConfirmDbExportPicker,
    OpenDbSettingsModal,
    CloseDbSettingsModal,
    ConfirmDbSettingsModal,
    DbSettingsFocusNext,
    DbSettingsFocusPrev,
    DbSettingsChar(char),
    DbSettingsBackspace,
    DbSettingsDelete,
    DbSettingsCursorLeft,
    DbSettingsCursorRight,
    DbSettingsCursorHome,
    DbSettingsCursorEnd,
    OpenBlockHistory,
    CloseBlockHistory,
    /// Clamps at the ends (no wrap — matches the connection picker's feel).
    MoveBlockHistoryCursor(i32),
    /// Lazy-rebuilds the FTS5 index on first open this session.
    OpenContentSearch,
    CloseContentSearch,
    ConfirmContentSearch,
    MoveContentSearchCursor(i32),
    ContentSearchChar(char),
    ContentSearchBackspace,
    ContentSearchDelete,
    ContentSearchCursorLeft,
    ContentSearchCursorRight,
    ContentSearchCursorHome,
    ContentSearchCursorEnd,
    OpenEnvironmentPicker,
    CloseEnvironmentPicker,
    MoveEnvironmentPickerCursor(i32),
    ConfirmEnvironmentPicker,
    OpenConnectionsPage,
    CloseConnectionsPage,
    MoveConnectionsPageCursor(i32),
    OpenConnectionForm,
    CloseConnectionForm,
    ConnectionFormFocusNext,
    ConnectionFormFocusPrev,
    /// Driver field uses `ConnectionFormCycleDriver`; readonly uses
    /// `ConnectionFormToggleReadonly`. Other fields take Char/Backspace/etc.
    ConnectionFormChar(char),
    ConnectionFormBackspace,
    ConnectionFormDelete,
    ConnectionFormCursorLeft,
    ConnectionFormCursorRight,
    ConnectionFormCursorHome,
    ConnectionFormCursorEnd,
    ConnectionFormCycleDriver(i32),
    ConnectionFormToggleReadonly,
    ConnectionFormSubmit,
    OpenConnectionEditForm,
    /// Writes into `App.session_overrides`; the underlying connection
    /// is never mutated.
    OpenSessionOverrideForm,
    ClearSessionOverride,
    TestSelectedConnection,
    OpenConnectionDeleteConfirm,
    ConfirmConnectionDelete,
    CancelConnectionDelete,
    OpenEnvsPage,
    CloseEnvsPage,
    EnvsPageFocusToggle,
    EnvsPageFocusEnvs,
    EnvsPageFocusVars,
    EnvsPageMoveEnvCursor(i32),
    EnvsPageMoveVarCursor(i32),
    EnvsPageActivateEnv,
    OpenEnvForm,
    OpenEnvEditForm,
    CloseEnvForm,
    EnvFormChar(char),
    EnvFormBackspace,
    EnvFormDelete,
    EnvFormCursorLeft,
    EnvFormCursorRight,
    EnvFormHome,
    EnvFormEnd,
    EnvFormSubmit,
    OpenVarForm,
    OpenVarEditForm,
    CloseVarForm,
    VarFormChar(char),
    VarFormBackspace,
    VarFormDelete,
    VarFormCursorLeft,
    VarFormCursorRight,
    VarFormHome,
    VarFormEnd,
    VarFormFocusNext,
    VarFormFocusPrev,
    VarFormToggleSecret,
    VarFormSubmit,
    OpenEnvDeleteConfirm,
    OpenVarDeleteConfirm,
    ConfirmEnvOrVarDelete,
    CancelEnvOrVarDelete,
    OpenEnvCloneForm,
    CloseEnvCloneForm,
    EnvCloneFormChar(char),
    EnvCloneFormBackspace,
    EnvCloneFormFocusToggle,
    EnvCloneFormMoveVarCursor(i32),
    EnvCloneFormToggleVar,
    EnvCloneFormToggleAll,
    EnvCloneFormSubmit,
    /// Modal-only — no conflict with vim count-prefix.
    ActivateEnvByIndex(usize),
    OpenHelp,
    JumpNextBlock,
    JumpPrevBlock,
    /// Looked up by alias (preferred) or `segment_idx` against
    /// `App.last_run_anchor` — cursor doesn't have to be on the block.
    RerunLastBlock,
    WriteFile,
    WriteAll,
    /// Restores only the anchor + linewise flag (not the moving end).
    ReselectVisual,
    ScrollCursorTo(ScrollPos),
    OpenBlockTemplatePicker,
    /// Standard-mode Backspace; handles segment-boundary crossing as
    /// plain text and demotes a block to `Prose` if its `raw` becomes
    /// unparseable.
    DeleteBackwardStandard,
    /// Prose inserts `/` and opens the block-template picker; inside a
    /// block or block result, inserts `/` literally.
    SlashKey,
    OpenTabPicker,
    CloseTabPicker,
    MoveTabPickerCursor(i32),
    ConfirmTabPicker,
    CloseBlockTemplatePicker,
    MoveBlockTemplatePickerCursor(i32),
    ConfirmBlockTemplatePicker,
    /// Seeds `App.standard.anchor` from the cursor on the first one.
    SelectExtend(Motion),
    ClearSelection,
    Copy,
    Cut,
    PasteSystem,
    /// Intercepted in [`crate::input::route::route`] BEFORE the
    /// per-profile branch, so this variant never reaches `apply_action`.
    /// Exists so the inspectable keymap table in [`crate::input::map`]
    /// can name the binding without a parallel `MetaAction` enum.
    ToggleEditorMode,
    OpenVaultPicker,
    CloseVaultPicker,
    MoveVaultPickerCursor(i32),
    ConfirmVaultPicker,
    OpenVaultCreateForm,
    CloseVaultCreateForm,
    VaultCreateFormFocusNext,
    VaultCreateFormFocusPrev,
    VaultCreateFormChar(char),
    VaultCreateFormBackspace,
    VaultCreateFormSubmit,
    OpenVaultCloneForm,
    CloseVaultCloneForm,
    VaultCloneFormFocusNext,
    VaultCloneFormFocusPrev,
    VaultCloneFormChar(char),
    VaultCloneFormBackspace,
    VaultCloneFormSubmit,
    OpenVaultOpenPicker,
    CloseVaultOpenPicker,
    MoveVaultOpenPickerCursor(i32),
    /// Always descends (or ascends on `..`). Never opens as vault, so a
    /// vault-as-parent doesn't trap navigation. Use
    /// `VaultOpenPickerOpenAsVault` to open.
    VaultOpenPickerEnter,
    /// Opens the highlighted directory (Directory OR Vault) as the
    /// active vault via `switch_vault`. Allows a vault inside another.
    VaultOpenPickerOpenAsVault,
    VaultOpenPickerUp,
    CloseVaultMissingSecrets,
    MoveVaultMissingSecretsCursor(i32),
    VaultMissingSecretsEnterEdit,
    VaultMissingSecretsCancelEdit,
    VaultMissingSecretsChar(char),
    VaultMissingSecretsBackspace,
    VaultMissingSecretsSave,
    VaultMissingSecretsSkip,
    /// `Alt+,` contextual entry: routes to [`Action::OpenDbSettingsModal`]
    /// when the cursor sits on a DB/HTTP block (block-scoped
    /// limit/timeout), otherwise opens the app-wide Settings page.
    /// Same chord, two surfaces — mirrors `Tab`'s context-sensitive
    /// behaviour.
    OpenSettings,
    CloseSettingsPage,
    SettingsNextSection,
    SettingsPrevSection,
    SettingsMoveCursor(i32),
    /// Activate the row under the cursor. Section-dispatched by the
    /// applier: Keymaps → enter rebind capture; Theme → apply the
    /// highlighted preset.
    SettingsActivateRow,
    /// Abort an in-progress rebind without committing.
    SettingsCancelCapture,
    /// Key the user pressed while in capture mode. Applier converts
    /// to a chord string, writes config, persists.
    SettingsCommitCapture(crossterm::event::KeyEvent),
    /// Restore the row under the cursor to its built-in default.
    SettingsResetBinding,
    ToggleAppView,
    BlocksPaneNextRegion,
    BlocksPanePrevRegion,
    /// 1-based; clamps to the block kind's region count.
    BlocksPaneJumpRegion(usize),
    /// 1-based pane index — used when the sidebar's pane-picker
    /// overlay is open. Closes the overlay either way.
    BlocksPanePickerChoose(usize),
    BlocksPanePickerCancel,
    /// NAV-mode row/col motion inside a table-shaped region (Headers).
    /// No-op when the focused region isn't a table.
    BlocksPaneRowUp,
    BlocksPaneRowDown,
    BlocksPaneColLeft,
    BlocksPaneColRight,
    /// Enter on the focused region: open the field at
    /// `(block_region, block_row, block_col)` for inline editing.
    /// Allocates the per-pane `BlockDraft` on first edit. Sub-mode
    /// resolves via the active profile (standard → INSERT, vim →
    /// NORMAL).
    BlocksRegionEnterEdit,
    /// Vim `i`/`a`/`o` from NAV: open the field already in INSERT,
    /// skipping the NORMAL transit. No-op in standard profile.
    BlocksRegionEnterEditInsert,
    /// Esc (standard, or vim NORMAL): commit the sub-Document into
    /// `BlockDraft` and return to NAV.
    BlocksRegionCommitEdit,
    /// Ctrl+C: discard the sub-Document without writing.
    BlocksRegionCancelEdit,
    /// Ctrl+S on the focused pane (BLOCKS view): serialize every dirty
    /// draft into its `.md` via `write_note`. No-op when nothing dirty.
    BlocksSaveDraft,
    /// `]` (vim) / `PageDown` (any profile) — select the next block
    /// in the workspace's flattened block list (cross-file, wrap).
    BlocksNextBlockMotion,
    /// `[` (vim) / `PageUp` (any profile) — previous block, wrap.
    BlocksPrevBlockMotion,
    /// BLOCKS NAV run: execute the block currently selected in the
    /// focused pane. Default chord `r`. Maps internally to
    /// [`Action::RunBlock`] after re-anchoring the executor on the
    /// pane's selected block. Routed only from BLOCKS view — in DOC
    /// the action is no-op so the chord can stay bound globally
    /// without shadowing standard typing.
    BlocksRunFocused,
    /// BLOCKS NAV cancel: stop the in-flight run started from this
    /// pane. Default chord `.`. Wraps `cancel_running_query`.
    BlocksCancelRun,
    /// Cycle response sub-tabs (Body / Headers / Cookies / Timing /
    /// History) inside the focused HTTP block's `[N] Response`
    /// region. Default chord `Alt+T`. No-op for DB / non-HTTP blocks.
    BlocksResponseNextTab,
    /// Reverse of [`Action::BlocksResponseNextTab`]. Default chord
    /// `Alt+Shift+T`.
    BlocksResponsePrevTab,
    /// NAV in HTTP `[2] Headers`: insert an empty row after the
    /// current `block_row`, advance cursor to the new row's key cell.
    /// Hydrates the draft on first use.
    BlocksHeaderInsertRow,
    /// NAV in HTTP `[2] Headers`: delete the current `block_row`.
    /// No-op when there are no rows. Cursor clamps to the row above.
    BlocksHeaderDeleteRow,
    /// NAV in HTTP `[2] Headers`: toggle the current row on/off (`Space`).
    /// A disabled row serializes with a `# ` prefix and is skipped on
    /// dispatch. Hydrates the draft on first use.
    BlocksHeaderToggleEnabled,
    /// EDIT INSERT on a single-line HTTP cell (header key/value or URL):
    /// `Enter`/`Tab` commit the current buffer and advance to the next field
    /// (key→value, value→next row's key, last-row value→insert+new row).
    /// Mirrors form-input conventions so Enter never injects a stray newline
    /// into a single-line cell.
    BlocksFieldAdvanceNext,
    /// vim NORMAL on a single-line HTTP cell: `o` commits the current edit
    /// and opens a new header row BELOW the current one, landing in INSERT
    /// on its key. Same semantics as NAV `o` but accessible from inside EDIT.
    BlocksFieldOpenBelow,
    /// vim NORMAL on a single-line HTTP cell: `O` commits and opens a new
    /// header row ABOVE the current one (mirror of [`BlocksFieldOpenBelow`]).
    BlocksFieldOpenAbove,
    /// `y`/`Enter` in the [`crate::modal::Modal::ConfirmPrompt`] opened by
    /// `delete_header_row`: read `ConfirmPayload::HeaderRow` and drop it.
    BlocksHeaderDeleteConfirm,
    /// `n`/`Esc`/`Ctrl+C` in the same prompt: close without deleting.
    BlocksHeaderDeleteCancel,
    /// Sidebar `n` while a `.md` file is focused (BLOCKS view) —
    /// append a new HTTP block to that file. Reuses existing tree.
    BlocksTreeNewBlock,
    /// `Ctrl+Shift+↑` on a block in the sidebar — swap with the
    /// block immediately above in the same file.
    BlocksTreeReorderUp,
    /// `Ctrl+Shift+↓` on a block in the sidebar — swap with the
    /// block immediately below in the same file.
    BlocksTreeReorderDown,
    /// `d`/`Delete` on a block in the sidebar — remove the block
    /// from its file. Intercepts before the generic tree-delete so
    /// a block-row chord can't accidentally delete the parent file.
    BlocksTreeDeleteBlock,
    /// `v` on a block in the sidebar — split the focused pane
    /// vertically and open the selected block in the new half.
    BlocksTreeOpenSplitVertical,
    /// `s` on a block in the sidebar — split the focused pane
    /// horizontally and open the selected block in the new half.
    BlocksTreeOpenSplitHorizontal,
    /// `Save` button on the unsaved-prompt modal: write every dirty
    /// pane, close the modal, replay the deferred `ToggleAppView`.
    BlocksUnsavedPromptSave,
    /// `Discard` button: drop every pane draft, close the modal,
    /// replay the deferred `ToggleAppView`.
    BlocksUnsavedPromptDiscard,
    /// `Cancel` button (or Esc): close the modal, stay in BLOCKS.
    BlocksUnsavedPromptCancel,
    /// Variant of [`Action::TreeActivate`] bound to `Ctrl+Enter` on a
    /// block row in the sidebar (BLOCKS view): instead of replacing the
    /// focused pane's active tab, open the picked block as a NEW tab
    /// in the focused pane. Falls back to `TreeActivate` semantics for
    /// non-block rows.
    TreeActivateNewTab,
    /// Open a new empty BLOCKS-view tab inside the focused pane (`Ctrl+T`).
    /// The greeter mode kicks in until the user picks a block in the
    /// sidebar (Enter replaces the active tab in place).
    BlocksTabNew,
    /// Close the active BLOCKS-view tab inside the focused pane. When
    /// the closed tab was the last one in the pane, the pane itself is
    /// collapsed via the same path as `Ctrl+W q`. Standard `Ctrl+W` in
    /// BLOCKS view; vim `:bd` / `Ctrl+Q` cover the same semantic.
    BlocksTabClose,
    /// Activate the next BLOCKS-view tab in the focused pane, wrapping
    /// from last → first. Bound to `gt` (vim NORMAL) and `Ctrl+PgDn`
    /// (standard) by default; both bindings are keymap-configurable.
    BlocksTabNext,
    /// Activate the previous BLOCKS-view tab, wrapping first → last.
    /// Bound to `gT` (vim NORMAL) and `Ctrl+PgUp` (standard) by default.
    BlocksTabPrev,
    Noop,
}
