use ropey::Rope;

use crate::buffer::{Cursor, Document, Segment};
use crate::vim::parser::InsertPos;

/// Position the cursor for `EnterInsert(pos)`. The actual mode swap
/// is done by the caller.
pub fn position_for_insert(doc: &mut Document, pos: InsertPos) {
    match pos {
        InsertPos::Current => {}
        InsertPos::After => move_right_within_line(doc),
        InsertPos::LineStart => move_to_first_non_blank(doc),
        InsertPos::LineEnd => move_to_line_end(doc),
        InsertPos::LineAbove => open_line_above(doc),
        InsertPos::LineBelow => open_line_below(doc),
    }
}

/// `<Esc>` from insert: vim recoils the cursor one column unless it's
/// already at the line start.
pub fn recoil_after_exit(doc: &mut Document) {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let rope = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => r,
                _ => return,
            };
            if offset == 0 {
                return;
            }
            let line_start = line_start_of_offset(rope, offset);
            if offset > line_start {
                doc.set_cursor(Cursor::InProse {
                    segment_idx,
                    offset: offset - 1,
                });
            }
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let Some(raw) = block_raw(doc, segment_idx) else {
                return;
            };
            let line_start = line_start_of_offset(&raw, offset);
            if offset > line_start {
                doc.set_cursor(Cursor::InBlock {
                    segment_idx,
                    offset: offset - 1,
                });
            }
        }
        Cursor::InBlockResult { .. } => {}
    }
}

fn move_right_within_line(doc: &mut Document) {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let rope = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => r,
                _ => return,
            };
            if offset >= rope.len_chars() {
                return;
            }
            if rope.char(offset) == '\n' {
                return;
            }
            doc.set_cursor(Cursor::InProse {
                segment_idx,
                offset: offset + 1,
            });
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            // `a` lands one past the cursor on the EOL position so
            // typing extends the line. Walk the raw rope as if it
            // were prose: header / body / closer all participate.
            let Some(raw) = block_raw(doc, segment_idx) else {
                return;
            };
            let total = raw.len_chars();
            if offset >= total {
                return;
            }
            if raw.char(offset) == '\n' {
                return;
            }
            doc.set_cursor(Cursor::InBlock {
                segment_idx,
                offset: offset + 1,
            });
        }
        Cursor::InBlockResult { .. } => {}
    }
}

fn move_to_first_non_blank(doc: &mut Document) {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let rope = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => r,
                _ => return,
            };
            let i = scan_first_non_blank(rope, offset);
            doc.set_cursor(Cursor::InProse {
                segment_idx,
                offset: i,
            });
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let Some(raw) = block_raw(doc, segment_idx) else {
                return;
            };
            let i = scan_first_non_blank(&raw, offset);
            doc.set_cursor(Cursor::InBlock {
                segment_idx,
                offset: i,
            });
        }
        Cursor::InBlockResult { .. } => {}
    }
}

fn move_to_line_end(doc: &mut Document) {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let rope = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => r,
                _ => return,
            };
            let i = line_end_of_offset(rope, offset);
            doc.set_cursor(Cursor::InProse {
                segment_idx,
                offset: i,
            });
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let Some(raw) = block_raw(doc, segment_idx) else {
                return;
            };
            let i = line_end_of_offset(&raw, offset);
            doc.set_cursor(Cursor::InBlock {
                segment_idx,
                offset: i,
            });
        }
        Cursor::InBlockResult { .. } => {}
    }
}

/// Insert a fresh line above the current one and place the cursor on
/// the new (empty) line. Mirrors vim `O`.
fn open_line_above(doc: &mut Document) {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let line_start = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => line_start_of_offset(r, offset),
                _ => return,
            };
            doc.set_cursor(Cursor::InProse {
                segment_idx,
                offset: line_start,
            });
            doc.insert_char_at_cursor('\n');
            doc.set_cursor(Cursor::InProse {
                segment_idx,
                offset: line_start,
            });
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let Some(raw) = block_raw(doc, segment_idx) else {
                return;
            };
            let line_start = line_start_of_offset(&raw, offset);
            // Move to col 0 of current line, insert newline (cursor
            // advances by 1), then jump back up to the now-empty
            // new line at the same offset.
            doc.set_cursor(Cursor::InBlock {
                segment_idx,
                offset: line_start,
            });
            doc.insert_char_at_cursor('\n');
            doc.set_cursor(Cursor::InBlock {
                segment_idx,
                offset: line_start,
            });
        }
        Cursor::InBlockResult { .. } => {}
    }
}

/// Insert a fresh line below the current one and place the cursor on
/// it. Mirrors vim `o`.
fn open_line_below(doc: &mut Document) {
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let line_end_offset = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => line_end_of_offset(r, offset),
                _ => return,
            };
            doc.set_cursor(Cursor::InProse {
                segment_idx,
                offset: line_end_offset,
            });
            doc.insert_char_at_cursor('\n');
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let Some(raw) = block_raw(doc, segment_idx) else {
                return;
            };
            // Land on EOL of current line, then `\n` pushes us to
            // the start of a new line. Closer is special: it has no
            // trailing `\n`, so EOL walks to the rope end and `o`
            // would append a line OUTSIDE the fence. Re-anchor on
            // the body's trailing `\n` instead so `o` extends the
            // block (vim mental model: stay inside the construct).
            let insert_at = match crate::buffer::block::raw_section_at(&raw, offset) {
                crate::buffer::block::RawSection::Closer => {
                    let closer_line_start = line_start_of_offset(&raw, offset);
                    if closer_line_start == 0 {
                        return;
                    }
                    closer_line_start - 1
                }
                _ => line_end_of_offset(&raw, offset),
            };
            doc.set_cursor(Cursor::InBlock {
                segment_idx,
                offset: insert_at,
            });
            doc.insert_char_at_cursor('\n');
        }
        Cursor::InBlockResult { .. } => {}
    }
}

