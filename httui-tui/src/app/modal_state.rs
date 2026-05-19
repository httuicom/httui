//! Modal / popup open-state (completion, detail modals, fence-edit,
//! confirm, export picker, content-search, run-history, settings).
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-modal_state) — pure code move, no behavior change. Re-exported
//! from `app/mod.rs` so the `crate::app::*` modal call sites keep
//! resolving. Exercised via the `commands::*` open paths and the
//! dispatch integration tests.

use crate::buffer::Document;

/// Open instance of the SQL completion popup. Anchored to the DB
/// block at `segment_idx`; `(anchor_line, anchor_offset)` is where
/// the prefix word starts inside the block body — Accept replaces
/// from there to the current cursor with the selected item's label.
///
/// The popup co-exists with `Mode::Insert` (mode never flips) so the
/// user can keep typing to filter the list. The dispatcher
/// intercepts a small set of keys (`Tab`/`Enter`/`Esc`/`Ctrl-n`/
/// `Ctrl-p`/`Down`/`Up`) and routes them to the popup; everything
/// else falls through to normal insert handling and triggers a
/// re-filter.
pub struct CompletionPopupState {
    pub segment_idx: usize,
    pub items: Vec<crate::sql_completion::CompletionItem>,
    pub selected: usize,
    /// `(line, offset)` where the prefix word starts in the block
    /// body — the renderer drops the popup right below this cell so
    /// the dropdown tracks the cursor as the user types.
    pub anchor_line: usize,
    pub anchor_offset: usize,
    /// What the user has typed so far — drives the popup header and
    /// gets replaced on Accept.
    pub prefix: String,
}

/// Open instance of the row-detail modal. The body lives in its own
/// `Document` so the editor's full motion vocabulary (`hjkl`, `wbe`,
/// `gg`/`G`, `Ctrl-d`/`Ctrl-u`, `f`/`F`, etc.) navigates the modal
/// out of the box — `parse_db_row_detail` filters `parse_normal`
/// down to motions and the dispatch routes them to `state.doc`.
///
/// `segment_idx` + `row` are kept as a back-pointer for the title +
/// status (and a future "jump back to the source row" command). The
/// body text is snapshotted at open time; re-running the underlying
/// block while the modal is up doesn't mutate it. `viewport_height`
/// is written back by the renderer so half/full-page motions know
/// how far to jump.
pub struct DbRowDetailState {
    /// Back-pointer to the source row in the editor's document.
    /// Used by `dispatch::db_row_payload` for the (yet-to-land)
    /// clipboard copy and by a future "jump back to row" command.
    #[allow(dead_code)]
    pub segment_idx: usize,
    #[allow(dead_code)]
    pub row: usize,
    pub title: String,
    pub doc: Document,
    pub viewport_height: u16,
    /// Top line of the visible window inside the modal. Persists
    /// across frames so the viewport behaves like the editor's: it
    /// stays put while the cursor moves inside the window, and only
    /// adjusts when the cursor would otherwise scroll off-screen
    /// (mirrors `app::clamp_viewport`).
    pub viewport_top: u16,
}

/// Open instance of the HTTP response-detail modal. Mirrors
/// [`DbRowDetailState`]: a sub-`Document` carries the rendered text
/// (status line + headers + body) so the editor's full motion engine
/// — built around [`Cursor::InProse`] — navigates the modal without
/// any extra wiring. The body is snapshotted at open time; running
/// the underlying block again while the modal is up doesn't mutate
/// it.
pub struct HttpResponseDetailState {
    /// Back-pointer to the source HTTP block. Kept for a future
    /// "jump back to block" command and for the title.
    #[allow(dead_code)]
    pub segment_idx: usize,
    pub title: String,
    pub doc: Document,
    pub viewport_height: u16,
    /// Top line of the visible window inside the modal. Same
    /// persistent-viewport contract as [`DbRowDetailState::viewport_top`].
    pub viewport_top: u16,
}

/// State for the inline fence-edit prompt. `kind` carries the field
/// being edited (alias today; limit / timeout once those slices land);
/// `input` is the actual text-edit buffer that the prompt parser
/// drives. `segment_idx` pins the block — the cursor may move while
/// the prompt is up, but the edit always commits to the block the
/// user opened the prompt against.
#[derive(Debug, Clone)]
pub struct FenceEditState {
    pub segment_idx: usize,
    pub kind: FenceEditKind,
    pub input: crate::vim::lineedit::LineEdit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceEditKind {
    /// `<C-a>` on a block — edit the alias used in `{{alias.path}}`
    /// refs and shown in the block title. Blank input clears the
    /// alias (block becomes anonymous).
    Alias,
}

impl FenceEditKind {
    pub fn label(self) -> &'static str {
        match self {
            FenceEditKind::Alias => "alias",
        }
    }
}

