//! Popup / picker / template open-state.
//!
//! Mechanically extracted from `app.rs` (tui-v2 vertical 1, fase 2
//! p1-picker_state) — pure code move, no behavior change. Re-exported
//! from `app/mod.rs` so the `crate::app::*` picker call sites keep
//! resolving. Exercised via the `commands::*` picker-open paths and
//! the dispatch integration tests.

use std::path::PathBuf;

/// Open instance of the connection picker popup. Anchored to the
/// DB block at `segment_idx` (the cursor was on it when the picker
/// opened); `connections` is the list pulled from `httui-core`'s
/// connection registry; `selected` indexes into it. The renderer
/// paints the popup just below the block (or above when there's no
/// room) — see `ui::connection_picker`.
pub struct ConnectionPickerState {
    pub segment_idx: usize,
    pub connections: Vec<ConnectionEntry>,
    pub selected: usize,
}

/// Lightweight snapshot of one connection — the picker only needs
/// the id (to write back to the fence) and the human label (to
/// show in the list). Cloned out of `httui-core`'s registry at
/// open-time so the picker doesn't hold a borrow on the pool
/// manager while it's up.
#[derive(Debug, Clone)]
pub struct ConnectionEntry {
    pub id: String,
    pub name: String,
    pub kind: String,
}

/// Open instance of the tab picker (`gb`). Lists every tab in the
/// `TabBar` by its focused-leaf path. Cloned at open-time so the
/// picker doesn't hold a borrow on `TabBar` while it's up; the user
/// can keep typing if a future iteration adds search.
pub struct TabPickerState {
    pub entries: Vec<TabPickerEntry>,
    pub selected: usize,
}

/// One row in the tab picker. `idx` is the back-pointer into
/// `TabBar.tabs` (the picker passes it to `set_active` on confirm).
/// `label` is the path or `(no file)`; `dirty` mirrors the document's
/// dirty flag so the renderer can paint a `*` marker.
#[derive(Debug, Clone)]
pub struct TabPickerEntry {
    pub idx: usize,
    pub label: String,
    pub dirty: bool,
}

/// Coordinates of the last block the user kicked a run on. Records
/// both the file path *and* the segment index so `gr` (rerun) can
/// gracefully decline when the user has switched to a different
/// document. Alias is preferred over segment index for resilience —
/// edits above the block shift the index but not the alias.
#[derive(Debug, Clone)]
pub struct LastRunAnchor {
    pub file_path: PathBuf,
    pub segment_idx: usize,
    pub alias: Option<String>,
}

/// One row in the block-template picker. Static / hand-curated —
/// the desktop's slash-command list (`src/lib/codemirror/cm-slash-commands.ts`)
/// has the canonical set; we ship a trimmed V1 with the three most
/// common templates. `text` is the fence to splice into the prose
/// segment; `Document::reparse_prose_at` then promotes it to a
/// `Segment::Block` so the user sees the rendered block immediately.
#[derive(Debug, Clone, Copy)]
pub struct BlockTemplate {
    pub label: &'static str,
    pub text: &'static str,
}

impl BlockTemplate {
    /// V1 template set. Order is "expected frequency" — HTTP GET
    /// first, then HTTP POST (with JSON body skeleton), then a
    /// SQLite query starter. Postgres / MySQL templates can land
    /// later; users with those drivers usually copy from an
    /// existing block anyway.
    pub const ALL: &'static [BlockTemplate] = &[
        BlockTemplate {
            label: "HTTP GET",
            text: "```http alias=req1\nGET https://example.com\n```\n",
        },
        BlockTemplate {
            label: "HTTP POST (JSON)",
            text: "```http alias=req1\nPOST https://example.com\nContent-Type: application/json\n\n{}\n```\n",
        },
        BlockTemplate {
            label: "SQLite Query",
            text: "```db-sqlite alias=db1\nSELECT 1;\n```\n",
        },
    ];
}

/// Open instance of the block-template picker (`gN`). The picker
/// lives over the editor, centered (no anchor — the templates aren't
/// tied to a source block). `selected` indexes into `BlockTemplate::ALL`.
pub struct BlockTemplatePickerState {
    pub selected: usize,
}

impl BlockTemplatePickerState {
    pub fn new() -> Self {
        Self { selected: 0 }
    }
}

impl Default for BlockTemplatePickerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Open instance of the environment picker popup (`gE`). Lists every
/// row from the `environments` table; `selected` indexes into
/// `entries`. The active env is identified by `active_id` so the
/// renderer can mark it. Confirm flips the active flag in SQLite,
/// refreshes the cached `App.active_env_name`, and dismisses.
pub struct EnvironmentPickerState {
    pub entries: Vec<EnvironmentEntry>,
    pub selected: usize,
    /// Id of the currently-active env, if any. Snapshotted at
    /// open-time so the renderer knows which row to flag without
    /// hitting SQLite again. May be stale if the user activates a
    /// different env, but the picker is dismissed on confirm.
    pub active_id: Option<String>,
}

/// Lightweight snapshot of one environment row — only the id (to
/// activate via `set_active_environment`) and the display name (for
/// the list). Cloned out of `httui-core::db::environments` at
/// open-time.
#[derive(Debug, Clone)]
pub struct EnvironmentEntry {
    pub id: String,
    pub name: String,
}
