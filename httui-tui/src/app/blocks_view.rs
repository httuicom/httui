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

/// Last recorded run of a block, distilled from `block_run_history`
/// into just what the sidebar badge needs. `status` overloads the
/// history column the same way the writer does: the HTTP status code
/// for HTTP blocks, the SELECT row-count / mutation rows-affected for
/// DB blocks (see `derive_db_history_stats`). `outcome` is the raw
/// history outcome (`"ok"`, `"error"`, `"cancelled"`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockLastRun {
    pub status: Option<i64>,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMeta {
    pub alias: Option<String>,
    pub block_type: String,
    pub line_start: usize,
    /// Populated by [`enrich_last_runs`] after the index is built —
    /// `None` until then, and for anonymous blocks (history is keyed
    /// by alias) or blocks that never ran.
    pub last_run: Option<BlockLastRun>,
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
                last_run: None,
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

/// Identifies which field inside a region's NAV grid is being edited.
/// Every variant points at a `String` slot in `ParsedBlock.params`
/// (or the alias / url). Commit reads the active sub-`Document` via
/// `to_markdown()` and writes the result into that slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditField {
    /// `[1] Request` line — the URL after the method token. The method
    /// is cycled rather than typed, so a single-line buffer over the
    /// URL alone covers the cenário-4 path.
    HttpUrl,
    /// `[2] Headers` row `i`, key column.
    HttpHeaderKey(usize),
    /// `[2] Headers` row `i`, value column.
    HttpHeaderValue(usize),
    /// `[3] Body` — HTTP request body (multi-line). Stored as the
    /// `body` string in `ParsedBlock.params`; serializer keeps line
    /// breaks verbatim.
    HttpBody,
    /// `[2] Query` — DB SQL body (multi-line). Stored as the `query`
    /// string in `ParsedBlock.params`.
    DbQuery,
}

/// Vim sub-mode within an active EDIT buffer. Standard profile is
/// always `Insert`; vim Enter lands in `Normal`, `i`/`a`/`o` skip
/// straight to `Insert`. `Esc` in INSERT (vim) flips back to NORMAL
/// without committing; `Esc` in NORMAL (vim) or `Esc` (standard)
/// commits the buffer + returns to NAV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditSubMode {
    Normal,
    Insert,
}

/// Edit state for the focused pane: which field is open + a sub-
/// [`Document`] carrying the in-progress text. The full vim engine
/// operates on `doc` via the [`App::document`]/`document_mut` redirect
/// — same trick `DbRowDetail` uses — so every motion / operator /
/// count / search / visual / undo lands here for free, no re-impl.
///
/// Lifecycle:
/// - `Enter` (NAV) → spawn with field's current value as initial doc
///   text. Standard sub-mode = `Insert`; vim sub-mode = `Normal`.
/// - `i`/`a`/`o` (NAV vim) → spawn directly in `Insert`.
/// - `Esc` (vim INSERT) → flip to `Normal` (no commit).
/// - `Esc` (vim NORMAL or standard INSERT) → commit
///   `doc.to_markdown()` into the draft, clear edit, return to NAV.
/// - `Ctrl+C` → drop the doc without writing.
pub struct RegionEdit {
    pub field: EditField,
    pub doc: crate::buffer::Document,
    pub sub_mode: EditSubMode,
}

impl std::fmt::Debug for RegionEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionEdit")
            .field("field", &self.field)
            .field("sub_mode", &self.sub_mode)
            .field("doc_len", &self.doc.to_markdown().len())
            .finish()
    }
}

impl RegionEdit {
    /// Standard-mode entry (`Enter` from NAV, standard) and vim
    /// `i`/`a`/`o`: spawn in `Insert`.
    pub fn insert(field: EditField, initial: impl Into<String>) -> Self {
        Self::with_sub_mode(field, initial, EditSubMode::Insert)
    }

    /// Vim `Enter` from NAV: spawn in `Normal` so the buffer is
    /// browseable without typing.
    pub fn normal(field: EditField, initial: impl Into<String>) -> Self {
        Self::with_sub_mode(field, initial, EditSubMode::Normal)
    }

    fn with_sub_mode(
        field: EditField,
        initial: impl Into<String>,
        sub_mode: EditSubMode,
    ) -> Self {
        let initial: String = initial.into();
        let mut doc = crate::buffer::Document::from_markdown(&initial)
            .unwrap_or_else(|_| crate::buffer::Document::from_markdown("").unwrap());
        // `Document::from_markdown` parks the cursor at offset 0 of
        // the first segment. INSERT lands at the end (continuation),
        // NORMAL keeps the head position (vim opens files at line 1
        // column 1) — same UX convention as `vim file.txt` vs.
        // typing into a form field.
        if matches!(sub_mode, EditSubMode::Insert) {
            let last_segment_idx = doc.segments().len().saturating_sub(1);
            let offset = initial
                .chars()
                .rev()
                .take_while(|c| *c != '\n')
                .count();
            let row_offset = initial.chars().count();
            // Prose-only sub-doc: single segment. Use the full char
            // count when there's no newline split; otherwise compute
            // end-of-last-line offset.
            let _ = offset;
            doc.set_cursor(crate::buffer::Cursor::InProse {
                segment_idx: last_segment_idx,
                offset: row_offset,
            });
        }
        Self {
            field,
            doc,
            sub_mode,
        }
    }

