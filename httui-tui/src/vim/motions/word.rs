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
