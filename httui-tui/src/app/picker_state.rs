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
#[derive(Debug)]
pub struct ConnectionPickerState {
    pub segment_idx: usize,
    pub connections: Vec<ConnectionEntry>,
    pub selected: usize,
}

/// V3 P3 (2026-05-23): inline create form. Opened by `n` inside
/// the Connections page (`Modal::Connections`). Submits to
/// `ConnectionsStore::create`; success closes the form and reloads
/// the page. The driver/readonly/text fields share a flat `focus`
/// cursor so Tab/Shift-Tab can cycle uniformly.
#[derive(Debug, Default)]
pub struct ConnectionFormState {
    pub name: crate::vim::lineedit::LineEdit,
    /// 0=postgres, 1=mysql, 2=sqlite. See `DRIVER_OPTIONS`.
    pub driver_idx: usize,
    pub host: crate::vim::lineedit::LineEdit,
    pub port: crate::vim::lineedit::LineEdit,
    pub database_name: crate::vim::lineedit::LineEdit,
    pub username: crate::vim::lineedit::LineEdit,
    pub password: crate::vim::lineedit::LineEdit,
    pub description: crate::vim::lineedit::LineEdit,
    pub is_readonly: bool,
    pub focus: ConnectionFormFocus,
    pub error: Option<String>,
}

impl ConnectionFormState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Tab order — flat list so focus cycling is just modular increment.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionFormFocus {
    #[default]
    Name,
    Driver,
    Host,
    Port,
    Database,
    Username,
    Password,
    Readonly,
    Description,
}

pub const DRIVER_OPTIONS: &[&str] = &["postgres", "mysql", "sqlite"];

impl ConnectionFormFocus {
    pub const ORDER: &'static [Self] = &[
        Self::Name,
        Self::Driver,
        Self::Host,
        Self::Port,
        Self::Database,
        Self::Username,
        Self::Password,
        Self::Readonly,
        Self::Description,
    ];

    pub fn next(self) -> Self {
        let idx = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        Self::ORDER[(idx + 1) % Self::ORDER.len()]
    }

    pub fn prev(self) -> Self {
        let idx = Self::ORDER.iter().position(|f| *f == self).unwrap_or(0);
        let len = Self::ORDER.len();
        Self::ORDER[(idx + len - 1) % len]
    }
}

/// Open instance of the **Connections page** — the dedicated
/// master-detail screen opened by `gC` / `Alt+P` (V3, 2026-05-23).
/// Unlike `ConnectionPickerState` (anchored popup for swapping a
/// block's connection), this is a fullscreen modal listing every
/// connection in `<vault>/connections.toml` with a detail panel for
/// the highlighted row. `connections` is a snapshot at open-time;
/// callers invoke `reload()` after mutating the store.
#[derive(Debug)]
pub struct ConnectionsPageState {
    pub connections: Vec<ConnectionDetail>,
    pub selected: usize,
}

