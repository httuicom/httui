//! Operator engine — `d` / `c` / `y` plus their linewise shortcuts
//! (`dd`, `cc`, `yy`) and the paste pair (`p`, `P`).
//!
//! This module owns range computation. The parser hands us a
//! `(Operator, Motion, count)` triple; we project the motion onto the
//! current cursor, classify the resulting range
//! ([`MotionClass`]: exclusive / inclusive / linewise), apply the op,
//! and update the unnamed register. Cross-segment ranges are refused
//! (no-op) — operating across blocks would require synthesizing a new
//! buffer topology, deferred to a later round.

use crate::buffer::{Cursor, Document, Segment};
use crate::vim::motions;
use crate::vim::parser::{Motion, MotionClass, Operator, PastePos, TextObject};
use crate::vim::register::Register;
use crate::vim::textobject;

/// Result of applying an operator. `enter_insert == true` after `c…`
/// or `cc` so the dispatcher can transition modes.
#[derive(Debug, Default, Clone, Copy)]
pub struct OpOutcome {
    pub enter_insert: bool,
}

/// Apply an operator paired with a motion: `dw`, `c$`, `yh`, …
pub fn apply_motion(
    op: Operator,
    motion: Motion,
    count: usize,
    doc: &mut Document,
    reg: &mut Register,
    viewport: u16,
) -> OpOutcome {
    let Some((seg_idx, range)) = compute_motion_range(motion, count, doc, viewport) else {
        return OpOutcome::default();
    };
    apply_range(op, seg_idx, range, doc, reg)
}

/// Apply a linewise shortcut: `dd`, `cc`, `yy`. `count` ≥ 1.
pub fn apply_linewise(
    op: Operator,
    count: usize,
    doc: &mut Document,
    reg: &mut Register,
) -> OpOutcome {
    let Some((seg_idx, range)) = compute_linewise_range(count, doc) else {
        return OpOutcome::default();
    };
    apply_range(op, seg_idx, range, doc, reg)
}

/// Apply an operator to the current visual selection (`d`/`c`/`y`/`x`
/// from charwise or linewise visual). Both endpoints must live in the
/// same prose segment — cross-segment selections are refused
/// (no-op), matching the rest of the operator engine.
pub fn apply_visual(
    op: Operator,
    anchor: Cursor,
    cursor: Cursor,
    linewise: bool,
    doc: &mut Document,
    reg: &mut Register,
) -> OpOutcome {
    let (
        Cursor::InProse {
            segment_idx: a_seg,
            offset: a_off,
        },
        Cursor::InProse {
            segment_idx: c_seg,
            offset: c_off,
        },
    ) = (anchor, cursor)
    else {
        return OpOutcome::default();
    };
    if a_seg != c_seg {
        return OpOutcome::default();
    }
    let rope = match doc.segments().get(a_seg) {
        Some(Segment::Prose(r)) => r,
        _ => return OpOutcome::default(),
    };
    let total = rope.len_chars();
    let (lo, hi) = (a_off.min(c_off), a_off.max(c_off));
    let range = if linewise {
        let lo_line = rope.char_to_line(lo.min(total));
        let hi_line = rope.char_to_line(hi.min(total));
        let start = rope.line_to_char(lo_line);
        let end = if hi_line + 1 >= rope.len_lines() {
            total
        } else {
            rope.line_to_char(hi_line + 1)
        };
        Range {
            start,
            end,
            linewise: true,
        }
    } else {
        // Charwise visual is inclusive at both ends.
        Range {
            start: lo,
            end: (hi + 1).min(total),
            linewise: false,
        }
    };
    apply_range(op, a_seg, range, doc, reg)
}

/// Apply an operator to a text object: `diw`, `ca"`, `yi(`, …
/// Text objects are charwise; `count` is unused for round 3 (vim's
/// `2iw` semantics — extending to adjacent words — is deferred).
pub fn apply_text_object(
    op: Operator,
    textobj: TextObject,
    _count: usize,
    doc: &mut Document,
    reg: &mut Register,
) -> OpOutcome {
    let Some((seg_idx, start, end)) = textobject::compute_range(textobj, doc) else {
        return OpOutcome::default();
    };
    apply_range(
        op,
        seg_idx,
        Range {
            start,
            end,
            linewise: false,
        },
        doc,
        reg,
    )
}

