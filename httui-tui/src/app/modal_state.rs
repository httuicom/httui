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
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
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

/// State for the run-confirm modal. Carries the segment to re-run
/// (the cursor may have moved in between) and the human reason

/// Generic y/n confirm modal. Replaces the per-flow confirm structs
/// (`DbConfirmRunState`, `ConnectionDeleteConfirmState`,
/// `EnvDeleteConfirmState`, `VarDeleteConfirmState`, …): the modal owns
/// only display strings + the actions to emit on `y`/`Enter` and
/// `n`/`Esc`. Each flow's data lives in [`ConfirmPayload`] so action
/// appliers can extract their context without resurrecting per-flow
/// variants. Keeps [`crate::input::action::Action`] `Copy`.
///
/// `on_confirm` / `on_cancel` are `Copy` so the modal handler can
/// emit them without consuming the state — the modal stays open until
/// the applier runs and explicitly closes it (the usual pattern).
#[derive(Debug)]
pub struct ConfirmPromptState {
    pub title: String,
    pub body: String,
    pub on_confirm: crate::input::action::Action,
    pub on_cancel: crate::input::action::Action,
    pub payload: ConfirmPayload,
}

/// Per-flow context carried by a [`ConfirmPromptState`]. Each variant
/// is read by exactly one confirm applier (e.g. `apply_confirm_db_run`
/// matches `DbSegment(idx)`); appliers ignore the variant tag and trust
/// the modal's `on_confirm` to land them on the right arm.
#[derive(Debug)]
pub enum ConfirmPayload {
    DbSegment(usize),
    ConnectionName(String),
    EnvName(String),
    Var { env_name: String, key: String },
    HeaderRow(usize),
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
/// `segment_idx` at confirm-time.
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
pub struct DbSettingsState {
    pub segment_idx: usize,
    pub fields: Vec<SettingsField>,
    /// Index into `fields`. Always within `0..fields.len()` while
    /// the modal is open; clamped by [`focus_next`]/[`focus_prev`].
    pub focus: usize,
}

/// Open state for the "you have unsaved blocks" prompt. Shown when the
/// user attempts to toggle DOC↔BLOCKS with at least one pane carrying a
/// `BlockDraft`. `dirty` is informational — the actual draft data still
/// lives on each pane; this struct only records the file list so the
/// modal can render "X files unsaved" without re-walking the tree.
#[derive(Debug, Clone)]
pub struct BlocksUnsavedPromptState {
    pub dirty: Vec<std::path::PathBuf>,
    /// What focus rests on. Save is the leftmost / safest action so
    /// pressing Enter immediately commits and proceeds — matches the
    /// vault picker / connections-page muscle memory.
    pub focus: BlocksUnsavedPromptFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlocksUnsavedPromptFocus {
    #[default]
    Save,
    Discard,
    Cancel,
}

impl BlocksUnsavedPromptFocus {
    pub fn next(self) -> Self {
        match self {
            Self::Save => Self::Discard,
            Self::Discard => Self::Cancel,
            Self::Cancel => Self::Save,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Save => Self::Cancel,
            Self::Discard => Self::Save,
            Self::Cancel => Self::Discard,
        }
    }

    /// Human label for chips / debug. Renderer reads chip text from a
    /// fixed table so this is only used by tests / future surfaces.
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Self::Save => "Save",
            Self::Discard => "Discard",
            Self::Cancel => "Cancel",
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vim::lineedit::LineEdit;

    // ---- BlockExportFormat ----

    #[test]
    fn block_export_format_db_list_is_the_four_tabular_serializers() {
        assert_eq!(
            BlockExportFormat::DB_FORMATS,
            &[
                BlockExportFormat::Csv,
                BlockExportFormat::Json,
                BlockExportFormat::Markdown,
                BlockExportFormat::Insert,
            ]
        );
    }

    #[test]
    fn block_export_format_http_list_is_the_five_code_generators() {
        assert_eq!(
            BlockExportFormat::HTTP_FORMATS,
            &[
                BlockExportFormat::Curl,
                BlockExportFormat::Fetch,
                BlockExportFormat::Python,
                BlockExportFormat::HTTPie,
                BlockExportFormat::HttpFile,
            ]
        );
    }

