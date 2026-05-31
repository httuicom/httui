//! Pure read layer for the BLOCKS-view pane renderer — no `Frame`,
//! no widgets.

use std::path::Path;

use crate::app::{BlockMeta, FileBlocks, RegionEdit};
use crate::pane::Pane;

pub(super) struct ParsedView {
    pub method: Option<String>,
    pub url: Option<String>,
    /// `(key, value, enabled)`. A disabled header (`# ` in the fence) carries
    /// `enabled == false`; the renderer shows it unchecked + struck through.
    pub headers: Vec<(String, String, bool)>,
    pub body: String,
    pub connection: Option<String>,
    pub cached: String,
    pub cached_json: Option<serde_json::Value>,
    pub raw: String,
}

impl ParsedView {
    pub(super) fn empty() -> Self {
        Self {
            method: None,
            url: None,
            headers: Vec::new(),
            body: String::new(),
            connection: None,
            cached: String::new(),
            cached_json: None,
            raw: String::new(),
        }
    }
}

pub(super) fn load_view(
    vault_path: &Path,
    file: &FileBlocks,
    block: &BlockMeta,
    pane: &Pane,
) -> ParsedView {
    // Run output never sits on disk (SQLite cache + in-memory
    // `BlockNode.cached_result`), so the pane document is the only
    // path the renderer can surface it from.
    let cached_from_pane = pane.document.as_ref().and_then(|doc| {
        for seg in doc.segments() {
            if let crate::buffer::Segment::Block(b) = seg {
                if b.block_type == block.block_type && b.alias == block.alias {
                    return b
                        .cached_result
                        .as_ref()
                        .map(|v| (serialize_cached_result(v), v.clone()));
                }
            }
        }
        None
    });
    // Draft wins over disk so committed-but-unsaved edits show.
    if let Some(draft) = pane.block_draft.as_ref() {
        if draft.block_line_start == block.line_start && draft.block.block_type == block.block_type
        {
            let raw = httui_core::blocks::serialize_block(&draft.block);
            let mut view = parsed_to_view(&draft.block, raw);
            if let Some((c, json)) = cached_from_pane {
                view.cached = c;
                view.cached_json = Some(json);
            }
            return view;
        }
    }
    let Ok(raw) =
        httui_core::fs::read_note(&vault_path.to_string_lossy(), &file.path.to_string_lossy())
    else {
        return ParsedView::empty();
    };
    let parsed = httui_core::blocks::parse_blocks(&raw);
    let Some(p) = parsed
        .iter()
        .find(|p| p.line_start == block.line_start && p.block_type == block.block_type)
    else {
        return ParsedView::empty();
    };
    let lines: Vec<&str> = raw.lines().collect();
    let end = p.line_end.min(lines.len().saturating_sub(1));
    let start = p.line_start.min(end);
    let raw_block = lines[start..=end].join("\n");
    let mut view = parsed_to_view(p, raw_block);
    if let Some((c, json)) = cached_from_pane {
        view.cached = c;
        view.cached_json = Some(json);
    }
    view
}

fn serialize_cached_result(v: &serde_json::Value) -> String {
    if let Some(obj) = v.as_object() {
        if let Some(status) = obj.get("status").and_then(|s| s.as_i64()) {
            let mut summary = format!("{status}");
            if let Some(text) = obj.get("status_text").and_then(|s| s.as_str()) {
                if !text.is_empty() {
                    summary.push(' ');
                    summary.push_str(text);
                }
            }
            if let Some(elapsed) = obj
                .get("elapsed_ms")
                .or_else(|| obj.get("total_ms"))
                .and_then(|v| v.as_u64())
            {
                summary.push_str(" · ");
                summary.push_str(&format!("{elapsed}ms"));
            }
            if let Some(size) = obj.get("size_bytes").and_then(|v| v.as_u64()) {
                summary.push_str(" · ");
                summary.push_str(&format_bytes(size));
            }
            let mut out = summary;
            if let Some(body) = obj.get("body") {
                let body_str = match body {
                    serde_json::Value::String(s) => s.clone(),
                    _ => serde_json::to_string_pretty(body).unwrap_or_else(|_| body.to_string()),
                };
                out.push('\n');
                out.push_str(&body_str);
            }
            return out;
        }
        if let Some(results) = obj.get("results").and_then(|r| r.as_array()) {
            if let Some(first) = results.first() {
                return serde_json::to_string_pretty(first).unwrap_or_else(|_| first.to_string());
            }
        }
    }
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

pub(super) fn format_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n}b")
    } else if n < 1024 * 1024 {
        format!("{:.1}kb", n as f64 / 1024.0)
    } else {
        format!("{:.1}mb", n as f64 / (1024.0 * 1024.0))
    }
}

