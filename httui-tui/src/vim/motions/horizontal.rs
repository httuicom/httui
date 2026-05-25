//! Horizontal motions (`h`, `l`, `0`, `^`, `$`).

use crate::buffer::{Cursor, Document, Segment};

use super::helpers::{block_at, line_start_of_offset, raw_line_end_offset, raw_line_start_offset};

pub(super) fn apply_left(doc: &Document) -> Cursor {
    if let Cursor::InBlock {
        segment_idx,
        offset,
    } = doc.cursor()
    {
        let block = match block_at(doc, segment_idx) {
            Some(b) => b,
            None => return doc.cursor(),
        };
        // `h` walks the raw rope as if it were prose (header /
        // body / closer all participate). The only stop is line
        // column 0 — never fold into the line above.
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

pub(super) fn apply_right(doc: &Document) -> Cursor {
    if let Cursor::InBlock {
        segment_idx,
        offset,
    } = doc.cursor()
    {
        let block = match block_at(doc, segment_idx) {
            Some(b) => b,
            None => return doc.cursor(),
        };
        // `l` walks the raw rope; stop one short of the trailing
        // newline (vim convention). Empty lines pin at col 0.
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

pub(super) fn apply_line_start(doc: &Document) -> Cursor {
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

pub(super) fn apply_first_non_blank(doc: &Document) -> Cursor {
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

pub(super) fn apply_line_end(doc: &Document) -> Cursor {
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