    #[test]
    fn block_export_format_label_covers_every_variant() {
        assert_eq!(BlockExportFormat::Csv.label(), "CSV");
        assert_eq!(BlockExportFormat::Json.label(), "JSON");
        assert_eq!(BlockExportFormat::Markdown.label(), "Markdown table");
        assert_eq!(BlockExportFormat::Insert.label(), "INSERT statements");
        assert_eq!(BlockExportFormat::Curl.label(), "cURL");
        assert_eq!(BlockExportFormat::Fetch.label(), "JavaScript (fetch)");
        assert_eq!(BlockExportFormat::Python.label(), "Python (requests)");
        assert_eq!(BlockExportFormat::HTTPie.label(), "HTTPie");
        assert_eq!(BlockExportFormat::HttpFile.label(), ".http file");
    }

    #[test]
    fn db_export_format_alias_points_at_block_export_format() {
        // The back-compat type alias must remain usable.
        let f: DbExportFormat = BlockExportFormat::Json;
        assert_eq!(f.label(), "JSON");
    }

    // ---- DbExportPickerState ----

    #[test]
    fn db_export_picker_new_seeds_selection_zero_and_keeps_formats() {
        let st = DbExportPickerState::new(9, BlockExportFormat::DB_FORMATS);
        assert_eq!(st.segment_idx, 9);
        assert_eq!(st.selected, 0);
        assert_eq!(st.formats.len(), 4);
        assert_eq!(st.formats[0], BlockExportFormat::Csv);

        let http = DbExportPickerState::new(0, BlockExportFormat::HTTP_FORMATS);
        assert_eq!(http.formats.len(), 5);
        assert_eq!(http.formats[0], BlockExportFormat::Curl);
    }

    // ---- ContentSearchState ----

    fn result(path: &str) -> httui_core::search::ContentSearchResult {
        httui_core::search::ContentSearchResult {
            file_path: path.to_string(),
            snippet: format!("…{path}…"),
        }
    }

    #[test]
    fn content_search_new_is_empty_and_not_building() {
        let st = ContentSearchState::new();
        assert!(st.query.is_empty());
        assert!(st.results.is_empty());
        assert_eq!(st.selected, 0);
        assert!(!st.building);
        assert!(st.chosen().is_none());
    }

    #[test]
    fn content_search_select_next_clamps_at_last_result() {
        let mut st = ContentSearchState::new();
        st.results = vec![result("a.md"), result("b.md"), result("c.md")];
        assert_eq!(st.chosen().unwrap().file_path, "a.md");
        st.select_next();
        assert_eq!(st.selected, 1);
        st.select_next();
        assert_eq!(st.selected, 2);
        // Already at the end — clamps, never overshoots.
        st.select_next();
        assert_eq!(st.selected, 2);
        assert_eq!(st.chosen().unwrap().file_path, "c.md");
    }

    #[test]
    fn content_search_select_next_is_a_noop_on_empty_results() {
        let mut st = ContentSearchState::new();
        st.select_next();
        assert_eq!(st.selected, 0);
    }

    #[test]
    fn content_search_select_prev_stops_at_zero() {
        let mut st = ContentSearchState::new();
        st.results = vec![result("a.md"), result("b.md")];
        st.selected = 1;
        st.select_prev();
        assert_eq!(st.selected, 0);
        // Can't go below zero.
        st.select_prev();
        assert_eq!(st.selected, 0);
    }

    // ---- DbSettingsState ----

    fn field(label: &'static str, key: &'static str, val: &str) -> SettingsField {
        SettingsField {
            label,
            key,
            input: LineEdit::from_str(val),
        }
    }

    fn db_settings(field_count: usize) -> DbSettingsState {
        let fields = (0..field_count)
            .map(|i| {
                if i == 0 {
                    field("Limit", "limit", "100")
                } else {
                    field("Timeout (ms)", "timeout_ms", "30000")
                }
            })
            .collect();
        DbSettingsState {
            segment_idx: 2,
            fields,
            focus: 0,
        }
    }

    #[test]
    fn db_settings_focused_input_mut_targets_the_focused_field() {
        let mut st = db_settings(2);
        assert_eq!(st.focused_input_mut().as_str(), "100");
        st.focus = 1;
        assert_eq!(st.focused_input_mut().as_str(), "30000");
        // Mutating through the borrow hits the live buffer.
        st.focused_input_mut().insert_char('0');
        assert_eq!(st.fields[1].input.as_str(), "300000");
    }

    #[test]
    fn db_settings_focused_input_mut_clamps_out_of_range_focus() {
        let mut st = db_settings(2);
        st.focus = 99; // defensive: open path never does this
                       // Falls back to the last field instead of panicking.
        assert_eq!(st.focused_input_mut().as_str(), "30000");
    }