/// State for the run-confirm modal. Carries the segment to re-run
/// (the cursor may have moved in between) and the human reason
/// shown to the user (e.g. "UPDATE without WHERE").
pub struct DbConfirmRunState {
    pub segment_idx: usize,
    pub reason: String,
}

/// Open instance of the DB export-format picker. Anchored to the DB
/// block at `segment_idx` (the cursor was on it when `gx` opened the
/// picker); `formats` is a fixed 4-element list (CSV/JSON/Markdown/
/// INSERT); `selected` indexes into it. Confirm copies the serialized
/// result to the clipboard via `httui-core::blocks::db_export`.
///
/// Snapshotting the columns/rows at open time would be wasteful — the
/// dispatch handler reads them straight from the document's
/// `cached_result` so we don't carry a copy around. The cursor IS
/// allowed to move while the picker is up; we re-resolve through
/// `segment_idx` at confirm-time, same contract as `FenceEditState`.
pub struct DbExportPickerState {
    pub segment_idx: usize,
    pub selected: usize,
    /// Which format list to render — DB or HTTP. Snapshotted at
    /// open-time so the picker survives unrelated edits to the doc
    /// while it's up.
    pub formats: &'static [BlockExportFormat],
}

impl DbExportPickerState {
    pub fn new(segment_idx: usize, formats: &'static [BlockExportFormat]) -> Self {
        Self {
            segment_idx,
            selected: 0,
            formats,
        }
    }
}

/// Wire format for the export menu. Block-type aware: DB blocks
/// export tabular results (CSV/JSON/Markdown/INSERT) while HTTP
/// blocks export the request as code in another runtime
/// (cURL/Fetch/Python/HTTPie/.http). The picker shows whichever set
/// matches the focused block; all formats per type are 4-5 entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockExportFormat {
    // DB
    Csv,
    Json,
    Markdown,
    Insert,
    // HTTP
    Curl,
    Fetch,
    Python,
    HTTPie,
    HttpFile,
}

impl BlockExportFormat {
    /// Format list for DB blocks — tabular result serializers.
    pub const DB_FORMATS: &'static [BlockExportFormat] = &[
        BlockExportFormat::Csv,
        BlockExportFormat::Json,
        BlockExportFormat::Markdown,
        BlockExportFormat::Insert,
    ];

    /// Format list for HTTP blocks — request → code generators.
    pub const HTTP_FORMATS: &'static [BlockExportFormat] = &[
        BlockExportFormat::Curl,
        BlockExportFormat::Fetch,
        BlockExportFormat::Python,
        BlockExportFormat::HTTPie,
        BlockExportFormat::HttpFile,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BlockExportFormat::Csv => "CSV",
            BlockExportFormat::Json => "JSON",
            BlockExportFormat::Markdown => "Markdown table",
            BlockExportFormat::Insert => "INSERT statements",
            BlockExportFormat::Curl => "cURL",
            BlockExportFormat::Fetch => "JavaScript (fetch)",
            BlockExportFormat::Python => "Python (requests)",
            BlockExportFormat::HTTPie => "HTTPie",
            BlockExportFormat::HttpFile => ".http file",
        }
    }
}

/// Backward-compat alias — older callers still reference the
/// DB-specific enum name. The TUI uses the block-aware variant
/// throughout, but external uses (sub-Crate exports etc.) stay
/// addressable. Remove once no usages remain.
#[allow(dead_code)]
pub type DbExportFormat = BlockExportFormat;

/// Open instance of the content-search modal. `<C-f>` opens it
/// over the active vault; the modal runs a synchronous FTS5 query
/// per keystroke against `httui-core::search::search_content`.
///
/// V1 caveats:
///  - Index is rebuilt synchronously on first open in a session
///    (briefly freezes the UI for large vaults). Async rebuild +
///    incremental updates are V2.
///  - Search is also synchronous (sub-millisecond on small vaults
///    via FTS5). If `search_content` ever gets expensive, debounce
///    on a tokio task with cancellation.
pub struct ContentSearchState {
    pub query: crate::vim::lineedit::LineEdit,
    pub results: Vec<httui_core::search::ContentSearchResult>,
    pub selected: usize,
    /// `true` while the async FTS5 rebuild kicked off by the open
    /// is still running. Used by the renderer to show an
    /// "indexing…" banner instead of the empty-state hint, and by
    /// the dispatch path to skip per-keystroke queries (which
    /// would race against the half-built index).
    pub building: bool,
}

