use httui_core::blocks::{parse_blocks, parser::ParsedBlock, serialize_block};
use ropey::Rope;

use crate::buffer::block::{BlockId, BlockNode, ExecutionState};
use crate::buffer::cursor::Cursor;
use crate::buffer::segment::Segment;
use crate::error::TuiResult;
use crate::vim::undo::{Snapshot, UndoStack};

/// In-memory representation of a markdown note, as a flat sequence of
/// typed segments (prose / block). Produced by
/// [`Document::from_markdown`] and rendered back via
/// [`Document::to_markdown`].
///
/// Round-1 mutation API: insert/delete a single char (or newline) at
/// the cursor. Block segments are read-only — calls are no-ops when
/// the cursor sits on a `BlockSelected`. Undo / redo arrive with the
/// vim engine round 3.
#[derive(Debug)]
pub struct Document {
    segments: Vec<Segment>,
    cursor: Cursor,
    next_block_id: u64,
    dirty: bool,
    undo: UndoStack,
}

impl Document {
    /// Parse a markdown string into a segmented document. Prose runs
    /// outside executable fences are kept verbatim in a [`Rope`]; known
    /// block types (http / db-* / e2e, and anything else registered in
    /// the core parser) become [`Segment::Block`].
    pub fn from_markdown(src: &str) -> TuiResult<Self> {
        let parsed = parse_blocks(src);
        let lines: Vec<&str> = src.lines().collect();

        let mut segments: Vec<Segment> = Vec::with_capacity(parsed.len() * 2 + 1);
        let mut next_id = 0u64;
        let mut line_cursor = 0usize;

        for block in &parsed {
            if block.line_start > line_cursor {
                let prose = lines[line_cursor..block.line_start].join("\n");
                if !prose.is_empty() {
                    segments.push(Segment::Prose(Rope::from_str(&prose)));
                }
            }
            // The block's raw markdown — keep ALL its source lines
            // (fence header + body + closer) verbatim so
            // `Cursor::InBlock` edits can mutate the rope directly
            // and re-parse stays lossless.
            let raw_text = lines[block.line_start..=block.line_end].join("\n");
            segments.push(Segment::Block(BlockNode {
                id: BlockId(next_id),
                raw: Rope::from_str(&raw_text),
                block_type: block.block_type.clone(),
                alias: block.alias.clone(),
                display_mode: block.display_mode.clone(),
                params: block.params.clone(),
                state: ExecutionState::Idle,
                cached_result: None,
            }));
            next_id += 1;
            line_cursor = block.line_end + 1;
        }

        if line_cursor < lines.len() {
            let prose = lines[line_cursor..].join("\n");
            if !prose.is_empty() {
                segments.push(Segment::Prose(Rope::from_str(&prose)));
            }
        }

        // Inject empty prose padding so the cursor never gets stranded
        // on a block: prepend before a leading block, append after a
        // trailing block, and slip an empty prose between adjacent
        // blocks. These synthetic empties round-trip cleanly because
        // `to_markdown` skips empty prose runs.
        segments = pad_with_prose(segments);

        let cursor = match segments.first() {
            Some(Segment::Prose(_)) => Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
            Some(Segment::Block(_)) => Cursor::InBlock {
                segment_idx: 0,
                offset: 0,
            },
            None => {
                segments.push(Segment::Prose(Rope::new()));
                Cursor::InProse {
                    segment_idx: 0,
                    offset: 0,
                }
            }
        };

        Ok(Self {
            segments,
            cursor,
            next_block_id: next_id,
            dirty: false,
            undo: UndoStack::new(),
        })
    }

