//! Per-vault TUI view state snapshot. Lives inside `user.toml` under
//! `[tui_view_state.<canonical-vault-path>]`. Blocks are keyed by
//! `alias + line_start` so a `.md` reorganised between sessions
//! degrades to "no block selected" rather than picking a wrong one.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Top-level per-vault snapshot. `last_view` chooses DOC or BLOCKS on
/// reopen; `blocks` is `Some` whenever there's enough state to restore
/// the BLOCKS workspace (sidebar + panes).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TuiViewState {
    /// `"doc"` or `"blocks"`. Unknown strings collapse to `doc` on
    /// read (defensive — older TUIs that didn't write this field).
    #[serde(default)]
    pub last_view: String,
    /// Whether the left tree/blocks sidebar is visible. Independent
    /// of view — the user can hide the sidebar in either DOC or
    /// BLOCKS.
    #[serde(default)]
    pub sidebar_open: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<BlocksWorkspaceSnapshot>,
}

/// Sidebar + pane tree for BLOCKS view. `expanded_files` and the
/// sidebar cursor reference files by vault-relative path so they
/// survive `BlockIndex` rebuilds without index gymnastics.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BlocksWorkspaceSnapshot {
    /// Vault-relative paths of `.md` files whose tree entry is
    /// expanded in the sidebar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expanded_files: Vec<String>,
    /// Sidebar cursor — file row when `block` is `None`, block row
    /// otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<SidebarPos>,
    /// Pane binary tree at the moment of capture.
    pub root: PaneSnapshot,
    /// Focus path: sequence of `0`/`1` (first/second child) from root
    /// down to the focused leaf. Empty when the root is the leaf.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focused: Vec<u8>,
}

/// Sidebar cursor anchor: which file is hovered, and (when an
/// expanded file is open) which block under it.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SidebarPos {
    /// Vault-relative path of the file the cursor is on.
    pub file: String,
    /// Identity of the block currently under the cursor; `None` when
    /// the cursor is on the file row itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<BlockKey>,
}

/// Stable identity for a block inside a `.md` across reopens. `alias`
/// wins when set; `line_start` is the fallback. Both are recorded so
/// restoration can fall back deterministically when one changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BlockKey {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default)]
    pub line_start: u32,
}

/// File + block identity. Used by `PaneLeafSnapshot` because the
/// BLOCKS sidebar can target a block in any vault file regardless of
/// which doc the pane has open.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BlockSelection {
    pub file: String,
    #[serde(flatten)]
    pub key: BlockKey,
}

/// One node of the pane binary tree. Serialised as a tagged union so
/// `[tui_view_state.<vault>.blocks.root.kind = "leaf" | "split"]`
/// stays readable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PaneSnapshot {
    Leaf(PaneLeafSnapshot),
    Split(PaneSplitSnapshot),
}

impl Default for PaneSnapshot {
    fn default() -> Self {
        PaneSnapshot::Leaf(PaneLeafSnapshot::default())
    }
}

/// Per-leaf state: file the pane has open + BLOCKS-view selection.
/// Mirrors `httui_tui::pane::Pane` fields we care about for restore.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PaneLeafSnapshot {
    /// Vault-relative file path. `None` for an empty pane (no doc
    /// opened in this leaf).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Block selected in BLOCKS view. Carries its own file path
    /// because the BLOCKS sidebar can pick a block from a file
    /// different from the pane's `document_path`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<BlockSelection>,
    /// Focused region inside the block (0..=N). Clamped by the
    /// applier when block kind disagrees on count.
    #[serde(default)]
    pub region: u32,
    /// Row inside a table-shaped region (Headers). Ignored for
    /// other regions.
    #[serde(default)]
    pub row: u32,
    /// Column inside a table-shaped region: `0 = key`, `1 = value`.
    /// Default `1` matches `Pane::empty()`.
    #[serde(default = "default_col")]
    pub col: u32,
}

fn default_col() -> u32 {
    1
}

/// Inner node: the split direction, ratio in `[0,1]`, and two
/// children. `direction` is `"vertical"` or `"horizontal"`; unknown
/// strings collapse to `vertical` on read.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaneSplitSnapshot {
    pub direction: String,
    pub ratio: f32,
    pub first: Box<PaneSnapshot>,
    pub second: Box<PaneSnapshot>,
}

/// Map of canonical vault path → snapshot. Lives in `UserFile`. The
/// `BTreeMap` ordering keeps `user.toml` diff-friendly.
pub type TuiViewStateMap = BTreeMap<String, TuiViewState>;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_leaf() -> PaneLeafSnapshot {
        PaneLeafSnapshot {
            file: Some("api.md".into()),
            block: Some(BlockSelection {
                file: "api.md".into(),
                key: BlockKey {
                    alias: Some("login".into()),
                    line_start: 2,
                },
            }),
            region: 1,
            row: 0,
            col: 1,
        }
    }

    #[test]
    fn leaf_only_snapshot_round_trips() {
        let snap = TuiViewState {
            last_view: "blocks".into(),
            sidebar_open: true,
            blocks: Some(BlocksWorkspaceSnapshot {
                expanded_files: vec!["api.md".into()],
                cursor: Some(SidebarPos {
                    file: "api.md".into(),
                    block: Some(BlockKey {
                        alias: Some("login".into()),
                        line_start: 2,
                    }),
                }),
                root: PaneSnapshot::Leaf(sample_leaf()),
                focused: vec![],
            }),
        };
        let raw = toml::to_string(&snap).unwrap();
        let back: TuiViewState = toml::from_str(&raw).unwrap();
        assert_eq!(back, snap);
    }

    #[test]
    fn split_snapshot_round_trips() {
        let snap = TuiViewState {
            last_view: "blocks".into(),
            sidebar_open: false,
            blocks: Some(BlocksWorkspaceSnapshot {
                expanded_files: vec![],
                cursor: None,
                root: PaneSnapshot::Split(PaneSplitSnapshot {
                    direction: "vertical".into(),
                    ratio: 0.5,
                    first: Box::new(PaneSnapshot::Leaf(sample_leaf())),
                    second: Box::new(PaneSnapshot::Leaf(PaneLeafSnapshot::default())),
                }),
                focused: vec![1],
            }),
        };
        let raw = toml::to_string(&snap).unwrap();
        let back: TuiViewState = toml::from_str(&raw).unwrap();
        assert_eq!(back, snap);
    }

    #[test]
    fn missing_fields_default() {
        let raw = "";
        let back: TuiViewState = toml::from_str(raw).unwrap();
        assert_eq!(back.last_view, "");
        assert!(back.blocks.is_none());
    }

    #[test]
    fn pane_leaf_defaults_col_to_one() {
        let raw = "kind = \"leaf\"\n";
        let back: PaneSnapshot = toml::from_str(raw).unwrap();
        let PaneSnapshot::Leaf(leaf) = back else {
            panic!("expected leaf");
        };
        assert_eq!(leaf.col, 1);
        assert_eq!(leaf.region, 0);
        assert!(leaf.file.is_none());
        assert!(leaf.block.is_none());
    }

    #[test]
    fn block_key_omits_none_alias() {
        let key = BlockKey {
            alias: None,
            line_start: 42,
        };
        let raw = toml::to_string(&key).unwrap();
        assert!(!raw.contains("alias"), "got: {raw}");
        assert!(raw.contains("line_start = 42"));
    }

    #[test]
    fn snapshot_default_is_doc_view() {
        let s = TuiViewState::default();
        assert_eq!(s.last_view, "");
        assert!(s.blocks.is_none());
    }
}