    #[test]
    fn db_settings_focus_next_wraps_with_multiple_fields() {
        let mut st = db_settings(2);
        assert_eq!(st.focus, 0);
        st.focus_next();
        assert_eq!(st.focus, 1);
        st.focus_next();
        assert_eq!(st.focus, 0); // wrap
    }

    #[test]
    fn db_settings_focus_prev_wraps_with_multiple_fields() {
        let mut st = db_settings(2);
        st.focus_prev();
        assert_eq!(st.focus, 1); // wrap backwards
        st.focus_prev();
        assert_eq!(st.focus, 0);
    }

    #[test]
    fn db_settings_focus_cycling_is_a_noop_with_a_single_field() {
        // The HTTP case ships one field (timeout only) — Tab/BackTab
        // must not move focus or modulo-by-zero.
        let mut st = db_settings(1);
        st.focus_next();
        assert_eq!(st.focus, 0);
        st.focus_prev();
        assert_eq!(st.focus, 0);
    }

    // ---- Document-carrying + plain state structs ----

    fn doc() -> Document {
        Document::from_markdown("status 200\nheader: x\n\nbody\n").unwrap()
    }

    #[test]
    fn db_row_detail_state_carries_a_navigable_sub_document() {
        let st = DbRowDetailState {
            segment_idx: 1,
            row: 4,
            title: " row 4 ".into(),
            doc: doc(),
            viewport_height: 12,
            viewport_top: 0,
        };
        assert_eq!(st.segment_idx, 1);
        assert_eq!(st.row, 4);
        assert_eq!(st.title, " row 4 ");
        assert_eq!(st.viewport_height, 12);
        assert!(!st.doc.segments().is_empty());
    }

    #[test]
    fn http_response_detail_state_carries_a_navigable_sub_document() {
        let st = HttpResponseDetailState {
            segment_idx: 2,
            title: " 200 OK ".into(),
            doc: doc(),
            viewport_height: 20,
            viewport_top: 3,
        };
        assert_eq!(st.segment_idx, 2);
        assert_eq!(st.title, " 200 OK ");
        assert_eq!(st.viewport_top, 3);
        assert!(!st.doc.segments().is_empty());
    }

    #[test]
    fn completion_popup_state_tracks_prefix_and_anchor() {
        let st = CompletionPopupState {
            segment_idx: 5,
            items: vec![crate::sql_completion::CompletionItem {
                label: "SELECT".into(),
                kind: crate::sql_completion::CompletionKind::Keyword,
                detail: None,
            }],
            selected: 0,
            anchor_line: 2,
            anchor_offset: 7,
            prefix: "SEL".into(),
        };
        assert_eq!(st.segment_idx, 5);
        assert_eq!(st.items.len(), 1);
        assert_eq!(st.items[0].label, "SELECT");
        assert_eq!(st.anchor_line, 2);
        assert_eq!(st.anchor_offset, 7);
        assert_eq!(st.prefix, "SEL");
    }

    #[test]
    fn confirm_prompt_state_carries_actions_and_payload() {
        let st = ConfirmPromptState {
            title: "Confirm write".into(),
            body: "UPDATE without WHERE".into(),
            on_confirm: crate::input::action::Action::ConfirmDbRun,
            on_cancel: crate::input::action::Action::CancelDbRun,
            payload: ConfirmPayload::DbSegment(8),
        };
        assert!(matches!(st.payload, ConfirmPayload::DbSegment(8)));
        assert!(matches!(
            st.on_confirm,
            crate::input::action::Action::ConfirmDbRun
        ));
    }

    #[test]
    fn block_history_state_carries_title_and_entries() {
        let entry = httui_core::block_history::HistoryEntry {
            id: 1,
            file_path: "/v/notes.md".into(),
            block_alias: "req1".into(),
            method: "GET".into(),
            url_canonical: "https://api.test/users".into(),
            status: Some(200),
            request_size: Some(0),
            response_size: Some(42),
            elapsed_ms: Some(120),
            outcome: "ok".into(),
            ran_at: "2026-05-18T00:00:00Z".into(),
            plan: None,
        };
        let st = BlockHistoryState {
            segment_idx: 6,
            title: "GET req1".into(),
            entries: vec![entry],
            selected: 0,
        };
        assert_eq!(st.segment_idx, 6);
        assert_eq!(st.title, "GET req1");
        assert_eq!(st.entries.len(), 1);
        assert_eq!(st.entries[st.selected].status, Some(200));
    }
}
