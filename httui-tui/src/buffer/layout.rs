//! Vertical layout for document segments.
//!
//! Each [`Segment`](crate::buffer::Segment) gets a fixed height per draw,
//! computed from its kind and current contents. Layout is recomputed
//! every frame — cheap until documents grow big, optimisation can wait.

use crate::buffer::block::BlockNode;
use crate::buffer::cursor::Cursor;
use crate::buffer::document::Document;
use crate::buffer::segment::Segment;

/// Resolved position of a segment inside the rendered document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentLayout {
    pub segment_idx: usize,
    pub y_start: u16,
    pub height: u16,
}

/// Lines a segment will occupy in the editor area.
///
/// Width is accepted for forward-compat (prose wrap, multi-column
/// blocks); current heuristic ignores it. `cursor_on_block` reserves
/// two extra rows inside the bordered card so the renderer can paint
/// the fence header / closer text right above and below the body —
/// keeps the card chrome visible while editing, matching the desktop
/// widget's look.
pub fn segment_height(seg: &Segment, _width: u16, cursor_on_block: bool) -> u16 {
    match seg {
        Segment::Prose(rope) => rope.len_lines().max(1) as u16,
        Segment::Block(b) => block_height(b, cursor_on_block),
    }
}

fn block_height(b: &BlockNode, cursor_on_block: bool) -> u16 {
    let fence_rows = if cursor_on_block { 2u16 } else { 0 };

    // Chrome shared by every block kind: top border + header bar +
    // footer bar + bottom border = 4 rows. Status banner is gone —
    // its info now lives in the header / footer bars.
    let chrome = 4u16;

    let card = if b.is_http() {
        // chrome + request body lines + response panel rows (only
        // when the block has a cached result). Two row counts to
        // pick from based on cursor state — keep them in lockstep
        // with the renderer (`ui::blocks::render_http_inner`),
        // which swaps between raw rope and structured `b.params`
        // depending on `selected`.
        let response_lines = if b.cached_result.is_some() {
            // Tab bar (1) + separator (1) + viewport (8) — mirrors
            // `ui::blocks::http_response_panel_height`.
            10u16
        } else {
            0
        };
        chrome
            .saturating_add(http_request_lines(b, cursor_on_block))
            .saturating_add(response_lines)
    } else if b.is_db() {
        let mode = b.effective_display_mode();
        let sql_lines = if mode.shows_input() {
            b.params
                .get("query")
                .and_then(|v| v.as_str())
                .map(|s| s.lines().count().max(1))
                .unwrap_or(1) as u16
        } else {
            0
        };
        let table_lines = if mode.shows_output() {
            db_table_height(b)
        } else {
            0
        };
        chrome.saturating_add(sql_lines).saturating_add(table_lines)
    } else if b.is_e2e() {
        let steps = b
            .params
            .get("steps")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        // chrome + steps + base_url line
        chrome.saturating_add(steps as u16).saturating_add(1)
    } else {
        chrome.saturating_add(1)
    };
    card.saturating_add(fence_rows)
}

/// How tall the DB result `Table` widget paints inside the card.
/// Must mirror `ui::blocks::db_result_table_height` so the segment's
/// reserved height matches what the renderer actually fills.
/// Rows the HTTP block's request panel will paint inside the card.
/// Two paths matching `ui::blocks::render_http_inner`:
///
/// - Cursor on (`cursor_on_block`): renderer paints the raw rope
///   line-by-line, so the row count is the body's raw line count.
/// - Cursor off: renderer expands `b.params` (method+URL, query
///   params one-per-row, headers one-per-row, body separator + body
///   lines), so the row count mirrors that expansion.
///
/// Picking the wrong path leaves dead rows under the card (visible
/// as empty lines between the closer and the footer bar).
fn http_request_lines(b: &BlockNode, cursor_on_block: bool) -> u16 {
    if cursor_on_block {
        // Renderer uses `raw_body_text(b)` and feeds it through
        // `highlight_http_message` — one Line per body row. Body
        // rows = raw lines minus fence header + closer.
        return crate::buffer::block::body_line_count(&b.raw).max(1) as u16;
    }
    let mut rows: u16 = 1; // method+URL row
    if let Some(params) = b.params.get("params").and_then(|v| v.as_array()) {
        rows = rows.saturating_add(params.len() as u16);
    }
    if let Some(headers) = b.params.get("headers").and_then(|v| v.as_array()) {
        rows = rows.saturating_add(headers.len() as u16);
    }
    if let Some(body) = b.params.get("body").and_then(|v| v.as_str()) {
        let trimmed = body.trim_end_matches('\n');
        if !trimmed.is_empty() {
            rows = rows.saturating_add(1); // separator blank row
            rows = rows.saturating_add(trimmed.lines().count() as u16);
        }
    }
    rows
}

