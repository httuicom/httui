//! The keypress-decoded `Action` set. Every input profile (standard /
//! vim) decodes raw keystrokes into this enum; `vim::dispatch`
//! interprets it against the app. Mechanically moved out of
//! `vim/parser.rs` (tui-v2 vertical 1, fase 1 p1) with no logic change.

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
    /// `<op><motion>` — e.g. `dw`, `c$`, `y3w`.
    OperatorMotion(Operator, Motion, usize),
    /// Doubled-key shortcut: `dd`, `cc`, `yy` (linewise).
    OperatorLinewise(Operator, usize),
    /// `<op><i|a><target>` — e.g. `diw`, `ca"`, `yi(`.
    OperatorTextObject(Operator, TextObject, usize),
    /// `p` / `P`.
    Paste(PastePos, usize),
    /// `u` — restore prior snapshot.
    Undo,
    /// `<C-r>` — replay one redo step.
    Redo,
    /// `.` — replay the last change.
    RepeatChange(usize),
    /// `/` (forward) or `?` (backward) — open the search prompt.
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
    /// `n` repeats the last search; `reverse=true` flips direction (`N`).
    SearchRepeat {
        reverse: bool,
    },
    /// `Ctrl+P` — open the quick-open modal.
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
    /// `Ctrl+E` — toggle the file-tree sidebar (and shift focus to it
    /// when opening). Issued from any non-modal mode.
    TreeToggle,
    /// `Tab` — when the tree is visible, swap focus between sidebar
    /// and editor without changing visibility.
    FocusSwap,
    TreeSelectNext,
    TreeSelectPrev,
    TreeSelectFirst,
    TreeSelectLast,
    /// `Enter`/`l` — open a file or expand a folder (depending on the
    /// selected entry's kind).
    TreeActivate,
    /// `h` — collapse the parent folder (or current if it's expanded).
    TreeCollapse,
    /// `R` — re-scan the vault and refresh.
    TreeRefresh,
    /// `gt` — next tab (wrap-around). With a count `<n>gt`, jump to
    /// the n-th tab (1-indexed).
    TabNext,
    /// `gT` — previous tab.
    TabPrev,
    /// `<n>gt` — go to tab `n`.
    TabGoto(usize),
    /// `Ctrl+W <suffix>` — split / focus / close-window operations on
    /// the active tab's pane tree.
    Window(WindowCmd),
    /// `a` in tree — open the in-tree prompt for creating a file in
    /// the selected entry's directory.
    TreeCreate,
    /// `r` in tree — open the in-tree prompt for renaming the
    /// selected file (folders not supported).
    TreeRename,
    /// `d` in tree — open the in-tree y/N confirmation prompt for
    /// deleting the selected file.
    TreeDelete,
    /// Char input inside the tree prompt.
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
    /// `v` — enter charwise visual mode anchored at the current cursor.
    EnterVisual,
    /// `V` — enter linewise visual mode.
    EnterVisualLine,
    /// Apply an operator to the current visual selection. Generated
    /// from `d`/`c`/`y`/`x` while in [`Mode::Visual`] / [`Mode::VisualLine`].
    /// Drops back to normal afterwards (or insert, for `c`).
    VisualOperator(Operator),
    /// `o` in visual mode — swap the anchor and the moving cursor.
    VisualSwap,
    /// `a{`/`i{`/`aw`/`i"` etc. while in Visual mode — extend the
    /// current selection to cover the resolved text object's range.
    /// The dispatch handler reads the range from the text-object
    /// engine and snaps anchor + cursor to its bounds, keeping the
    /// user in visual mode so they can layer more motions on top.
    VisualSelectTextObject(TextObject),
    /// `Esc` / a second `v` (or `V` in linewise) — leave visual.
    ExitVisual,
    /// `r` in normal mode with the cursor on a block segment — run
    /// the block. Other block types may delegate; for now only DB
    /// blocks have an executor wired up.
    RunBlock,
    /// `<CR>` in normal mode with the cursor parked on a DB result
    /// row — open the row-detail modal. Dispatch validates the
    /// cursor; if it's anywhere else the action is a no-op.
    OpenDbRowDetail,
    /// `Esc` / `q` inside the row-detail modal — close it and return
    /// to normal mode.
    CloseDbRowDetail,
    /// `y` inside the row-detail modal — copy the current row's
    /// values to the system clipboard as pretty-printed JSON.
    CopyDbRowDetailJson,
    /// `Ctrl-c` inside the HTTP response-detail modal — close it and
    /// return to normal mode.
    CloseHttpResponseDetail,
    /// `Y` inside the HTTP response-detail modal — copy the full
    /// response body to the system clipboard.
    CopyHttpResponseBody,
    /// `gc` chord on a DB block — open the connection picker popup
    /// anchored to that block. Mnemonic: `g`-prefixed "goto" family
    /// (gg, gt, gd, gf …) extended with `gc` = goto connection.
    /// Dispatch validates the cursor; on a non-DB position it
    /// surfaces a status hint.
    OpenConnectionPicker,
    /// `<C-x>` on a DB block — wrap its query in the dialect's
    /// EXPLAIN keyword and run it (the block's own `params["query"]`
    /// stays untouched). Output replaces the result tab like a
    /// normal run. Mnemonic: "X" = E**X**plain.
    ExplainBlock,
    /// `<C-S-c>` on an HTTP block — resolve `{{refs}}` and copy a
    /// cURL command to the clipboard, no picker. Spec'd by Story
    /// 24.7 as the express version of the export menu — the menu
    /// itself sits behind `gx` and lets the user pick the format.
    CopyAsCurl,
    /// `gd` on a block — cycle the display mode (Input → Split →
    /// Output → Input). Persists via `display=` in the fence so the
    /// next save carries the choice. Mnemonic: "go display".
    CycleDisplayMode,
    /// `ga` on a block — open an inline alias-edit popup prefilled
    /// with the current alias. Confirm writes back to the block +
    /// persists into the fence on the next save; cancel leaves the
    /// alias untouched. Mnemonic: "go alias", `g`-prefix family
    /// alongside `gd` (display mode).
    OpenFenceEditAlias,
    /// One typeable character into the fence-edit prompt's input
    /// buffer. Driven by `parse_fence_edit`; mirrors `TreePromptChar`.
    FenceEditChar(char),
    FenceEditBackspace,
    FenceEditDelete,
    FenceEditCursorLeft,
    FenceEditCursorRight,
    FenceEditCursorHome,
    FenceEditCursorEnd,
    /// `<CR>` inside the prompt — validate + commit the edit.
    FenceEditConfirm,
    /// `<Esc>` / `<C-c>` inside the prompt — close without writing.
    FenceEditCancel,
    /// `Esc` / `Ctrl-C` inside the picker — close without picking.
    CloseConnectionPicker,
    /// `j` / `Down` / `k` / `Up` inside the picker — move the
    /// selection cursor by `i32` (positive = next, negative = prev).
    MoveConnectionPickerCursor(i32),
    /// `Enter` inside the picker — apply the selected connection
    /// to the anchored block and close the popup.
    ConfirmConnectionPicker,
    /// `D` (capital) inside the connection picker — delete the
    /// highlighted connection. The picker stays open with the list
    /// reloaded; if the deleted connection was the block's current
    /// `connection=`, the block keeps its stale id (block re-runs
    /// will surface the missing-connection error). No confirm step
    /// for V1: connection rows are configuration, not data — easy
    /// to recreate via desktop.
    DeleteConnectionInPicker,
    /// `Ctrl+n` / `Down` while the SQL completion popup is open —
    /// move the highlight one item forward (wraps).
    CompletionNext,
    /// `Ctrl+p` / `Up` while the SQL completion popup is open —
    /// move the highlight one item back (wraps).
    CompletionPrev,
    /// `Tab` / `Enter` while the SQL completion popup is open —
    /// splice the selected item's label in place of the prefix word
    /// at the cursor and close the popup.
    CompletionAccept,
    /// `Esc` / `Ctrl+C` while the popup is open — close it without
    /// inserting anything; subsequent keys go to insert as usual.
    CompletionDismiss,
    /// `y` (or `Enter`) while the unscoped-destructive confirm
    /// modal is up — run the query anyway, bypassing the gate.
    ConfirmDbRun,
    /// `n`/`Esc`/`Ctrl+C` while the confirm modal is up — close
    /// the modal without running.
    CancelDbRun,
    /// `gx` chord on a DB block — open the export-format picker
    /// popup. Mnemonic: `g`-prefixed "go" family extended with
    /// `gx` = goto eXport. Dispatch validates the cursor on a
    /// db-* block with at least one select row before opening; on
    /// a non-DB / empty-result position it surfaces a status hint.
    OpenDbExportPicker,
    /// `Esc` / `Ctrl-C` inside the export picker — close without
    /// copying anything to the clipboard.
    CloseDbExportPicker,
    /// `j` / `Down` / `k` / `Up` (and `Ctrl-n` / `Ctrl-p`) inside
    /// the export picker — move the highlight by `i32` (positive
    /// = next, negative = prev). Wraps via the dispatch handler.
    MoveDbExportPickerCursor(i32),
    /// `Enter` inside the export picker — serialize the result with
    /// the highlighted format and copy to the clipboard. Closes the
    /// popup on success; on failure (no clipboard) keeps the popup
    /// open and shows the error in the status line.
    ConfirmDbExportPicker,
    /// `gs` chord on a DB block — open the settings modal (limit +
    /// timeout). Memory `project_tui_block_settings_modal.md`: a
    /// single modal with multiple inputs, NOT chord-per-field. Tab
    /// cycles the focused input; Enter saves all; Esc cancels.
    OpenDbSettingsModal,
    /// `<Esc>` / `<C-c>` inside the settings modal — close without
    /// writing back to the block.
    CloseDbSettingsModal,
    /// `<CR>` inside the settings modal — validate inputs (numeric
    /// or empty) and commit to `block.params`. Empty inputs clear
    /// the corresponding field on the block.
    ConfirmDbSettingsModal,
    /// `Tab` / `Down` — focus next field; `Shift-Tab` / `Up` —
    /// focus previous. Wraps both directions.
    DbSettingsFocusNext,
    DbSettingsFocusPrev,
    /// One typeable char into the focused input. Mirrors
    /// `FenceEditChar` but routed by the modal's focused-field
    /// resolver.
    DbSettingsChar(char),
    DbSettingsBackspace,
    DbSettingsDelete,
    DbSettingsCursorLeft,
    DbSettingsCursorRight,
    DbSettingsCursorHome,
    DbSettingsCursorEnd,
    /// `gh` chord on an HTTP block — open the read-only history
    /// modal. Lists the most-recent N rows from `block_run_history`
    /// for the current `(file_path, alias)`. Dispatch validates the
    /// cursor + alias before opening; non-HTTP / anonymous blocks
    /// surface a status hint.
    OpenBlockHistory,
    /// `Esc` / `Ctrl-C` inside the history modal — close.
    CloseBlockHistory,
    /// `j` / `k` / arrows / `Ctrl-n` / `Ctrl-p` inside the history
    /// modal — move highlight by `i32` (positive = next, negative
    /// = prev). Clamps at the ends (no wrap — clamp matches the
    /// connection picker's feel for one-list popups).
    MoveBlockHistoryCursor(i32),
    /// `<C-f>` from normal mode — open the content-search modal.
    /// Lazy-rebuilds the FTS5 index on first open this session
    /// (sync — briefly freezes the UI on big vaults; async is V2).
    OpenContentSearch,
    /// `Esc` / `Ctrl-C` while the modal is up — close without
    /// opening anything.
    CloseContentSearch,
    /// `<CR>` while the modal is up — open the highlighted result
    /// in a new tab. Closes the modal on success.
    ConfirmContentSearch,
    /// `j` / `k` / arrows / `Ctrl-n` / `Ctrl-p` — move the
    /// highlight by `i32` (positive = next, negative = prev).
    /// Clamps at the ends (no wrap).
    MoveContentSearchCursor(i32),
    /// One typeable char into the query field. Each insert
    /// triggers a re-query.
    ContentSearchChar(char),
    ContentSearchBackspace,
    ContentSearchDelete,
    ContentSearchCursorLeft,
    ContentSearchCursorRight,
    ContentSearchCursorHome,
    ContentSearchCursorEnd,
    /// `gE` chord from normal mode — open the environment picker
    /// modal. Lists every row from the `environments` table; confirm
    /// flips `is_active` via `set_active_environment` and refreshes
    /// the status-bar chip. Mnemonic: capital E to avoid colliding
    /// with `ge` (motion: backward word-end).
    OpenEnvironmentPicker,
    /// `Esc` / `Ctrl-C` inside the env picker — close without
    /// switching the active env.
    CloseEnvironmentPicker,
    /// `j` / `k` / arrows / `Ctrl-n` / `Ctrl-p` inside the env
    /// picker — move the selection by `i32` (clamps at the ends).
    MoveEnvironmentPickerCursor(i32),
    /// `Enter` inside the env picker — call `set_active_environment`
    /// for the highlighted entry, refresh the cached display name,
    /// and close the popup.
    ConfirmEnvironmentPicker,
    /// `g?` chord from normal mode — open the keymap help modal.
    /// Read-only listing of the chord vocabulary grouped by section.
    /// Mnemonic: `g`-prefix family + `?` = "help".
    OpenHelp,
    /// `Esc` / `q` / `Ctrl-C` inside the help modal — close.
    CloseHelp,
    /// `g]` chord — jump to the next executable block in document
    /// order. No-op when the cursor is already past the last block
    /// (no wrap, matching vim's `]m` / `]]` motion conventions).
    /// Lands the cursor on the first body offset of the target
    /// block so the user can immediately edit / run it.
    JumpNextBlock,
    /// `g[` chord — jump to the previous executable block. Same
    /// no-wrap rule as `g]`.
    JumpPrevBlock,
    /// `gr` chord — rerun the last block that was dispatched in
    /// this session. The cursor doesn't have to be on the block;
    /// we look it up by alias (preferred) or `segment_idx` against
    /// `App.last_run_anchor`. Mnemonic: g + r = "go rerun".
    RerunLastBlock,
    /// `Ctrl-S` from normal or insert mode — save the active
    /// document. Bound deliberately as the universal save shortcut
    /// (VSCode / JetBrains / Sublime convention) so users coming
    /// from non-vim editors don't have to memorize `:w`. Same code
    /// path as `:w`; in insert mode the cursor stays in insert
    /// (saves don't leave the typing flow).
    WriteFile,
    /// `gW` chord — save every dirty tab in one shot. Vim's `:wa`
    /// in chord form. Mnemonic: g + capital W = "go (write) all";
    /// lowercase `gw` is taken by vim's "format text" motion.
    WriteAll,
    /// `gv` chord — re-enter the last visual selection. Vim
    /// convention. V1 only restores the anchor + linewise flag
    /// (not the moving end), so the user re-extends from the
    /// anchor with motions; the cursor lands on the anchor.
    ReselectVisual,
    /// `zz` / `zt` / `zb` chords — re-anchor the viewport so the
    /// cursor's line lands at the center / top / bottom of the
    /// pane. Vim convention; useful after a long jump (`<n>G`,
    /// search) when the cursor is in an awkward viewport position.
    ScrollCursorTo(ScrollPos),
    /// `gN` chord — open the block-template picker. Lowercase `gn`
    /// is taken by vim's "find next match" motion, so the new-block
    /// chord uses capital N.
    OpenBlockTemplatePicker,
    /// Standard-mode `/` key. Context-aware in `apply/slash.rs`:
    /// in prose, inserts `/` and opens the block-template picker
    /// (paridade com slash-commands do desktop); in a block / block
    /// result, inserts `/` literally so URLs and paths stay typeable.
    /// Vim's `/` continues to mean "open search prompt" via
    /// `EnterSearch(false)` — this variant is decoded only by
    /// `input::standard::resolve`. Added by tui-V2 / vertical 2.
    SlashKey,
    /// `gb` chord — open the tab picker. Lists every open tab by
    /// its focused-leaf path; Enter switches the active tab to the
    /// picked index. Mnemonic: g + b = "go (to) buffer".
    OpenTabPicker,
    /// `Esc` / `Ctrl-C` inside the tab picker — close without
    /// switching tabs.
    CloseTabPicker,
    /// `j` / `Down` / `k` / `Up` (and `Ctrl-n` / `Ctrl-p`) inside
    /// the tab picker — move the highlight by `i32` (clamps at the
    /// ends).
    MoveTabPickerCursor(i32),
    /// `Enter` inside the tab picker — switch `tabs.active` to the
    /// highlighted index and dismiss.
    ConfirmTabPicker,
    /// `Esc` / `Ctrl-C` inside the template picker — close without
    /// inserting anything.
    CloseBlockTemplatePicker,
    /// `j` / `Down` / `k` / `Up` (and `Ctrl-n` / `Ctrl-p`) inside
    /// the template picker — move the highlight by `i32` (positive
    /// = next, negative = prev). Clamps at the ends.
    MoveBlockTemplatePickerCursor(i32),
    /// `Enter` inside the template picker — splice the selected
    /// template's text into the prose segment at cursor and re-parse
    /// so the typed fence promotes to a block.
    ConfirmBlockTemplatePicker,
    /// Standard (non-modal) profile: `Shift`+arrow / `Shift`+Home /
    /// `Shift`+End. Extends the Standard selection by the wrapped
    /// motion, seeding `App.standard.anchor` from the cursor on the
    /// first one. No vim equivalent — the vim path never decodes
    /// into this (Cenário 2 stays byte-identical). Fase 3 p1.
    SelectExtend(Motion),
    /// Standard profile: a plain (non-Shift) arrow while a selection
    /// is active drops the anchor before moving. Routed by
    /// `route_standard` (fase 3 p2). Never decoded by the vim path.
    ClearSelection,
    /// Standard profile: `Ctrl+C` — copy the active selection to the
    /// system clipboard (no-op without a selection). Fase 3 p1.
    Copy,
    /// Standard profile: `Ctrl+X` — cut the active selection (copy +
    /// delete + collapse anchor). No-op without a selection. Fase 3 p1.
    Cut,
    /// Standard profile: `Ctrl+V` — paste the system clipboard at the
    /// cursor, replacing the selection if one is active. Fase 3 p1.
    PasteSystem,
    /// Cross-profile meta-action: `Ctrl+Shift+M` flips
    /// `config.editor.mode` between Standard and Vim at runtime.
    /// Intercepted in [`crate::input::route::route`] BEFORE the
    /// per-profile branch, so this variant never reaches
    /// `apply_action` — the multi-arm group treats it as a no-op
    /// for exhaustiveness. The variant exists so the inspectable
    /// keymap table in [`crate::input::map`] can name the binding
    /// without needing a parallel `MetaAction` enum. Fase 6 p2.
    ToggleEditorMode,
    Noop,
}