    /// Serialize the document back to markdown. Parse → serialize →
    /// parse yields a semantically-equivalent document (same blocks,
    /// same order, same prose text) but is **not** guaranteed
    /// byte-identical — canonical forms are enforced (e.g. DB info
    /// strings emit `alias → connection → limit → timeout → display`).
    pub fn to_markdown(&self) -> String {
        // Filter out the synthetic empty-prose padding before
        // serializing — those segments only exist for the cursor's
        // benefit and shouldn't bleed into the file on disk.
        let visible: Vec<&Segment> = self
            .segments
            .iter()
            .filter(|s| !is_empty_prose(s))
            .collect();
        let mut out = String::new();
        let last_idx = visible.len().saturating_sub(1);
        for (i, seg) in visible.iter().enumerate() {
            match seg {
                Segment::Prose(r) => out.push_str(&r.to_string()),
                Segment::Block(b) => {
                    let adapter = ParsedBlock {
                        block_type: b.block_type.clone(),
                        alias: b.alias.clone(),
                        display_mode: b.display_mode.clone(),
                        params: b.params.clone(),
                        line_start: 0,
                        line_end: 0,
                    };
                    out.push_str(&serialize_block(&adapter));
                }
            }
            // Separator between segments: one `\n` unless the prior chunk
            // already supplied one. The last segment intentionally has no
            // trailing newline — the prose rope carries any newline the
            // original file had.
            if i < last_idx && !out.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    pub fn block_ids(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.segments.iter().filter_map(|s| match s {
            Segment::Block(b) => Some(b.id),
            _ => None,
        })
    }

    pub fn find_block_by_alias(&self, alias: &str) -> Option<&BlockNode> {
        self.segments.iter().find_map(|s| match s {
            Segment::Block(b) if b.alias.as_deref() == Some(alias) => Some(b),
            _ => None,
        })
    }

    pub fn find_block_by_id(&self, id: BlockId) -> Option<&BlockNode> {
        self.segments.iter().find_map(|s| match s {
            Segment::Block(b) if b.id == id => Some(b),
            _ => None,
        })
    }

    /// Replace the segment at `segment_idx` with `new`. No-op if the
    /// index is out of range. Used by the in-block↔prose swap so the
    /// motion/operator engine can run on the SQL body as if it were
    /// regular prose.
    pub fn replace_segment(&mut self, segment_idx: usize, new: Segment) {
        if let Some(slot) = self.segments.get_mut(segment_idx) {
            *slot = new;
        }
    }

    /// Re-parse the prose segment at `segment_idx` and splice any
    /// blocks the user just finished typing back into the document.
    ///
    /// The CM6 desktop does this on every keystroke via a transaction
    /// filter; we do it on Insert→Normal transitions (cheaper, and the
    /// failure mode of "fence half-typed mid-stream" doesn't pollute
    /// the segment list). Returns `true` when the splice changed
    /// anything — caller doesn't need to clear the dirty flag, this
    /// already does.
    ///
    /// Block IDs are minted off `self.next_block_id` so subsequent
    /// re-parses don't collide. Cursor lands on the first Prose
    /// segment at or after `segment_idx` post-splice — typical flow
    /// (user closed the fence at the bottom of a prose) puts them on
    /// the freshly-created trailing prose ready to type more.
    pub fn reparse_prose_at(&mut self, segment_idx: usize) -> bool {
        let text = match self.segments.get(segment_idx) {
            Some(Segment::Prose(rope)) => rope.to_string(),
            _ => return false,
        };
        let parsed = parse_blocks(&text);
        if parsed.is_empty() {
            return false;
        }

        let lines: Vec<&str> = text.lines().collect();
        let mut new_segs: Vec<Segment> = Vec::with_capacity(parsed.len() * 2 + 1);
        let mut line_cursor = 0usize;
        for block in &parsed {
            if block.line_start > line_cursor {
                let prose = lines[line_cursor..block.line_start].join("\n");
                if !prose.is_empty() {
                    new_segs.push(Segment::Prose(Rope::from_str(&prose)));
                }
            }
            let raw_text = lines[block.line_start..=block.line_end].join("\n");
            new_segs.push(Segment::Block(BlockNode {
                id: BlockId(self.next_block_id),
                raw: Rope::from_str(&raw_text),
                block_type: block.block_type.clone(),
                alias: block.alias.clone(),
                display_mode: block.display_mode.clone(),
                params: block.params.clone(),
                state: ExecutionState::Idle,
                cached_result: None,
            }));
            self.next_block_id += 1;
            line_cursor = block.line_end + 1;
        }
        if line_cursor < lines.len() {
            let prose = lines[line_cursor..].join("\n");
            if !prose.is_empty() {
                new_segs.push(Segment::Prose(Rope::from_str(&prose)));
            }
        }

        // Replace the original prose with the splice; re-pad so we
        // don't leave two adjacent blocks (or a trailing block that
        // strands the cursor) — `pad_with_prose` is idempotent.
        self.segments.splice(segment_idx..segment_idx + 1, new_segs);
        self.segments = pad_with_prose(std::mem::take(&mut self.segments));

        // Cursor: jump to the first Prose at or after `segment_idx`.
        // Typical flow — user just typed the closing fence at the end
        // of a paragraph — leaves them on the trailing empty prose
        // pad_with_prose just appended. Falls back to the last
        // segment if nothing matches (defensive — can't happen in
        // practice because pad_with_prose guarantees a trailing
        // Prose).
        let landing = self
            .segments
            .iter()
            .enumerate()
            .skip(segment_idx)
            .find_map(|(i, seg)| matches!(seg, Segment::Prose(_)).then_some(i))
            .unwrap_or_else(|| self.segments.len().saturating_sub(1));
        self.cursor = match self.segments.get(landing) {
            Some(Segment::Prose(_)) => Cursor::InProse {
                segment_idx: landing,
                offset: 0,
            },
            Some(Segment::Block(_)) => Cursor::InBlock {
                segment_idx: landing,
                offset: 0,
            },
            None => Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
        };
        self.dirty = true;
        true
    }

    /// Yank the block at `segment_idx` as canonical fence markdown,
    /// terminated with `\n`. Returns `None` when the segment isn't a
    /// block. Doesn't mutate the document — used by the linewise
    /// `yy`-on-block path so the user can paste the block somewhere
    /// else and have re-parse rebuild it.
    pub fn yank_block_at(&self, segment_idx: usize) -> Option<String> {
        let Segment::Block(b) = self.segments.get(segment_idx)? else {
            return None;
        };
        let mut text = b.to_fence_markdown();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        Some(text)
    }

    /// Yank + remove the block segment. Adjacent prose runs are
    /// merged so we don't strand empty Prose pads next to each
    /// other. Cursor lands at the start of the merged-or-adjacent
    /// prose. Returns the yanked fence markdown so the caller can
    /// drop it into the unnamed register.
    pub fn delete_block_at(&mut self, segment_idx: usize) -> Option<String> {
        let yanked = self.yank_block_at(segment_idx)?;
        self.segments.remove(segment_idx);

        // Merge a Prose neighbor pair into one segment if removing
        // the block left two Proses adjacent. Keeps the segment
        // list clean and keeps subsequent re-parses cheap (one
        // segment to scan instead of two).
        let prev_is_prose = segment_idx > 0
            && matches!(self.segments.get(segment_idx - 1), Some(Segment::Prose(_)));
        let next_is_prose = matches!(self.segments.get(segment_idx), Some(Segment::Prose(_)));
        let landing_idx = if prev_is_prose && next_is_prose {
            // Take the trailing prose, append it (with a newline
            // separator) to the leading prose, drop the trailing one.
            let trailing = match self.segments.remove(segment_idx) {
                Segment::Prose(r) => r,
                _ => Rope::new(),
            };
            if let Some(Segment::Prose(leading)) = self.segments.get_mut(segment_idx - 1) {
                if leading.len_chars() > 0 && trailing.len_chars() > 0 {
                    leading.append(Rope::from_str("\n"));
                }
                leading.append(trailing);
            }
            segment_idx - 1
        } else if prev_is_prose {
            segment_idx - 1
        } else if next_is_prose {
            segment_idx
        } else {
            // Ended up with a block neighbor on at least one side —
            // re-pad will splice an empty prose; cursor lands on
            // whichever pad is closest to the deletion point.
            segment_idx.min(self.segments.len().saturating_sub(1))
        };

        self.segments = pad_with_prose(std::mem::take(&mut self.segments));
        // Re-pad may have shifted indices; clamp.
        let landing_idx = landing_idx.min(self.segments.len().saturating_sub(1));
        self.cursor = match self.segments.get(landing_idx) {
            Some(Segment::Prose(_)) => Cursor::InProse {
                segment_idx: landing_idx,
                offset: 0,
            },
            _ => Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            },
        };
        self.dirty = true;
        Some(yanked)
    }

    /// Immutable handle to the block at `segment_idx`. Returns `None`
    /// when the segment is prose or out of range. Mirrors
    /// [`block_at_mut`](Self::block_at_mut).
    pub fn block_at(&self, segment_idx: usize) -> Option<&BlockNode> {
        match self.segments.get(segment_idx)? {
            Segment::Block(b) => Some(b),
            _ => None,
        }
    }

    /// Mutable handle to the block at `segment_idx`. Used by the run
    /// dispatcher to flip [`ExecutionState`] and stash `cached_result`.
    pub fn block_at_mut(&mut self, segment_idx: usize) -> Option<&mut BlockNode> {
        match self.segments.get_mut(segment_idx)? {
            Segment::Block(b) => Some(b),
            _ => None,
        }
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, c: Cursor) {
        self.cursor = c;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    /// Force the dirty flag to `true`. Used by edits that swap the
    /// document wholesale (e.g. `:%s/foo/bar/g` re-parses the
    /// markdown and replaces the doc) — those don't go through the
    /// per-keystroke `insert_*` paths that normally set the flag.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

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
                        self.replace_segment(
                            segment_idx,
                            Segment::Prose(Rope::from_str(&text)),
                        );
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

    // ─── undo / redo ───

    /// Capture the current state onto the undo past stack. Called by
    /// the dispatch layer immediately before any undoable command —
    /// `i`/`a`/`o`/`O`, operators that modify (`d`/`c`), paste.
    pub fn snapshot(&mut self) {
        self.undo.push(self.snapshot_of_self());
    }

    /// Restore the most recent past snapshot. Returns `false` when the
    /// stack is empty (nothing to undo).
    pub fn undo(&mut self) -> bool {
        let Some(snap) = self.undo.pop_undo() else {
            return false;
        };
        let current = self.snapshot_of_self();
        self.undo.push_redo(current);
        self.restore(snap);
        true
    }

    /// Pop a redo snapshot (set up by a prior `undo`) and apply it.
    /// Returns `false` if the redo stack is empty.
    pub fn redo(&mut self) -> bool {
        let Some(snap) = self.undo.pop_redo() else {
            return false;
        };
        let current = self.snapshot_of_self();
        self.undo.push_past(current);
        self.restore(snap);
        true
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    /// Translate a `(segment_idx, offset)` pair to a flat char offset
    /// inside `to_markdown()`. Inverse of segment-by-segment
    /// reconstruction — used by cross-segment operators that need to
    /// reason about the doc as one rope.
    ///
    /// Mirrors `to_markdown`'s separator logic: empty-prose padding
    /// is filtered before counting, and a `\n` separator is added
    /// between visible segments unless one already trails. Out-of-
    /// range inputs clamp to the doc's end.
    pub fn global_offset_for(&self, segment_idx: usize, offset: usize) -> usize {
        let mut global = 0usize;
        let last_idx = self
            .segments
            .iter()
            .enumerate()
            .filter(|(_, s)| !is_empty_prose(s))
            .map(|(i, _)| i)
            .next_back()
            .unwrap_or(0);
        let mut emitted_so_far = String::new();
        for (i, seg) in self.segments.iter().enumerate() {
            if is_empty_prose(seg) {
                if i == segment_idx {
                    return global;
                }
                continue;
            }
            let seg_text = match seg {
                Segment::Prose(r) => r.to_string(),
                Segment::Block(b) => {
                    // Mirror `to_markdown`'s serialize_block path so
                    // global offsets line up with the on-disk text.
                    let adapter = ParsedBlock {
                        block_type: b.block_type.clone(),
                        alias: b.alias.clone(),
                        display_mode: b.display_mode.clone(),
                        params: b.params.clone(),
                        line_start: 0,
                        line_end: 0,
                    };
                    httui_core::blocks::serialize_block(&adapter)
                }
            };
            let seg_len = seg_text.chars().count();
            if i == segment_idx {
                return global + offset.min(seg_len);
            }
            global += seg_len;
            emitted_so_far.push_str(&seg_text);
            // Same separator rule as `to_markdown`: one `\n` unless
            // the prior chunk already ended in one.
            if i < last_idx && !emitted_so_far.ends_with('\n') {
                global += 1;
                emitted_so_far.push('\n');
            }
        }
        global
    }

    /// Reparse the document from a fresh markdown string, preserving
    /// the cursor (clamped) plus any cached block state we can recover.
    /// Cached state (`state`, `cached_result`) is keyed by `alias`
    /// across the rebuild — same convention the executor uses to look
    /// up by-alias references.
    ///
    /// Used by cross-segment operators (visual yank/delete spanning
    /// prose + blocks) that round-trip the doc through markdown so
    /// they don't have to teach every operator about every segment
    /// boundary. The undo stack and `next_block_id` counter are
    /// preserved on this side; the new `Document` is folded in.
    pub fn replace_with_text(&mut self, text: &str, new_cursor: Cursor) -> TuiResult<()> {
        // Capture cached state by alias on the way out.
        let mut cached: std::collections::HashMap<
            String,
            (ExecutionState, Option<serde_json::Value>),
        > = std::collections::HashMap::new();
        for seg in &self.segments {
            if let Segment::Block(b) = seg {
                if let Some(alias) = &b.alias {
                    cached.insert(alias.clone(), (b.state.clone(), b.cached_result.clone()));
                }
            }
        }
        let fresh = Document::from_markdown(text)?;
        let mut new_segments = fresh.segments;
        // Restore cached state by alias on the rebuilt blocks. Blocks
        // that lost their alias mid-edit (or had none) start fresh —
        // matches the contract that cached_result lives off the markdown.
        for seg in new_segments.iter_mut() {
            if let Segment::Block(b) = seg {
                if let Some(alias) = b.alias.as_deref() {
                    if let Some((state, result)) = cached.remove(alias) {
                        b.state = state;
                        b.cached_result = result;
                    }
                }
                // Mint fresh IDs from our own counter so we don't
                // collide with previously-minted ones still alive
                // in undo snapshots.
                b.id = BlockId(self.next_block_id);
                self.next_block_id += 1;
            }
        }
        self.segments = new_segments;
        self.cursor = clamp_cursor(&self.segments, new_cursor);
        self.dirty = true;
        Ok(())
    }

    fn snapshot_of_self(&self) -> Snapshot {
        Snapshot {
            segments: self.segments.clone(),
            cursor: self.cursor,
            next_block_id: self.next_block_id,
        }
    }

    fn restore(&mut self, snap: Snapshot) {
        self.segments = snap.segments;
        self.cursor = snap.cursor;
        self.next_block_id = snap.next_block_id;
        // Conservatively flag dirty after any history move — proving the
        // restored state matches disk would require comparing against the
        // last-saved snapshot, which we don't track yet.
        self.dirty = true;
    }
}

/// Clamp a `Cursor` to a fresh segment list — used after
/// `replace_with_text` rebuilds the document. Out-of-range segments
/// fall back to the first prose segment (or segment 0). Offsets
/// past the segment's content clamp to its last valid position.
fn clamp_cursor(segments: &[Segment], cursor: Cursor) -> Cursor {
    let total_segs = segments.len();
    let fallback = || -> Cursor {
        // First Prose segment, offset 0. Always exists thanks to
        // pad_with_prose's invariants.
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
fn pad_with_prose(segments: Vec<Segment>) -> Vec<Segment> {
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

fn is_empty_prose(seg: &Segment) -> bool {
    matches!(seg, Segment::Prose(r) if r.len_chars() == 0)
}

/// Translate a `(line, char_offset_in_line)` pair into an index into
/// the flat char vector. Lines are split by `\n`; offsets past the end
/// of a line clamp at the line's end (just before the newline).
fn chars_index_for_line_col(chars: &[char], line: usize, offset: usize) -> usize {
    let mut current_line = 0usize;
    let mut col = 0usize;
    for (idx, c) in chars.iter().enumerate() {
        if current_line == line {
            if col == offset {
                return idx;
            }
            if *c == '\n' {
                // Asked for an offset past the end of this line —
                // clamp to the position right before the newline.
                return idx;
            }
            col += 1;
        }
        if *c == '\n' {
            if current_line == line {
                return idx;
            }
            current_line += 1;
            col = 0;
        }
    }
    chars.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::body_line_col_to_raw_offset;

    // Fixtures. Each sample is a self-contained markdown doc exercising
    // a distinct topology (block count, surrounding prose, edge placement).

    const EMPTY: &str = "";

    const ONLY_PROSE: &str = "# Title\n\nA paragraph with *emphasis* and a [link](https://x.com).\n\n- item 1\n- item 2\n";

    const ONLY_HTTP: &str = "```http alias=login\n{\"method\":\"POST\",\"url\":\"https://api.test.com/login\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";

    const ONLY_DB: &str = "```db-postgres alias=users connection=prod limit=10 timeout=5000 display=split\nSELECT * FROM users\n```\n";

    const ONLY_E2E: &str = "```e2e alias=flow\n{\"base_url\":\"https://api.test.com\",\"steps\":[{\"name\":\"Health\",\"method\":\"GET\",\"url\":\"/health\"}]}\n```\n";

    const PROSE_BLOCK_PROSE: &str = "# Header\n\nIntro text.\n\n```http alias=h\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n\nOutro text.\n";

    const TWO_BLOCKS_CONSECUTIVE: &str = "```http alias=a\n{\"method\":\"GET\",\"url\":\"https://a.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n```http alias=b\n{\"method\":\"GET\",\"url\":\"https://b.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";

    const COMPLEX: &str = "# API Usage\n\nReport for the last 30 days.\n\n- Bullet 1\n- Bullet 2\n\n```db-postgres alias=q1 connection=prod\nSELECT count(*) FROM events\n```\n\nAfter the query, some notes.\n\n```http alias=api\n{\"method\":\"GET\",\"url\":\"https://x.com/metrics\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n\nFinal line.\n";

    const STARTS_WITH_BLOCK: &str = "```http alias=head\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n\nSome prose after.\n";

    const ENDS_WITH_BLOCK: &str = "Some prose before.\n\n```http alias=tail\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";

    const WITH_NON_EXECUTABLE_FENCE: &str = "Here is JS:\n\n```javascript\nconsole.log(\"hi\");\n```\n\nAnd a real block:\n\n```http alias=x\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";

    // ─── Roundtrip: semantic equivalence ───

    fn assert_semantic_roundtrip(md: &str) {
        let doc = Document::from_markdown(md).unwrap();
        let serialized = doc.to_markdown();
        let reparsed = Document::from_markdown(&serialized).unwrap();

        assert_eq!(
            doc.segment_count(),
            reparsed.segment_count(),
            "segment count differs after roundtrip\nbefore: {:#?}\nafter: {:#?}",
            describe_segments(&doc),
            describe_segments(&reparsed)
        );

        for (a, b) in doc.segments().iter().zip(reparsed.segments().iter()) {
            match (a, b) {
                (Segment::Prose(ra), Segment::Prose(rb)) => {
                    assert_eq!(ra.to_string().trim_end(), rb.to_string().trim_end());
                }
                (Segment::Block(ba), Segment::Block(bb)) => {
                    assert_eq!(ba.block_type, bb.block_type);
                    assert_eq!(ba.alias, bb.alias);
                    assert_eq!(ba.display_mode, bb.display_mode);
                    assert_eq!(ba.params, bb.params);
                }
                _ => panic!("segment kind mismatch"),
            }
        }
    }

    fn describe_segments(doc: &Document) -> Vec<String> {
        doc.segments()
            .iter()
            .map(|s| match s {
                Segment::Prose(r) => format!("Prose({:?})", r.to_string()),
                Segment::Block(b) => format!("Block(type={}, alias={:?})", b.block_type, b.alias),
            })
            .collect()
    }

    #[test]
    fn roundtrip_empty() {
        let doc = Document::from_markdown(EMPTY).unwrap();
        // Empty input gets a single empty prose so cursor has somewhere to live.
        assert_eq!(doc.segment_count(), 1);
        assert!(doc.segments()[0].is_prose());
    }

    #[test]
    fn roundtrip_only_prose() {
        assert_semantic_roundtrip(ONLY_PROSE);
    }

    #[test]
    fn roundtrip_only_http() {
        assert_semantic_roundtrip(ONLY_HTTP);
    }

    #[test]
    fn roundtrip_only_db() {
        assert_semantic_roundtrip(ONLY_DB);
    }

    #[test]
    fn roundtrip_only_e2e() {
        assert_semantic_roundtrip(ONLY_E2E);
    }

    #[test]
    fn roundtrip_prose_block_prose() {
        assert_semantic_roundtrip(PROSE_BLOCK_PROSE);
    }

    #[test]
    fn roundtrip_two_blocks_consecutive() {
        assert_semantic_roundtrip(TWO_BLOCKS_CONSECUTIVE);
    }

    #[test]
    fn roundtrip_complex() {
        assert_semantic_roundtrip(COMPLEX);
    }

    #[test]
    fn roundtrip_starts_with_block() {
        assert_semantic_roundtrip(STARTS_WITH_BLOCK);
    }

    #[test]
    fn roundtrip_ends_with_block() {
        assert_semantic_roundtrip(ENDS_WITH_BLOCK);
    }

    #[test]
    fn roundtrip_with_non_executable_fence() {
        assert_semantic_roundtrip(WITH_NON_EXECUTABLE_FENCE);
    }

    // ─── Idempotency ───

    #[test]
    fn double_serialize_converges() {
        for md in [
            ONLY_PROSE,
            ONLY_HTTP,
            ONLY_DB,
            PROSE_BLOCK_PROSE,
            COMPLEX,
            STARTS_WITH_BLOCK,
            TWO_BLOCKS_CONSECUTIVE,
        ] {
            let s1 = Document::from_markdown(md).unwrap().to_markdown();
            let s2 = Document::from_markdown(&s1).unwrap().to_markdown();
            assert_eq!(s1, s2, "second serialization must match first");
        }
    }

    // ─── Cursor defaults ───

    #[test]
    fn cursor_starts_in_prose_when_doc_starts_with_prose() {
        let doc = Document::from_markdown(ONLY_PROSE).unwrap();
        assert_eq!(
            doc.cursor(),
            Cursor::InProse {
                segment_idx: 0,
                offset: 0
            }
        );
    }

    #[test]
    fn cursor_starts_in_prose_padding_when_doc_starts_with_block() {
        // The parser injects an empty prose segment ahead of any leading
        // block so the user has somewhere to type when they land on the
        // file. The block then sits at segment index 1.
        let doc = Document::from_markdown(ONLY_HTTP).unwrap();
        assert_eq!(
            doc.cursor(),
            Cursor::InProse {
                segment_idx: 0,
                offset: 0,
            }
        );
        assert!(doc.segments()[0].is_prose());
        assert!(doc.segments()[1].is_block());
    }

    #[test]
    fn cursor_starts_in_prose_for_empty_doc() {
        let doc = Document::from_markdown(EMPTY).unwrap();
        assert_eq!(
            doc.cursor(),
            Cursor::InProse {
                segment_idx: 0,
                offset: 0
            }
        );
    }

    /// Helper for tests: build an `InBlock` cursor at body `(line, col)`
    /// for the block at `segment_idx`. Mirrors what production code uses
    /// via `body_line_col_to_raw_offset`.
    fn cursor_in_body(doc: &Document, segment_idx: usize, line: usize, col: usize) -> Cursor {
        let raw = match doc.segments().get(segment_idx) {
            Some(Segment::Block(b)) => &b.raw,
            _ => panic!("expected block segment at {segment_idx}"),
        };
        Cursor::InBlock {
            segment_idx,
            offset: body_line_col_to_raw_offset(raw, line, col),
        }
    }

    #[test]
    fn set_cursor_persists() {
        let mut doc = Document::from_markdown(COMPLEX).unwrap();
        let target = cursor_in_body(&doc, 1, 0, 0);
        doc.set_cursor(target);
        assert_eq!(doc.cursor(), target);
    }

    #[test]
    fn insert_char_in_block_appends_to_query() {
        let md = "# t\n\n```db-sqlite alias=q\nSELECT 1\n```\n";
        let mut doc = Document::from_markdown(md).unwrap();
        // Find the block segment.
        let block_idx = doc.segments().iter().position(|s| s.is_block()).unwrap();
        // Park cursor at end of first SQL line ("SELECT 1" has 8
        // chars, so col 8 is EOL).
        doc.set_cursor(cursor_in_body(&doc, block_idx, 0, 8));
        doc.insert_char_at_cursor('!');
        let query = doc.segments()[block_idx]
            .as_block()
            .unwrap()
            .params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();
        assert_eq!(query, "SELECT 1!");
        // After inserting one char the cursor advances one column —
        // body (line 0, col 9) under the new model.
        let raw = doc.segments()[block_idx].as_block().unwrap().raw.clone();
        assert_eq!(
            doc.cursor(),
            Cursor::InBlock {
                segment_idx: block_idx,
                offset: body_line_col_to_raw_offset(&raw, 0, 9),
            }
        );
    }

    #[test]
    fn newline_in_block_splits_line() {
        let md = "# t\n\n```db-sqlite alias=q\nSELECT 1\n```\n";
        let mut doc = Document::from_markdown(md).unwrap();
        let block_idx = doc.segments().iter().position(|s| s.is_block()).unwrap();
        doc.set_cursor(cursor_in_body(&doc, block_idx, 0, 6));
        doc.insert_newline_at_cursor();
        let query = doc.segments()[block_idx]
            .as_block()
            .unwrap()
            .params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();
        assert_eq!(query, "SELECT\n 1");
        let raw = doc.segments()[block_idx].as_block().unwrap().raw.clone();
        assert_eq!(
            doc.cursor(),
            Cursor::InBlock {
                segment_idx: block_idx,
                offset: body_line_col_to_raw_offset(&raw, 1, 0),
            }
        );
    }

    #[test]
    fn backspace_in_block_at_col_zero_joins_lines() {
        let md = "# t\n\n```db-sqlite alias=q\nA\nB\n```\n";
        let mut doc = Document::from_markdown(md).unwrap();
        let block_idx = doc.segments().iter().position(|s| s.is_block()).unwrap();
        doc.set_cursor(cursor_in_body(&doc, block_idx, 1, 0));
        doc.delete_char_before_cursor();
        let query = doc.segments()[block_idx]
            .as_block()
            .unwrap()
            .params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();
        assert_eq!(query, "AB");
        let raw = doc.segments()[block_idx].as_block().unwrap().raw.clone();
        assert_eq!(
            doc.cursor(),
            Cursor::InBlock {
                segment_idx: block_idx,
                offset: body_line_col_to_raw_offset(&raw, 0, 1),
            }
        );
    }

    // ─── Stable IDs ───

    #[test]
    fn block_ids_are_sequential() {
        let doc = Document::from_markdown(COMPLEX).unwrap();
        let ids: Vec<u64> = doc.block_ids().map(|b| b.0).collect();
        assert_eq!(ids, vec![0, 1]);
    }

    #[test]
    fn block_ids_are_unique() {
        let doc = Document::from_markdown(TWO_BLOCKS_CONSECUTIVE).unwrap();
        let ids: Vec<u64> = doc.block_ids().map(|b| b.0).collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]);
    }

    #[test]
    fn find_block_by_id_returns_right_block() {
        let doc = Document::from_markdown(COMPLEX).unwrap();
        let ids: Vec<BlockId> = doc.block_ids().collect();
        let first = doc.find_block_by_id(ids[0]).unwrap();
        assert_eq!(first.alias.as_deref(), Some("q1"));
        let second = doc.find_block_by_id(ids[1]).unwrap();
        assert_eq!(second.alias.as_deref(), Some("api"));
    }

    #[test]
    fn find_block_by_id_rejects_unknown_id() {
        let doc = Document::from_markdown(COMPLEX).unwrap();
        assert!(doc.find_block_by_id(BlockId(999)).is_none());
    }

    // ─── find_block_by_alias ───

    #[test]
    fn find_block_by_alias_finds_match() {
        let doc = Document::from_markdown(COMPLEX).unwrap();
        let b = doc.find_block_by_alias("api").unwrap();
        assert!(b.is_http());
    }

    #[test]
    fn find_block_by_alias_returns_none_for_unknown() {
        let doc = Document::from_markdown(COMPLEX).unwrap();
        assert!(doc.find_block_by_alias("missing").is_none());
    }

    #[test]
    fn find_block_by_alias_skips_blocks_without_alias() {
        let md = "```http\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        let doc = Document::from_markdown(md).unwrap();
        assert!(doc.find_block_by_alias("").is_none());
    }

    // ─── Segment topology edge cases ───

    #[test]
    fn non_executable_fence_stays_in_prose() {
        let doc = Document::from_markdown(WITH_NON_EXECUTABLE_FENCE).unwrap();
        // The javascript fence must be inside a prose segment; only the
        // http block is counted.
        let blocks: Vec<&BlockNode> = doc.segments().iter().filter_map(|s| s.as_block()).collect();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].is_http());

        let prose_concat: String = doc
            .segments()
            .iter()
            .filter_map(|s| s.as_prose())
            .map(|r| r.to_string())
            .collect();
        assert!(prose_concat.contains("```javascript"));
    }

    #[test]
    fn starts_with_block_pads_prose_before_it() {
        // Padding empty prose so the cursor has a landing zone above
        // the block. The block then lives at index 1.
        let doc = Document::from_markdown(STARTS_WITH_BLOCK).unwrap();
        assert!(doc.segments()[0].is_prose());
        assert!(doc.segments()[1].is_block());
    }

    #[test]
    fn ends_with_block_pads_prose_after_it() {
        // Padding empty prose so `j` can land below a trailing block.
        let doc = Document::from_markdown(ENDS_WITH_BLOCK).unwrap();
        let last = doc.segments().last().unwrap();
        assert!(last.is_prose());
        assert!(last.as_prose().unwrap().len_chars() == 0);
    }

    #[test]
    fn two_consecutive_blocks_yield_two_block_segments() {
        let doc = Document::from_markdown(TWO_BLOCKS_CONSECUTIVE).unwrap();
        let blocks = doc.segments().iter().filter(|s| s.is_block()).count();
        assert_eq!(blocks, 2);
    }

    #[test]
    fn execution_state_defaults_to_idle() {
        let doc = Document::from_markdown(COMPLEX).unwrap();
        for seg in doc.segments() {
            if let Segment::Block(b) = seg {
                assert_eq!(b.state, ExecutionState::Idle);
                assert!(b.cached_result.is_none());
            }
        }
    }

    // ─── undo / redo ───

    #[test]
    fn undo_restores_pre_edit_state() {
        let mut d = Document::from_markdown("hello\n").unwrap();
        d.snapshot();
        d.insert_char_at_cursor('X');
        assert_eq!(d.text_in_segment_range(0, 0, 6), "Xhello");
        assert!(d.undo());
        assert_eq!(d.text_in_segment_range(0, 0, 5), "hello");
    }

    #[test]
    fn redo_reapplies_undone_change() {
        let mut d = Document::from_markdown("hello\n").unwrap();
        d.snapshot();
        d.insert_char_at_cursor('X');
        d.undo();
        assert!(d.redo());
        assert_eq!(d.text_in_segment_range(0, 0, 6), "Xhello");
    }

    #[test]
    fn fresh_doc_cannot_undo() {
        let d = Document::from_markdown("hi\n").unwrap();
        assert!(!d.can_undo());
        assert!(!d.can_redo());
    }

    #[test]
    fn new_snapshot_clears_redo_stack() {
        let mut d = Document::from_markdown("hello\n").unwrap();
        d.snapshot();
        d.insert_char_at_cursor('A');
        d.undo();
        assert!(d.can_redo());
        // A new edit invalidates the redo branch.
        d.snapshot();
        d.insert_char_at_cursor('B');
        assert!(!d.can_redo());
    }

    // ─── reparse_prose_at (Story: live block authoring) ───

    #[test]
    fn reparse_prose_promotes_typed_fence_into_block() {
        // User starts with a plain prose paragraph and types a fence
        // inside Insert mode; on Esc we re-parse that segment and the
        // fence becomes a real `Segment::Block`.
        let mut d =
            Document::from_markdown("hello\n\n```db-postgres alias=q\nSELECT 1\n```\n").unwrap();
        // Pretend the whole document was one prose run that just got
        // typed (the constructor would already have split it; force
        // the test condition by collapsing).
        d.segments = vec![Segment::Prose(Rope::from_str(
            "hello\n\n```db-postgres alias=q\nSELECT 1\n```\n",
        ))];
        let changed = d.reparse_prose_at(0);
        assert!(changed, "fence should have been detected");
        let block_count = d
            .segments
            .iter()
            .filter(|s| matches!(s, Segment::Block(_)))
            .count();
        assert_eq!(block_count, 1, "exactly one block should appear");
        // Cursor lands on a Prose (the trailing pad / continuation),
        // not stranded on the new Block.
        assert!(matches!(d.cursor, Cursor::InProse { .. }));
    }

    #[test]
    fn reparse_prose_is_noop_when_no_fence_present() {
        // Plain prose with no triple-backticks → re-parse changes
        // nothing; no churn on every Insert exit.
        let mut d = Document::from_markdown("just words\n").unwrap();
        let segs_before = d.segments.len();
        let changed = d.reparse_prose_at(0);
        assert!(!changed);
        assert_eq!(d.segments.len(), segs_before);
    }

    #[test]
    fn yank_block_at_returns_canonical_fence_markdown() {
        // The yanked text should round-trip through `parse_blocks` to
        // the same logical block — that's the contract paste relies
        // on. Ends with `\n` so paste's linewise semantics insert a
        // clean line.
        let d = Document::from_markdown("```db-postgres alias=q\nSELECT 1\n```\n").unwrap();
        let blk_idx = d
            .segments
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        let yanked = d.yank_block_at(blk_idx).unwrap();
        assert!(yanked.starts_with("```db-postgres"), "got: {yanked}");
        assert!(yanked.contains("alias=q"));
        assert!(yanked.contains("SELECT 1"));
        assert!(yanked.ends_with('\n'));
        // Round-trip: parse the yanked text → exactly one block.
        let reparsed = httui_core::blocks::parse_blocks(&yanked);
        assert_eq!(reparsed.len(), 1);
        assert_eq!(reparsed[0].alias.as_deref(), Some("q"));
    }

    #[test]
    fn yank_block_at_returns_none_for_prose_segment() {
        // Defensive — caller is supposed to gate on cursor being
        // `InBlock`/`InBlockResult`, but the helper still no-ops
        // gracefully if asked about a prose segment.
        let d = Document::from_markdown("hello\n").unwrap();
        assert!(d.yank_block_at(0).is_none());
    }

    #[test]
    fn delete_block_at_removes_segment_and_yanks_text() {
        // After delete the document has no Block segments and the
        // returned yank carries the original fence text. The two
        // surrounding prose runs (one is the synthetic empty pad)
        // collapse into one merged Prose so the segment list stays
        // clean.
        let mut d =
            Document::from_markdown("before\n\n```db-postgres alias=q\nSELECT 1\n```\n\nafter\n")
                .unwrap();
        let blk_idx = d
            .segments
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        let yanked = d.delete_block_at(blk_idx).expect("yanked something");
        assert!(yanked.contains("alias=q"));
        assert!(d.segments.iter().all(|s| !matches!(s, Segment::Block(_))));
        // Document text now has both `before` and `after` prose
        // contents reachable.
        let text: String = d
            .segments
            .iter()
            .filter_map(|s| match s {
                Segment::Prose(r) => Some(r.to_string()),
                _ => None,
            })
            .collect();
        assert!(text.contains("before"));
        assert!(text.contains("after"));
    }

    #[test]
    fn delete_then_paste_round_trips_block() {
        // Cut → paste cycle: the fence text yanked from `delete_block_at`
        // can be inserted into another prose and a re-parse rebuilds
        // the block. This is the cut/paste-to-move-block flow.
        let mut d =
            Document::from_markdown("head\n\n```db-postgres alias=q\nSELECT 1\n```\n\ntail\n")
                .unwrap();
        let blk_idx = d
            .segments
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        let yanked = d.delete_block_at(blk_idx).unwrap();
        // Pretend we pasted the yanked text into the only remaining
        // prose, then reparsed (this is what `apply_paste` does).
        let target_idx = d
            .segments
            .iter()
            .position(|s| matches!(s, Segment::Prose(_)))
            .unwrap();
        if let Some(Segment::Prose(rope)) = d.segments.get_mut(target_idx) {
            rope.append(Rope::from_str(&format!("\n{yanked}")));
        }
        assert!(d.reparse_prose_at(target_idx));
        assert_eq!(
            d.segments
                .iter()
                .filter(|s| matches!(s, Segment::Block(_)))
                .count(),
            1,
            "block reappears after paste + reparse"
        );
    }

    #[test]
    fn reparse_prose_assigns_fresh_block_ids() {
        // Newly-spliced blocks must not collide with IDs the document
        // already minted — `next_block_id` is bumped per insert.
        let mut d =
            Document::from_markdown("```db-postgres alias=existing\nSELECT 1\n```\n\n").unwrap();
        let existing_id = match d.segments.iter().find(|s| matches!(s, Segment::Block(_))) {
            Some(Segment::Block(b)) => b.id,
            _ => panic!("expected a block"),
        };
        // Append a new fence to the trailing prose and re-parse.
        let trailing_idx = d.segments.len() - 1;
        if let Some(Segment::Prose(rope)) = d.segments.get_mut(trailing_idx) {
            rope.append(Rope::from_str("```db-mysql alias=fresh\nSELECT 2\n```\n"));
        }
        assert!(d.reparse_prose_at(trailing_idx));
        let new_ids: Vec<BlockId> = d
            .segments
            .iter()
            .filter_map(|s| match s {
                Segment::Block(b) => Some(b.id),
                _ => None,
            })
            .collect();
        assert!(new_ids.contains(&existing_id), "existing block kept its ID");
        assert!(
            new_ids.iter().all(|id| std::iter::once(id).count() == 1),
            "no duplicate IDs"
        );
    }

    #[test]
    fn insert_into_fence_opener_dissolves_block_to_prose() {
        let mut d = Document::from_markdown(ONLY_HTTP).expect("parse");
        let block_idx = d
            .segments
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        d.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: 0,
        });
        d.insert_char_at_cursor('x');
        assert!(
            matches!(d.segments.get(block_idx), Some(Segment::Prose(_))),
            "block must dissolve to prose when fence opener breaks; got {:?}",
            d.segments.get(block_idx),
        );
        match d.cursor {
            Cursor::InProse { segment_idx, .. } => {
                assert_eq!(segment_idx, block_idx);
            }
            other => panic!("cursor must be InProse after dissolve; got {other:?}"),
        }
    }

    #[test]
    fn delete_inside_fence_opener_dissolves_block_to_prose() {
        let mut d = Document::from_markdown(ONLY_HTTP).expect("parse");
        let block_idx = d
            .segments
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        d.set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: 1,
        });
        d.delete_char_before_cursor();
        assert!(
            matches!(d.segments.get(block_idx), Some(Segment::Prose(_))),
            "block must dissolve to prose when fence opener breaks"
        );
    }
}