/// Insert the contents of `reg` at / after the cursor according to `pos`.
/// `count` repeats the paste body. Linewise registers paste on a new
/// line; charwise ones inline. Cursor lands on the last char inserted
/// (charwise) or the first char of the new line (linewise) — same as vim.
pub fn paste(pos: PastePos, count: usize, doc: &mut Document, reg: &Register) {
    if reg.is_empty() {
        return;
    }
    let count = count.max(1);
    let body = reg.text.repeat(count);

    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            if !matches!(doc.segments().get(segment_idx), Some(Segment::Prose(_))) {
                return;
            }
            if reg.linewise {
                paste_linewise(pos, segment_idx, offset, &body, doc);
            } else {
                paste_charwise(pos, segment_idx, offset, &body, doc);
            }
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            paste_into_block(pos, segment_idx, offset, &body, doc);
        }
        Cursor::InBlockResult { .. } => {}
    }
}

fn paste_into_block(
    pos: PastePos,
    segment_idx: usize,
    offset: usize,
    body: &str,
    doc: &mut Document,
) {
    let new_len;
    let insert_at;
    {
        let Some(b) = doc.block_at_mut(segment_idx) else {
            return;
        };
        let total = b.raw.len_chars();
        insert_at = match pos {
            PastePos::Before => offset.min(total),
            PastePos::After => (offset + 1).min(total),
        };
        b.raw.insert(insert_at, body);
        b.reparse_from_raw();
        new_len = b.raw.len_chars();
    }
    let inserted = body.chars().count();
    let new_offset = (insert_at + inserted.saturating_sub(1)).min(new_len.saturating_sub(1));
    doc.set_cursor(Cursor::InBlock {
        segment_idx,
        offset: new_offset,
    });
    doc.mark_dirty();
}

// ───────────── range computation ─────────────

#[derive(Debug, Clone, Copy)]
struct Range {
    start: usize,
    end: usize,
    linewise: bool,
}

fn compute_motion_range(
    motion: Motion,
    count: usize,
    doc: &mut Document,
    viewport: u16,
) -> Option<(usize, Range)> {
    let Cursor::InProse {
        segment_idx: src_seg,
        offset: src_off,
    } = doc.cursor()
    else {
        return None;
    };

    let target = motions::target(motion, doc, count, viewport);
    let Cursor::InProse {
        segment_idx: dst_seg,
        offset: dst_off,
    } = target
    else {
        return None;
    };

    if dst_seg != src_seg {
        // Cross-segment operators are deferred. Falling through to a
        // partial range that stops at the segment boundary would surprise
        // users — better to no-op and let them issue separate commands.
        return None;
    }

    let rope = match doc.segments().get(src_seg)? {
        Segment::Prose(r) => r,
        _ => return None,
    };
    let total = rope.len_chars();

    let class = motion.class();
    let (lo, hi) = (src_off.min(dst_off), src_off.max(dst_off));
    let range = match class {
        MotionClass::Exclusive => Range {
            start: lo,
            end: hi.min(total),
            linewise: false,
        },
        MotionClass::Inclusive => Range {
            start: lo,
            end: (hi + 1).min(total),
            linewise: false,
        },
        MotionClass::Linewise => {
            let lo_line = rope.char_to_line(lo.min(total));
            let hi_line = rope.char_to_line(hi.min(total));
            let start = rope.line_to_char(lo_line);
            let end = if hi_line + 1 < rope.len_lines() {
                rope.line_to_char(hi_line + 1)
            } else {
                total
            };
            Range {
                start,
                end,
                linewise: true,
            }
        }
    };
    if range.end <= range.start {
        return None;
    }
    Some((src_seg, range))
}

