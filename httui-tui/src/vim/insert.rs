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
            // Closer has no trailing `\n`, so anchoring on EOL would
            // append outside the fence. Stay inside the block by
            // re-anchoring on the body's trailing `\n`.
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
        let md = "```http alias=req1\nGET /x\n{{\n```\n";
        assert_open_below_lands_before_closer(md, "{{", 0);
        assert_open_below_lands_before_closer(md, "{{", 1);
        assert_open_below_lands_before_closer(md, "{{", 2);
    }

    #[test]
    fn o_on_last_body_line_inserts_above_closer_not_below() {
        let md = "```http alias=req1\nGET /x\n{{\n```\n";
        let mut doc = Document::from_markdown(md).expect("parse");
        let block_idx = doc
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        let raw_before = block_raw(&doc, block_idx);
        let body_line_start = raw_before.find("{{").expect("body marker present");
        doc.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: body_line_start,
        });
        open_line_below(&mut doc);
        let after = block_raw(&doc, block_idx);
        let closer_pos = after.rfind("```").expect("closer present");
        let tail = &after[closer_pos + 3..];
        assert!(
            tail.trim().is_empty(),
            "open_line_below pushed content AFTER the closer (tail={tail:?})\n\
             full raw after = {after:?}",
        );
        let between_start = after.find("{{").unwrap() + 2;
        let between = &after[between_start..closer_pos];
        assert!(
            between.contains("\n\n"),
            "expected blank line between `{{` and closer; got {between:?}",
        );
    }

    fn doc(md: &str) -> Document {
        Document::from_markdown(md).unwrap()
    }

    #[test]
    fn position_current_is_noop() {
        let mut d = doc("hello\n");
        let before = d.cursor();
        position_for_insert(&mut d, InsertPos::Current);
        assert_eq!(d.cursor(), before);
    }

    #[test]
    fn position_after_moves_cursor_right() {
        let mut d = doc("ab\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        position_for_insert(&mut d, InsertPos::After);
        if let Cursor::InProse { offset, .. } = d.cursor() {
            assert_eq!(offset, 1);
        } else {
            panic!()
        }
    }

    #[test]
    fn position_line_start_skips_indent() {
        let mut d = doc("   hello\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 5,
        });
        position_for_insert(&mut d, InsertPos::LineStart);
        if let Cursor::InProse { offset, .. } = d.cursor() {
            assert_eq!(offset, 3);
        } else {
            panic!()
        }
    }

    #[test]
    fn position_line_end_lands_at_eol() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        position_for_insert(&mut d, InsertPos::LineEnd);
        if let Cursor::InProse { offset, .. } = d.cursor() {
            assert!(offset >= "hello world".len());
        } else {
            panic!()
        }
    }

    #[test]
    fn recoil_in_prose_moves_back_one_unless_at_line_start() {
        let mut d = doc("ab\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 2,
        });
        recoil_after_exit(&mut d);
        if let Cursor::InProse { offset, .. } = d.cursor() {
            assert_eq!(offset, 1);
        } else {
            panic!()
        }
    }

    #[test]
    fn recoil_at_line_start_stays() {
        let mut d = doc("ab\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        recoil_after_exit(&mut d);
        if let Cursor::InProse { offset, .. } = d.cursor() {
            assert_eq!(offset, 0);
        } else {
            panic!()
        }
    }

    fn doc_with_block() -> (Document, usize) {
        let md = "```http alias=req1\nGET /x\n{{\n```\n";
        let d = Document::from_markdown(md).expect("parse");
        let idx = d
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        (d, idx)
    }

    fn body_marker_offset(d: &Document, idx: usize, needle: &str) -> usize {
        match d.segments().get(idx) {
            Some(Segment::Block(b)) => b.raw.to_string().find(needle).expect("needle"),
            _ => panic!("not a block"),
        }
    }

    #[test]
    fn position_line_above_in_prose_inserts_blank_line() {
        let mut d = doc("first\nsecond\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 7,
        });
        position_for_insert(&mut d, InsertPos::LineAbove);
        if let Cursor::InProse { offset, .. } = d.cursor() {
            assert_eq!(offset, 6);
        } else {
            panic!()
        }
        let text = match d.segments().first() {
            Some(Segment::Prose(r)) => r.to_string(),
            _ => panic!(),
        };
        assert!(text.starts_with("first\n\nsecond"), "got {text:?}");
    }

    #[test]
    fn position_line_above_in_block_inserts_blank_line() {
        let (mut d, idx) = doc_with_block();
        let body_offset = body_marker_offset(&d, idx, "{{");
        d.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: body_offset,
        });
        position_for_insert(&mut d, InsertPos::LineAbove);
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, body_offset);
        } else {
            panic!()
        }
    }

    #[test]
    fn position_line_below_in_prose_inserts_after_eol() {
        let mut d = doc("first\nsecond\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        position_for_insert(&mut d, InsertPos::LineBelow);
        let text = match d.segments().first() {
            Some(Segment::Prose(r)) => r.to_string(),
            _ => panic!(),
        };
        assert!(text.contains("first\n\n"), "got {text:?}");
    }

    #[test]
    fn position_after_in_block_moves_right_inside_body() {
        let (mut d, idx) = doc_with_block();
        let g_offset = body_marker_offset(&d, idx, "GET");
        d.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: g_offset,
        });
        position_for_insert(&mut d, InsertPos::After);
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, g_offset + 1);
        } else {
            panic!()
        }
    }

    #[test]
    fn position_after_in_block_at_eol_stays() {
        let (mut d, idx) = doc_with_block();
        // newline right after `GET /x`
        let nl_offset = body_marker_offset(&d, idx, "GET /x\n") + "GET /x".len();
        d.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: nl_offset,
        });
        position_for_insert(&mut d, InsertPos::After);
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, nl_offset);
        } else {
            panic!()
        }
    }

    #[test]
    fn position_line_start_in_block_skips_indent() {
        // `GET /x` has no indent; instead use raw mutation: re-parse markdown
        // with leading spaces in body.
        let md = "```http alias=req1\n   GET /x\n```\n";
        let mut d = Document::from_markdown(md).expect("parse");
        let idx = d
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        let space_offset = body_marker_offset(&d, idx, "   GET");
        d.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: space_offset,
        });
        position_for_insert(&mut d, InsertPos::LineStart);
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, space_offset + 3);
        } else {
            panic!()
        }
    }

    #[test]
    fn position_line_end_in_block_lands_at_eol() {
        let (mut d, idx) = doc_with_block();
        let g_offset = body_marker_offset(&d, idx, "GET");
        d.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: g_offset,
        });
        position_for_insert(&mut d, InsertPos::LineEnd);
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, g_offset + "GET /x".len());
        } else {
            panic!()
        }
    }

    #[test]
    fn recoil_in_block_moves_back_one() {
        let (mut d, idx) = doc_with_block();
        let g_offset = body_marker_offset(&d, idx, "GET");
        d.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: g_offset + 2,
        });
        recoil_after_exit(&mut d);
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, g_offset + 1);
        } else {
            panic!()
        }
    }

    #[test]
    fn recoil_at_block_line_start_stays() {
        let (mut d, idx) = doc_with_block();
        let g_offset = body_marker_offset(&d, idx, "GET");
        d.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: g_offset,
        });
        recoil_after_exit(&mut d);
        if let Cursor::InBlock { offset, .. } = d.cursor() {
            assert_eq!(offset, g_offset);
        } else {
            panic!()
        }
    }

    #[test]
    fn recoil_in_block_result_is_noop() {
        let (mut d, idx) = doc_with_block();
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: idx,
            row: 0,
        });
        let before = d.cursor();
        recoil_after_exit(&mut d);
        assert_eq!(d.cursor(), before);
    }

    #[test]
    fn position_for_insert_block_result_variants_are_noop() {
        let (mut d, idx) = doc_with_block();
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: idx,
            row: 0,
        });
        let before = d.cursor();
        for pos in [
            InsertPos::After,
            InsertPos::LineStart,
            InsertPos::LineEnd,
            InsertPos::LineAbove,
            InsertPos::LineBelow,
        ] {
            position_for_insert(&mut d, pos);
            assert_eq!(d.cursor(), before);
        }
    }
}
