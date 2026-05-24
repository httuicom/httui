use ropey::Rope;
use serde_json::Value;

/// Where in a block's `raw` rope the cursor sits.
///
/// A block's `raw` text is `\`\`\`<info>\n<body>\n\`\`\`` — three
/// regions the cursor model needs to discriminate so motions, render,
/// and edits can act differently on each. `Body { line, col }` indexes
/// the body sub-region (line 0 = first body line); `Header` is the
/// info-string row at the top; `Closer` is the trailing ` \`\`\` ` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawSection {
    Header,
    Body { line: usize, col: usize },
    Closer,
}

/// Resolve which section of the block's raw rope `offset` falls into,
/// plus the line / column inside the body when applicable. Out-of-range
/// offsets clamp to the last line.
///
/// Convention: a 1-line raw (degenerate, only opener) treats every
/// offset as `Header`; a 2-line raw treats line 0 as `Header` and line
/// 1 as `Closer`. The common 3+ line shape (header + body lines +
/// closer) is what callers actually see.
pub fn raw_section_at(raw: &Rope, offset: usize) -> RawSection {
    let total = raw.len_chars();
    let off = offset.min(total);
    let raw_lines = raw.len_lines();
    if raw_lines == 0 {
        return RawSection::Header;
    }
    let line = raw.char_to_line(off);
    let line_start = raw.line_to_char(line);
    let col = off.saturating_sub(line_start);

    // Last visible line is the closer when there are at least 2 lines.
    // `len_lines` overcounts by 1 when the rope ends with `\n` — strip
    // that virtual trailing line so the closer is the LAST line with
    // content, not the synthetic empty one.
    let visible_lines =
        if raw.len_chars() > 0 && raw.char(raw.len_chars().saturating_sub(1)) == '\n' {
            raw_lines.saturating_sub(1)
        } else {
            raw_lines
        };
    if line == 0 {
        RawSection::Header
    } else if visible_lines >= 2 && line + 1 >= visible_lines {
        RawSection::Closer
    } else {
        RawSection::Body {
            line: line - 1,
            col,
        }
    }
}

/// Char offset at the start of body line `body_line` (0-indexed within
/// body) inside `raw`. Body line 0 is raw line 1 (right after the
/// fence header). Out-of-range body lines clamp to the last body line.
pub fn body_line_to_raw_offset(raw: &Rope, body_line: usize) -> usize {
    let raw_line = body_line.saturating_add(1);
    let total_lines = raw.len_lines();
    let line = raw_line.min(total_lines.saturating_sub(1));
    raw.line_to_char(line)
}

/// Number of body lines in the block's raw rope (raw lines minus the
/// fence header and closer). Returns 0 if the raw is degenerate
/// (header only or header + closer with no body).
pub fn body_line_count(raw: &Rope) -> usize {
    let raw_lines = raw.len_lines();
    let visible = if raw.len_chars() > 0 && raw.char(raw.len_chars().saturating_sub(1)) == '\n' {
        raw_lines.saturating_sub(1)
    } else {
        raw_lines
    };
    visible.saturating_sub(2)
}

/// Char offset where the closer line begins. For 1-line raws the
/// closer is conceptually absent — returns the rope's length.
pub fn closer_raw_offset(raw: &Rope) -> usize {
    let raw_lines = raw.len_lines();
    let visible = if raw.len_chars() > 0 && raw.char(raw.len_chars().saturating_sub(1)) == '\n' {
        raw_lines.saturating_sub(1)
    } else {
        raw_lines
    };
    if visible <= 1 {
        return raw.len_chars();
    }
    raw.line_to_char(visible - 1)
}

/// Convert a body `(line, col)` pair into a char offset on the raw
/// rope. Lines past the end of the body clamp to the last body line;
/// columns past EOL clamp to the line's end (just before the trailing
/// newline).
pub fn body_line_col_to_raw_offset(raw: &Rope, body_line: usize, col: usize) -> usize {
    let line_start = body_line_to_raw_offset(raw, body_line);
    let line_idx = raw.char_to_line(line_start);
    let next_start = if line_idx + 1 < raw.len_lines() {
        raw.line_to_char(line_idx + 1)
    } else {
        raw.len_chars()
    };
    // Stop just before the trailing newline so we never hand a caller
    // an offset that lands on `\n` itself.
    let line_end =
        if next_start > line_start && raw.get_char(next_start.saturating_sub(1)) == Some('\n') {
            next_start.saturating_sub(1)
        } else {
            next_start
        };
    line_start.saturating_add(col).min(line_end)
}

