//! Tab bar / sub-tabs / separator and the per-tab Line builders for
//! the block result panel. Shared between DB and HTTP renderers via
//! the `ResultPanelTab` enum.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::buffer::block::{BlockNode, ExecutionState};

pub(super) fn render_result_separator(frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line: String = "─".repeat(area.width as usize);
    let style = Style::default().fg(Color::DarkGray);
    frame.render_widget(Paragraph::new(Line::from(Span::styled(line, style))), area);
}

/// Strip of chip-styled tabs for multi-statement results.
pub(super) fn render_result_subtabs(
    frame: &mut Frame,
    area: Rect,
    b: &BlockNode,
    selected: usize,
) {
    let Some(results) = b
        .cached_result
        .as_ref()
        .and_then(|v| v.get("results"))
        .and_then(|v| v.as_array())
    else {
        return;
    };
    let active = Style::default()
        .bg(Color::Rgb(50, 60, 90))
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let inactive = Style::default().fg(Color::DarkGray);
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(results.len() * 3 + 1);
    spans.push(Span::raw(" "));
    for (i, r) in results.iter().enumerate() {
        let kind = r
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_uppercase();
        let style = if i == selected { active } else { inactive };
        spans.push(Span::styled(format!(" {} {} ", i + 1, kind), style));
        spans.push(Span::raw("  "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

pub(super) fn render_result_tab_bar_inner(
    frame: &mut Frame,
    area: Rect,
    selected: crate::app::ResultPanelTab,
    result_count: Option<usize>,
) {
    render_result_tab_bar_for(frame, area, selected, result_count, "")
}

pub(super) fn render_result_tab_bar_for(
    frame: &mut Frame,
    area: Rect,
    selected: crate::app::ResultPanelTab,
    result_count: Option<usize>,
    block_type: &str,
) {
    use crate::app::ResultPanelTab;
    let active_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let inactive_style = Style::default().fg(Color::DarkGray);
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(" "));
    for tab in ResultPanelTab::variants_for(block_type) {
        let style = if *tab == selected {
            active_style
        } else {
            inactive_style
        };
        let label = match (*tab, result_count) {
            // Pluralize Result(s) when multi-statement returned >1
            // (DB only — HTTP never has multi).
            (ResultPanelTab::Result, Some(n)) if n > 1 => format!("Results ({n})"),
            _ => tab.label_for(block_type).to_string(),
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw("    "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Pull a `DbResult::Error` (kind == "error") off `results[0]` and
/// format it as a red banner block. Returns `None` when the first
/// result isn't an error (Result tab keeps its normal rendering path).
pub(super) fn build_error_lines(b: &BlockNode) -> Option<Vec<Line<'static>>> {
    let first = b
        .cached_result
        .as_ref()?
        .get("results")?
        .as_array()?
        .first()?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("error") {
        return None;
    }
    let message = first
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("(no message)")
        .to_string();
    let loc = match (
        first.get("line").and_then(|v| v.as_u64()),
        first.get("column").and_then(|v| v.as_u64()),
    ) {
        (Some(l), Some(c)) => Some(format!(" at {l}:{c}")),
        (Some(l), None) => Some(format!(" at line {l}")),
        _ => None,
    };
    let header_text = match loc {
        Some(suffix) => format!("error{suffix}"),
        None => "error".to_string(),
    };
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  ✖ {header_text}"),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for chunk in message.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {chunk}"),
            Style::default().fg(Color::White),
        )));
    }
    Some(lines)
}

/// Messages tab — pulls `messages[]` off the cached response and
/// lists them as `[severity] text`. Empty list shows a dim placeholder.
pub(super) fn build_messages_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no messages)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(value) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let Some(messages) = value.get("messages").and_then(|v| v.as_array()) else {
        return vec![placeholder];
    };
    if messages.is_empty() {
        return vec![placeholder];
    }
    messages
        .iter()
        .map(|m| {
            let sev = m
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("notice");
            let text = m.get("text").and_then(|v| v.as_str()).unwrap_or("");
            Line::from(vec![
                Span::styled(
                    format!(" [{sev}] "),
                    Style::default().fg(match sev {
                        "error" => Color::Red,
                        "warning" => Color::Yellow,
                        _ => Color::LightBlue,
                    }),
                ),
                Span::raw(text.to_string()),
            ])
        })
        .collect()
}

