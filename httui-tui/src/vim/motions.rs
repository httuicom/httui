use ropey::Rope;

use crate::buffer::block::{
    body_line_col_to_raw_offset, closer_raw_offset, header_raw_offset, raw_section_at, BlockNode,
    RawSection,
};
use crate::buffer::layout::layout_document;
use crate::buffer::{Cursor, Document, Segment};
use crate::vim::parser::Motion;

/// Compute where a motion would land **without** keeping the change.
/// Internally calls [`apply`] against a snapshot — uses `&mut Document`
/// to reuse the existing engine, but restores the original cursor
/// before returning. Used by the operator engine to derive ranges.
pub fn target(motion: Motion, doc: &mut Document, count: usize, viewport_height: u16) -> Cursor {
    let saved = doc.cursor();
    apply(motion, doc, count, viewport_height);
    let result = doc.cursor();
    doc.set_cursor(saved);
    result
}

/// Apply a motion `count` times, mutating the document's cursor in place.
pub fn apply(motion: Motion, doc: &mut Document, count: usize, viewport_height: u16) {
    let count = count.max(1);
    match motion {
        Motion::HalfPageDown => half_page(doc, (viewport_height as i32 / 2) * count as i32),
        Motion::HalfPageUp => half_page(doc, -(viewport_height as i32 / 2) * count as i32),
        _ => {
            for _ in 0..count {
                let next = compute_next(motion, doc);
                if next == doc.cursor() {
                    break;
                }
                doc.set_cursor(next);
                if is_absolute(motion) {
                    break;
                }
            }
        }
    }
}

fn is_absolute(motion: Motion) -> bool {
    matches!(
        motion,
        Motion::LineStart
            | Motion::FirstNonBlank
            | Motion::LineEnd
            | Motion::DocStart
            | Motion::DocEnd
            | Motion::GotoLine(_)
    )
}

fn compute_next(motion: Motion, doc: &Document) -> Cursor {
    match motion {
        Motion::Left => apply_left(doc),
        Motion::Right => apply_right(doc),
        Motion::Down => apply_down(doc),
        Motion::Up => apply_up(doc),
        Motion::LineStart => apply_line_start(doc),
        Motion::FirstNonBlank => apply_first_non_blank(doc),
        Motion::LineEnd => apply_line_end(doc),
        Motion::WordForward => apply_word_forward(doc),
        Motion::WordBackward => apply_word_backward(doc),
        Motion::WordEnd => apply_word_end(doc),
        Motion::DocStart => apply_doc_start(doc),
        Motion::DocEnd => apply_doc_end(doc),
        Motion::GotoLine(n) => apply_goto_line(doc, n),
        Motion::FindForward(c) => apply_find(doc, c, true, false),
        Motion::FindBackward(c) => apply_find(doc, c, false, false),
        Motion::TillForward(c) => apply_find(doc, c, true, true),
        Motion::TillBackward(c) => apply_find(doc, c, false, true),
        // half-page handled by `apply` directly
        Motion::HalfPageDown | Motion::HalfPageUp => doc.cursor(),
    }
}

fn half_page(doc: &mut Document, delta: i32) {
    let count = delta.unsigned_abs() as usize;
    for _ in 0..count {
        let next = if delta > 0 {
            apply_down(doc)
        } else {
            apply_up(doc)
        };
        if next == doc.cursor() {
            break;
        }
        doc.set_cursor(next);
    }
}

// ───── horizontal ─────

fn apply_left(doc: &Document) -> Cursor {
    if let Cursor::InBlock {
        segment_idx,
        offset,
    } = doc.cursor()
    {
        let block = match block_at(doc, segment_idx) {
            Some(b) => b,
            None => return doc.cursor(),
        };
        // Phase 5: `h` walks the raw rope as if it were prose —
        // header / body / closer all participate. The only stop is
        // line column 0 (don't fold into the line above).
        let line_start = raw_line_start_offset(&block.raw, offset);
        if offset > line_start {
            return Cursor::InBlock {
                segment_idx,
                offset: offset - 1,
            };
        }
        return doc.cursor();
    }
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return doc.cursor();
    };
    let rope = match doc.segments().get(segment_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return doc.cursor(),
    };
    if offset == 0 {
        return doc.cursor();
    }
    let line_start = line_start_of_offset(rope, offset);
    if offset > line_start {
        Cursor::InProse {
            segment_idx,
            offset: offset - 1,
        }
    } else {
        doc.cursor()
    }
}