fn compute_linewise_range(count: usize, doc: &Document) -> Option<(usize, Range)> {
    let Cursor::InProse {
        segment_idx,
        offset,
    } = doc.cursor()
    else {
        return None;
    };
    let rope = match doc.segments().get(segment_idx)? {
        Segment::Prose(r) => r,
        _ => return None,
    };
    let total = rope.len_chars();
    if total == 0 {
        return None;
    }
    let off = offset.min(total);
    let line_start = rope.char_to_line(off);
    let last_line = rope.len_lines().saturating_sub(1);
    let line_end = (line_start + count.max(1) - 1).min(last_line);
    let start = rope.line_to_char(line_start);
    let end = if line_end + 1 < rope.len_lines() {
        rope.line_to_char(line_end + 1)
    } else {
        total
    };
    if end <= start {
        return None;
    }
    Some((
        segment_idx,
        Range {
            start,
            end,
            linewise: true,
        },
    ))
}

fn apply_range(
    op: Operator,
    seg_idx: usize,
    mut range: Range,
    doc: &mut Document,
    reg: &mut Register,
) -> OpOutcome {
    // Capture the full range for the register *before* we trim for change.
    let yanked = doc.text_in_segment_range(seg_idx, range.start, range.end);
    if range.linewise {
        // Ensure the register text always ends with `\n` so that paste
        // semantics are well-defined regardless of where the source line
        // sat in the document.
        let normalized = if yanked.ends_with('\n') {
            yanked
        } else {
            format!("{yanked}\n")
        };
        reg.set_linewise(normalized);
    } else {
        reg.set_charwise(yanked);
    }

    if matches!(op, Operator::Yank) {
        return OpOutcome::default();
    }

    // For change on a linewise range, drop the trailing newline from the
    // deletion so the user lands on the line they're editing instead of
    // collapsing onto the next one.
    if matches!(op, Operator::Change) && range.linewise {
        if let Some(Segment::Prose(rope)) = doc.segments().get(seg_idx) {
            if range.end > range.start && range.end <= rope.len_chars() {
                let last = rope.char(range.end - 1);
                if last == '\n' {
                    range.end -= 1;
                }
            }
        }
    }

    doc.delete_range_in_segment(seg_idx, range.start, range.end);
    doc.set_cursor(Cursor::InProse {
        segment_idx: seg_idx,
        offset: range.start,
    });
    OpOutcome {
        enter_insert: matches!(op, Operator::Change),
    }
}

// ───────────── paste ─────────────

fn paste_charwise(pos: PastePos, seg_idx: usize, offset: usize, body: &str, doc: &mut Document) {
    let rope = match doc.segments().get(seg_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return,
    };
    let total = rope.len_chars();
    let on_newline_or_end =
        offset >= total || rope.char(offset.min(total.saturating_sub(1))) == '\n';
    let insert_at = match pos {
        // `p` after cursor — but only if there's a char to come after.
        // On EOL / EOF we still paste at offset (vim's "after" degenerates
        // to "at" when there's nothing to advance past).
        PastePos::After => {
            if !on_newline_or_end {
                offset + 1
            } else {
                offset
            }
        }
        PastePos::Before => offset,
    };
    let n = doc.insert_text_in_segment(seg_idx, insert_at, body);
    if n > 0 {
        doc.set_cursor(Cursor::InProse {
            segment_idx: seg_idx,
            offset: insert_at + n - 1,
        });
    }
}

fn paste_linewise(pos: PastePos, seg_idx: usize, offset: usize, body: &str, doc: &mut Document) {
    let rope = match doc.segments().get(seg_idx) {
        Some(Segment::Prose(r)) => r,
        _ => return,
    };
    let total = rope.len_chars();
    let off = offset.min(total);
    let line = rope.char_to_line(off);
    let insert_at = match pos {
        PastePos::After => {
            if line + 1 < rope.len_lines() {
                rope.line_to_char(line + 1)
            } else {
                total
            }
        }
        PastePos::Before => rope.line_to_char(line),
    };
    // Body always ends with `\n` (normalized at yank time). When pasting
    // at end-of-segment with no trailing newline, prepend one so the
    // existing last line stays intact.
    let body = if pos == PastePos::After && insert_at == total && !ends_with_newline(rope) {
        format!("\n{}", body)
    } else {
        body.to_string()
    };
    doc.insert_text_in_segment(seg_idx, insert_at, &body);
    doc.set_cursor(Cursor::InProse {
        segment_idx: seg_idx,
        offset: insert_at,
    });
}

