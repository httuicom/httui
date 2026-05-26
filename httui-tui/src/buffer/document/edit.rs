//! Single-char insert / delete operations and prose-range editing
//! used by the dispatch layer. Block segments mutate via the raw
//! rope and re-parse — when the edit invalidates the fence the
//! block dissolves back to prose.

use ropey::Rope;

use crate::buffer::cursor::Cursor;
use crate::buffer::segment::Segment;

use super::Document;

impl Document {
    /// Insert one character at the cursor position. Routes to either
    /// the prose rope (for `Cursor::InProse`) or the block's `raw`
    /// rope (for `Cursor::InBlock`).
    ///
    /// `Cursor::InBlock` edits go straight to `block.raw` and trigger
    /// `reparse_from_raw` so the derived fields (`alias`,
    /// `display_mode`, `params`) follow the source of truth. The
    /// fence header and closer are now editable too — typing on
    /// them mutates the rope and may flip the parse to "no longer
    /// a valid block", in which case `reparse_from_raw` keeps
    /// last-good derived fields and lets the user fix the typo.
    pub fn insert_char_at_cursor(&mut self, ch: char) {
        match self.cursor {
            Cursor::InProse {
                segment_idx,
                offset,
            } => {
                if let Some(Segment::Prose(rope)) = self.segments.get_mut(segment_idx) {
                    let off = offset.min(rope.len_chars());
                    rope.insert_char(off, ch);
                    self.cursor = Cursor::InProse {
                        segment_idx,
                        offset: off + 1,
                    };
                    self.dirty = true;
                }
            }
            Cursor::InBlock {
                segment_idx,
                offset,
            } => {
                let Some(Segment::Block(b)) = self.segments.get_mut(segment_idx) else {
                    return;
                };
                let off = offset.min(b.raw.len_chars());
                b.raw.insert_char(off, ch);
                let still_block = b.reparse_from_raw();
                let new_offset = off + 1;
                if still_block {
                    self.cursor = Cursor::InBlock {
                        segment_idx,
                        offset: new_offset,
                    };
                } else {
                    let text = b.raw.to_string();
                    self.replace_segment(segment_idx, Segment::Prose(Rope::from_str(&text)));
                    self.cursor = Cursor::InProse {
                        segment_idx,
                        offset: new_offset,
                    };
                }
                self.dirty = true;
            }
            // Result rows are read-only — typing in the table is a no-op.
            Cursor::InBlockResult { .. } => {}
        }
    }

    /// Insert a newline at the cursor.
    pub fn insert_newline_at_cursor(&mut self) {
        self.insert_char_at_cursor('\n');
    }

    /// Backspace: remove the char immediately before the cursor.
    /// At the start of a non-first line, fold into the previous line.
    pub fn delete_char_before_cursor(&mut self) {
        match self.cursor {
            Cursor::InProse {
                segment_idx,
                offset,
            } => {
                if offset == 0 {
                    return;
                }
                if let Some(Segment::Prose(rope)) = self.segments.get_mut(segment_idx) {
                    if offset > 0 && offset <= rope.len_chars() {
                        rope.remove(offset - 1..offset);
                        self.cursor = Cursor::InProse {
                            segment_idx,
                            offset: offset - 1,
                        };
                        self.dirty = true;
                    }
                }
            }
            Cursor::InBlock {
                segment_idx,
                offset,
            } => {
                if offset == 0 {
                    return;
                }
                let Some(Segment::Block(b)) = self.segments.get_mut(segment_idx) else {
                    return;
                };
                if offset > b.raw.len_chars() {
                    return;
                }
                b.raw.remove(offset - 1..offset);
                let still_block = b.reparse_from_raw();
                let new_offset = offset - 1;
                if still_block {
                    self.cursor = Cursor::InBlock {
                        segment_idx,
                        offset: new_offset,
                    };
                } else {
                    let text = b.raw.to_string();
                    self.replace_segment(segment_idx, Segment::Prose(Rope::from_str(&text)));
                    self.cursor = Cursor::InProse {
                        segment_idx,
                        offset: new_offset,
                    };
                }
                self.dirty = true;
            }
            Cursor::InBlockResult { .. } => {}
        }
    }