/// Char offset at the start of the fence header. Always 0 — the
/// header is the first line of the raw rope. Provided as a named
/// helper so callers don't sprinkle magic zeros.
pub fn header_raw_offset() -> usize {
    0
}

/// Extract the body of a block's raw rope as a `String` — everything
/// between the fence header (line 0) and the closer line, excluding
/// both. Returns an empty string for degenerate raws (no body lines).
/// Used by the completion popup for any block kind that doesn't keep
/// a parsed copy of the body in `params` (HTTP, raw fences, etc.).
pub fn body_text(raw: &Rope) -> String {
    let body_lines = body_line_count(raw);
    if body_lines == 0 {
        return String::new();
    }
    let start = body_line_to_raw_offset(raw, 0);
    let end = closer_raw_offset(raw);
    raw.slice(start..end).to_string()
}

/// Document-scoped identifier for a block node.
///
/// Stable across mutations for the lifetime of the [`Document`][doc] that
/// minted it. Not persisted on disk — blocks are identified on-disk by
/// their hashed content (see `httui_core::block_results`).
///
/// [doc]: crate::buffer::Document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u64);

/// Runtime execution state of a block.
///
/// Drives UI affordances (color, spinner, error banner) and gates ops
/// like re-run / cancel. Transitions:
/// - `Idle` → `Running` (user hits run)
/// - `Cached` → `Running` (explicit re-run ignores cache)
/// - `Running` → `Success | Error(_)` (executor returns)
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionState {
    Idle,
    Cached,
    Running,
    Success,
    Error(String),
}

/// A parsed executable block, stored inline inside a [`Segment::Block`].
///
/// Mirrors `httui_core::blocks::ParsedBlock` plus TUI-only fields
/// ([`id`](Self::id), [`state`](Self::state), [`cached_result`](Self::cached_result)).
/// `block_type` is a free-form string so new types (`graphql`, `grpc`, …)
/// plug in via `BlockTypeRegistry` without editing this struct.
#[derive(Debug, Clone)]
pub struct BlockNode {
    pub id: BlockId,
    /// The block's raw markdown — `\`\`\`<info>` line + body lines +
    /// `\`\`\`` closer. This is the source of truth: editing it (via
    /// `Cursor::InBlock`) is equivalent to editing prose, and the
    /// derived fields below (`block_type`, `alias`, `display_mode`,
    /// `params`) are kept in sync via `reparse_from_raw`. Cached
    /// state (`state`, `cached_result`) survives re-parses because
    /// they're keyed on `id`, not on text.
    pub raw: Rope,
    pub block_type: String,
    pub alias: Option<String>,
    pub display_mode: Option<String>,
    pub params: Value,
    pub state: ExecutionState,
    pub cached_result: Option<Value>,
}

impl BlockNode {
    pub fn is_db(&self) -> bool {
        self.block_type == "db" || self.block_type.starts_with("db-")
    }

    pub fn is_http(&self) -> bool {
        self.block_type == "http"
    }

    pub fn is_e2e(&self) -> bool {
        self.block_type == "e2e"
    }

