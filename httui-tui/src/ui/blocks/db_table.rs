//! DB block inner rendering: SQL body, result table, viewport
//! sliding window, summary line, and per-cell formatting helpers.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
    Frame,
};

use crate::buffer::block::{BlockNode, ExecutionState};

use super::result_tabs::{
    build_error_lines, build_messages_lines, build_plan_lines, build_stats_lines,
    render_result_separator, render_result_subtabs, render_result_tab_bar_inner,
};
use super::{overlay_refs_on_spans, raw_body_text, ref_highlight, render_fence_closer_row,
    ConnectionNames};

/// Height (in rows) of the result table viewport inside a DB card.
pub(super) const MAX_VISIBLE_ROWS: usize = 10;
/// Cap on column width so a single fat field can't hog the row.
const MAX_COL_WIDTH: usize = 30;
/// `scrolloff` band for the result-table viewport.
const RESULT_SCROLL_OFF: usize = 2;

/// Render the DB block's content area between the chrome header and
/// footer bars.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_db_inner(
    frame: &mut Frame,
    inner: Rect,
    b: &BlockNode,
    selected_row: Option<usize>,
    viewport_top: Option<&mut u16>,
    _names: &ConnectionNames,
    result_tab: crate::app::ResultPanelTab,
    selected: bool,
) {
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mode = b.effective_display_mode();
    let show_input = mode.shows_input();
    let show_output = mode.shows_output();

    let query_string;
    let query: &str = if selected {
        query_string = raw_body_text(b);
        &query_string
    } else {
        b.params.get("query").and_then(|v| v.as_str()).unwrap_or("")
    };
    let sql_lines = query.lines().count().max(1) as u16;
    let table_height = if show_output {
        db_result_table_height(b)
    } else {
        0
    };
    let closer_height = if selected && show_input { 1 } else { 0 };

    let mut constraints: Vec<Constraint> = Vec::new();
    if show_input {
        constraints.push(Constraint::Length(sql_lines));
    }
    if closer_height > 0 {
        constraints.push(Constraint::Length(closer_height));
    }
    if table_height > 0 {
        constraints.push(Constraint::Length(table_height));
    }
    if constraints.is_empty() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let mut idx = 0;

    if show_input {
        let mut sql_lines_styled = super::super::sql_highlight::highlight(query);
        let error_refs = match &b.state {
            ExecutionState::Error(msg) => ref_highlight::parse_error_refs(msg),
            _ => std::collections::HashSet::new(),
        };
        sql_lines_styled = sql_lines_styled
            .into_iter()
            .map(|spans| overlay_refs_on_spans(spans, &error_refs))
            .collect();
        if let Some((err_line, _err_col)) = error_position(b) {
            if let Some(target) = (err_line as usize)
                .checked_sub(1)
                .and_then(|i| sql_lines_styled.get_mut(i))
            {
                for span in target.iter_mut() {
                    span.style = span.style.bg(Color::Rgb(70, 25, 25));
                }
            }
        }
        let sql_para = Paragraph::new(
            sql_lines_styled
                .into_iter()
                .map(Line::from)
                .collect::<Vec<_>>(),
        );
        frame.render_widget(sql_para, chunks[idx]);
        idx += 1;
    }

    if closer_height > 0 {
        render_fence_closer_row(frame, chunks[idx], b);
        idx += 1;
    }

    if table_height > 0 {
        let panel_chunk = chunks[idx];
        let result_count = b
            .cached_result
            .as_ref()
            .and_then(|v| v.get("results"))
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let multi = result_count > 1;
        let mut y = panel_chunk.y;
        let row = |y: u16| Rect {
            x: panel_chunk.x,
            y,
            width: panel_chunk.width,
            height: 1,
        };
        let tab_bar_rect = row(y);
        y = y.saturating_add(1);
        let separator_rect = row(y);
        y = y.saturating_add(1);
        let subtabs_rect = if multi { Some(row(y)) } else { None };
        if multi {
            y = y.saturating_add(1);
        }
        let used = y.saturating_sub(panel_chunk.y);
        let content_rect = Rect {
            x: panel_chunk.x,
            y,
            width: panel_chunk.width,
            height: panel_chunk.height.saturating_sub(used),
        };
        render_result_tab_bar_inner(
            frame,
            tab_bar_rect,
            result_tab,
            if multi { Some(result_count) } else { None },
        );
        render_result_separator(frame, separator_rect);
        if let Some(rect) = subtabs_rect {
            render_result_subtabs(frame, rect, b, 0);
        }
        match result_tab {
            crate::app::ResultPanelTab::Result => {
                if let Some((table, viewport_selected)) =
                    build_result_table(b, selected_row, viewport_top)
                {
                    let mut state = ratatui::widgets::TableState::default();
                    state.select(viewport_selected);
                    let table = table.row_highlight_style(
                        Style::default()
                            .bg(super::super::palette::SELECTION_BG)
                            .add_modifier(Modifier::BOLD),
                    );
                    frame.render_stateful_widget(table, content_rect, &mut state);
                } else if let Some(lines) = build_error_lines(b) {
                    frame.render_widget(Paragraph::new(lines), content_rect);
                }
            }
            crate::app::ResultPanelTab::Messages => {
                let lines = build_messages_lines(b);
                frame.render_widget(Paragraph::new(lines), content_rect);
            }
            crate::app::ResultPanelTab::Plan => {
                let lines = build_plan_lines(b);
                frame.render_widget(Paragraph::new(lines), content_rect);
            }
            crate::app::ResultPanelTab::Stats => {
                let lines = build_stats_lines(b);
                frame.render_widget(Paragraph::new(lines), content_rect);
            }
            // DB blocks don't expose `Raw`; fall back to stats so we
            // never panic on unexpected tab state.
            crate::app::ResultPanelTab::Raw => {
                let lines = build_stats_lines(b);
                frame.render_widget(Paragraph::new(lines), content_rect);
            }
        }
    } else {
        let _ = (selected_row, viewport_top);
    }
}

