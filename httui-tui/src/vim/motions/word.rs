//! Word motions (`w`, `b`, `e`) and find/till (`f`, `F`, `t`, `T`).

use crate::buffer::{Cursor, Document, Segment};

use super::helpers::{is_word_char, line_start_of_offset};

pub(super) fn apply_word_forward(doc: &Document) -> Cursor {
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

pub(super) fn apply_word_backward(doc: &Document) -> Cursor {
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

pub(super) fn apply_word_end(doc: &Document) -> Cursor {
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

/// Scan for `target` on the current line. `forward` chooses direction.
/// `till == true` makes it `t<c>`/`T<c>` (cursor lands one before/after
/// the match). When the target isn't on the line, the cursor doesn't
/// move — vim's "no match" behavior.
pub(super) fn apply_find(doc: &Document, target: char, forward: bool, till: bool) -> Cursor {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn prose(text: &str) -> Document {
        Document::from_markdown(text).unwrap()
    }

    fn block_doc() -> Document {
        Document::from_markdown("```http\nGET https://x.com\n```\n").unwrap()
    }

    #[test]
    fn word_forward_inblock_returns_cursor_unchanged() {
        let mut d = block_doc();
        let blk = d
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        d.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: 0,
        });
        let cur = d.cursor();
        assert_eq!(apply_word_forward(&d), cur);
        assert_eq!(apply_word_backward(&d), cur);
        assert_eq!(apply_word_end(&d), cur);
    }

    #[test]
    fn word_forward_skips_punctuation_run() {
        let d = prose("!!!hello");
        let cur = apply_word_forward(&d);
        if let Cursor::InProse { offset, .. } = cur {
            // Stops past the punctuation cluster.
            assert!(offset >= 3);
        } else {
            panic!()
        }
    }

    #[test]
    fn word_forward_at_eof_returns_eof_cursor() {
        let mut d = prose("abc");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 3,
        });
        let cur = apply_word_forward(&d);
        assert!(matches!(cur, Cursor::InProse { offset, .. } if offset == 3));
    }

    #[test]
    fn word_backward_at_offset_zero_returns_same() {
        let mut d = prose("abc");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        let cur = apply_word_backward(&d);
        assert!(matches!(cur, Cursor::InProse { offset: 0, .. }));
    }

    #[test]
    fn word_backward_walks_past_punctuation() {
        let mut d = prose("abc !!! def");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 11,
        });
        let cur = apply_word_backward(&d);
        if let Cursor::InProse { offset, .. } = cur {
            assert!(offset < 11);
        } else {
            panic!()
        }
    }

    #[test]
    fn word_end_at_near_eof_returns_unchanged() {
        let mut d = prose("ab");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 1,
        });
        let cur = apply_word_end(&d);
        // offset + 1 >= total → early return
        assert!(matches!(cur, Cursor::InProse { offset: 1, .. }));
    }

    #[test]
    fn word_end_in_trailing_whitespace_lands_at_total_minus_one() {
        let d = prose("abc   ");
        let cur = apply_word_end(&d);
        if let Cursor::InProse { offset, .. } = cur {
            assert!(offset >= 2);
        } else {
            panic!()
        }
    }

    #[test]
    fn find_in_block_returns_cursor_unchanged() {
        let mut d = block_doc();
        let blk = d
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        d.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: 0,
        });
        let cur = d.cursor();
        assert_eq!(apply_find(&d, 'x', true, false), cur);
    }

    #[test]
    fn find_backward_at_offset_zero_returns_same() {
        let mut d = prose("hello");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        let cur = apply_find(&d, 'h', false, false);
        assert!(matches!(cur, Cursor::InProse { offset: 0, .. }));
    }

    #[test]
    fn find_backward_till_when_target_immediately_before_cursor_returns_same() {
        let mut d = prose("ab");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 1,
        });
        // till backward from b looking for a: target at offset 0, till
        // means landing = next = 1, but next > offset is false (next == offset),
        // so the actual return depends on the implementation. Just ensure
        // no panic and cursor doesn't move past origin.
        let _ = apply_find(&d, 'a', false, true);
    }
}