    /// Re-derive `block_type` / `alias` / `display_mode` / `params`
    /// from the current `raw` rope. Preserves `id`, `state`, and
    /// `cached_result` — those live on the BlockNode, not in the
    /// markdown, and survive every edit.
    ///
    /// When `raw` parses cleanly to exactly one block, the derived
    /// fields are replaced wholesale. When it parses to zero blocks
    /// (e.g. user is mid-typing a fence and the closer is missing),
    /// the derived fields stay at their last-good values — the user
    /// can keep editing `raw` and the moment the fence becomes valid
    /// again the next `reparse_from_raw` picks the new values up. We
    /// deliberately do NOT dissolve the segment back into prose here:
    /// that would surprise the user mid-edit and would lose the
    /// block's `id` / `cached_result`.
    ///
    /// Returns `true` when a valid re-parse updated the derived
    /// fields; `false` when the rope is currently malformed.
    pub fn reparse_from_raw(&mut self) -> bool {
        let text = self.raw.to_string();
        let parsed = httui_core::blocks::parse_blocks(&text);
        // A clean single-block parse is the only state where it's
        // safe to replace the derived fields. The "zero blocks"
        // branch keeps last-good fields so cached_result lookups
        // and run gating stay sane while the user types.
        let Some(p) = parsed.first() else {
            return false;
        };
        if parsed.len() != 1 {
            return false;
        }
        self.block_type = p.block_type.clone();
        self.alias = p.alias.clone();
        self.display_mode = p.display_mode.clone();
        self.params = p.params.clone();
        true
    }

    /// Round-trip the block back to its canonical fence markdown.
    /// Bridges to `httui_core::blocks::serialize_block` by stuffing
    /// the BlockNode's fields into a synthetic `ParsedBlock` (line
    /// numbers stubbed — the serializer doesn't read them). Used by
    /// the cut/yank path to produce register text that, when pasted
    /// into prose and re-parsed, recreates the block faithfully.
    pub fn to_fence_markdown(&self) -> String {
        let parsed = httui_core::blocks::parser::ParsedBlock {
            block_type: self.block_type.clone(),
            alias: self.alias.clone(),
            display_mode: self.display_mode.clone(),
            params: self.params.clone(),
            line_start: 0,
            line_end: 0,
        };
        httui_core::blocks::serialize_block(&parsed)
    }

    /// Resolve the block's *effective* display mode for the renderer.
    /// Honors the explicit `display_mode` token from the fence when
    /// present; otherwise falls back to "input" while idle (no result
    /// to show) and "split" once the block has produced one. Mirrors
    /// desktop's behavior so the same vault opens the same way in
    /// both apps.
    pub fn effective_display_mode(&self) -> DisplayMode {
        if let Some(m) = self.display_mode.as_deref().and_then(DisplayMode::parse) {
            return m;
        }
        if self.cached_result.is_some() {
            DisplayMode::Split
        } else {
            DisplayMode::Input
        }
    }
}

/// Which sections of a block render inside its card.
///
/// - `Input` — only the editable body (SQL for DB, request line for HTTP).
/// - `Output` — only the result panel (status + table / messages / plan).
/// - `Split` — both, stacked.
///
/// Persisted as a fence token (`display=input|output|split`) by
/// `httui_core::blocks::serializer`. `BlockNode::display_mode` stays an
/// `Option<String>` to keep the parser/serializer roundtrip lossless;
/// this enum is the typed view callers actually want.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Input,
    Output,
    Split,
}