pub(super) fn db_summary(b: &BlockNode) -> Option<String> {
    let result = b.cached_result.as_ref()?;
    let elapsed = result.get("stats")?.get("elapsed_ms")?.as_u64()?;
    let results = result.get("results")?.as_array()?;
    let first = results.first()?;
    let kind = first.get("kind")?.as_str()?;
    let extras = match results.len() {
        0 | 1 => String::new(),
        n => format!(" (+{} more)", n - 1),
    };
    match kind {
        "select" => {
            let rows = first.get("rows")?.as_array()?.len();
            let has_more = first
                .get("has_more")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let suffix = if has_more { "+" } else { "" };
            Some(format!("{rows}{suffix} rows · {elapsed}ms{extras}"))
        }
        "mutation" => {
            let affected = first.get("rows_affected")?.as_u64()?;
            Some(format!("{affected} affected · {elapsed}ms{extras}"))
        }
        "error" => first.get("message").and_then(|v| v.as_str()).map(|m| {
            let pos = first
                .get("line")
                .and_then(|l| l.as_u64())
                .map(|line| {
                    let col = first.get("column").and_then(|c| c.as_u64()).unwrap_or(1);
                    format!(" at {line}:{col}")
                })
                .unwrap_or_default();
            format!("error: {m}{pos}{extras}")
        }),
        _ => None,
    }
}

/// Extract `(line, column)` from the first result if it's an Error
/// variant with positional info.
fn error_position(b: &BlockNode) -> Option<(u64, u64)> {
    let result = b.cached_result.as_ref()?;
    let first = result.get("results")?.as_array()?.first()?;
    if first.get("kind")?.as_str()? != "error" {
        return None;
    }
    let line = first.get("line")?.as_u64()?;
    let column = first.get("column").and_then(|c| c.as_u64()).unwrap_or(1);
    Some((line, column))
}

/// Persistent-viewport scroll for the result table.
pub(super) fn clamp_result_viewport(
    viewport_top: usize,
    viewport: usize,
    cursor: usize,
    total: usize,
) -> usize {
    if viewport == 0 || total <= viewport {
        return 0;
    }
    let scrolloff = RESULT_SCROLL_OFF.min(viewport / 2);
    let upper = cursor.saturating_sub(scrolloff);
    let lower = cursor
        .saturating_add(scrolloff + 1)
        .saturating_sub(viewport);
    let next = if viewport_top > upper {
        upper
    } else if viewport_top < lower {
        lower
    } else {
        viewport_top
    };
    next.min(total - viewport)
}

