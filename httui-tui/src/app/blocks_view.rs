use std::collections::HashSet;
use std::path::{Path, PathBuf};

use httui_core::blocks::parser::ParsedBlock;
use httui_core::fs::FileEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppView {
    #[default]
    Doc,
    Blocks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMeta {
    pub alias: Option<String>,
    pub block_type: String,
    pub line_start: usize,
}

impl BlockMeta {
    pub fn label(&self) -> String {
        self.alias
            .clone()
            .unwrap_or_else(|| format!("L{}", self.line_start + 1))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBlocks {
    pub path: PathBuf,
    pub display: String,
    pub blocks: Vec<BlockMeta>,
}

#[derive(Debug, Clone, Default)]
pub struct BlockIndex {
    pub files: Vec<FileBlocks>,
}

impl BlockIndex {
    pub fn total_blocks(&self) -> usize {
        self.files.iter().map(|f| f.blocks.len()).sum()
    }

    pub fn build(vault: &Path) -> Self {
        let entries =
            httui_core::fs::list_workspace(&vault.to_string_lossy()).unwrap_or_default();
        let mut files: Vec<FileBlocks> = Vec::new();
        collect_files(vault, &entries, &mut files);
        files.retain(|f| !f.blocks.is_empty());
        files.sort_by_key(|f| f.display.to_lowercase());
        Self { files }
    }
}

fn collect_files(vault: &Path, entries: &[FileEntry], out: &mut Vec<FileBlocks>) {
    for entry in entries {
        if entry.is_dir {
            if let Some(children) = entry.children.as_deref() {
                collect_files(vault, children, out);
            }
            continue;
        }
        if !entry.name.ends_with(".md") {
            continue;
        }
        let rel = PathBuf::from(&entry.path);
        let Ok(text) = httui_core::fs::read_note(&vault.to_string_lossy(), &entry.path) else {
            continue;
        };
        let parsed = httui_core::blocks::parse_blocks(&text);
        let blocks: Vec<BlockMeta> = parsed
            .iter()
            .map(|p: &ParsedBlock| BlockMeta {
                alias: p.alias.clone(),
                block_type: p.block_type.clone(),
                line_start: p.line_start,
            })
            .collect();
        out.push(FileBlocks {
            path: rel.clone(),
            display: rel.to_string_lossy().into_owned(),
            blocks,
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockRef {
    pub file_idx: usize,
    pub block_idx: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarRowKind {
    File,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarRow {
    pub file_idx: usize,
    pub block_idx: Option<usize>,
    pub kind: SidebarRowKind,
}

#[derive(Debug, Clone)]
pub struct BlocksWorkspace {
    pub index: BlockIndex,
    pub expanded: HashSet<usize>,
    pub cursor: usize,
    pub selected: Option<BlockRef>,
    pub region: usize,
    /// Some(block) = sidebar picked a block while multiple panes were
    /// open — next digit chooses which pane to open it in. Esc cancels.
    pub pane_picker: Option<BlockRef>,
}

impl BlocksWorkspace {
    pub fn new(index: BlockIndex) -> Self {
        let expanded = if index.files.len() == 1 {
            let mut set = HashSet::new();
            set.insert(0);
            set
        } else {
            HashSet::new()
        };
        Self {
            index,
            expanded,
            cursor: 0,
            selected: None,
            region: 0,
            pane_picker: None,
        }
    }

    pub fn region_count(&self) -> usize {
        self.selected_block()
            .map(|(_, b)| region_count_for(&b.block_type))
            .unwrap_or(0)
    }

    pub fn next_region(&mut self) {
        let count = self.region_count();
        if count == 0 {
            return;
        }
        self.region = (self.region + 1) % count;
    }

    pub fn prev_region(&mut self) {
        let count = self.region_count();
        if count == 0 {
            return;
        }
        self.region = (self.region + count - 1) % count;
    }

    pub fn set_region(&mut self, index: usize) {
        let count = self.region_count();
        if count == 0 {
            return;
        }
        self.region = index.min(count - 1);
    }

    pub fn select(&mut self, target: BlockRef) {
        self.selected = Some(target);
        self.region = 0;
    }

    pub fn rows(&self) -> Vec<SidebarRow> {
        let mut rows = Vec::new();
        for (fi, f) in self.index.files.iter().enumerate() {
            rows.push(SidebarRow {
                file_idx: fi,
                block_idx: None,
                kind: SidebarRowKind::File,
            });
            if self.expanded.contains(&fi) {
                for (bi, _) in f.blocks.iter().enumerate() {
                    rows.push(SidebarRow {
                        file_idx: fi,
                        block_idx: Some(bi),
                        kind: SidebarRowKind::Block,
                    });
                }
            }
        }
        rows
    }

    pub fn row_count(&self) -> usize {
        self.rows().len()
    }

    pub fn current_row(&self) -> Option<SidebarRow> {
        self.rows().into_iter().nth(self.cursor)
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let count = self.row_count();
        if count == 0 {
            self.cursor = 0;
            return;
        }
        let last = (count - 1) as isize;
        let next = (self.cursor as isize + delta).clamp(0, last);
        self.cursor = next as usize;
    }

    pub fn activate(&mut self) {
        let Some(row) = self.current_row() else {
            return;
        };
        match row.kind {
            SidebarRowKind::File => {
                if self.expanded.contains(&row.file_idx) {
                    self.expanded.remove(&row.file_idx);
                } else {
                    self.expanded.insert(row.file_idx);
                }
            }
            SidebarRowKind::Block => {
                if let Some(bi) = row.block_idx {
                    self.select(BlockRef {
                        file_idx: row.file_idx,
                        block_idx: bi,
                    });
                }
            }
        }
    }

    pub fn selected_block(&self) -> Option<(&FileBlocks, &BlockMeta)> {
        let r = self.selected?;
        let file = self.index.files.get(r.file_idx)?;
        let block = file.blocks.get(r.block_idx)?;
        Some((file, block))
    }
}

pub fn region_count_for(block_type: &str) -> usize {
    if block_type == "http" {
        4
    } else if block_type.starts_with("db") {
        3
    } else {
        0
    }
}

pub fn region_label(block_type: &str, index: usize) -> &'static str {
    if block_type == "http" {
        match index {
            0 => "Request",
            1 => "Headers",
            2 => "Body",
            3 => "Response",
            _ => "?",
        }
    } else if block_type.starts_with("db") {
        match index {
            0 => "Connection",
            1 => "Query",
            2 => "Result",
            _ => "?",
        }
    } else {
        "?"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, body).unwrap();
    }

    fn seed() -> TempDir {
        let v = TempDir::new().unwrap();
        write(
            v.path(),
            "api.md",
            "# api\n\n```http alias=login\nGET https://x.com\n```\n",
        );
        write(
            v.path(),
            "users.md",
            "# users\n\n```http alias=list\nGET https://x.com/users\n```\n\n```db-sqlite alias=audit\nSELECT 1\n```\n",
        );
        write(v.path(), "empty.md", "# just prose\n");
        v
    }

    #[test]
    fn build_index_includes_files_with_blocks_only() {
        let v = seed();
        let idx = BlockIndex::build(v.path());
        let names: Vec<&str> = idx.files.iter().map(|f| f.display.as_str()).collect();
        assert!(names.contains(&"api.md"));
        assert!(names.contains(&"users.md"));
        assert!(!names.contains(&"empty.md"));
    }

    #[test]
    fn build_index_counts_blocks_per_file() {
        let v = seed();
        let idx = BlockIndex::build(v.path());
        let api = idx.files.iter().find(|f| f.display == "api.md").unwrap();
        assert_eq!(api.blocks.len(), 1);
        let users = idx.files.iter().find(|f| f.display == "users.md").unwrap();
        assert_eq!(users.blocks.len(), 2);
        assert_eq!(idx.total_blocks(), 3);
    }

    #[test]
    fn block_meta_label_uses_alias_or_line() {
        let with_alias = BlockMeta {
            alias: Some("login".into()),
            block_type: "http".into(),
            line_start: 5,
        };
        assert_eq!(with_alias.label(), "login");
        let no_alias = BlockMeta {
            alias: None,
            block_type: "http".into(),
            line_start: 5,
        };
        assert_eq!(no_alias.label(), "L6");
    }

    #[test]
    fn workspace_default_expands_single_file() {
        let idx = BlockIndex {
            files: vec![FileBlocks {
                path: PathBuf::from("api.md"),
                display: "api.md".into(),
                blocks: vec![BlockMeta {
                    alias: Some("q".into()),
                    block_type: "http".into(),
                    line_start: 1,
                }],
            }],
        };
        let ws = BlocksWorkspace::new(idx);
        assert!(ws.expanded.contains(&0));
    }

    #[test]
    fn workspace_default_collapses_multi_file() {
        let v = seed();
        let ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        assert!(!ws.expanded.contains(&0));
        assert!(!ws.expanded.contains(&1));
    }

    #[test]
    fn rows_lists_only_files_when_collapsed() {
        let v = seed();
        let ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        let rows = ws.rows();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| matches!(r.kind, SidebarRowKind::File)));
    }

    #[test]
    fn rows_inserts_blocks_under_expanded_file() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        ws.expanded.insert(0);
        let rows = ws.rows();
        assert!(rows.len() > 2);
        assert!(matches!(rows[0].kind, SidebarRowKind::File));
        assert!(matches!(rows[1].kind, SidebarRowKind::Block));
    }

    #[test]
    fn move_cursor_clamps_within_visible_rows() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        ws.move_cursor(99);
        assert_eq!(ws.cursor, ws.row_count() - 1);
        ws.move_cursor(-99);
        assert_eq!(ws.cursor, 0);
    }

    #[test]
    fn activate_on_file_toggles_expand() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        ws.cursor = 0;
        ws.activate();
        assert!(ws.expanded.contains(&0));
        ws.activate();
        assert!(!ws.expanded.contains(&0));
    }

    #[test]
    fn activate_on_block_sets_selected() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        ws.expanded.insert(0);
        ws.cursor = 1;
        ws.activate();
        assert_eq!(
            ws.selected,
            Some(BlockRef {
                file_idx: 0,
                block_idx: 0
            })
        );
    }

    #[test]
    fn selected_block_returns_meta_pair() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        ws.expanded.insert(0);
        ws.cursor = 1;
        ws.activate();
        let (_file, block) = ws.selected_block().unwrap();
        assert!(block.alias.is_some());
    }

    #[test]
    fn current_row_returns_none_for_empty_index() {
        let ws = BlocksWorkspace::new(BlockIndex::default());
        assert!(ws.current_row().is_none());
    }

    #[test]
    fn region_count_per_kind() {
        assert_eq!(region_count_for("http"), 4);
        assert_eq!(region_count_for("db-postgres"), 3);
        assert_eq!(region_count_for("db"), 3);
        assert_eq!(region_count_for("unknown"), 0);
    }

    #[test]
    fn region_label_classifies_per_kind() {
        assert_eq!(region_label("http", 0), "Request");
        assert_eq!(region_label("http", 3), "Response");
        assert_eq!(region_label("db-postgres", 0), "Connection");
        assert_eq!(region_label("db", 2), "Result");
        assert_eq!(region_label("http", 99), "?");
        assert_eq!(region_label("custom", 0), "?");
    }

    #[test]
    fn next_region_cycles_within_block_kind() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        ws.expanded.insert(0);
        ws.cursor = 1;
        ws.activate();
        assert_eq!(ws.region, 0);
        ws.next_region();
        assert_eq!(ws.region, 1);
        ws.next_region();
        ws.next_region();
        assert_eq!(ws.region, 3);
        ws.next_region();
        assert_eq!(ws.region, 0);
    }

    #[test]
    fn prev_region_wraps_to_last() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        ws.expanded.insert(0);
        ws.cursor = 1;
        ws.activate();
        ws.prev_region();
        assert_eq!(ws.region, 3);
    }

    #[test]
    fn set_region_clamps_to_last() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        ws.expanded.insert(0);
        ws.cursor = 1;
        ws.activate();
        ws.set_region(99);
        assert_eq!(ws.region, 3);
    }

    #[test]
    fn select_resets_region_to_zero() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        ws.expanded.insert(0);
        ws.cursor = 1;
        ws.activate();
        ws.set_region(2);
        ws.select(BlockRef {
            file_idx: 0,
            block_idx: 0,
        });
        assert_eq!(ws.region, 0);
    }

    #[test]
    fn region_methods_noop_when_no_selection() {
        let v = seed();
        let mut ws = BlocksWorkspace::new(BlockIndex::build(v.path()));
        assert_eq!(ws.region_count(), 0);
        ws.next_region();
        assert_eq!(ws.region, 0);
        ws.set_region(5);
        assert_eq!(ws.region, 0);
    }
}