fn apply_right(doc: &Document) -> Cursor {
    if let Cursor::InBlock {
        segment_idx,
        offset,
    } = doc.cursor()
    {
        let block = match block_at(doc, segment_idx) {
            Some(b) => b,
            None => return doc.cursor(),
        };
        // Phase 5: `l` walks the raw rope. Stop one short of the
        // trailing newline (vim's `l` doesn't park on `\n`); empty
        // lines pin the cursor at col 0.
        let line_end = raw_line_end_offset(&block.raw, offset);
        if offset < line_end {
            return Cursor::InBlock {
                segment_idx,
                offset: offset + 1,
            };
        }
        return doc.cursor();
    }
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return doc.cursor();
    };
    let rope = match doc.segments().get(segment_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return doc.cursor(),
    };
    let next = offset + 1;
    if next > rope.len_chars() {
        return doc.cursor();
    }
    if rope.get_char(offset).is_some_and(|c| c == '\n') {
        return doc.cursor();
    }
    Cursor::InProse {
        segment_idx,
        offset: next,
    }
}

fn apply_line_start(doc: &Document) -> Cursor {
    if let Cursor::InBlock {
        segment_idx,
        offset,
    } = doc.cursor()
    {
        let block = match block_at(doc, segment_idx) {
            Some(b) => b,
            None => return doc.cursor(),
        };
        return Cursor::InBlock {
            segment_idx,
            offset: raw_line_start_offset(&block.raw, offset),
        };
    }
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return doc.cursor();
    };
    let rope = match doc.segments().get(segment_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return doc.cursor(),
    };
    Cursor::InProse {
        segment_idx,
        offset: line_start_of_offset(rope, offset),
    }
}

fn apply_first_non_blank(doc: &Document) -> Cursor {
    if let Cursor::InBlock {
        segment_idx,
        offset,
    } = doc.cursor()
    {
        let block = match block_at(doc, segment_idx) {
            Some(b) => b,
            None => return doc.cursor(),
        };
        let start = raw_line_start_offset(&block.raw, offset);
        let end = raw_line_end_offset(&block.raw, offset);
        let mut i = start;
        while i < end && block.raw.char(i).is_whitespace() {
            i += 1;
        }
        return Cursor::InBlock {
            segment_idx,
            offset: i,
        };
    }
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return doc.cursor();
    };
    let rope = match doc.segments().get(segment_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return doc.cursor(),
    };
    let start = line_start_of_offset(rope, offset);
    let total = rope.len_chars();
    let mut i = start;
    while i < total {
        let c = rope.char(i);
        if c == '\n' || !c.is_whitespace() {
            break;
        }
        i += 1;
    }
    Cursor::InProse {
        segment_idx,
        offset: i,
    }
}

fn apply_line_end(doc: &Document) -> Cursor {
    if let Cursor::InBlock {
        segment_idx,
        offset,
    } = doc.cursor()
    {
        let block = match block_at(doc, segment_idx) {
            Some(b) => b,
            None => return doc.cursor(),
        };
        return Cursor::InBlock {
            segment_idx,
            offset: raw_line_end_offset(&block.raw, offset),
        };
    }
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return doc.cursor();
    };
    let rope = match doc.segments().get(segment_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return doc.cursor(),
    };
    let total = rope.len_chars();
    let mut i = offset;
    while i < total && rope.char(i) != '\n' {
        i += 1;
    }
    // Stand on the last non-newline char (vim `$` semantics).
    if i > offset && i < total && rope.char(i) == '\n' && i > 0 {
        // i is on '\n'; back up one if there's content before.
    }
    Cursor::InProse {
        segment_idx,
        offset: i,
    }
}

// ───── vertical (cross-segment) ─────

fn apply_down(doc: &Document) -> Cursor {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            if let Some(Segment::Prose(rope)) = doc.segments().get(segment_idx) {
                let line = rope.char_to_line(offset.min(rope.len_chars()));
                if line + 1 < rope.len_lines() {
                    return Cursor::InProse {
                        segment_idx,
                        offset: rope.line_to_char(line + 1),
                    };
                }
            }
            jump_to_segment(doc, segment_idx + 1, true).unwrap_or(doc.cursor())
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let block = match block_at(doc, segment_idx) {
                Some(b) => b,
                None => return doc.cursor(),
            };
            match raw_section_at(&block.raw, offset) {
                RawSection::Header => {
                    // From the ` ```<info> ` row, j drops into the
                    // body's first line — the renderer kept that row
                    // at the same visual position whether we're in
                    // raw or render mode.
                    body_cursor(segment_idx, block, 0, 0)
                }
                RawSection::Body { line, .. } => {
                    // Canonical order: body → closer → result → exit.
                    // Mirrors the renderer (closer fences the editable
                    // region; result panel lives below).
                    let lines = block_query_line_count(doc, segment_idx);
                    if line + 1 < lines {
                        return body_cursor(segment_idx, block, line + 1, 0);
                    }
                    closer_cursor(segment_idx, block)
                }
                RawSection::Closer => {
                    // Closer → result row 0 when present, else exit.
                    if block_result_row_count(doc, segment_idx) > 0 {
                        return Cursor::InBlockResult {
                            segment_idx,
                            row: 0,
                        };
                    }
                    jump_to_segment(doc, segment_idx + 1, true).unwrap_or(doc.cursor())
                }
            }
        }
        Cursor::InBlockResult { segment_idx, row } => {
            let total = block_result_row_count(doc, segment_idx);
            if row + 1 < total {
                return Cursor::InBlockResult {
                    segment_idx,
                    row: row + 1,
                };
            }
            // Result panel is the last cursor inside the block —
            // closer was already visited on the way in.
            jump_to_segment(doc, segment_idx + 1, true).unwrap_or(doc.cursor())
        }
    }
}