/// Build a `ratatui::Table` widget for a DB block's `select` result.
pub(super) fn build_result_table(
    b: &BlockNode,
    selected_row: Option<usize>,
    viewport_top: Option<&mut u16>,
) -> Option<(Table<'static>, Option<usize>)> {
    let result = b.cached_result.as_ref()?;
    let first = result
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return None;
    }
    let columns_meta: Vec<(String, String)> = first
        .get("columns")
        .and_then(|v| v.as_array())?
        .iter()
        .map(|c| {
            let name = c
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("?")
                .to_string();
            let ty = c
                .get("type")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            (name, ty)
        })
        .collect();
    if columns_meta.is_empty() {
        return None;
    }
    let columns: Vec<String> = columns_meta.iter().map(|(n, _)| n.clone()).collect();
    let rows: Vec<&serde_json::Value> = first
        .get("rows")
        .and_then(|v| v.as_array())?
        .iter()
        .collect();

    let total = rows.len();
    let offset = match (viewport_top, selected_row) {
        (Some(slot), Some(sel)) => {
            let next = clamp_result_viewport(*slot as usize, MAX_VISIBLE_ROWS, sel, total);
            *slot = next as u16;
            next
        }
        _ => 0,
    };
    let end = (offset + MAX_VISIBLE_ROWS).min(total);
    let visible_rows: &[&serde_json::Value] = &rows[offset..end];

    let mut widths: Vec<u16> = columns
        .iter()
        .map(|n| n.chars().count().min(MAX_COL_WIDTH) as u16)
        .collect();
    for row in visible_rows.iter() {
        for (i, name) in columns.iter().enumerate() {
            let cell = format_cell(row.get(name).unwrap_or(&serde_json::Value::Null));
            let len = cell.chars().count().min(MAX_COL_WIDTH) as u16;
            if len > widths[i] {
                widths[i] = len;
            }
        }
    }

    let name_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let type_style = Style::default().fg(Color::DarkGray);
    let header = Row::new(
        columns_meta
            .iter()
            .enumerate()
            .map(|(i, (name, ty))| {
                let col_w = widths[i] as usize;
                let name_chars = name.chars().count();
                let mut spans: Vec<Span<'static>> = Vec::with_capacity(3);
                spans.push(Span::styled(
                    truncate_with_ellipsis(name, col_w),
                    name_style,
                ));
                // Only render the type label when it fits AFTER the
                // name with a 1-space gap.
                if !ty.is_empty() {
                    let used = name_chars.min(col_w);
                    let remaining = col_w.saturating_sub(used + 1);
                    if remaining >= ty.chars().count() {
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(ty.clone(), type_style));
                    }
                }
                Cell::from(Line::from(spans))
            })
            .collect::<Vec<_>>(),
    )
    .height(1);

    let aligns: Vec<bool> = columns_meta
        .iter()
        .map(|(_, ty)| is_numeric_type(ty))
        .collect();

    let zebra_bg = Color::Rgb(18, 20, 26);

    let table_rows: Vec<Row> = visible_rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let cells: Vec<Cell> = columns
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let raw = format_cell(row.get(name).unwrap_or(&serde_json::Value::Null));
                    let truncated = truncate_with_ellipsis(&raw, MAX_COL_WIDTH);
                    if aligns[i] {
                        let pad = (widths[i] as usize).saturating_sub(truncated.chars().count());
                        Cell::from(format!("{}{}", " ".repeat(pad), truncated))
                    } else {
                        Cell::from(truncated)
                    }
                })
                .collect();
            let row = Row::new(cells);
            if row_idx % 2 == 1 {
                row.style(Style::default().bg(zebra_bg))
            } else {
                row
            }
        })
        .collect();

    let viewport_selected = selected_row.map(|sel| sel.saturating_sub(offset));
    let constraints: Vec<Constraint> = widths.iter().map(|w| Constraint::Length(*w)).collect();
    Some((
        Table::new(table_rows, constraints)
            .header(header)
            .column_spacing(3),
        viewport_selected,
    ))
}

fn is_numeric_type(ty: &str) -> bool {
    let lower = ty.to_lowercase();
    matches!(
        lower.as_str(),
        "int"
            | "integer"
            | "bigint"
            | "smallint"
            | "tinyint"
            | "int2"
            | "int4"
            | "int8"
            | "float"
            | "float4"
            | "float8"
            | "real"
            | "double"
            | "double precision"
            | "decimal"
            | "numeric"
            | "money"
    ) || lower.starts_with("int")
        || lower.starts_with("float")
        || lower.starts_with("decimal")
        || lower.starts_with("numeric")
}

pub(super) fn db_result_table_height(b: &BlockNode) -> u16 {
    const ERROR_PANEL_ROWS: u16 = 6;
    let Some(result) = b.cached_result.as_ref() else {
        return 0;
    };
    let results = result.get("results").and_then(|v| v.as_array());
    let Some(first) = results.and_then(|a| a.first()) else {
        return 0;
    };
    let kind = first.get("kind").and_then(|v| v.as_str()).unwrap_or("");
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
        1
    } else {
        let visible = row_count.min(MAX_VISIBLE_ROWS);
        1 + visible
    };
    let multi = results.map(|a| a.len() > 1).unwrap_or(false);
    let chrome_extra = 2 + if multi { 1 } else { 0 };
    (table_rows + chrome_extra) as u16
}