impl ContentSearchState {
    pub fn new() -> Self {
        Self {
            query: crate::vim::lineedit::LineEdit::new(),
            results: Vec::new(),
            selected: 0,
            building: false,
        }
    }

    pub fn select_next(&mut self) {
        if !self.results.is_empty() {
            self.selected = (self.selected + 1).min(self.results.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Selected result, if any. Used by Confirm to open the file.
    pub fn chosen(&self) -> Option<&httui_core::search::ContentSearchResult> {
        self.results.get(self.selected)
    }
}

/// Open instance of the block run-history modal. `gh` opens it on
/// an HTTP block; the modal lists the most-recent N runs read from
/// `httui-core::block_history` (V1: HTTP only — DB history will reuse
/// the same modal once the executor records it). Pure read-only — the
/// only interactions are j/k navigation and Esc/Ctrl-C to close.
pub struct BlockHistoryState {
    pub segment_idx: usize,
    /// Header info shown in the modal title — `<METHOD> <alias>`.
    /// Snapshotted at open-time so the title doesn't shimmer if the
    /// user edits the block while the modal is up.
    pub title: String,
    pub entries: Vec<httui_core::block_history::HistoryEntry>,
    pub selected: usize,
}

/// Open instance of the DB block settings modal. `gs` opens it; the
/// modal carries one [`crate::vim::lineedit::LineEdit`] per editable
/// field (limit, timeout_ms) prefilled from the block's params, plus
/// a focus enum that Tab/BackTab cycle through. Confirm validates +
/// writes back to `block.params`; cancel discards the buffers.
///
/// Modal vs chord-per-field: this is a deliberate UX call recorded in
/// the user-memory `project_tui_block_settings_modal.md` — limit and
/// timeout are rare enough that they don't deserve top-level chords;
/// a single `gs` opens a compact form. Adding new fields means
/// extending [`DbSettingsFocus`] + this struct, not a new chord.
/// One editable field in the block-settings modal. Fields are
/// data-driven — the modal renders the `label` row, the input row
/// below, and dispatches typing into `input`. Confirm walks the
/// vec, parses each `input` per `kind`, and writes the result back
/// into `block.params[key]` (or removes the key when blank).
///
/// V1 only carries one kind (positive integer); a select / boolean
/// kind would slot in as a new enum variant the same way.
pub struct SettingsField {
    pub label: &'static str,
    /// JSON key in `block.params` — what the confirm path writes
    /// (or removes when the input is empty after trimming).
    pub key: &'static str,
    pub input: crate::vim::lineedit::LineEdit,
}

/// Multi-input settings modal state. Generic across block types —
/// `open_db_settings_modal` populates the fields vector based on
/// what the focused block supports (DB: limit + timeout; HTTP:
/// timeout only). Tab/BackTab cycle `focus`, Enter validates +
/// commits all fields, Esc cancels.
///
/// Pinned by user-memory `project_tui_block_settings_modal.md` —
/// settings live behind one chord (`gs`), one popup, multiple
/// inputs; never chord-per-field.
pub struct DbSettingsState {
    pub segment_idx: usize,
    pub fields: Vec<SettingsField>,
    /// Index into `fields`. Always within `0..fields.len()` while
    /// the modal is open; clamped by [`focus_next`]/[`focus_prev`].
    pub focus: usize,
}

impl DbSettingsState {
    /// Borrow the LineEdit for the focused field. Used by char /
    /// backspace / cursor-move actions to route into the right
    /// buffer without growing the action count. Falls back to the
    /// first field when `focus` is somehow out of range (defensive
    /// — open path always seeds a valid index).
    pub fn focused_input_mut(&mut self) -> &mut crate::vim::lineedit::LineEdit {
        let idx = self.focus.min(self.fields.len().saturating_sub(1));
        &mut self.fields[idx].input
    }

    /// Cycle focus to the next field, wrapping at the end. No-op
    /// when there's a single field (the only HTTP case today).
    pub fn focus_next(&mut self) {
        let n = self.fields.len();
        if n <= 1 {
            return;
        }
        self.focus = (self.focus + 1) % n;
    }

    /// Cycle focus to the previous field, wrapping at the start.
    pub fn focus_prev(&mut self) {
        let n = self.fields.len();
        if n <= 1 {
            return;
        }
        self.focus = (self.focus + n - 1) % n;
    }
}