    /// Forward delete (`x`, `Del`): remove the char under the cursor.
    pub fn delete_char_at_cursor(&mut self) {
        match self.cursor {
            Cursor::InProse {
                segment_idx,
                offset,
            } => {
                if let Some(Segment::Prose(rope)) = self.segments.get_mut(segment_idx) {
                    if offset < rope.len_chars() {
                        rope.remove(offset..offset + 1);
                        self.dirty = true;
                    }
                }
            }
            Cursor::InBlock {
                segment_idx,
                offset,
            } => {
                let Some(Segment::Block(b)) = self.segments.get_mut(segment_idx) else {
                    return;
                };
                if offset < b.raw.len_chars() {
                    b.raw.remove(offset..offset + 1);
                    let still_block = b.reparse_from_raw();
                    if !still_block {
                        let text = b.raw.to_string();
                        self.replace_segment(segment_idx, Segment::Prose(Rope::from_str(&text)));
                        self.cursor = Cursor::InProse {
                            segment_idx,
                            offset,
                        };
                    }
                    self.dirty = true;
                }
            }
            Cursor::InBlockResult { .. } => {}
        }
    }

    /// Read a substring (in chars) from a prose segment. Out-of-bounds
    /// indices are clamped. Returns an empty string for non-prose
    /// segments or invalid indices.
    pub fn text_in_segment_range(&self, segment_idx: usize, start: usize, end: usize) -> String {
        let Some(Segment::Prose(rope)) = self.segments.get(segment_idx) else {
            return String::new();
        };
        let total = rope.len_chars();
        let s = start.min(total);
        let e = end.min(total).max(s);
        rope.slice(s..e).to_string()
    }

    /// Delete a char range from a prose segment. Cursor placement is
    /// the caller's responsibility (the operator engine moves the
    /// cursor to `start` after deletion). No-op for non-prose segments
    /// or empty ranges. Marks the document dirty when something is
    /// actually removed.
    pub fn delete_range_in_segment(&mut self, segment_idx: usize, start: usize, end: usize) {
        let Some(Segment::Prose(rope)) = self.segments.get_mut(segment_idx) else {
            return;
        };
        let total = rope.len_chars();
        let s = start.min(total);
        let e = end.min(total);
        if e <= s {
            return;
        }
        rope.remove(s..e);
        self.dirty = true;
    }

    /// Insert `text` into a prose segment at char `offset`. Returns the
    /// number of chars inserted (so callers can place the cursor at
    /// `offset + n` if desired). No-op for non-prose segments.
    pub fn insert_text_in_segment(
        &mut self,
        segment_idx: usize,
        offset: usize,
        text: &str,
    ) -> usize {
        let Some(Segment::Prose(rope)) = self.segments.get_mut(segment_idx) else {
            return 0;
        };
        let total = rope.len_chars();
        let off = offset.min(total);
        rope.insert(off, text);
        if !text.is_empty() {
            self.dirty = true;
        }
        text.chars().count()
    }
}

#[cfg(test)]
mod tests {
    use super::Document;
    use crate::buffer::cursor::Cursor;
    use crate::buffer::segment::Segment;

    #[test]
    fn delete_char_at_cursor_in_prose_removes_under_cursor() {
        let mut d = Document::from_markdown("hello\n").unwrap();
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        d.delete_char_at_cursor();
        let text = match &d.segments()[0] {
            Segment::Prose(r) => r.to_string(),
            _ => panic!(),
        };
        assert!(text.starts_with("ello"));
    }