fn db_table_height(b: &BlockNode) -> u16 {
    const MAX_VISIBLE: usize = 10;
    const ERROR_PANEL_ROWS: u16 = 6;
    let Some(result) = b.cached_result.as_ref() else {
        return 0;
    };
    let results = result.get("results").and_then(|v| v.as_array());
    let Some(first) = results.and_then(|a| a.first()) else {
        return 0;
    };
    let kind = first.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    // Reserve a fixed slot so the error banner + message have room.
    if kind == "error" {
        let multi = results.map(|a| a.len() > 1).unwrap_or(false);
        let chrome_extra = 2 + if multi { 1 } else { 0 };
        return ERROR_PANEL_ROWS + chrome_extra;
    }
    if kind != "select" {
        return 0;
    }
    let row_count = first
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let table_rows = if row_count == 0 {
        // Header-only.
        1
    } else {
        // Cap at the viewport size — extra rows are reachable via
        // scroll, not by growing the card.
        let visible = row_count.min(MAX_VISIBLE);
        1 + visible // +1 for the table header row
    };
    // Chrome rows the renderer carves on top of the panel:
    //   +1 tab bar (Results / Messages / Plan / Stats)
    //   +1 separator under the tab strip
    //   +1 sub-tabs strip when results.len() > 1
    let multi = results.map(|a| a.len() > 1).unwrap_or(false);
    let chrome_extra = 2 + if multi { 1 } else { 0 };
    (table_rows + chrome_extra) as u16
}

/// Walk all segments and produce their `(idx, y_start, height)` triples.
/// The block under the cursor reserves two extra rows so the renderer
/// can paint the fence header / closer in raw view.
pub fn layout_document(doc: &Document, viewport_width: u16) -> Vec<SegmentLayout> {
    let cursor_seg = match doc.cursor() {
        Cursor::InBlock { segment_idx, .. } | Cursor::InBlockResult { segment_idx, .. } => {
            Some(segment_idx)
        }
        _ => None,
    };
    let mut out = Vec::with_capacity(doc.segment_count());
    let mut y: u16 = 0;
    for (idx, seg) in doc.segments().iter().enumerate() {
        // Block-only flag: cursor_on_block reserves rows for the
        // fence header / closer that the bordered card displays
        // when the cursor is inside. Prose segments don't care.
        let cursor_on_block = cursor_seg == Some(idx) && matches!(seg, Segment::Block(_));
        let height = segment_height(seg, viewport_width, cursor_on_block);
        out.push(SegmentLayout {
            segment_idx: idx,
            y_start: y,
            height,
        });
        y = y.saturating_add(height);
    }
    out
}