fn apply_up(doc: &Document) -> Cursor {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            if let Some(Segment::Prose(rope)) = doc.segments().get(segment_idx) {
                let line = rope.char_to_line(offset.min(rope.len_chars()));
                if line > 0 {
                    return Cursor::InProse {
                        segment_idx,
                        offset: rope.line_to_char(line - 1),
                    };
                }
            }
            if segment_idx == 0 {
                return doc.cursor();
            }
            jump_to_segment(doc, segment_idx - 1, false).unwrap_or(doc.cursor())
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let block = match block_at(doc, segment_idx) {
                Some(b) => b,
                None => return doc.cursor(),
            };
            match raw_section_at(&block.raw, offset) {
                RawSection::Closer => {
                    // Reverse of the down path. Closer sits between
                    // body and result panel, so k from the closer
                    // always lands on the body's last line —
                    // independent of whether a result exists.
                    let last_line = block_query_line_count(doc, segment_idx).saturating_sub(1);
                    body_cursor(segment_idx, block, last_line, 0)
                }
                RawSection::Body { line, .. } => {
                    if line > 0 {
                        return body_cursor(segment_idx, block, line - 1, 0);
                    }
                    // Top of the body → land on the fence header
                    // instead of jumping out of the block. Another
                    // `k` takes us out.
                    header_cursor(segment_idx)
                }
                RawSection::Header => {
                    // Above the header — leave the block for the
                    // previous segment.
                    if segment_idx == 0 {
                        return doc.cursor();
                    }
                    jump_to_segment(doc, segment_idx - 1, false).unwrap_or(doc.cursor())
                }
            }
        }
        Cursor::InBlockResult { segment_idx, row } => {
            if row > 0 {
                return Cursor::InBlockResult {
                    segment_idx,
                    row: row - 1,
                };
            }
            // First row of the result panel → closer (it sits
            // between body and result; cursor matches the renderer).
            let block = match block_at(doc, segment_idx) {
                Some(b) => b,
                None => return doc.cursor(),
            };
            closer_cursor(segment_idx, block)
        }
    }
}

fn apply_doc_start(doc: &Document) -> Cursor {
    if let Some(seg) = doc.segments().first() {
        match seg {
            Segment::Prose(_) => Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            Segment::Block(b) => body_cursor(0, b, 0, 0),
        }
    } else {
        doc.cursor()
    }
}

fn apply_doc_end(doc: &Document) -> Cursor {
    let last = doc.segment_count().saturating_sub(1);
    let seg = match doc.segments().get(last) {
        Some(s) => s,
        None => return doc.cursor(),
    };
    match seg {
        Segment::Prose(rope) => {
            let lines = rope.len_lines();
            let off = if lines == 0 {
                0
            } else {
                rope.line_to_char(lines - 1)
            };
            Cursor::InProse {
                segment_idx: last,
                offset: off,
            }
        }
        Segment::Block(b) => body_cursor(last, b, 0, 0),
    }
}

fn apply_goto_line(doc: &Document, n: usize) -> Cursor {
    let layouts = layout_document(doc, 80);
    let mut accum = 0usize;
    for layout in &layouts {
        let height = layout.height as usize;
        if accum + height >= n {
            let seg = match doc.segments().get(layout.segment_idx) {
                Some(s) => s,
                None => return doc.cursor(),
            };
            return match seg {
                Segment::Prose(rope) => {
                    let line_in_seg = n.saturating_sub(accum + 1);
                    let off = if line_in_seg < rope.len_lines() {
                        rope.line_to_char(line_in_seg)
                    } else {
                        0
                    };
                    Cursor::InProse {
                        segment_idx: layout.segment_idx,
                        offset: off,
                    }
                }
                Segment::Block(b) => body_cursor(layout.segment_idx, b, 0, 0),
            };
        }
        accum += height;
    }
    apply_doc_end(doc)
}

// ───── word motions (current segment, naive vim semantics) ─────