fn parsed_to_view(p: &httui_core::blocks::parser::ParsedBlock, raw: String) -> ParsedView {
    let method = p
        .params
        .get("method")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let url = p
        .params
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let headers = p
        .params
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|h| {
                    let k = h
                        .get("key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let v = h
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let enabled = h.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                    (k, v, enabled)
                })
                .collect()
        })
        .unwrap_or_default();
    let body = p
        .params
        .get("body")
        .and_then(|v| v.as_str())
        .or_else(|| p.params.get("query").and_then(|v| v.as_str()))
        .unwrap_or("")
        .trim_end_matches('\n')
        .to_string();
    let connection = p
        .params
        .get("connection")
        .or_else(|| p.params.get("connection_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    ParsedView {
        method,
        url,
        headers,
        body,
        connection,
        cached: String::new(),
        cached_json: None,
        raw,
    }
}

// Display path + line_start so same-alias blocks in different files
// don't share viewport state.
pub(super) fn block_node_id(file: &FileBlocks, block: &BlockMeta) -> crate::buffer::block::BlockId {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    file.display.hash(&mut h);
    block.line_start.hash(&mut h);
    crate::buffer::block::BlockId(h.finish())
}

// `None` when the pane isn't on this file — caller falls back to disk.
pub(super) fn block_node_from_pane(
    pane: &Pane,
    file: &FileBlocks,
    block: &BlockMeta,
) -> Option<crate::buffer::block::BlockNode> {
    let pane_path = pane.document_path.as_ref()?;
    let pane_rel = if pane_path.is_absolute() {
        pane_path.strip_prefix(pane_path.ancestors().last()?).ok()?
    } else {
        pane_path.as_path()
    };
    let target_rel = file.path.as_path();
    let matches = pane_path.ends_with(target_rel)
        || pane_rel.ends_with(target_rel)
        || pane_path == target_rel;
    if !matches {
        return None;
    }
    let doc = pane.document.as_ref()?;
    for seg in doc.segments() {
        if let crate::buffer::Segment::Block(b) = seg {
            if b.block_type == block.block_type && b.alias == block.alias {
                return Some(b.clone());
            }
        }
    }
    None
}

// Disk read carries no `cached_result` — results only live in the
// in-memory pane Document.
pub(super) fn load_block_node(
    vault: &Path,
    file: &FileBlocks,
    block: &BlockMeta,
) -> Option<crate::buffer::block::BlockNode> {
    use crate::buffer::Segment;
    let doc = crate::document_loader::load_document(vault, &file.path).ok()?;
    let mut found: Option<crate::buffer::block::BlockNode> = None;
    let mut next_line = 0usize;
    for seg in doc.segments() {
        match seg {
            Segment::Prose(rope) => {
                next_line += rope.len_lines().max(1);
            }
            Segment::Block(b) => {
                if next_line == block.line_start && b.block_type == block.block_type {
                    found = Some(b.clone());
                    break;
                }
                next_line += b.raw.lines().count().max(1);
            }
        }
    }
    found
}

pub(super) fn edit_cursor_row_col(edit: &RegionEdit) -> (usize, usize) {
    let offset = match edit.doc.cursor() {
        crate::buffer::Cursor::InProse { offset, .. } => offset,
        crate::buffer::Cursor::InBlock { offset, .. } => offset,
        crate::buffer::Cursor::InBlockResult { .. } => 0,
    };
    let text = edit.current_text();
    let mut row = 0usize;
    let mut col = 0usize;
    for ch in text.chars().take(offset) {
        if ch == '\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (row, col)
}