fn ends_with_newline(rope: &ropey::Rope) -> bool {
    let n = rope.len_chars();
    n > 0 && rope.char(n - 1) == '\n'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;
    use crate::vim::register::Register;

    fn doc(md: &str) -> Document {
        Document::from_markdown(md).unwrap()
    }

    fn cursor_offset(d: &Document) -> usize {
        match d.cursor() {
            Cursor::InProse { offset, .. } => offset,
            _ => panic!("not in prose"),
        }
    }

    fn prose(d: &Document, idx: usize) -> String {
        match d.segments().get(idx) {
            Some(Segment::Prose(r)) => r.to_string(),
            _ => panic!("not prose"),
        }
    }

    // NOTE: `Document::from_markdown` consumes the source via `lines()` and
    // re-joins with `\n` — the trailing newline of the original markdown is
    // dropped. So a source `"hello\n"` produces a prose rope of `"hello"`.
    // Expected strings below reflect that.

    #[test]
    fn visual_charwise_yank_captures_inclusive_range() {
        // "hello world" → anchor at h (0), cursor at o of "hello" (4). Yank.
        let mut d = doc("hello world\n");
        let mut r = Register::empty();
        let anchor = Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        };
        let cursor = Cursor::InProse {
            segment_idx: 0,
            offset: 4,
        };
        d.set_cursor(cursor);
        let out = apply_visual(Operator::Yank, anchor, cursor, false, &mut d, &mut r);
        assert!(!out.enter_insert);
        assert_eq!(r.text, "hello"); // inclusive both ends
        assert!(!r.linewise);
        // Yank doesn't mutate the buffer.
        assert_eq!(prose(&d, 0), "hello world");
    }

    #[test]
    fn visual_charwise_delete_removes_inclusive_range() {
        let mut d = doc("hello world\n");
        let mut r = Register::empty();
        let anchor = Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        };
        let cursor = Cursor::InProse {
            segment_idx: 0,
            offset: 4,
        };
        d.set_cursor(cursor);
        apply_visual(Operator::Delete, anchor, cursor, false, &mut d, &mut r);
        assert_eq!(prose(&d, 0), " world");
        assert_eq!(cursor_offset(&d), 0);
    }

    #[test]
    fn visual_linewise_yank_captures_full_lines() {
        let mut d = doc("alpha\nbeta\ngamma\n");
        let mut r = Register::empty();
        // Anchor on line 0 (offset 2 within "alpha"), cursor on line 1.
        let anchor = Cursor::InProse {
            segment_idx: 0,
            offset: 2,
        };
        let cursor = Cursor::InProse {
            segment_idx: 0,
            offset: 8,
        }; // mid "beta"
        d.set_cursor(cursor);
        apply_visual(Operator::Yank, anchor, cursor, true, &mut d, &mut r);
        // Whole lines selected, register normalized with trailing newline.
        assert_eq!(r.text, "alpha\nbeta\n");
        assert!(r.linewise);
        // Buffer untouched on yank.
        assert_eq!(prose(&d, 0), "alpha\nbeta\ngamma");
    }

    #[test]
    fn dw_deletes_word_and_trailing_space() {
        let mut d = doc("hello world\n");
        let mut r = Register::empty();
        let out = apply_motion(Operator::Delete, Motion::WordForward, 1, &mut d, &mut r, 10);
        assert!(!out.enter_insert);
        assert_eq!(prose(&d, 0), "world");
        assert_eq!(r.text, "hello ");
        assert!(!r.linewise);
    }

    #[test]
    fn d_dollar_deletes_to_end_of_line_inclusive() {
        let mut d = doc("hello world\n");
        let mut r = Register::empty();
        apply_motion(Operator::Delete, Motion::LineEnd, 1, &mut d, &mut r, 10);
        assert_eq!(prose(&d, 0), "");
        assert_eq!(r.text, "hello world");
    }

    #[test]
    fn d0_deletes_back_to_line_start() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 6,
        });
        let mut r = Register::empty();
        apply_motion(Operator::Delete, Motion::LineStart, 1, &mut d, &mut r, 10);
        assert_eq!(prose(&d, 0), "world");
        assert_eq!(cursor_offset(&d), 0);
    }

    #[test]
    fn dh_dl_delete_one_char() {
        let mut d = doc("abc\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 1,
        });
        let mut r = Register::empty();
        apply_motion(Operator::Delete, Motion::Right, 1, &mut d, &mut r, 10);
        assert_eq!(prose(&d, 0), "ac");

        let mut r2 = Register::empty();
        apply_motion(Operator::Delete, Motion::Left, 1, &mut d, &mut r2, 10);
        assert_eq!(prose(&d, 0), "c");
    }

    #[test]
    fn de_inclusive_to_word_end() {
        let mut d = doc("hello world\n");
        let mut r = Register::empty();
        apply_motion(Operator::Delete, Motion::WordEnd, 1, &mut d, &mut r, 10);
        assert_eq!(prose(&d, 0), " world");
        assert_eq!(r.text, "hello");
    }

    #[test]
    fn dd_deletes_whole_line_and_register_is_linewise() {
        let mut d = doc("aa\nbb\ncc\n");
        let mut r = Register::empty();
        apply_linewise(Operator::Delete, 1, &mut d, &mut r);
        assert_eq!(prose(&d, 0), "bb\ncc");
        assert_eq!(r.text, "aa\n");
        assert!(r.linewise);
    }

    #[test]
    fn count_dd_deletes_multiple_lines() {
        let mut d = doc("aa\nbb\ncc\ndd\n");
        let mut r = Register::empty();
        apply_linewise(Operator::Delete, 2, &mut d, &mut r);
        assert_eq!(prose(&d, 0), "cc\ndd");
        assert_eq!(r.text, "aa\nbb\n");
    }

    #[test]
    fn yw_yanks_without_modifying() {
        let mut d = doc("hello world\n");
        let mut r = Register::empty();
        apply_motion(Operator::Yank, Motion::WordForward, 1, &mut d, &mut r, 10);
        assert_eq!(prose(&d, 0), "hello world");
        assert_eq!(r.text, "hello ");
    }

    #[test]
    fn yy_yanks_line_linewise() {
        let mut d = doc("aa\nbb\n");
        let mut r = Register::empty();
        apply_linewise(Operator::Yank, 1, &mut d, &mut r);
        assert_eq!(prose(&d, 0), "aa\nbb");
        assert_eq!(r.text, "aa\n");
        assert!(r.linewise);
    }

    #[test]
    fn cw_deletes_and_signals_insert() {
        let mut d = doc("hello world\n");
        let mut r = Register::empty();
        let out = apply_motion(Operator::Change, Motion::WordForward, 1, &mut d, &mut r, 10);
        assert!(out.enter_insert);
        assert_eq!(prose(&d, 0), "world");
        assert_eq!(r.text, "hello ");
    }

    #[test]
    fn cc_keeps_newline_for_in_place_edit() {
        let mut d = doc("hello\nworld\n");
        let mut r = Register::empty();
        let out = apply_linewise(Operator::Change, 1, &mut d, &mut r);
        assert!(out.enter_insert);
        // Line content gone, newline preserved.
        assert_eq!(prose(&d, 0), "\nworld");
        // Register holds the full linewise content (including newline).
        assert_eq!(r.text, "hello\n");
        assert!(r.linewise);
    }

    #[test]
    fn p_charwise_inserts_after_cursor() {
        let mut d = doc("ac\n");
        let mut r = Register::empty();
        r.set_charwise("b".into());
        // cursor on 'a' (offset 0)
        paste(PastePos::After, 1, &mut d, &r);
        assert_eq!(prose(&d, 0), "abc");
        assert_eq!(cursor_offset(&d), 1);
    }

    #[test]
    fn capital_p_charwise_inserts_at_cursor() {
        let mut d = doc("bc\n");
        let mut r = Register::empty();
        r.set_charwise("a".into());
        paste(PastePos::Before, 1, &mut d, &r);
        assert_eq!(prose(&d, 0), "abc");
        assert_eq!(cursor_offset(&d), 0);
    }

    #[test]
    fn p_linewise_inserts_below() {
        let mut d = doc("aa\nbb\n");
        let mut r = Register::empty();
        r.set_linewise("xx\n".into());
        // cursor on line 0
        paste(PastePos::After, 1, &mut d, &r);
        assert_eq!(prose(&d, 0), "aa\nxx\nbb");
    }

    #[test]
    fn capital_p_linewise_inserts_above() {
        let mut d = doc("aa\nbb\n");
        let mut r = Register::empty();
        r.set_linewise("xx\n".into());
        paste(PastePos::Before, 1, &mut d, &r);
        assert_eq!(prose(&d, 0), "xx\naa\nbb");
    }

    #[test]
    fn paste_with_count_repeats() {
        let mut d = doc("ac\n");
        let mut r = Register::empty();
        r.set_charwise("b".into());
        paste(PastePos::After, 3, &mut d, &r);
        assert_eq!(prose(&d, 0), "abbbc");
    }

    #[test]
    fn dw_then_p_round_trips_text() {
        let mut d = doc("hello world\n");
        let mut r = Register::empty();
        apply_motion(Operator::Delete, Motion::WordForward, 1, &mut d, &mut r, 10);
        // doc: "world", register: "hello "
        // cursor at 0; charwise paste before puts "hello " at start.
        paste(PastePos::Before, 1, &mut d, &r);
        assert_eq!(prose(&d, 0), "hello world");
    }

    // ── text objects ──

    #[test]
    fn diw_deletes_inner_word() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 2,
        });
        let mut r = Register::empty();
        apply_text_object(
            Operator::Delete,
            TextObject::Word { around: false },
            1,
            &mut d,
            &mut r,
        );
        assert_eq!(prose(&d, 0), " world");
        assert_eq!(r.text, "hello");
    }

    #[test]
    fn daw_deletes_word_with_trailing_space() {
        let mut d = doc("hello world\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 2,
        });
        let mut r = Register::empty();
        apply_text_object(
            Operator::Delete,
            TextObject::Word { around: true },
            1,
            &mut d,
            &mut r,
        );
        assert_eq!(prose(&d, 0), "world");
        assert_eq!(r.text, "hello ");
    }

    #[test]
    fn ciquote_changes_inner_string() {
        let mut d = doc("say \"hello\" loud\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 6,
        });
        let mut r = Register::empty();
        let out = apply_text_object(
            Operator::Change,
            TextObject::Quote {
                delim: '"',
                around: false,
            },
            1,
            &mut d,
            &mut r,
        );
        assert!(out.enter_insert);
        assert_eq!(prose(&d, 0), "say \"\" loud");
    }

    #[test]
    fn yi_paren_yanks_inner_pair() {
        let mut d = doc("call(arg)\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 5,
        });
        let mut r = Register::empty();
        apply_text_object(
            Operator::Yank,
            TextObject::Pair {
                open: '(',
                close: ')',
                around: false,
            },
            1,
            &mut d,
            &mut r,
        );
        // Doc unchanged; register has "arg".
        assert_eq!(prose(&d, 0), "call(arg)");
        assert_eq!(r.text, "arg");
    }

    #[test]
    fn da_paren_deletes_around_pair() {
        let mut d = doc("call(arg)\n");
        d.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: 5,
        });
        let mut r = Register::empty();
        apply_text_object(
            Operator::Delete,
            TextObject::Pair {
                open: '(',
                close: ')',
                around: true,
            },
            1,
            &mut d,
            &mut r,
        );
        assert_eq!(prose(&d, 0), "call");
        assert_eq!(r.text, "(arg)");
    }
}
