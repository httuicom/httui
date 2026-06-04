//! File-tree state for the left sidebar.
//!
//! Visible-when-toggled list of files and directories under the active
//! vault. Folders can be expanded / collapsed; the visible flat list
//! [`entries`](FileTree::entries) is rebuilt every time the user
//! changes the expanded set or refreshes the tree.
//!
//! Selection (`selected`) is an index into `entries`. Expansion state
//! (`expanded`) is keyed by the entry's path relative to the vault, so
//! a `refresh()` after creating / deleting files preserves whatever
//! the user had open.

use std::collections::HashSet;
use std::path::Path;

use httui_core::fs::FileEntry;

use crate::app::BlockIndex;

#[derive(Debug, Default)]
pub struct FileTree {
    /// Whether the sidebar is rendered. Independent of focus — focus
    /// is encoded by [`crate::vim::mode::Mode::Tree`].
    pub visible: bool,
    /// Flattened, ordered list of currently visible entries.
    pub entries: Vec<TreeNode>,
    /// Index into `entries` of the selected row. Clamped after every
    /// refresh.
    pub selected: usize,
    /// Set of folder paths (relative to vault) currently expanded.
    pub expanded: HashSet<String>,
    /// Active in-tree prompt (`a`/`r`/`d` shortcuts). When `Some`, the
    /// app is in [`crate::vim::mode::Mode::TreePrompt`] and the input
    /// runs through a tree-specific parser, not cmdline.
    pub prompt: Option<TreePrompt>,
    /// Some(idx) switches the tree to "blocks" rendering: each file
    /// shows its executable blocks as expandable children. None = the
    /// classic filesystem rendering.
    pub block_index: Option<BlockIndex>,
}

/// Inline prompt for tree-driven file ops. Each kind has a different
/// label and a different post-Enter behavior; the user-typed payload
/// goes into `input`, a [`LineEdit`] so cursor navigation works.
#[derive(Debug, Clone)]
pub struct TreePrompt {
    pub kind: TreePromptKind,
    pub input: crate::vim::lineedit::LineEdit,
}

impl TreePrompt {
    /// Create a new prompt with the cursor anchored at the end of any
    /// pre-fill text (so the user can type immediately).
    pub fn new(kind: TreePromptKind, prefill: String) -> Self {
        Self {
            kind,
            input: crate::vim::lineedit::LineEdit::from_str(prefill),
        }
    }

    pub fn buffer(&self) -> &str {
        self.input.as_str()
    }

    pub fn cursor_col(&self) -> usize {
        self.input.cursor_col()
    }
}

#[derive(Debug, Clone)]
pub enum TreePromptKind {
    /// "new file in <dir>/: <name>" — `dir` is read-only context;
    /// `buffer` is what the user types after the slash.
    Create { dir: String },
    /// "rename to: <buffer>" — `from` is the original relative path
    /// (used when constructing the rename call); buffer starts pre-
    /// filled with `from` and the user edits the destination.
    Rename { from: String },
    /// "delete <target>? (y/N)" — `buffer` accumulates the answer; we
    /// commit on Enter when buffer is `y` or `Y`.
    Delete { target: String },
    /// BLOCKS-view destructive confirm for a block (NOT a file).
    /// Carries the vault-relative `.md` path + block index inside that
    /// file + a human label (alias or line number) shown to the user.
    DeleteBlock {
        rel_path: String,
        block_idx: usize,
        label: String,
    },
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    /// Path relative to the vault (matches [`FileEntry::path`]).
    pub path: String,
    pub is_dir: bool,
    /// Indentation level, 0 = vault root.
    pub depth: usize,
    /// When `Some`, this row represents an executable block under its
    /// host `.md` (rendered as a child row). File / dir rows leave it
    /// `None`.
    pub block: Option<TreeBlockMeta>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeBlockMeta {
    pub file_idx: usize,
    pub block_idx: usize,
    pub block_type: String,
    pub label: String,
}

impl FileTree {
    /// Re-scan the vault and rebuild `entries`. Called on toggle,
    /// expand/collapse, and explicit refresh (`R`). When
    /// `block_index` is set, the tree paints executable blocks as
    /// children of each `.md` instead of the raw filesystem layout.
    pub fn refresh(&mut self, vault: &Path) {
        self.entries.clear();
        if let Some(index) = self.block_index.clone() {
            flatten_blocks(&index, &self.expanded, &mut self.entries);
        } else {
            let raw = httui_core::fs::list_workspace(&vault.to_string_lossy()).unwrap_or_default();
            flatten(&raw, 0, &self.expanded, &mut self.entries);
        }
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
    }

    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1).min(self.entries.len() - 1);
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn select_first(&mut self) {
        self.selected = 0;
    }

    pub fn select_last(&mut self) {
        if !self.entries.is_empty() {
            self.selected = self.entries.len() - 1;
        }
    }

    pub fn current(&self) -> Option<&TreeNode> {
        self.entries.get(self.selected)
    }