fn apply_word_forward(doc: &Document) -> Cursor {
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return doc.cursor();
    };
    let rope = match doc.segments().get(segment_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return doc.cursor(),
    };
    let total = rope.len_chars();
    let mut i = offset.min(total);
    if i < total && !rope.char(i).is_whitespace() {
        if is_word_char(rope.char(i)) {
            while i < total && is_word_char(rope.char(i)) {
                i += 1;
            }
        } else {
            while i < total && !is_word_char(rope.char(i)) && !rope.char(i).is_whitespace() {
                i += 1;
            }
        }
    }
    while i < total && rope.char(i).is_whitespace() {
        i += 1;
    }
    Cursor::InProse {
        segment_idx,
        offset: i,
    }
}

fn apply_word_backward(doc: &Document) -> Cursor {
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return doc.cursor();
    };
    let rope = match doc.segments().get(segment_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return doc.cursor(),
    };
    if offset == 0 {
        return doc.cursor();
    }
    let mut i = offset - 1;
    while i > 0 && rope.char(i).is_whitespace() {
        i -= 1;
    }
    if is_word_char(rope.char(i)) {
        while i > 0 && is_word_char(rope.char(i - 1)) {
            i -= 1;
        }
    } else {
        while i > 0 && !is_word_char(rope.char(i - 1)) && !rope.char(i - 1).is_whitespace() {
            i -= 1;
        }
    }
    Cursor::InProse {
        segment_idx,
        offset: i,
    }
}

fn apply_word_end(doc: &Document) -> Cursor {
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return doc.cursor();
    };
    let rope = match doc.segments().get(segment_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return doc.cursor(),
    };
    let total = rope.len_chars();
    if offset + 1 >= total {
        return doc.cursor();
    }
    let mut i = offset + 1;
    while i < total && rope.char(i).is_whitespace() {
        i += 1;
    }
    if i >= total {
        return Cursor::InProse {
            segment_idx,
            offset: total.saturating_sub(1),
        };
    }
    let in_word = is_word_char(rope.char(i));
    while i < total
        && (if in_word {
            is_word_char(rope.char(i))
        } else {
            !is_word_char(rope.char(i)) && !rope.char(i).is_whitespace()
        })
    {
        i += 1;
    }
    Cursor::InProse {
        segment_idx,
        offset: i.saturating_sub(1),
    }
}

// ───── find / till ─────

/// Scan for `target` on the current line. `forward` chooses direction.
/// `till == true` makes it `t<c>`/`T<c>` (cursor lands one before/after
/// the match). When the target isn't on the line, the cursor doesn't
/// move — vim's "no match" behavior.
fn apply_find(doc: &Document, target: char, forward: bool, till: bool) -> Cursor {
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return doc.cursor();
    };
    let rope = match doc.segments().get(segment_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return doc.cursor(),
    };
    let total = rope.len_chars();
    let line_start = line_start_of_offset(rope, offset);
    let line_end = {
        let mut i = line_start;
        while i < total && rope.char(i) != '\n' {
            i += 1;
        }
        i
    };

    if forward {
        // Search strictly after the cursor.
        let mut i = offset.saturating_add(1);
        while i < line_end {
            if rope.char(i) == target {
                let landing = if till { i.saturating_sub(1) } else { i };
                if landing < offset {
                    return doc.cursor();
                }
                return Cursor::InProse {
                    segment_idx,
                    offset: landing,
                };
            }
            i += 1;
        }
    } else {
        // Search strictly before the cursor.
        if offset == 0 || offset <= line_start {
            return doc.cursor();
        }
        let mut i = offset - 1;
        loop {
            if rope.char(i) == target {
                let landing = if till {
                    let next = i + 1;
                    if next > offset {
                        return doc.cursor();
                    }
                    next
                } else {
                    i
                };
                return Cursor::InProse {
                    segment_idx,
                    offset: landing,
                };
            }
            if i <= line_start {
                break;
            }
            i -= 1;
        }
    }
    doc.cursor()
}

// ───── helpers ─────

