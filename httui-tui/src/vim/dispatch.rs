//! Facade for the input dispatcher.
//!
//! The top-level `dispatch` router and the exhaustive `apply_action`
//! `Action` interpreter moved to `crate::input::dispatch` (tui-v2
//! vertical 1, fase 1 p6-router). This module is now a thin re-export
//! so the existing `crate::vim::dispatch::*` call sites
//! (`app.rs::handle_key`, `crate::input::apply::replay`,
//! `vim::mod::dispatch`) keep resolving unchanged.

pub use crate::input::dispatch::dispatch;
// `apply_action` is `pub(crate)` — the `replay` helpers reach it via
// `crate::vim::dispatch::apply_action`.
pub(crate) use crate::input::dispatch::apply_action;

// Test-only re-exports: the in-file `mod tests` below uses
// `use super::*`, so the symbols its cases reference must resolve
// through this module.
#[allow(unused_imports)]
pub(crate) use crate::buffer::Segment;
#[allow(unused_imports)]
pub(crate) use crate::input::apply::modal_detail::{
    build_http_response_body_text, build_http_response_modal_title, format_size,
};
#[allow(unused_imports)]
pub(crate) use crate::input::apply::navigation::should_prefetch;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_prefetch_skips_when_backend_says_done() {
        // Cursor near the bottom but the server already exhausted
        // pages — no further fetch should fire.
        assert!(!should_prefetch(95, 100, false, 5));
        assert!(!should_prefetch(99, 100, false, 5));
    }

    #[test]
    fn should_prefetch_waits_for_threshold_when_more_pages_exist() {
        // Plenty of headroom: don't trigger.
        assert!(!should_prefetch(0, 100, true, 5));
        assert!(!should_prefetch(94, 100, true, 5));
        // Within the threshold band: trigger.
        assert!(should_prefetch(95, 100, true, 5));
        assert!(should_prefetch(99, 100, true, 5));
        // Past the loaded set (the engine reaches this momentarily
        // when motion overshoots before append finishes).
        assert!(should_prefetch(100, 100, true, 5));
    }

    #[test]
    fn should_prefetch_fires_immediately_for_small_initial_pages() {
        // 3 rows back, threshold 5, has_more true → trigger from
        // row 0 (page is smaller than the prefetch window).
        assert!(should_prefetch(0, 3, true, 5));
        assert!(should_prefetch(2, 3, true, 5));
    }

    #[test]
    fn should_prefetch_handles_empty_set() {
        // Defensive: no rows + has_more shouldn't crash on the
        // arithmetic and shouldn't fire (cursor can't be in an
        // empty result anyway).
        assert!(should_prefetch(0, 0, true, 5));
        assert!(!should_prefetch(0, 0, false, 5));
    }

    #[test]
    fn format_size_picks_unit_by_magnitude() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 kB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
    }

    #[test]
    fn build_http_response_body_text_lays_out_status_headers_and_body() {
        // Synthesize a fenced HTTP block + cached_result mimicking the
        // shape `http_response_to_json` emits, then check the rendered
        // modal body has the section headings, status line and an
        // indented body line.
        let block_md = "```http alias=req1\nGET https://api.example.com/users\n```\n";
        let doc = crate::buffer::Document::from_markdown(block_md).unwrap();
        let mut block = doc
            .segments()
            .iter()
            .find_map(|s| match s {
                Segment::Block(b) => Some(b.clone()),
                _ => None,
            })
            .expect("block segment in parsed doc");
        block.cached_result = Some(serde_json::json!({
            "status": 200,
            "status_text": "OK",
            "headers": [
                {"key": "content-type", "value": "application/json"},
                {"key": "x-trace", "value": "abc"},
            ],
            "cookies": [],
            "body": serde_json::json!({"ok": true}),
            "size_bytes": 42,
            "timing": {"total_ms": 142, "ttfb_ms": 30},
        }));

        let text = build_http_response_body_text(&block).expect("body text");
        // Status line is line zero with code and human label.
        assert!(text.starts_with("200 OK"), "got: {text}");
        assert!(text.contains("142 ms"));
        assert!(text.contains("42 B"));
        // Section heading + a header pair.
        assert!(text.contains("\nHeaders\n"));
        assert!(text.contains("  content-type: application/json"));
        // Body section + pretty JSON line.
        assert!(text.contains("\nBody\n"));
        assert!(text.contains("  \"ok\": true"));
    }

    #[test]
    fn build_http_response_modal_title_includes_status_size_alias() {
        let block_md = "```http alias=login\nPOST https://example.com/login\n```\n";
        let doc = crate::buffer::Document::from_markdown(block_md).unwrap();
        let mut block = doc
            .segments()
            .iter()
            .find_map(|s| match s {
                Segment::Block(b) => Some(b.clone()),
                _ => None,
            })
            .expect("block segment in parsed doc");
        block.cached_result = Some(serde_json::json!({
            "status": 201,
            "size_bytes": 1500,
        }));
        let title = build_http_response_modal_title(&block);
        // Padded with spaces (it's a window title, ratatui paints it
        // with a space-buffer so it doesn't kiss the corner).
        assert!(title.starts_with(' '));
        assert!(title.ends_with(' '));
        assert!(title.contains("201"));
        assert!(title.contains("1.5 kB"));
        assert!(title.contains("login"));
    }

    /// Build a Document with prose / block / prose / block / prose
    /// to exercise `apply_jump_block` against the segment iterator.
    fn doc_with_two_blocks() -> crate::buffer::Document {
        let md = "intro\n\n```http alias=a\nGET https://a.test\n```\n\nmid\n\n```http alias=b\nGET https://b.test\n```\n\nend\n";
        crate::buffer::Document::from_markdown(md).unwrap()
    }

    fn block_indices(doc: &crate::buffer::Document) -> Vec<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
            .collect()
    }

    /// Stand-alone reimplementation of the navigator inside
    /// `apply_jump_block` — same predicate (skip current, no wrap),
    /// no `App` plumbing. Lets the test cover the search rule
    /// without spinning up a full `App`.
    fn next_block(doc: &crate::buffer::Document, cur: usize) -> Option<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .skip(cur + 1)
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
    }

    fn prev_block(doc: &crate::buffer::Document, cur: usize) -> Option<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .take(cur)
            .rev()
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
    }

    #[test]
    fn jump_next_block_walks_forward_no_wrap() {
        let doc = doc_with_two_blocks();
        let blocks = block_indices(&doc);
        assert_eq!(blocks.len(), 2);
        // From the first prose segment (idx 0) → first block.
        assert_eq!(next_block(&doc, 0), Some(blocks[0]));
        // From the first block → second block.
        assert_eq!(next_block(&doc, blocks[0]), Some(blocks[1]));
        // From the second block → no more (no wrap).
        assert_eq!(next_block(&doc, blocks[1]), None);
    }

    #[test]
    fn jump_prev_block_walks_backward_no_wrap() {
        let doc = doc_with_two_blocks();
        let blocks = block_indices(&doc);
        // From the second block → first block.
        assert_eq!(prev_block(&doc, blocks[1]), Some(blocks[0]));
        // From the first block → no earlier block.
        assert_eq!(prev_block(&doc, blocks[0]), None);
        // From the last prose segment → previous block.
        let last = doc.segments().len() - 1;
        assert_eq!(prev_block(&doc, last), Some(blocks[1]));
    }

    #[test]
    fn jump_block_no_blocks_yields_none() {
        let md = "just prose\n\nno blocks at all\n";
        let doc = crate::buffer::Document::from_markdown(md).unwrap();
        assert_eq!(next_block(&doc, 0), None);
        assert_eq!(prev_block(&doc, 0), None);
    }
}