    /// Toggle expansion of the selected entry. In filesystem mode this
    /// only acts on directories; in block mode it also expands files
    /// (revealing their blocks as children). Block rows are not
    /// expandable. Returns `true` when the tree changed.
    pub fn toggle_expand(&mut self) -> bool {
        let Some(node) = self.entries.get(self.selected) else {
            return false;
        };
        if node.block.is_some() {
            return false;
        }
        let is_block_mode = self.block_index.is_some();
        if !node.is_dir && !is_block_mode {
            return false;
        }
        let path = node.path.clone();
        if self.expanded.contains(&path) {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path);
        }
        true
    }

    /// Collapse the parent directory of the current entry — vim
    /// `h`-on-file behavior.
    pub fn collapse_parent(&mut self) -> bool {
        let Some(current) = self.entries.get(self.selected) else {
            return false;
        };
        if current.is_dir && self.expanded.contains(&current.path) {
            self.expanded.remove(&current.path);
            return true;
        }
        // Walk up to find the enclosing dir, remove it from expanded,
        // and re-anchor selection on it.
        let target_depth = current.depth.saturating_sub(1);
        let mut parent_idx = None;
        for i in (0..self.selected).rev() {
            if self.entries[i].depth == target_depth && self.entries[i].is_dir {
                parent_idx = Some(i);
                break;
            }
        }
        let Some(idx) = parent_idx else {
            return false;
        };
        let path = self.entries[idx].path.clone();
        self.expanded.remove(&path);
        self.selected = idx;
        true
    }
}

fn flatten(
    entries: &[FileEntry],
    depth: usize,
    expanded: &HashSet<String>,
    out: &mut Vec<TreeNode>,
) {
    for e in entries {
        out.push(TreeNode {
            name: e.name.clone(),
            path: e.path.clone(),
            is_dir: e.is_dir,
            depth,
            block: None,
        });
        if e.is_dir && expanded.contains(&e.path) {
            if let Some(children) = e.children.as_deref() {
                flatten(children, depth + 1, expanded, out);
            }
        }
    }
}

fn flatten_blocks(index: &BlockIndex, expanded: &HashSet<String>, out: &mut Vec<TreeNode>) {
    for (file_idx, file) in index.files.iter().enumerate() {
        let path = file.display.clone();
        out.push(TreeNode {
            name: file.display.clone(),
            path: path.clone(),
            is_dir: false,
            depth: 0,
            block: None,
        });
        if !expanded.contains(&path) {
            continue;
        }
        for (block_idx, block) in file.blocks.iter().enumerate() {
            out.push(TreeNode {
                name: block.label(),
                path: path.clone(),
                is_dir: false,
                depth: 1,
                block: Some(TreeBlockMeta {
                    file_idx,
                    block_idx,
                    block_type: block.block_type.clone(),
                    label: block.label(),
                }),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(dir: &Path, rel: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, "").unwrap();
    }

    #[test]
    fn refresh_lists_top_level_only_initially() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "a.md");
        touch(v.path(), "sub/inner.md");

        let mut t = FileTree::default();
        t.refresh(v.path());

        // sub appears as a folder; inner.md is hidden inside it.
        let names: Vec<&str> = t.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"a.md"));
        assert!(names.contains(&"sub"));
        assert!(!names.contains(&"inner.md"));
    }

    #[test]
    fn expanding_folder_reveals_children() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "a.md");
        touch(v.path(), "sub/inner.md");

        let mut t = FileTree::default();
        t.refresh(v.path());
        // Move selection to "sub" (folders sort first → index 0).
        let sub_idx = t
            .entries
            .iter()
            .position(|e| e.name == "sub")
            .expect("sub present");
        t.selected = sub_idx;
        assert!(t.toggle_expand());
        t.refresh(v.path());

        let names: Vec<&str> = t.entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"inner.md"));
    }

    #[test]
    fn collapse_parent_returns_to_folder_row() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "sub/x.md");

        let mut t = FileTree::default();
        t.refresh(v.path());
        t.expanded.insert("sub".into());
        t.refresh(v.path());

        // Select inner file.
        let inner_idx = t.entries.iter().position(|e| e.name == "x.md").unwrap();
        t.selected = inner_idx;

        assert!(t.collapse_parent());
        t.refresh(v.path());
        // Selection should be on the parent folder, which is no longer expanded.
        assert_eq!(t.entries[t.selected].name, "sub");
        assert!(!t.expanded.contains("sub"));
    }

    #[test]
    fn refresh_clamps_selection_when_entries_shrink() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "a.md");
        touch(v.path(), "b.md");
        touch(v.path(), "c.md");

        let mut t = FileTree::default();
        t.refresh(v.path());
        t.selected = 2;

        // Drop two files and refresh.
        fs::remove_file(v.path().join("b.md")).unwrap();
        fs::remove_file(v.path().join("c.md")).unwrap();
        t.refresh(v.path());

        assert!(t.selected < t.entries.len());
    }

    #[test]
    fn select_clamps_to_bounds() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "a.md");
        touch(v.path(), "b.md");

        let mut t = FileTree::default();
        t.refresh(v.path());
        t.select_next();
        t.select_next();
        t.select_next(); // would be 3, clamps to 1
        assert_eq!(t.selected, 1);
        t.select_prev();
        assert_eq!(t.selected, 0);
        t.select_prev(); // already 0
        assert_eq!(t.selected, 0);
    }
}