impl DisplayMode {
    /// Wire format — same string the fence carries and that
    /// `httui_core::blocks::parser` reads back.
    pub fn as_str(self) -> &'static str {
        match self {
            DisplayMode::Input => "input",
            DisplayMode::Output => "output",
            DisplayMode::Split => "split",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "input" => Some(Self::Input),
            "output" => Some(Self::Output),
            "split" => Some(Self::Split),
            _ => None,
        }
    }

    /// Cycle order used by the `gd` keymap: Input → Split → Output → Input.
    /// Split sits in the middle so the most-useful modes (Input alone and
    /// Output alone) are always one keystroke apart through Split.
    pub fn next(self) -> Self {
        match self {
            DisplayMode::Input => DisplayMode::Split,
            DisplayMode::Split => DisplayMode::Output,
            DisplayMode::Output => DisplayMode::Input,
        }
    }

    pub fn shows_input(self) -> bool {
        matches!(self, DisplayMode::Input | DisplayMode::Split)
    }

    pub fn shows_output(self) -> bool {
        matches!(self, DisplayMode::Output | DisplayMode::Split)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn block(ty: &str) -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: Rope::new(),
            block_type: ty.into(),
            alias: None,
            display_mode: None,
            params: json!({}),
            state: ExecutionState::Idle,
            cached_result: None,
        }
    }

    #[test]
    fn category_helpers_match_canonical_types() {
        assert!(block("http").is_http());
        assert!(block("e2e").is_e2e());
        assert!(block("db").is_db());
        assert!(block("db-postgres").is_db());
        assert!(block("db-mysql").is_db());
        assert!(block("db-sqlite").is_db());
    }

    #[test]
    fn category_helpers_reject_unrelated_types() {
        let g = block("graphql");
        assert!(!g.is_http());
        assert!(!g.is_e2e());
        assert!(!g.is_db());
    }

    #[test]
    fn display_mode_roundtrip_through_wire_format() {
        for m in [DisplayMode::Input, DisplayMode::Output, DisplayMode::Split] {
            assert_eq!(DisplayMode::parse(m.as_str()), Some(m));
        }
        assert_eq!(DisplayMode::parse("anything-else"), None);
    }

    #[test]
    fn display_mode_cycle_visits_each_then_repeats() {
        // Input → Split → Output → Input. Three presses of `gd` from
        // any starting point land back where you began.
        let mut m = DisplayMode::Input;
        let visited: Vec<DisplayMode> = std::iter::from_fn(|| {
            m = m.next();
            Some(m)
        })
        .take(3)
        .collect();
        assert_eq!(
            visited,
            vec![DisplayMode::Split, DisplayMode::Output, DisplayMode::Input]
        );
    }

    #[test]
    fn effective_mode_defaults_to_input_when_idle() {
        // No explicit `display=` and no cached result yet — render
        // should hide the (empty) result panel and show only the
        // editable body. Same as desktop's default.
        let mut b = block("db-postgres");
        assert_eq!(b.effective_display_mode(), DisplayMode::Input);
        b.cached_result = Some(json!({ "results": [] }));
        // Producing a result flips the default to Split so the user
        // can see what they ran *and* what came back.
        assert_eq!(b.effective_display_mode(), DisplayMode::Split);
    }

    #[test]
    fn effective_mode_honors_explicit_token() {
        // An explicit `display=output` wins over the contextual
        // default — that's the whole point of persisting the choice.
        let mut b = block("db-postgres");
        b.display_mode = Some("output".into());
        assert_eq!(b.effective_display_mode(), DisplayMode::Output);
        b.display_mode = Some("garbage".into());
        // Unknown token → fall through to the contextual default,
        // not panic.
        assert_eq!(b.effective_display_mode(), DisplayMode::Input);
    }

    // ─── raw-rope section helpers ───

    fn raw_for(text: &str) -> Rope {
        Rope::from_str(text)
    }

    #[test]
    fn raw_section_at_classifies_header_body_closer() {
        // ```http alias=q\nGET /\nHEADERS\n```\n
        // line 0: "```http alias=q"
        // line 1: "GET /"           (body line 0)
        // line 2: "HEADERS"         (body line 1)
        // line 3: "```"             (closer)
        let raw = raw_for("```http alias=q\nGET /\nHEADERS\n```\n");
        // Offset 0 → header.
        assert_eq!(raw_section_at(&raw, 0), RawSection::Header);
        // Mid-header still header.
        assert_eq!(raw_section_at(&raw, 5), RawSection::Header);
        // First body line, col 0.
        let body0 = body_line_to_raw_offset(&raw, 0);
        assert_eq!(
            raw_section_at(&raw, body0),
            RawSection::Body { line: 0, col: 0 }
        );
        // Mid-body line 1.
        let body1 = body_line_to_raw_offset(&raw, 1);
        assert_eq!(
            raw_section_at(&raw, body1 + 3),
            RawSection::Body { line: 1, col: 3 }
        );
        // Closer.
        let closer = closer_raw_offset(&raw);
        assert_eq!(raw_section_at(&raw, closer), RawSection::Closer);
    }

    #[test]
    fn body_line_count_excludes_fence_lines() {
        let raw = raw_for("```db-postgres\nSELECT 1\nFROM users\n```\n");
        assert_eq!(body_line_count(&raw), 2);
    }

    #[test]
    fn body_line_count_zero_when_no_body() {
        let raw = raw_for("```http\n```\n");
        assert_eq!(body_line_count(&raw), 0);
    }

    #[test]
    fn body_line_col_clamps_past_eol() {
        let raw = raw_for("```db\nABC\n```\n");
        // Body line 0 is "ABC" (len 3). Asking for col 99 clamps to 3.
        let off = body_line_col_to_raw_offset(&raw, 0, 99);
        let line0_start = body_line_to_raw_offset(&raw, 0);
        assert_eq!(off, line0_start + 3);
    }

    #[test]
    fn closer_raw_offset_points_to_closer_line_start() {
        let raw = raw_for("```db\nSELECT 1\n```\n");
        let off = closer_raw_offset(&raw);
        // The 3 chars at `off..off+3` should be the backticks.
        let s: String = raw.slice(off..off + 3).to_string();
        assert_eq!(s, "```");
    }

    #[test]
    fn header_raw_offset_is_zero() {
        assert_eq!(header_raw_offset(), 0);
    }

    // ─── reparse_from_raw ───

    fn block_with_raw(text: &str) -> BlockNode {
        // Build through parse_blocks so the initial derived fields
        // are already populated; the test then mutates `raw` and
        // checks that reparse rederives them.
        let parsed = httui_core::blocks::parse_blocks(text);
        let p = parsed.into_iter().next().expect("fixture must parse");
        BlockNode {
            id: BlockId(0),
            raw: Rope::from_str(text.trim_end_matches('\n')),
            block_type: p.block_type,
            alias: p.alias,
            display_mode: p.display_mode,
            params: p.params,
            state: ExecutionState::Idle,
            cached_result: None,
        }
    }

    #[test]
    fn reparse_from_raw_updates_alias_and_query() {
        let mut b = block_with_raw("```db-postgres alias=q\nSELECT 1\n```\n");
        // Mutate raw directly: rename alias and append a char to the body.
        let new_text = "```db-postgres alias=renamed\nSELECT 11\n```";
        b.raw = Rope::from_str(new_text);
        assert!(b.reparse_from_raw());
        assert_eq!(b.alias.as_deref(), Some("renamed"));
        assert_eq!(
            b.params.get("query").and_then(|v| v.as_str()),
            Some("SELECT 11")
        );
    }

    #[test]
    fn reparse_preserves_id_and_cached_result() {
        // Cached state survives a successful reparse — that's the
        // whole point of the (id, cached_result) pair living off the
        // markdown.
        let mut b = block_with_raw("```db-postgres alias=q\nSELECT 1\n```\n");
        b.id = BlockId(42);
        b.state = ExecutionState::Success;
        b.cached_result = Some(serde_json::json!({"results": [{"rows": []}]}));
        b.raw = Rope::from_str("```db-postgres alias=q\nSELECT 2\n```");
        assert!(b.reparse_from_raw());
        assert_eq!(b.id, BlockId(42));
        assert_eq!(b.state, ExecutionState::Success);
        assert!(b.cached_result.is_some());
    }

    #[test]
    fn reparse_keeps_last_good_when_raw_yields_multiple_blocks() {
        // User types an extra ``` mid-body that splits the rope into
        // two parsed blocks. We can't pick which one wins, so derived
        // fields stay frozen until the user restores a single-block
        // shape.
        let mut b = block_with_raw("```db-postgres alias=q\nSELECT 1\n```\n");
        let original_query = b
            .params
            .get("query")
            .and_then(|v| v.as_str())
            .map(String::from);
        // Insert a stray closer and a new opener — now `parse_blocks`
        // sees two blocks.
        b.raw = Rope::from_str(
            "```db-postgres alias=q\nSELECT 1\n```\n```db-postgres alias=q2\nSELECT 2\n```",
        );
        assert!(
            !b.reparse_from_raw(),
            "two-block raw must reject the reparse"
        );
        // last-good values stay put.
        assert_eq!(b.alias.as_deref(), Some("q"));
        assert_eq!(
            b.params
                .get("query")
                .and_then(|v| v.as_str())
                .map(String::from),
            original_query
        );
    }
}
