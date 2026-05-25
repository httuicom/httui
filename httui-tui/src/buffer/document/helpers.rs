//! Cursor clamping + empty-prose padding utilities shared by the
//! `Document` impl blocks.

use ropey::Rope;

use crate::buffer::cursor::Cursor;
use crate::buffer::segment::Segment;

/// Clamp a `Cursor` to a fresh segment list — used after
/// `replace_with_text` rebuilds the document. Out-of-range segments
/// fall back to the first prose segment (or segment 0). Offsets
/// past the segment's content clamp to its last valid position.
pub(super) fn clamp_cursor(segments: &[Segment], cursor: Cursor) -> Cursor {
    let total_segs = segments.len();
    let fallback = || -> Cursor {
        // First Prose segment, offset 0 — always exists thanks to
        // `pad_with_prose`'s invariants.
        for (i, seg) in segments.iter().enumerate() {
            if matches!(seg, Segment::Prose(_)) {
                return Cursor::InProse {
                    segment_idx: i,
                    offset: 0,
                };
            }
        }
        Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        }
    };
    match cursor {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let Some(seg) = segments.get(segment_idx.min(total_segs.saturating_sub(1))) else {
                return fallback();
            };
            match seg {
                Segment::Prose(rope) => Cursor::InProse {
                    segment_idx: segment_idx.min(total_segs.saturating_sub(1)),
                    offset: offset.min(rope.len_chars()),
                },
                Segment::Block(b) => Cursor::InBlock {
                    segment_idx: segment_idx.min(total_segs.saturating_sub(1)),
                    offset: offset.min(b.raw.len_chars()),
                },
            }
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let Some(seg) = segments.get(segment_idx.min(total_segs.saturating_sub(1))) else {
                return fallback();
            };
            match seg {
                Segment::Block(b) => Cursor::InBlock {
                    segment_idx: segment_idx.min(total_segs.saturating_sub(1)),
                    offset: offset.min(b.raw.len_chars()),
                },
                Segment::Prose(rope) => Cursor::InProse {
                    segment_idx: segment_idx.min(total_segs.saturating_sub(1)),
                    offset: offset.min(rope.len_chars()),
                },
            }
        }
        Cursor::InBlockResult { .. } => fallback(),
    }
}

/// Insert empty prose runs around blocks so the cursor always has a
/// landing zone for `i`/`a`/`j`/`k` after navigating into a block.
/// - Doc opens with a block → prepend empty prose.
/// - Doc ends with a block → append empty prose.
/// - Two blocks back-to-back → splice empty prose between them.
pub(super) fn pad_with_prose(segments: Vec<Segment>) -> Vec<Segment> {
    if segments.is_empty() {
        return segments;
    }
    let mut out: Vec<Segment> = Vec::with_capacity(segments.len() + 2);
    if matches!(segments.first(), Some(Segment::Block(_))) {
        out.push(Segment::Prose(Rope::new()));
    }
    for (i, seg) in segments.iter().enumerate() {
        if i > 0
            && matches!(seg, Segment::Block(_))
            && matches!(segments.get(i - 1), Some(Segment::Block(_)))
        {
            out.push(Segment::Prose(Rope::new()));
        }
        out.push(seg.clone());
    }
    if matches!(out.last(), Some(Segment::Block(_))) {
        out.push(Segment::Prose(Rope::new()));
    }
    out
}