/// Total document height in cells. Useful for clamping the viewport.
pub fn document_height(layouts: &[SegmentLayout]) -> u16 {
    layouts
        .last()
        .map(|l| l.y_start.saturating_add(l.height))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::ExecutionState;
    use crate::buffer::Document;

    #[test]
    fn prose_height_is_line_count() {
        let doc = Document::from_markdown("a\nb\nc\n").unwrap();
        let layouts = layout_document(&doc, 80);
        assert_eq!(layouts.len(), 1);
        assert!(layouts[0].height >= 3);
    }

    #[test]
    fn http_block_height_includes_chrome_plus_request_lines() {
        // chrome (4: border + header bar + footer bar + border) +
        // 1 request line (method+URL only — empty headers/body) = 5.
        let md = "```http alias=h\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        let doc = Document::from_markdown(md).unwrap();
        let layouts = layout_document(&doc, 80);
        assert_eq!(
            layouts
                .iter()
                .find(|l| matches!(
                    doc.segments()[l.segment_idx],
                    crate::buffer::Segment::Block(_)
                ))
                .unwrap()
                .height,
            5
        );
    }

    #[test]
    fn db_block_height_grows_with_query() {
        let md = "```db-postgres alias=q\nSELECT *\nFROM users\nWHERE id > 10\n```\n";
        let doc = Document::from_markdown(md).unwrap();
        let layouts = layout_document(&doc, 80);
        let block_h = layouts
            .iter()
            .find(|l| {
                matches!(
                    doc.segments()[l.segment_idx],
                    crate::buffer::Segment::Block(_)
                )
            })
            .unwrap()
            .height;
        // 3 SQL lines + chrome (4: border + header + footer + border).
        assert_eq!(block_h, 7);
    }

    #[test]
    fn y_start_is_cumulative() {
        let md = "intro\n\n```http\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n\noutro\n";
        let doc = Document::from_markdown(md).unwrap();
        let layouts = layout_document(&doc, 80);
        let mut expected_y = 0u16;
        for l in &layouts {
            assert_eq!(l.y_start, expected_y);
            expected_y = expected_y.saturating_add(l.height);
        }
    }

    #[test]
    fn document_height_matches_sum() {
        let md = "abc\n```http\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        let doc = Document::from_markdown(md).unwrap();
        let layouts = layout_document(&doc, 80);
        let sum: u16 = layouts.iter().map(|l| l.height).sum();
        assert_eq!(document_height(&layouts), sum);
    }

    fn db_block_index(doc: &Document) -> usize {
        doc.segments()
            .iter()
            .enumerate()
            .find_map(|(i, s)| matches!(s, crate::buffer::Segment::Block(_)).then_some(i))
            .expect("doc has a block")
    }

    #[test]
    fn db_output_mode_drops_sql_lines_from_height() {
        // `display=output` hides the SQL body. With no cached result
        // and no run history, the block collapses to chrome-only
        // (4 rows: top border + header bar + footer bar + bottom
        // border).
        let md = "```db-postgres alias=q\nSELECT *\nFROM users\nWHERE id > 10\n```\n";
        let mut doc = Document::from_markdown(md).unwrap();
        let idx = db_block_index(&doc);
        doc.block_at_mut(idx).unwrap().display_mode = Some("output".into());
        let layouts = layout_document(&doc, 80);
        let block_h = layouts
            .iter()
            .find(|l| l.segment_idx == idx)
            .unwrap()
            .height;
        assert_eq!(block_h, 4);
    }

    #[test]
    fn db_block_height_grows_by_two_when_cursor_enters() {
        // Cursor-on-block adds a fence header row above the body
        // and a fence closer row below — both inside the bordered
        // card so the chrome stays consistent with the desktop
        // widget. Layout therefore reserves two extra rows on the
        // selected block; once the cursor leaves, those rows
        // collapse back.
        let md = "```db-postgres alias=q\nSELECT *\nFROM users\nWHERE id > 10\n```\n";
        let mut doc = Document::from_markdown(md).unwrap();
        let block_idx = doc
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .unwrap();
        doc.set_cursor(crate::buffer::Cursor::InBlock {
            segment_idx: block_idx,
            offset: 0,
        });
        let with_cursor = layout_document(&doc, 80)
            .iter()
            .find(|l| l.segment_idx == block_idx)
            .unwrap()
            .height;
        doc.set_cursor(crate::buffer::Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        let without_cursor = layout_document(&doc, 80)
            .iter()
            .find(|l| l.segment_idx == block_idx)
            .unwrap()
            .height;
        assert_eq!(with_cursor, without_cursor + 2);
    }

    #[test]
    fn db_split_mode_with_result_includes_sql_status_and_table() {
        // `display=split` with a `select` result. Layout:
        //   chrome 4 (top border + header bar + footer bar + bottom)
        //   SQL body 3 lines
        //   tab bar 1 + separator 1 + result panel (header + 2 rows)
        // = 4 + 3 + (1 + 1 + 1 + 2) = 12. No sub-tabs row because the
        // response carries only one result set.
        let md = "```db-postgres alias=q\nSELECT *\nFROM users\nWHERE id > 10\n```\n";
        let mut doc = Document::from_markdown(md).unwrap();
        let idx = db_block_index(&doc);
        let block = doc.block_at_mut(idx).unwrap();
        block.display_mode = Some("split".into());
        block.state = ExecutionState::Success;
        block.cached_result = Some(serde_json::json!({
            "results": [{
                "kind": "select",
                "columns": [{"name": "id"}],
                "rows": [{"id": 1}, {"id": 2}],
                "has_more": false
            }],
            "stats": { "elapsed_ms": 7 }
        }));
        let layouts = layout_document(&doc, 80);
        let block_h = layouts
            .iter()
            .find(|l| l.segment_idx == idx)
            .unwrap()
            .height;
        assert_eq!(block_h, 4 + 3 + (1 + 2 + 2));
    }
}