/// Full detail of one connection — superset of `ConnectionEntry`
/// (which carries only the picker's needs). The Connections page
/// shows every public field in the right pane.
#[derive(Debug, Clone)]
pub struct ConnectionDetail {
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub has_password: bool,
    pub ssl_mode: Option<String>,
    pub is_readonly: bool,
    pub description: Option<String>,
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
#[derive(Debug)]
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
#[derive(Debug)]
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
#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_template_all_ships_the_v1_set_in_frequency_order() {
        let all = BlockTemplate::ALL;
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].label, "HTTP GET");
        assert_eq!(all[1].label, "HTTP POST (JSON)");
        assert_eq!(all[2].label, "SQLite Query");
    }

    #[test]
    fn block_template_texts_are_valid_fences_that_parse_to_blocks() {
        // Every template's `text` must round-trip through the parser
        // into a single Block segment — otherwise `gN` would splice
        // prose that never gets promoted.
        for tpl in BlockTemplate::ALL {
            let doc = crate::buffer::Document::from_markdown(tpl.text)
                .unwrap_or_else(|_| panic!("template {:?} should parse", tpl.label));
            let blocks = doc.segments().iter().filter(|s| s.is_block()).count();
            assert_eq!(blocks, 1, "template {:?} -> one block", tpl.label);
        }
        // Spot-check the fence kinds.
        assert!(BlockTemplate::ALL[0].text.starts_with("```http"));
        assert!(BlockTemplate::ALL[2].text.starts_with("```db-sqlite"));
    }

    #[test]
    fn block_template_is_copy_and_debug() {
        let t = BlockTemplate::ALL[0];
        let copied = t; // Copy — `t` stays usable below.
        assert_eq!(copied.label, "HTTP GET");
        assert_eq!(t.text, copied.text);
        // Debug is derived — keep it covered.
        assert!(format!("{t:?}").contains("HTTP GET"));
    }

    #[test]
    fn block_template_picker_state_new_starts_at_zero() {
        let s = BlockTemplatePickerState::new();
        assert_eq!(s.selected, 0);
    }

    #[test]
    fn block_template_picker_state_default_matches_new() {
        let d = BlockTemplatePickerState::default();
        assert_eq!(d.selected, BlockTemplatePickerState::new().selected);
    }

    #[test]
    fn connection_picker_state_holds_anchor_and_entries() {
        let state = ConnectionPickerState {
            segment_idx: 4,
            connections: vec![
                ConnectionEntry {
                    id: "c1".into(),
                    name: "Local PG".into(),
                    kind: "postgres".into(),
                },
                ConnectionEntry {
                    id: "c2".into(),
                    name: "SQLite".into(),
                    kind: "sqlite".into(),
                },
            ],
            selected: 1,
        };
        assert_eq!(state.segment_idx, 4);
        assert_eq!(state.connections.len(), 2);
        let chosen = &state.connections[state.selected];
        assert_eq!(chosen.id, "c2");
        assert_eq!(chosen.name, "SQLite");
        assert_eq!(chosen.kind, "sqlite");
        // Debug + Clone are derived — exercise them so the derives
        // stay covered.
        let cloned = chosen.clone();
        assert!(format!("{cloned:?}").contains("SQLite"));
    }

    #[test]
    fn tab_picker_state_entry_carries_back_pointer_and_dirty_flag() {
        let state = TabPickerState {
            entries: vec![
                TabPickerEntry {
                    idx: 0,
                    label: "a.md".into(),
                    dirty: false,
                },
                TabPickerEntry {
                    idx: 1,
                    label: "(no file)".into(),
                    dirty: true,
                },
            ],
            selected: 1,
        };
        let row = &state.entries[state.selected];
        assert_eq!(row.idx, 1);
        assert_eq!(row.label, "(no file)");
        assert!(row.dirty);
        assert!(format!("{:?}", row.clone()).contains("no file"));
    }

    #[test]
    fn last_run_anchor_records_path_segment_and_alias() {
        let a = LastRunAnchor {
            file_path: std::path::PathBuf::from("/v/notes.md"),
            segment_idx: 7,
            alias: Some("req1".into()),
        };
        assert_eq!(a.segment_idx, 7);
        assert_eq!(a.alias.as_deref(), Some("req1"));
        let cloned = a.clone();
        assert_eq!(cloned.file_path, std::path::PathBuf::from("/v/notes.md"));
        assert!(format!("{cloned:?}").contains("req1"));
    }

    #[test]
    fn environment_picker_state_marks_active_row() {
        let state = EnvironmentPickerState {
            entries: vec![
                EnvironmentEntry {
                    id: "e1".into(),
                    name: "dev".into(),
                },
                EnvironmentEntry {
                    id: "e2".into(),
                    name: "prod".into(),
                },
            ],
            selected: 0,
            active_id: Some("e2".into()),
        };
        assert_eq!(state.entries.len(), 2);
        assert_eq!(state.active_id.as_deref(), Some("e2"));
        let active = state
            .entries
            .iter()
            .find(|e| Some(&e.id) == state.active_id.as_ref())
            .unwrap();
        assert_eq!(active.name, "prod");
        assert!(format!("{:?}", active.clone()).contains("prod"));
    }
}