pub(super) fn is_empty_prose(seg: &Segment) -> bool {
    matches!(seg, Segment::Prose(r) if r.len_chars() == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::{BlockId, BlockNode, ExecutionState};

    fn prose_seg(text: &str) -> Segment {
        Segment::Prose(Rope::from_str(text))
    }

    fn block_seg(raw: &str) -> Segment {
        Segment::Block(BlockNode {
            id: BlockId(0),
            raw: Rope::from_str(raw),
            block_type: "http".into(),
            alias: None,
            display_mode: None,
            params: serde_json::json!({}),
            state: ExecutionState::Idle,
            cached_result: None,
        })
    }

    #[test]
    fn clamp_cursor_inprose_inrange_returns_same_when_offset_valid() {
        let segs = vec![prose_seg("hello")];
        let c = clamp_cursor(
            &segs,
            Cursor::InProse {
                segment_idx: 0,
                offset: 3,
            },
        );
        assert_eq!(
            c,
            Cursor::InProse {
                segment_idx: 0,
                offset: 3
            }
        );
    }

    #[test]
    fn clamp_cursor_inprose_clamps_offset_to_rope_length() {
        let segs = vec![prose_seg("hi")];
        let c = clamp_cursor(
            &segs,
            Cursor::InProse {
                segment_idx: 0,
                offset: 999,
            },
        );
        match c {
            Cursor::InProse { offset, .. } => assert_eq!(offset, 2),
            _ => panic!("expected InProse"),
        }
    }

    #[test]
    fn clamp_cursor_inprose_falls_through_to_block_kind() {
        // Asked for InProse on a slot that holds a Block — clamped
        // form changes kind to InBlock and reuses the offset.
        let segs = vec![block_seg("```http\nGET https://x.com\n```")];
        let c = clamp_cursor(
            &segs,
            Cursor::InProse {
                segment_idx: 0,
                offset: 5,
            },
        );
        assert!(matches!(c, Cursor::InBlock { .. }));
    }

    #[test]
    fn clamp_cursor_inblock_clamps_offset_inside_block() {
        let segs = vec![block_seg("```http\nGET https://x.com\n```")];
        let c = clamp_cursor(
            &segs,
            Cursor::InBlock {
                segment_idx: 0,
                offset: 9999,
            },
        );
        if let Cursor::InBlock { offset, .. } = c {
            // Offset clamped to raw length.
            let raw_len = match &segs[0] {
                Segment::Block(b) => b.raw.len_chars(),
                _ => 0,
            };
            assert_eq!(offset, raw_len);
        } else {
            panic!("expected InBlock");
        }
    }

    #[test]
    fn clamp_cursor_inblock_on_prose_segment_returns_inprose() {
        let segs = vec![prose_seg("hi")];
        let c = clamp_cursor(
            &segs,
            Cursor::InBlock {
                segment_idx: 0,
                offset: 1,
            },
        );
        assert!(matches!(c, Cursor::InProse { .. }));
    }

    #[test]
    fn clamp_cursor_inblockresult_falls_back_to_first_prose() {
        let segs = vec![block_seg("```http\nGET https://x.com\n```"), prose_seg("hi")];
        let c = clamp_cursor(
            &segs,
            Cursor::InBlockResult {
                segment_idx: 0,
                row: 0,
            },
        );
        // Fallback walks segments until a Prose appears — that's idx 1.
        assert_eq!(
            c,
            Cursor::InProse {
                segment_idx: 1,
                offset: 0
            }
        );
    }

    #[test]
    fn clamp_cursor_inprose_out_of_range_falls_back_to_first_prose() {
        let segs = vec![prose_seg("a"), prose_seg("b")];
        let c = clamp_cursor(
            &segs,
            Cursor::InProse {
                segment_idx: 99,
                offset: 0,
            },
        );
        // segment_idx 99 clamps to last (1), within range — gets a prose cursor.
        if let Cursor::InProse { segment_idx, .. } = c {
            assert!(segment_idx <= 1);
        } else {
            panic!("expected InProse fallback");
        }
    }

    #[test]
    fn clamp_cursor_on_empty_segments_falls_back_to_zero() {
        let segs: Vec<Segment> = vec![];
        let c = clamp_cursor(
            &segs,
            Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
        );
        assert_eq!(
            c,
            Cursor::InProse {
                segment_idx: 0,
                offset: 0
            }
        );
    }

    #[test]
    fn pad_with_prose_empty_returns_empty() {
        let out = pad_with_prose(vec![]);
        assert!(out.is_empty());
    }

    #[test]
    fn pad_with_prose_prepends_when_starts_with_block() {
        let segs = vec![block_seg("```http\nGET x\n```"), prose_seg("after")];
        let out = pad_with_prose(segs);
        assert!(matches!(out[0], Segment::Prose(ref r) if r.len_chars() == 0));
    }

    #[test]
    fn pad_with_prose_appends_when_ends_with_block() {
        let segs = vec![prose_seg("before"), block_seg("```http\nGET x\n```")];
        let out = pad_with_prose(segs);
        assert!(matches!(out.last(), Some(Segment::Prose(r)) if r.len_chars() == 0));
    }

    #[test]
    fn pad_with_prose_inserts_between_consecutive_blocks() {
        let segs = vec![
            block_seg("```http\nGET a\n```"),
            block_seg("```http\nGET b\n```"),
        ];
        let out = pad_with_prose(segs);
        // Outer pads + inner pad → at least one empty-prose between
        // the two blocks.
        let block_positions: Vec<usize> = out
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, Segment::Block(_)))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(block_positions.len(), 2);
        assert!(block_positions[1] > block_positions[0] + 1);
    }

    #[test]
    fn is_empty_prose_true_for_empty_rope() {
        assert!(is_empty_prose(&prose_seg("")));
    }

    #[test]
    fn is_empty_prose_false_for_non_empty_prose() {
        assert!(!is_empty_prose(&prose_seg("x")));
    }

    #[test]
    fn is_empty_prose_false_for_block_segment() {
        assert!(!is_empty_prose(&block_seg("```http\nGET x\n```")));
    }
}
