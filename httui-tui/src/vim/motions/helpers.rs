//! Shared low-level helpers for the motion modules — rope offset
//! math, block navigation cursor builders, result-row counting.

use ropey::Rope;

use crate::buffer::block::{
    body_line_col_to_raw_offset, closer_raw_offset, header_raw_offset, BlockNode,
};
use crate::buffer::{Cursor, Document, Segment};

pub(super) fn line_start_of_offset(rope: &Rope, offset: usize) -> usize {
    let off = offset.min(rope.len_chars());
    let line = rope.char_to_line(off);
    rope.line_to_char(line)
}

pub(super) fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Number of lines in a block's editable body (the SQL of `db-*`
/// blocks for now). Returns 1 for non-DB or empty bodies so motions
/// always have at least one valid line to land on.
pub(super) fn block_query_line_count(doc: &Document, segment_idx: usize) -> usize {
    let Some(b) = block_at(doc, segment_idx) else {
        return 1;
    };
    crate::buffer::block::body_line_count(&b.raw).max(1)
}

/// Number of rows in a DB block's result table. Returns 0 for
/// non-DB blocks, blocks that haven't run, mutations, or errors.
/// Returns the full count — `j`/`k` walk every row and the renderer
/// scrolls its 10-row viewport to keep the selected one visible.
pub(super) fn block_result_row_count(doc: &Document, segment_idx: usize) -> usize {
    let seg = match doc.segments().get(segment_idx) {
        Some(s) => s,
        None => return 0,
    };
    let block = match seg {
        Segment::Block(b) => b,
        _ => return 0,
    };
    let result = match block.cached_result.as_ref() {
        Some(r) => r,
        None => return 0,
    };
    // HTTP blocks: treat the response panel as a single landing
    // row so j/k can park the cursor on it (`<CR>` then opens the
    // detail viewer). Body line counting could be exposed later
    // for line-by-line scrolling, but parking-only suffices for
    // V1 — the panel viewport is internally scrollable already.
    if block.is_http() {
        return 1;
    }
    let first = match result
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
    {
        Some(f) => f,
        None => return 0,
    };
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return 0;
    }
    first
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

pub(super) fn jump_to_segment(doc: &Document, idx: usize, going_down: bool) -> Option<Cursor> {
    let seg = doc.segments().get(idx)?;
    Some(match seg {
        Segment::Block(b) => {
            // Enter the block at the section visually closest to the
            // direction of travel. Going down → fence header (top of
            // the card). Going up → result panel's last row when one
            // exists (the panel is painted below the closer), else
            // the closer fence.
            if going_down {
                header_cursor(idx)
            } else {
                let total = block_result_row_count(doc, idx);
                if total > 0 {
                    Cursor::InBlockResult {
                        segment_idx: idx,
                        row: total - 1,
                    }
                } else {
                    closer_cursor(idx, b)
                }
            }
        }
        Segment::Prose(rope) => {
            let offset = if going_down {
                0
            } else {
                let lines = rope.len_lines();
                if lines == 0 {
                    0
                } else {
                    rope.line_to_char(lines - 1)
                }
            };
            Cursor::InProse {
                segment_idx: idx,
                offset,
            }
        }
    })
}

/// Char offset at the start of the line containing `offset` in `rope`.
/// Used by InBlock motions so they walk the raw rope as if it were
/// prose.
pub(super) fn raw_line_start_offset(rope: &Rope, offset: usize) -> usize {
    let off = offset.min(rope.len_chars());
    let line = rope.char_to_line(off);
    rope.line_to_char(line)
}

/// Rightmost cursor position on the line containing `offset` —
/// the offset of the last non-newline char. For an empty line this
/// pins at the line start; for the final line (no trailing `\n`) it
/// returns one before `len_chars`. Used by vim's `l`, `$`, and the
/// raw-rope edge clamps.
pub(super) fn raw_line_end_offset(rope: &Rope, offset: usize) -> usize {
    let off = offset.min(rope.len_chars());
    let line = rope.char_to_line(off);
    let line_start = rope.line_to_char(line);
    let next_line_start = if line + 1 < rope.len_lines() {
        rope.line_to_char(line + 1)
    } else {
        rope.len_chars()
    };
    let has_trailing_newline = next_line_start > line_start
        && rope.get_char(next_line_start.saturating_sub(1)) == Some('\n');
    let content_end = if has_trailing_newline {
        next_line_start.saturating_sub(1)
    } else {
        next_line_start
    };
    if content_end > line_start {
        content_end.saturating_sub(1)
    } else {
        line_start
    }
}

/// Borrow the [`BlockNode`] living at `segment_idx`, if any. Wrapping
/// the segment lookup keeps the InBlock-flavoured motions readable —
/// they almost always need the raw rope to resolve a section.
pub(super) fn block_at(doc: &Document, segment_idx: usize) -> Option<&BlockNode> {
    match doc.segments().get(segment_idx)? {
        Segment::Block(b) => Some(b),
        _ => None,
    }
}

/// Build a `Cursor::InBlock` whose offset corresponds to the body
/// `(line, col)` position on the block's raw rope. Used everywhere the
/// previous `Cursor::InBlock { line, offset }` constructor lived.
pub(super) fn body_cursor(
    segment_idx: usize,
    block: &BlockNode,
    body_line: usize,
    body_col: usize,
) -> Cursor {
    Cursor::InBlock {
        segment_idx,
        offset: body_line_col_to_raw_offset(&block.raw, body_line, body_col),
    }
}

/// Build a `Cursor::InBlock` parked on the fence header row (offset 0
/// of the raw rope).
pub(super) fn header_cursor(segment_idx: usize) -> Cursor {
    Cursor::InBlock {
        segment_idx,
        offset: header_raw_offset(),
    }
}

/// Build a `Cursor::InBlock` parked on the ` \`\`\` ` closer row
/// (start of the closer line in the raw rope).
pub(super) fn closer_cursor(segment_idx: usize, block: &BlockNode) -> Cursor {
    Cursor::InBlock {
        segment_idx,
        offset: closer_raw_offset(&block.raw),
    }
}