fn line_start_of_offset(rope: &Rope, offset: usize) -> usize {
    let off = offset.min(rope.len_chars());
    let line = rope.char_to_line(off);
    rope.line_to_char(line)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Number of lines in a block's editable body (the SQL of `db-*`
/// blocks for now). Returns 1 for non-DB or empty bodies so motions
/// always have at least one valid line to land on.
fn block_query_line_count(doc: &Document, segment_idx: usize) -> usize {
    let Some(b) = block_at(doc, segment_idx) else {
        return 1;
    };
    crate::buffer::block::body_line_count(&b.raw).max(1)
}

/// Number of rows in a DB block's result table. Returns 0 for
/// non-DB blocks, blocks that haven't run, mutations, or errors.
/// Returns the full count — `j`/`k` walk every row and the renderer
/// scrolls its 10-row viewport to keep the selected one visible.
fn block_result_row_count(doc: &Document, segment_idx: usize) -> usize {
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

fn jump_to_segment(doc: &Document, idx: usize, going_down: bool) -> Option<Cursor> {
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
/// prose (Phase 5 unification).
fn raw_line_start_offset(rope: &Rope, offset: usize) -> usize {
    let off = offset.min(rope.len_chars());
    let line = rope.char_to_line(off);
    rope.line_to_char(line)
}

/// Rightmost cursor position on the line containing `offset` —
/// the offset of the last non-newline char. For an empty line this
/// pins at the line start; for the final line (no trailing `\n`) it
/// returns one before `len_chars`. Used by vim's `l`, `$`, and the
/// raw-rope edge clamps.
fn raw_line_end_offset(rope: &Rope, offset: usize) -> usize {
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
fn block_at(doc: &Document, segment_idx: usize) -> Option<&BlockNode> {
    match doc.segments().get(segment_idx)? {
        Segment::Block(b) => Some(b),
        _ => None,
    }
}

/// Build a `Cursor::InBlock` whose offset corresponds to the body
/// `(line, col)` position on the block's raw rope. Used everywhere the
/// previous `Cursor::InBlock { line, offset }` constructor lived.
fn body_cursor(segment_idx: usize, block: &BlockNode, body_line: usize, body_col: usize) -> Cursor {
    Cursor::InBlock {
        segment_idx,
        offset: body_line_col_to_raw_offset(&block.raw, body_line, body_col),
    }
}

/// Build a `Cursor::InBlock` parked on the fence header row (offset 0
/// of the raw rope).
fn header_cursor(segment_idx: usize) -> Cursor {
    Cursor::InBlock {
        segment_idx,
        offset: header_raw_offset(),
    }
}

/// Build a `Cursor::InBlock` parked on the ` \`\`\` ` closer row
/// (start of the closer line in the raw rope).
fn closer_cursor(segment_idx: usize, block: &BlockNode) -> Cursor {
    Cursor::InBlock {
        segment_idx,
        offset: closer_raw_offset(&block.raw),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;
    use serde_json::Value;

    fn doc(md: &str) -> Document {
        Document::from_markdown(md).unwrap()
    }

    // ───── canonical cursor sequence ─────
    //
    // Contract (block_navigation_canon.md): the cursor walks every
    // block of any kind in the SAME order — header → body → result
    // (if any) → closer. `j` follows that order top-to-bottom, `k`
    // is its exact reverse. HTTP and DB must produce identical
    // sequences for matching `(body_lines, result_rows)` shapes so
    // muscle memory carries between block types.
    //
    // The renderer is free to paint the closer wherever it makes
    // visual sense (HTTP sandwiches the response panel below the
    // closer; DB paints the closer at the bottom of the card) —
    // that's an orthogonal concern. The cursor flow is unified
    // here so motions stay predictable across block types.

    /// Label cursors by their semantic location so we can compare
    /// walks across block types without caring about offsets. Prose
    /// segments are bucketed into Above/Below relative to the block.
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Loc {
        ProseAbove,
        ProseBelow,
        Header,
        Body(usize),
        Closer,
        Result(usize),
    }

    fn classify(d: &Document, block_idx: usize) -> Loc {
        match d.cursor() {
            Cursor::InProse { segment_idx, .. } => {
                if segment_idx < block_idx {
                    Loc::ProseAbove
                } else {
                    Loc::ProseBelow
                }
            }
            Cursor::InBlock {
                segment_idx,
                offset,
            } => {
                assert_eq!(segment_idx, block_idx, "cursor escaped the target block");
                let Segment::Block(b) = &d.segments()[segment_idx] else {
                    panic!("expected block at {segment_idx}")
                };
                match raw_section_at(&b.raw, offset) {
                    RawSection::Header => Loc::Header,
                    RawSection::Body { line, .. } => Loc::Body(line),
                    RawSection::Closer => Loc::Closer,
                }
            }
            Cursor::InBlockResult { segment_idx, row } => {
                assert_eq!(segment_idx, block_idx, "cursor escaped the target block");
                Loc::Result(row)
            }
        }
    }

    /// Apply `motion` from the current cursor until it crosses the
    /// block boundary on the far side (or stops moving), returning
    /// the labeled stops. We don't walk through the surrounding
    /// prose past the first line that lands there — the contract
    /// under test is the block traversal, not prose line counts.
    fn walk(d: &mut Document, block_idx: usize, motion: Motion) -> Vec<Loc> {
        let mut path = vec![classify(d, block_idx)];
        let mut entered = false;
        for _ in 0..32 {
            let before = d.cursor();
            apply(motion, d, 1, 10);
            if d.cursor() == before {
                break;
            }
            let loc = classify(d, block_idx);
            let in_block = !matches!(loc, Loc::ProseAbove | Loc::ProseBelow);
            if in_block {
                entered = true;
            }
            let leaving = entered && !in_block;
            path.push(loc);
            if leaving {
                break;
            }
        }
        path
    }

    fn block_pos(d: &Document) -> usize {
        d.segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("fixture must contain a block")
    }

    fn attach_http_response(d: &mut Document, block_idx: usize) {
        let mut block = match d.segments()[block_idx].clone() {
            Segment::Block(b) => b,
            _ => unreachable!(),
        };
        block.cached_result = Some(serde_json::json!({
            "status": 200,
            "status_text": "OK",
            "headers": [],
            "body": serde_json::json!({"ok": true}),
            "size_bytes": 13,
            "timing": {"total_ms": 50, "ttfb_ms": 30},
        }));
        d.replace_segment(block_idx, Segment::Block(block));
    }

    fn attach_db_rows(d: &mut Document, block_idx: usize, row_count: usize) {
        let mut block = match d.segments()[block_idx].clone() {
            Segment::Block(b) => b,
            _ => unreachable!(),
        };
        let rows: Vec<Value> = (0..row_count)
            .map(|i| serde_json::json!([format!("r{i}")]))
            .collect();
        block.cached_result = Some(serde_json::json!({
            "results": [{
                "kind": "select",
                "columns": ["col"],
                "rows": rows,
            }],
        }));
        d.replace_segment(block_idx, Segment::Block(block));
    }

    /// Park the cursor on the last character of the prose segment
    /// directly above the block, so the next `j` enters the block.
    fn park_above(d: &mut Document, block_idx: usize) {
        let prose_idx = block_idx
            .checked_sub(1)
            .expect("fixture needs prose above the block");
        let rope = match &d.segments()[prose_idx] {
            Segment::Prose(r) => r.clone(),
            _ => panic!("expected prose above block"),
        };
        let last_line = rope.len_lines().saturating_sub(1);
        d.set_cursor(Cursor::InProse {
            segment_idx: prose_idx,
            offset: rope.line_to_char(last_line),
        });
    }

    /// Park the cursor on the first character of the prose segment
    /// directly below the block, so the next `k` enters the block.
    fn park_below(d: &mut Document, block_idx: usize) {
        let prose_idx = block_idx + 1;
        assert!(
            matches!(d.segments().get(prose_idx), Some(Segment::Prose(_))),
            "fixture needs prose below the block"
        );
        d.set_cursor(Cursor::InProse {
            segment_idx: prose_idx,
            offset: 0,
        });
    }

    fn http_doc(body: &str) -> Document {
        let md = format!("head\n\n```http alias=req1\n{body}\n```\n\ntail\n");
        doc(&md)
    }

    fn db_doc(body: &str) -> Document {
        let md = format!("head\n\n```db-postgres alias=q\n{body}\n```\n\ntail\n");
        doc(&md)
    }

    fn expect_canon(body_lines: usize, result_rows: usize) -> (Vec<Loc>, Vec<Loc>) {
        let mut down = vec![Loc::ProseAbove, Loc::Header];
        for i in 0..body_lines {
            down.push(Loc::Body(i));
        }
        down.push(Loc::Closer);
        for i in 0..result_rows {
            down.push(Loc::Result(i));
        }
        down.push(Loc::ProseBelow);
        let mut up = down.clone();
        up.reverse();
        (down, up)
    }

    #[test]
    fn canonical_down_http_idle() {
        let mut d = http_doc("GET https://example.com/users");
        let idx = block_pos(&d);
        park_above(&mut d, idx);
        let (expected, _) = expect_canon(1, 0);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_up_http_idle() {
        let mut d = http_doc("GET https://example.com/users");
        let idx = block_pos(&d);
        park_below(&mut d, idx);
        let (_, expected) = expect_canon(1, 0);
        assert_eq!(walk(&mut d, idx, Motion::Up), expected);
    }

    #[test]
    fn canonical_down_http_with_result() {
        let mut d = http_doc("GET https://example.com/users");
        let idx = block_pos(&d);
        attach_http_response(&mut d, idx);
        park_above(&mut d, idx);
        // HTTP result panel is a single landing row (see
        // block_result_row_count) — the inner viewport is internally
        // scrollable, j/k only park the cursor on it.
        let (expected, _) = expect_canon(1, 1);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_up_http_with_result() {
        let mut d = http_doc("GET https://example.com/users");
        let idx = block_pos(&d);
        attach_http_response(&mut d, idx);
        park_below(&mut d, idx);
        let (_, expected) = expect_canon(1, 1);
        assert_eq!(walk(&mut d, idx, Motion::Up), expected);
    }

    #[test]
    fn canonical_down_db_idle() {
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        park_above(&mut d, idx);
        let (expected, _) = expect_canon(1, 0);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_up_db_idle() {
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        park_below(&mut d, idx);
        let (_, expected) = expect_canon(1, 0);
        assert_eq!(walk(&mut d, idx, Motion::Up), expected);
    }

    #[test]
    fn canonical_down_db_with_result() {
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        attach_db_rows(&mut d, idx, 3);
        park_above(&mut d, idx);
        let (expected, _) = expect_canon(1, 3);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_up_db_with_result() {
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        attach_db_rows(&mut d, idx, 3);
        park_below(&mut d, idx);
        let (_, expected) = expect_canon(1, 3);
        assert_eq!(walk(&mut d, idx, Motion::Up), expected);
    }

    #[test]
    fn canonical_down_multi_body_db_with_result() {
        // Multi-line body + multi-row result — the densest realistic
        // shape. Confirms body and result enumerate fully before the
        // closer is touched.
        let mut d = db_doc("SELECT id, name\nFROM users\nWHERE id = 1");
        let idx = block_pos(&d);
        attach_db_rows(&mut d, idx, 2);
        park_above(&mut d, idx);
        let (expected, _) = expect_canon(3, 2);
        assert_eq!(walk(&mut d, idx, Motion::Down), expected);
    }

    #[test]
    fn canonical_http_and_db_idle_have_same_shape() {
        // Critère 1 du bug report: same body shape ⇒ identical
        // sequence regardless of block type.
        let mut h = http_doc("GET https://example.com/users");
        let mut q = db_doc("SELECT 1");
        let hi = block_pos(&h);
        let qi = block_pos(&q);
        park_above(&mut h, hi);
        park_above(&mut q, qi);
        assert_eq!(walk(&mut h, hi, Motion::Down), walk(&mut q, qi, Motion::Down));
    }

    #[test]
    fn canonical_up_is_reverse_of_down() {
        // Critère 2: apply_up must be exactly the reverse of
        // apply_down across the block boundary. Test the worst
        // case — DB with result — to exercise every section.
        let mut d = db_doc("SELECT 1");
        let idx = block_pos(&d);
        attach_db_rows(&mut d, idx, 3);
        park_above(&mut d, idx);
        let down = walk(&mut d, idx, Motion::Down);
        park_below(&mut d, idx);
        let mut up = walk(&mut d, idx, Motion::Up);
        up.reverse();
        assert_eq!(down, up);
    }

    #[test]
    fn left_stops_at_line_start() {
        let mut d = doc("hello\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        apply(Motion::Left, &mut d, 1, 10);
        assert_eq!(
            d.cursor(),
            Cursor::InProse {
                segment_idx: 0,
                offset: 0
            }
        );
    }

    #[test]
    fn right_advances_inside_line() {
        let mut d = doc("ab\n");
        apply(Motion::Right, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 1),
            _ => panic!(),
        }
    }

    #[test]
    fn line_end_lands_before_newline() {
        let mut d = doc("hello world\n");
        apply(Motion::LineEnd, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => {
                assert_eq!(offset, "hello world".len());
            }
            _ => panic!(),
        }
    }

    #[test]
    fn line_start_resets_offset() {
        let mut d = doc("hello\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 4,
        });
        apply(Motion::LineStart, &mut d, 1, 10);
        assert_eq!(
            d.cursor(),
            Cursor::InProse {
                segment_idx: 0,
                offset: 0
            }
        );
    }

    #[test]
    fn first_non_blank_skips_indent() {
        let mut d = doc("   indented\n");
        apply(Motion::FirstNonBlank, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 3),
            _ => panic!(),
        }
    }

    #[test]
    fn down_advances_line() {
        let mut d = doc("a\nb\nc\n");
        apply(Motion::Down, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert!(offset > 0),
            _ => panic!(),
        }
    }

    #[test]
    fn count_amplifies_down() {
        let mut d = doc("a\nb\nc\nd\ne\n");
        apply(Motion::Down, &mut d, 3, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => {
                let line = d.segments()[0].as_prose().unwrap().char_to_line(offset);
                assert_eq!(line, 3);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn doc_start_and_end() {
        let mut d = doc("a\nb\nc\n");
        apply(Motion::DocEnd, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert!(offset > 0),
            _ => panic!(),
        }
        apply(Motion::DocStart, &mut d, 1, 10);
        assert_eq!(
            d.cursor(),
            Cursor::InProse {
                segment_idx: 0,
                offset: 0
            }
        );
    }

    #[test]
    fn word_forward_skips_to_next_word() {
        let mut d = doc("hello world foo\n");
        apply(Motion::WordForward, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 6),
            _ => panic!(),
        }
    }

    #[test]
    fn word_backward_returns_to_previous() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 6,
        });
        apply(Motion::WordBackward, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 0),
            _ => panic!(),
        }
    }

    #[test]
    fn word_end_lands_on_last_char() {
        let mut d = doc("hello world\n");
        apply(Motion::WordEnd, &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 4),
            _ => panic!(),
        }
    }

    #[test]
    fn half_page_down_walks_lines() {
        let md = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let mut d = doc(md);
        apply(Motion::HalfPageDown, &mut d, 1, 8);
        match d.cursor() {
            Cursor::InProse { offset, .. } => {
                let line = d.segments()[0].as_prose().unwrap().char_to_line(offset);
                assert_eq!(line, 4); // half of 8
            }
            _ => panic!(),
        }
    }

    // ─── find / till ───

    #[test]
    fn f_lands_on_target_char() {
        let mut d = doc("hello world\n");
        apply(Motion::FindForward('o'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 4),
            _ => panic!(),
        }
    }

    #[test]
    fn f_with_count_finds_nth() {
        let mut d = doc("a-b-c-d\n");
        apply(Motion::FindForward('-'), &mut d, 2, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 3),
            _ => panic!(),
        }
    }

    #[test]
    fn t_lands_one_before_target() {
        let mut d = doc("hello world\n");
        apply(Motion::TillForward('o'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 3),
            _ => panic!(),
        }
    }

    #[test]
    fn capital_f_searches_backward() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 8,
        });
        apply(Motion::FindBackward('o'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 7),
            _ => panic!(),
        }
    }

    #[test]
    fn capital_t_lands_one_after_backward_target() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 8,
        });
        apply(Motion::TillBackward('o'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 8),
            _ => panic!(),
        }
    }

    #[test]
    fn find_does_not_cross_newline() {
        let mut d = doc("abc\nxyz\n");
        apply(Motion::FindForward('x'), &mut d, 1, 10);
        // 'x' is on line 2; forward find from line 1 must not match.
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 0),
            _ => panic!(),
        }
    }

    #[test]
    fn find_no_match_keeps_cursor() {
        let mut d = doc("hello\n");
        apply(Motion::FindForward('z'), &mut d, 1, 10);
        match d.cursor() {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 0),
            _ => panic!(),
        }
    }

    fn cursor_section(d: &Document) -> Option<RawSection> {
        let Cursor::InBlock {
            segment_idx,
            offset,
        } = d.cursor()
        else {
            return None;
        };
        let Segment::Block(b) = d.segments().get(segment_idx)? else {
            return None;
        };
        Some(raw_section_at(&b.raw, offset))
    }

    #[test]
    fn h_l_walk_the_fence_header_after_phase_5() {
        // Phase 5 lifts the V1 restriction that horizontal motions
        // were no-ops on fence rows. The cursor on the header now
        // walks character-by-character via h / l just like prose,
        // letting the user edit `alias=foo` etc. without leaving
        // the block.
        let mut d = doc("```db-postgres alias=q\nSELECT 1\n```\n");
        // Park cursor at offset 0 (fence header).
        let block_idx = d
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        d.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: 0,
        });
        assert!(is_header(&d));
        // Five `l`s: cursor advances to offset 5 (mid-header), still header.
        for _ in 0..5 {
            apply(Motion::Right, &mut d, 1, 10);
        }
        assert!(is_header(&d));
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, 5);
        }
        // `0` resets to start of the header.
        apply(Motion::LineStart, &mut d, 1, 10);
        assert!(is_header(&d));
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, 0);
        }
        // `$` lands on last char of the header (one before the
        // newline). Header is "```db-postgres alias=q" (22 chars),
        // so EOL is offset 21.
        apply(Motion::LineEnd, &mut d, 1, 10);
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, 21);
        }
    }

    #[test]
    fn h_at_column_zero_does_not_cross_lines_in_block() {
        // Vim's `h` never folds into the previous line — same
        // contract for InBlock now that motions walk the raw rope.
        let mut d = doc("```db-postgres alias=q\nSELECT 1\n```\n");
        let block_idx = d
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        // Park cursor at body line 0 col 0 — first char of "SELECT".
        let raw = match d.segments().get(block_idx) {
            Some(Segment::Block(b)) => b.raw.clone(),
            _ => panic!(),
        };
        let body_start = crate::buffer::block::body_line_to_raw_offset(&raw, 0);
        d.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: body_start,
        });
        let before = d.cursor();
        apply(Motion::Left, &mut d, 1, 10);
        // Cursor stays put — refused to cross into the header line.
        assert_eq!(d.cursor(), before);
    }

    fn is_header(d: &Document) -> bool {
        matches!(cursor_section(d), Some(RawSection::Header))
    }
}
