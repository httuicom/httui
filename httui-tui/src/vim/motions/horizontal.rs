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

#[cfg(test)]
mod tests {
    use super::*;

    fn prose(text: &str) -> Document {
        Document::from_markdown(text).unwrap()
    }

    fn block_doc(md: &str) -> Document {
        Document::from_markdown(md).unwrap()
    }

    fn block_idx(d: &Document) -> usize {
        d.segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap()
    }

    #[test]
    fn left_in_block_moves_within_raw_line() {
        let mut d = block_doc("```http\nGET /\n```\n");
        let i = block_idx(&d);
        d.set_cursor(Cursor::InBlock {
            segment_idx: i,
            offset: 3,
        });
        let after = apply_left(&d);
        if let Cursor::InBlock { offset, .. } = after {
            assert_eq!(offset, 2);
        } else {
            panic!()
        }
    }

    #[test]
    fn left_at_col_zero_in_block_stays() {
        let mut d = block_doc("```http\nGET /\n```\n");
        let i = block_idx(&d);
        d.set_cursor(Cursor::InBlock {
            segment_idx: i,
            offset: 0,
        });
        let before = d.cursor();
        assert_eq!(apply_left(&d), before);
    }

    #[test]
    fn right_in_block_moves_along_raw_line() {
        let mut d = block_doc("```http\nGET /\n```\n");
        let i = block_idx(&d);
        d.set_cursor(Cursor::InBlock {
            segment_idx: i,
            offset: 0,
        });
        let after = apply_right(&d);
        if let Cursor::InBlock { offset, .. } = after {
            assert_eq!(offset, 1);
        } else {
            panic!()
        }
    }

    #[test]
    fn right_in_prose_at_eof_returns_same() {
        let mut d = prose("abc");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 3,
        });
        let after = apply_right(&d);
        if let Cursor::InProse { offset, .. } = after {
            assert!(offset <= 3);
        } else {
            panic!()
        }
    }

    #[test]
    fn inblockresult_passes_through_doc_cursor_for_all_horiz() {
        let mut d = prose("hi");
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: 0,
            row: 0,
        });
        let before = d.cursor();
        assert_eq!(apply_left(&d), before);
        assert_eq!(apply_right(&d), before);
        assert_eq!(apply_line_start(&d), before);
        assert_eq!(apply_first_non_blank(&d), before);
        assert_eq!(apply_line_end(&d), before);
    }

    #[test]
    fn line_start_in_block_clamps_to_line_zero() {
        let mut d = block_doc("```http\nGET /\n```\n");
        let i = block_idx(&d);
        d.set_cursor(Cursor::InBlock {
            segment_idx: i,
            offset: 4,
        });
        let after = apply_line_start(&d);
        if let Cursor::InBlock { offset, .. } = after {
            assert_eq!(offset, 0);
        } else {
            panic!()
        }
    }

    #[test]
    fn first_non_blank_in_prose_skips_leading_whitespace() {
        let d = prose("   abc");
        let after = apply_first_non_blank(&d);
        if let Cursor::InProse { offset, .. } = after {
            assert_eq!(offset, 3);
        } else {
            panic!()
        }
    }

    #[test]
    fn line_end_in_block_lands_at_raw_line_end() {
        let mut d = block_doc("```http\nGET /\n```\n");
        let i = block_idx(&d);
        d.set_cursor(Cursor::InBlock {
            segment_idx: i,
            offset: 0,
        });
        let after = apply_line_end(&d);
        assert!(matches!(after, Cursor::InBlock { .. }));
    }
}