    #[test]
    fn delete_char_at_cursor_at_eof_is_noop_in_prose() {
        let mut d = Document::from_markdown("hi\n").unwrap();
        let end = match &d.segments()[0] {
            Segment::Prose(r) => r.len_chars(),
            _ => 0,
        };
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: end,
        });
        d.delete_char_at_cursor();
        assert_eq!(
            match &d.segments()[0] {
                Segment::Prose(r) => r.to_string(),
                _ => String::new(),
            },
            "hi"
        );
    }

    #[test]
    fn delete_char_at_cursor_in_block_dissolves_when_fence_breaks() {
        let md = "```http alias=q\nGET https://x.com\n```\n";
        let mut d = Document::from_markdown(md).unwrap();
        let blk = d.segments().iter().position(|s| s.is_block()).unwrap();
        // Park inside the fence opener — removing one char breaks the fence.
        d.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: 0,
        });
        d.delete_char_at_cursor();
        assert!(matches!(d.segments().get(blk), Some(Segment::Prose(_))));
    }

    #[test]
    fn delete_char_at_cursor_inside_block_keeps_block_when_fence_holds() {
        let md = "```db-sqlite alias=q\nSELECT 1\n```\n";
        let mut d = Document::from_markdown(md).unwrap();
        let blk = d.segments().iter().position(|s| s.is_block()).unwrap();
        let raw = match d.segments().get(blk) {
            Some(Segment::Block(b)) => b.raw.clone(),
            _ => panic!(),
        };
        let body_off = crate::buffer::block::body_line_col_to_raw_offset(&raw, 0, 0);
        d.set_cursor(Cursor::InBlock {
            segment_idx: blk,
            offset: body_off,
        });
        d.delete_char_at_cursor();
        assert!(matches!(d.segments().get(blk), Some(Segment::Block(_))));
    }

    #[test]
    fn delete_char_at_cursor_in_block_result_is_noop() {
        let mut d = Document::from_markdown("hi\n").unwrap();
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: 0,
            row: 0,
        });
        d.delete_char_at_cursor();
        assert!(matches!(d.cursor(), Cursor::InBlockResult { .. }));
    }

    #[test]
    fn text_in_segment_range_clamps_to_segment_length() {
        let d = Document::from_markdown("hello\n").unwrap();
        let text = d.text_in_segment_range(0, 0, 99);
        assert!(text.starts_with("hello"));
    }

    #[test]
    fn text_in_segment_range_returns_empty_for_block_segment() {
        let d = Document::from_markdown("```http\nGET https://x.com\n```\n").unwrap();
        let blk = d.segments().iter().position(|s| s.is_block()).unwrap();
        assert_eq!(d.text_in_segment_range(blk, 0, 10), "");
    }

    #[test]
    fn delete_range_in_segment_removes_range() {
        let mut d = Document::from_markdown("abcdef\n").unwrap();
        d.delete_range_in_segment(0, 1, 4);
        assert_eq!(
            match &d.segments()[0] {
                Segment::Prose(r) => r.to_string(),
                _ => String::new(),
            },
            "aef"
        );
        assert!(d.is_dirty());
    }

    #[test]
    fn delete_range_in_segment_noop_when_empty_range() {
        let mut d = Document::from_markdown("abc\n").unwrap();
        d.delete_range_in_segment(0, 2, 2);
        assert_eq!(
            match &d.segments()[0] {
                Segment::Prose(r) => r.to_string(),
                _ => String::new(),
            },
            "abc"
        );
    }

    #[test]
    fn delete_range_in_segment_clamps_end() {
        let mut d = Document::from_markdown("abc\n").unwrap();
        d.delete_range_in_segment(0, 1, 999);
        let s = match &d.segments()[0] {
            Segment::Prose(r) => r.to_string(),
            _ => String::new(),
        };
        assert!(s.starts_with("a"));
    }

    #[test]
    fn insert_text_in_segment_returns_char_count_and_inserts() {
        let mut d = Document::from_markdown("ab\n").unwrap();
        let n = d.insert_text_in_segment(0, 1, "XYZ");
        assert_eq!(n, 3);
        assert_eq!(
            match &d.segments()[0] {
                Segment::Prose(r) => r.to_string(),
                _ => String::new(),
            },
            "aXYZb"
        );
        assert!(d.is_dirty());
    }

    #[test]
    fn insert_text_in_segment_noop_for_block() {
        let md = "```http\nGET https://x.com\n```\n";
        let mut d = Document::from_markdown(md).unwrap();
        let blk = d.segments().iter().position(|s| s.is_block()).unwrap();
        let n = d.insert_text_in_segment(blk, 0, "x");
        assert_eq!(n, 0);
    }

    #[test]
    fn insert_text_in_segment_empty_string_does_not_mark_dirty() {
        let mut d = Document::from_markdown("hi\n").unwrap();
        d.mark_clean();
        d.insert_text_in_segment(0, 0, "");
        assert!(!d.is_dirty());
    }

    #[test]
    fn insert_newline_at_cursor_inserts_newline() {
        let mut d = Document::from_markdown("ab\n").unwrap();
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 1,
        });
        d.insert_newline_at_cursor();
        assert_eq!(
            match &d.segments()[0] {
                Segment::Prose(r) => r.to_string(),
                _ => String::new(),
            },
            "a\nb"
        );
    }

    #[test]
    fn delete_char_before_cursor_at_offset_zero_in_prose_is_noop() {
        let mut d = Document::from_markdown("abc\n").unwrap();
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        d.delete_char_before_cursor();
        assert_eq!(
            match &d.segments()[0] {
                Segment::Prose(r) => r.to_string(),
                _ => String::new(),
            },
            "abc"
        );
    }

    #[test]
    fn delete_char_before_cursor_in_block_result_is_noop() {
        let mut d = Document::from_markdown("hi\n").unwrap();
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: 0,
            row: 0,
        });
        d.delete_char_before_cursor();
        assert!(matches!(d.cursor(), Cursor::InBlockResult { .. }));
    }

    #[test]
    fn insert_char_at_cursor_in_block_result_is_noop() {
        let mut d = Document::from_markdown("hi\n").unwrap();
        d.set_cursor(Cursor::InBlockResult {
            segment_idx: 0,
            row: 0,
        });
        d.insert_char_at_cursor('z');
        assert!(matches!(d.cursor(), Cursor::InBlockResult { .. }));
    }
}