fn block_raw(doc: &Document, segment_idx: usize) -> Option<Rope> {
    let seg = doc.segments().get(segment_idx)?;
    let Segment::Block(b) = seg else { return None };
    Some(b.raw.clone())
}

fn line_start_of_offset(rope: &Rope, offset: usize) -> usize {
    let off = offset.min(rope.len_chars());
    let line = rope.char_to_line(off);
    rope.line_to_char(line)
}

fn line_end_of_offset(rope: &Rope, offset: usize) -> usize {
    let total = rope.len_chars();
    let mut i = offset.min(total);
    while i < total && rope.char(i) != '\n' {
        i += 1;
    }
    i
}

fn scan_first_non_blank(rope: &Rope, offset: usize) -> usize {
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
    i
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;

    fn block_raw(doc: &Document, idx: usize) -> String {
        match doc.segments().get(idx) {
            Some(Segment::Block(b)) => b.raw.to_string(),
            _ => String::new(),
        }
    }

    fn assert_open_below_lands_before_closer(
        md: &str,
        cursor_offset_from_marker: &str,
        cursor_marker_offset_within_match: usize,
    ) {
        let mut doc = Document::from_markdown(md).expect("parse");
        let block_idx = doc
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        let raw_before = block_raw(&doc, block_idx);
        let marker_pos = raw_before
            .find(cursor_offset_from_marker)
            .expect("marker present");
        let cursor_offset = marker_pos + cursor_marker_offset_within_match;
        doc.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: cursor_offset,
        });
        open_line_below(&mut doc);
        let after = block_raw(&doc, block_idx);
        let closer_pos = after.rfind("```").expect("closer present");
        let tail = &after[closer_pos + 3..];
        assert!(
            tail.trim().is_empty(),
            "open_line_below(cursor={cursor_offset}) pushed content AFTER closer.\n\
             before = {raw_before:?}\n after = {after:?}\n tail = {tail:?}"
        );
    }

    #[test]
    fn o_on_closer_inserts_above_closer_not_outside_block() {
        // Pressing `j` from the last body line lands the cursor on
        // the closer row. From the user's perspective they are
        // "above the ```" (visually right after the last body line),
        // so `o` must extend the block — never append a line below
        // the closer (escaping the fence).
        let md = "```http alias=req1\nGET /x\n{{\n```\n";
        let mut doc = Document::from_markdown(md).expect("parse");
        let block_idx = doc
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        let raw_before = block_raw(&doc, block_idx);
        let closer_offset = raw_before.rfind("```").expect("closer present");
        doc.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: closer_offset,
        });
        open_line_below(&mut doc);
        let after = block_raw(&doc, block_idx);
        let closer_pos = after.rfind("```").expect("closer present");
        let tail = &after[closer_pos + 3..];
        assert!(
            tail.trim().is_empty(),
            "o from closer must not push content past ```; tail={tail:?}, after={after:?}"
        );
        // A new blank line must have appeared between `{{` and ```.
        let between_start = after.find("{{").unwrap() + 2;
        let between = &after[between_start..closer_pos];
        assert!(
            between.contains("\n\n"),
            "expected blank body line between `{{` and closer; got {between:?}"
        );
    }

    #[test]
    fn o_on_each_position_of_last_body_line_lands_above_closer() {
        // The bug report: cursor "on a line above ```", press `o`,
        // new line appears BELOW ```. Walk every plausible cursor
        // offset on the last body line and assert the closer never
        // gets pushed.
        let md = "```http alias=req1\nGET /x\n{{\n```\n";
        // First `{` of last body line.
        assert_open_below_lands_before_closer(md, "{{", 0);
        // Between the two braces.
        assert_open_below_lands_before_closer(md, "{{", 1);
        // Past the second `{` (still on the body line, before its \n).
        assert_open_below_lands_before_closer(md, "{{", 2);
    }

    #[test]
    fn o_on_last_body_line_inserts_above_closer_not_below() {
        // Repro for the "new line lands BELOW ``` instead of between
        // body and ```" bug. Cursor is parked on the last body line
        // (offset somewhere inside `{{`); `o` must open a fresh body
        // line that sits BEFORE the closer fence, never after it.
        let md = "```http alias=req1\nGET /x\n{{\n```\n";
        let mut doc = Document::from_markdown(md).expect("parse");
        let block_idx = doc
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        let raw_before = block_raw(&doc, block_idx);
        // Find offset of the `{` in `{{` — last body line's first char.
        let body_line_start = raw_before.find("{{").expect("body marker present");
        doc.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: body_line_start,
        });
        open_line_below(&mut doc);
        let after = block_raw(&doc, block_idx);
        // The closer must remain the LAST non-empty token of the rope:
        // anything after the closer means we opened the wrong row.
        let closer_pos = after.rfind("```").expect("closer present");
        let tail = &after[closer_pos + 3..];
        assert!(
            tail.trim().is_empty(),
            "open_line_below pushed content AFTER the closer (tail={tail:?})\n\
             full raw after = {after:?}",
        );
        // And there must be at least one new blank line between `{{`
        // and the closer.
        let between_start = after.find("{{").unwrap() + 2;
        let between = &after[between_start..closer_pos];
        assert!(
            between.contains("\n\n"),
            "expected blank line between `{{` and closer; got {between:?}",
        );
    }
}