/// Plan tab — renders `cached_result["plan"]`.
pub(super) fn build_plan_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no plan — run <C-x> on this block to populate)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(value) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let plan = match value.get("plan") {
        Some(p) if !p.is_null() => p,
        _ => return vec![placeholder],
    };

    // Postgres path: results[0].rows is a list of `{"QUERY PLAN":
    // "..."}` cells — unwrap to the raw text since psql already
    // formats indentation and `->` arrows there.
    if let Some(rows) = plan
        .get("results")
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .and_then(|first| first.get("rows"))
        .and_then(|rs| rs.as_array())
    {
        let lines: Vec<Line<'static>> = rows
            .iter()
            .filter_map(|row| {
                row.as_object()?
                    .values()
                    .next()
                    .and_then(|v| v.as_str())
                    .map(|s| Line::from(Span::raw(format!(" {s}"))))
            })
            .collect();
        if !lines.is_empty() {
            return lines;
        }
    }

    // Fallback for non-postgres dialects (MySQL/SQLite EXPLAIN, or
    // FORMAT JSON variants): pretty-print the whole plan blob.
    let json = serde_json::to_string_pretty(plan).unwrap_or_else(|_| String::from("(plan)"));
    json.lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect()
}

/// Stats tab — formats the connection meta + per-execution stats.
pub(super) fn build_stats_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::White);
    let row = |label: &str, value: String| {
        Line::from(vec![
            Span::styled(format!(" {label}: "), label_style),
            Span::styled(value, value_style),
        ])
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    let Some(value) = b.cached_result.as_ref() else {
        return vec![Line::from(Span::styled(
            " (no result yet — run with `r`)",
            label_style,
        ))];
    };

    let elapsed = value
        .get("stats")
        .and_then(|s| s.get("elapsed_ms"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let results = value
        .get("results")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let total_rows: u64 = value
        .get("results")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    if r.get("kind").and_then(|k| k.as_str()) == Some("select") {
                        r.get("rows")
                            .and_then(|rs| rs.as_array())
                            .map(|rs| rs.len() as u64)
                    } else {
                        None
                    }
                })
                .sum()
        })
        .unwrap_or(0);
    let cached = matches!(b.state, ExecutionState::Cached);

    lines.push(row("elapsed", format!("{elapsed}ms")));
    lines.push(row("results", results.to_string()));
    lines.push(row("rows", total_rows.to_string()));
    lines.push(row("cached", if cached { "yes" } else { "no" }.to_string()));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::BlockId;
    use serde_json::json;

    fn block_with(result: Option<serde_json::Value>) -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "db-sqlite".into(),
            alias: None,
            display_mode: None,
            params: json!({}),
            state: ExecutionState::Idle,
            cached_result: result,
        }
    }

    fn lines_text(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn error_lines_none_when_first_result_is_not_error() {
        let b = block_with(Some(json!({
            "results": [{"kind": "select", "rows": []}],
        })));
        assert!(build_error_lines(&b).is_none());
    }

    #[test]
    fn error_lines_format_with_line_column_suffix() {
        let b = block_with(Some(json!({
            "results": [{
                "kind": "error",
                "message": "syntax error",
                "line": 2,
                "column": 5,
            }],
        })));
        let lines = build_error_lines(&b).unwrap();
        let text = lines_text(&lines);
        assert!(text.contains("syntax error"));
        assert!(text.contains("at 2:5"));
    }

    #[test]
    fn error_lines_format_with_only_line_suffix() {
        let b = block_with(Some(json!({
            "results": [{"kind": "error", "message": "oops", "line": 9}],
        })));
        let text = lines_text(&build_error_lines(&b).unwrap());
        assert!(text.contains("at line 9"));
    }

    #[test]
    fn error_lines_format_without_position() {
        let b = block_with(Some(json!({
            "results": [{"kind": "error", "message": "boom"}],
        })));
        let text = lines_text(&build_error_lines(&b).unwrap());
        assert!(text.contains("error"));
        assert!(text.contains("boom"));
    }

    #[test]
    fn error_lines_default_message_when_missing() {
        let b = block_with(Some(json!({
            "results": [{"kind": "error"}],
        })));
        let text = lines_text(&build_error_lines(&b).unwrap());
        assert!(text.contains("no message"));
    }

    #[test]
    fn messages_lines_placeholder_when_missing_or_empty() {
        let b = block_with(None);
        assert!(lines_text(&build_messages_lines(&b)).contains("no messages"));
        let b = block_with(Some(json!({})));
        assert!(lines_text(&build_messages_lines(&b)).contains("no messages"));
        let b = block_with(Some(json!({"messages": []})));
        assert!(lines_text(&build_messages_lines(&b)).contains("no messages"));
    }

    #[test]
    fn messages_lines_render_each_with_severity_tag() {
        let b = block_with(Some(json!({
            "messages": [
                {"severity": "error", "text": "bad"},
                {"severity": "warning", "text": "be careful"},
                {"severity": "notice", "text": "ok"},
            ],
        })));
        let text = lines_text(&build_messages_lines(&b));
        assert!(text.contains("[error]"));
        assert!(text.contains("bad"));
        assert!(text.contains("[warning]"));
        assert!(text.contains("[notice]"));
    }

    #[test]
    fn plan_lines_placeholder_when_no_plan_field() {
        let b = block_with(None);
        assert!(lines_text(&build_plan_lines(&b)).contains("no plan"));
        let b = block_with(Some(json!({})));
        assert!(lines_text(&build_plan_lines(&b)).contains("no plan"));
        let b = block_with(Some(json!({"plan": serde_json::Value::Null})));
        assert!(lines_text(&build_plan_lines(&b)).contains("no plan"));
    }

    #[test]
    fn plan_lines_unwraps_postgres_shape() {
        let b = block_with(Some(json!({
            "plan": {
                "results": [{
                    "rows": [
                        {"QUERY PLAN": "Seq Scan on users"},
                        {"QUERY PLAN": "  Filter: id = 1"},
                    ]
                }]
            }
        })));
        let text = lines_text(&build_plan_lines(&b));
        assert!(text.contains("Seq Scan"));
        assert!(text.contains("Filter"));
    }

    #[test]
    fn plan_lines_pretty_prints_arbitrary_json_fallback() {
        let b = block_with(Some(json!({
            "plan": {"node": "scan", "cost": 12}
        })));
        let text = lines_text(&build_plan_lines(&b));
        assert!(text.contains("node"));
        assert!(text.contains("scan"));
    }

    #[test]
    fn stats_lines_default_when_no_cached_result() {
        let b = block_with(None);
        assert!(lines_text(&build_stats_lines(&b)).contains("no result yet"));
    }

    #[test]
    fn stats_lines_emits_elapsed_results_rows_cached_rows() {
        let b = block_with(Some(json!({
            "stats": {"elapsed_ms": 25},
            "results": [
                {"kind": "select", "rows": [{"a": 1}, {"a": 2}]},
                {"kind": "select", "rows": [{"a": 3}]},
            ],
        })));
        let text = lines_text(&build_stats_lines(&b));
        assert!(text.contains("25ms"));
        assert!(text.contains("results: 2"));
        assert!(text.contains("rows: 3"));
        assert!(text.contains("cached: no"));
    }

    #[test]
    fn stats_lines_marks_cached_yes_when_state_is_cached() {
        let mut b = block_with(Some(json!({
            "stats": {"elapsed_ms": 1},
            "results": [],
        })));
        b.state = ExecutionState::Cached;
        let text = lines_text(&build_stats_lines(&b));
        assert!(text.contains("cached: yes"));
    }

    #[test]
    fn subtabs_no_op_when_no_results() {
        // Smoke: don't panic on empty cached_result.
        let b = block_with(None);
        let backend = ratatui::backend::TestBackend::new(40, 5);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_result_subtabs(
                    f,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 40,
                        height: 1,
                    },
                    &b,
                    0,
                );
            })
            .unwrap();
    }
}