    /// Read the current buffer contents as a plain string. Strips
    /// the trailing newline that `Document::to_markdown` always
    /// appends so the saved value matches what the user typed.
    pub fn current_text(&self) -> String {
        let s = self.doc.to_markdown();
        s.strip_suffix('\n').map(str::to_string).unwrap_or(s)
    }
}

/// Pending edits for the block currently selected in a pane. Stored as
/// a mutable `ParsedBlock` so committing a field edit is a simple
/// in-place mutation and saving is `serialize_block` → replace by
/// `[line_start..=line_end]` → `write_note`.
///
/// `file_path` is vault-relative (matches `BlockMeta` / `FileBlocks`),
/// so it survives a workspace switch unchanged. `block_line_start`
/// identifies the block within the file (the pair `(line_start,
/// block_type)` is the same identity the renderer uses in
/// `load_parsed`).
#[derive(Debug, Clone)]
pub struct BlockDraft {
    pub file_path: PathBuf,
    pub block_line_start: usize,
    pub block: ParsedBlock,
}

impl BlockDraft {
    /// Apply a key/value patch to the `headers` array, lazily growing
    /// the array to fit `row`. Returns `false` if `col` isn't `0`/`1`
    /// (defensive — the applier should never pass anything else).
    pub fn set_header(&mut self, row: usize, col: usize, value: String) -> bool {
        if col > 1 {
            return false;
        }
        let params = self
            .block
            .params
            .as_object_mut()
            .expect("ParsedBlock.params is always an object after parse");
        let headers = params
            .entry("headers".to_string())
            .or_insert_with(|| serde_json::Value::Array(Vec::new()));
        let arr = headers
            .as_array_mut()
            .expect("headers field is always an array");
        while arr.len() <= row {
            arr.push(serde_json::json!({"key": "", "value": ""}));
        }
        let row_obj = arr[row]
            .as_object_mut()
            .expect("header rows are always objects");
        let field = if col == 0 { "key" } else { "value" };
        row_obj.insert(field.to_string(), serde_json::Value::String(value));
        true
    }

    /// Replace the URL on an HTTP block.
    pub fn set_url(&mut self, value: String) {
        let params = self
            .block
            .params
            .as_object_mut()
            .expect("ParsedBlock.params is always an object after parse");
        params.insert("url".to_string(), serde_json::Value::String(value));
    }

    /// Read the current URL string from the draft.
    pub fn url(&self) -> &str {
        self.block
            .params
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }

    /// Read the current header at `(row, col)`. Empty string when the
    /// row doesn't exist yet.
    pub fn header_at(&self, row: usize, col: usize) -> &str {
        let arr = self
            .block
            .params
            .get("headers")
            .and_then(|v| v.as_array());
        let Some(arr) = arr else { return "" };
        let Some(row_val) = arr.get(row) else {
            return "";
        };
        let field = if col == 0 { "key" } else { "value" };
        row_val.get(field).and_then(|v| v.as_str()).unwrap_or("")
    }

    /// Count of header rows in the draft (0 when the block has none).
    pub fn header_count(&self) -> usize {
        self.block
            .params
            .get("headers")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0)
    }

    /// Set the HTTP body string. The serializer reads `body` directly.
    pub fn set_body(&mut self, value: String) {
        let params = self
            .block
            .params
            .as_object_mut()
            .expect("ParsedBlock.params is always an object");
        params.insert("body".to_string(), serde_json::Value::String(value));
    }

    pub fn body(&self) -> &str {
        self.block
            .params
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("")
    }

    /// Set the DB query string. Stored under `query`; the serializer
    /// renders the body straight from this field.
    pub fn set_query(&mut self, value: String) {
        let params = self
            .block
            .params
            .as_object_mut()
            .expect("ParsedBlock.params is always an object");
        params.insert("query".to_string(), serde_json::Value::String(value));
    }

    pub fn query(&self) -> &str {
        self.block
            .params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
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
            last_run: None,
        };
        assert_eq!(with_alias.label(), "login");
        let no_alias = BlockMeta {
            alias: None,
            block_type: "http".into(),
            line_start: 5,
            last_run: None,
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
                    last_run: None,
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