fn truncate_with_ellipsis(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count <= width {
        return s.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let head: String = s.chars().take(width.saturating_sub(1)).collect();
    format!("{head}…")
}

/// Render a JSON cell as a flat string.
fn format_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "(null)".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(_) => "[…]".into(),
        serde_json::Value::Object(_) => "{…}".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::BlockId;
    use serde_json::json;

    fn db_block(result: Option<serde_json::Value>) -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "db-sqlite".into(),
            alias: None,
            display_mode: None,
            params: json!({"query": "SELECT 1"}),
            state: if result.is_some() {
                ExecutionState::Success
            } else {
                ExecutionState::Idle
            },
            cached_result: result,
        }
    }

    #[test]
    fn db_summary_select_with_count_and_elapsed() {
        let b = db_block(Some(json!({
            "stats": {"elapsed_ms": 12},
            "results": [{"kind": "select", "rows": [{"a": 1}, {"a": 2}], "has_more": false}],
        })));
        let s = db_summary(&b).unwrap();
        assert!(s.contains("2 rows"));
        assert!(s.contains("12ms"));
    }

    #[test]
    fn db_summary_select_has_more_appends_plus() {
        let b = db_block(Some(json!({
            "stats": {"elapsed_ms": 1},
            "results": [{"kind": "select", "rows": [{"a": 1}], "has_more": true}],
        })));
        let s = db_summary(&b).unwrap();
        assert!(s.contains("1+"));
    }

    #[test]
    fn db_summary_mutation_format() {
        let b = db_block(Some(json!({
            "stats": {"elapsed_ms": 4},
            "results": [{"kind": "mutation", "rows_affected": 3}],
        })));
        let s = db_summary(&b).unwrap();
        assert!(s.contains("3 affected"));
    }

    #[test]
    fn db_summary_error_includes_position_suffix() {
        let b = db_block(Some(json!({
            "stats": {"elapsed_ms": 1},
            "results": [{"kind": "error", "message": "bad sql", "line": 2, "column": 5}],
        })));
        let s = db_summary(&b).unwrap();
        assert!(s.contains("bad sql"));
        assert!(s.contains("at 2:5"));
    }

    #[test]
    fn db_summary_multi_result_appends_more_suffix() {
        let b = db_block(Some(json!({
            "stats": {"elapsed_ms": 1},
            "results": [
                {"kind": "select", "rows": [], "has_more": false},
                {"kind": "select", "rows": [], "has_more": false},
            ],
        })));
        let s = db_summary(&b).unwrap();
        assert!(s.contains("(+1 more)"));
    }

    #[test]
    fn db_summary_returns_none_for_unknown_kind() {
        let b = db_block(Some(json!({
            "stats": {"elapsed_ms": 1},
            "results": [{"kind": "wat", "rows": []}],
        })));
        assert!(db_summary(&b).is_none());
    }

    #[test]
    fn error_position_extracts_line_and_column() {
        let b = db_block(Some(json!({
            "results": [{"kind": "error", "line": 7, "column": 3}],
        })));
        assert_eq!(error_position(&b), Some((7, 3)));
    }

    #[test]
    fn error_position_defaults_column_when_missing() {
        let b = db_block(Some(json!({
            "results": [{"kind": "error", "line": 4}],
        })));
        assert_eq!(error_position(&b), Some((4, 1)));
    }

    #[test]
    fn error_position_returns_none_for_select_result() {
        let b = db_block(Some(json!({
            "results": [{"kind": "select", "rows": []}],
        })));
        assert!(error_position(&b).is_none());
    }

    #[test]
    fn clamp_viewport_no_scroll_when_total_fits_window() {
        assert_eq!(clamp_result_viewport(0, 10, 4, 5), 0);
    }

    #[test]
    fn clamp_viewport_scrolls_down_to_keep_cursor_visible() {
        assert_eq!(clamp_result_viewport(0, 10, 25, 80), 18);
    }

    #[test]
    fn clamp_viewport_scrolls_up_to_keep_cursor_visible() {
        assert_eq!(clamp_result_viewport(20, 10, 5, 80), 3);
    }

    #[test]
    fn clamp_viewport_zero_returns_zero() {
        assert_eq!(clamp_result_viewport(7, 0, 50, 100), 0);
    }

    #[test]
    fn build_result_table_none_without_cache() {
        let b = db_block(None);
        assert!(build_result_table(&b, None, None).is_none());
    }

    #[test]
    fn build_result_table_none_for_mutation_kind() {
        let b = db_block(Some(json!({
            "stats": {"elapsed_ms": 1},
            "results": [{"kind": "mutation", "rows_affected": 3}],
        })));
        assert!(build_result_table(&b, None, None).is_none());
    }

    #[test]
    fn build_result_table_some_for_select_with_columns() {
        let b = db_block(Some(json!({
            "stats": {"elapsed_ms": 1},
            "results": [{
                "kind": "select",
                "columns": [{"name": "id", "type": "int"}],
                "rows": [{"id": 1}, {"id": 2}],
                "has_more": false,
            }],
        })));
        let (_, sel) = build_result_table(&b, Some(1), None).unwrap();
        assert_eq!(sel, Some(1));
    }

    #[test]
    fn build_result_table_persistent_viewport_slides() {
        let rows: Vec<serde_json::Value> = (0..30).map(|i| json!({"id": i})).collect();
        let b = db_block(Some(json!({
            "stats": {"elapsed_ms": 1},
            "results": [{
                "kind": "select",
                "columns": [{"name": "id", "type": "int"}],
                "rows": rows,
                "has_more": false,
            }],
        })));
        let mut vt: u16 = 0;
        build_result_table(&b, Some(15), Some(&mut vt));
        assert_eq!(vt, 8);
    }

    #[test]
    fn db_result_table_height_zero_when_no_cache() {
        let b = db_block(None);
        assert_eq!(db_result_table_height(&b), 0);
    }

    #[test]
    fn db_result_table_height_for_select_with_rows() {
        let b = db_block(Some(json!({
            "results": [{
                "kind": "select",
                "columns": [{"name": "id", "type": "int"}],
                "rows": [{"id": 1}, {"id": 2}, {"id": 3}],
                "has_more": false,
            }],
        })));
        // header (1) + 3 rows + tab bar + separator = 6
        assert_eq!(db_result_table_height(&b), 6);
    }

    #[test]
    fn db_result_table_height_caps_at_viewport_for_huge_result() {
        let rows: Vec<serde_json::Value> = (0..40).map(|i| json!({"id": i})).collect();
        let b = db_block(Some(json!({
            "results": [{
                "kind": "select",
                "columns": [{"name": "id", "type": "int"}],
                "rows": rows,
                "has_more": false,
            }],
        })));
        assert_eq!(db_result_table_height(&b), (1 + MAX_VISIBLE_ROWS + 2) as u16);
    }

    #[test]
    fn db_result_table_height_error_kind_gets_fixed_panel() {
        let b = db_block(Some(json!({
            "results": [{"kind": "error", "message": "x"}],
        })));
        // ERROR_PANEL_ROWS (6) + 2 chrome rows = 8
        assert_eq!(db_result_table_height(&b), 8);
    }

    #[test]
    fn is_numeric_type_matches_common_sql_types() {
        for t in &[
            "int", "INTEGER", "bigint", "float", "real", "decimal", "numeric",
            "money", "int4", "FLOAT8",
        ] {
            assert!(is_numeric_type(t), "expected {t} numeric");
        }
        assert!(!is_numeric_type("text"));
        assert!(!is_numeric_type("varchar"));
    }

    #[test]
    fn truncate_with_ellipsis_passes_short_strings_through() {
        assert_eq!(truncate_with_ellipsis("abc", 5), "abc");
        assert_eq!(truncate_with_ellipsis("abc", 3), "abc");
    }

    #[test]
    fn truncate_with_ellipsis_drops_with_ellipsis_when_over_width() {
        let r = truncate_with_ellipsis("abcdef", 4);
        assert_eq!(r.chars().count(), 4);
        assert!(r.ends_with('…'));
    }

    #[test]
    fn truncate_with_ellipsis_zero_width_yields_empty() {
        assert_eq!(truncate_with_ellipsis("abc", 0), "");
    }

    #[test]
    fn format_cell_translates_each_json_kind() {
        use serde_json::json;
        assert_eq!(format_cell(&json!(null)), "(null)");
        assert_eq!(format_cell(&json!(true)), "true");
        assert_eq!(format_cell(&json!(42)), "42");
        assert_eq!(format_cell(&json!("hi")), "hi");
        assert_eq!(format_cell(&json!([1, 2])), "[…]");
        assert_eq!(format_cell(&json!({"k": 1})), "{…}");
    }
}
