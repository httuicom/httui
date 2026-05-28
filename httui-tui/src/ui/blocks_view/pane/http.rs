use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_http_regions(
    frame: &mut Frame,
    area: Rect,
    region: usize,
    parsed: &ParsedView,
    block_type: &str,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
    running: Option<&str>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Min(3)])
        .split(area);

    // Request card: one border + a [Headers│Body] tab bar. The active
    // tab follows the focused region (1=Headers, 2=Body); when focus is
    // on the URL (0) or Response (3) the card shows Headers, unfocused.
    let req_focused = pane_focused && (region == 1 || region == 2);
    let active_req = if region == 2 { 2 } else { 1 };
    let card = card_block("[2] REQUEST", req_focused);
    let inner = card.inner(chunks[0]);
    frame.render_widget(card, chunks[0]);
    if inner.width > 0 && inner.height > 0 {
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
            ])
            .split(inner);
        render_request_tabbar(frame, parts[0], active_req, req_focused);
        render_tab_separator(frame, parts[1]);
        if parts[2].height > 0 {
            if active_req == 2 {
                render_multiline_region(
                    frame,
                    parts[2],
                    block_type,
                    region == 2,
                    &parsed.body,
                    "(no body)",
                    pane,
                    pane_focused,
                    |f| matches!(f, EditField::HttpBody),
                    visual_overlay,
                );
            } else {
                render_headers_region(
                    frame,
                    parts[2],
                    region == 1,
                    parsed,
                    pane,
                    pane_focused,
                    visual_overlay,
                );
            }
        }
    }

    render_http_response_region(
        frame,
        chunks[1],
        block_type,
        region == 3,
        parsed,
        pane,
        running,
    );
}

/// `Headers │ Body` tab cells for the HTTP request card. `active` is the
/// focused region (1=Headers, 2=Body) mapped to cell index.
fn render_request_tabbar(frame: &mut Frame, area: Rect, active: usize, focused: bool) {
    let labels = ["Headers".to_string(), "Body".to_string()];
    let active_cell = if active == 2 { 1 } else { 0 };
    render_subtab_cells(frame, area, &labels, active_cell, focused);
}

/// `[4] Response` with sub-tabs `Body / Headers / Cookies / Timing /
/// History`. Tab header sits inside the region's inner area; body
/// fills the remainder. Sub-tab driven by `pane.response_subtab`.
fn render_http_response_region(
    frame: &mut Frame,
    area: Rect,
    block_type: &str,
    focused: bool,
    parsed: &ParsedView,
    pane: &Pane,
    running: Option<&str>,
) {
    let _ = block_type;
    let block_widget = card_block("[3] RESPONSE", focused);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);
    let active = pane.response_subtab.min(4);
    paint_response_tabs(frame, chunks[0], active, focused);
    render_tab_separator(frame, chunks[1]);
    if chunks[2].height == 0 {
        return;
    }
    let lines = if let Some(label) = running {
        vec![label.to_string()]
    } else if active == 4 {
        history_lines(pane)
    } else if parsed.cached_json.is_none() {
        vec!["(no response — press r to run)".to_string()]
    } else {
        response_subtab_lines(parsed, active)
    };
    let lines: Vec<Line> = lines
        .into_iter()
        .map(|s| Line::from(Span::raw(s)))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        chunks[2],
    );
}

fn paint_response_tabs(frame: &mut Frame, area: Rect, active: usize, focused: bool) {
    let labels: Vec<String> = ["Body", "Headers", "Cookies", "Timing", "History"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    render_subtab_cells(frame, area, &labels, active, focused);
}

fn history_lines(pane: &Pane) -> Vec<String> {
    let Some(history) = pane.response_history.as_deref() else {
        return vec!["(no history — switch to History tab to load)".to_string()];
    };
    if history.entries.is_empty() {
        return vec!["(no runs recorded for this block)".to_string()];
    }
    history
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let cursor = if i == history.cursor { "▸" } else { " " };
            let status = e
                .status
                .map(|s| format!("{s}"))
                .unwrap_or_else(|| "—".to_string());
            let elapsed = e
                .elapsed_ms
                .map(|m| format!("{m}ms"))
                .unwrap_or_else(|| "—".to_string());
            let size = e
                .response_size
                .map(|s| format_bytes(s as u64))
                .unwrap_or_else(|| "—".to_string());
            format!(
                "{cursor} {ran_at}  {method:<6} {status:<4} {elapsed:>7}  {size}",
                ran_at = e.ran_at,
                method = e.method
            )
        })
        .collect()
}

fn response_subtab_lines(parsed: &ParsedView, subtab: usize) -> Vec<String> {
    let Some(json) = parsed.cached_json.as_ref() else {
        return vec!["(no cached response)".to_string()];
    };
    match subtab {
        0 => cached_body_lines(json),
        1 => cached_headers_lines(json),
        2 => cached_cookies_lines(json),
        3 => cached_timing_lines(json),
        _ => vec!["(history: press Enter on an entry to replay)".to_string()],
    }
}

fn cached_body_lines(json: &serde_json::Value) -> Vec<String> {
    let summary_first_line = parsed_cached_summary_line(json);
    let body = json
        .get("body")
        .or_else(|| json.get("results"))
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let body_text = match &body {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        _ => serde_json::to_string_pretty(&body).unwrap_or_else(|_| body.to_string()),
    };
    let mut out = Vec::new();
    if let Some(summary) = summary_first_line {
        out.push(summary);
        out.push(String::new());
    }
    out.extend(body_text.lines().map(str::to_string));
    if out.is_empty() {
        out.push("(empty body)".to_string());
    }
    out
}

fn parsed_cached_summary_line(json: &serde_json::Value) -> Option<String> {
    let obj = json.as_object()?;
    let status = obj.get("status").and_then(|s| s.as_i64())?;
    let mut summary = format!("{status}");
    if let Some(text) = obj.get("status_text").and_then(|s| s.as_str()) {
        if !text.is_empty() {
            summary.push(' ');
            summary.push_str(text);
        }
    }
    if let Some(elapsed) = obj
        .get("elapsed_ms")
        .or_else(|| obj.get("total_ms"))
        .and_then(|v| v.as_u64())
    {
        summary.push_str(" · ");
        summary.push_str(&format!("{elapsed}ms"));
    }
    if let Some(size) = obj.get("size_bytes").and_then(|v| v.as_u64()) {
        summary.push_str(" · ");
        summary.push_str(&format_bytes(size));
    }
    Some(summary)
}

fn cached_headers_lines(json: &serde_json::Value) -> Vec<String> {
    let arr = json
        .get("headers")
        .and_then(|h| h.as_array())
        .cloned()
        .unwrap_or_default();
    if arr.is_empty() {
        return vec!["(no headers)".to_string()];
    }
    arr.iter()
        .map(|h| {
            let name = h.get("name").or_else(|| h.get("key")).and_then(|v| v.as_str()).unwrap_or("");
            let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
            format!("{name}: {value}")
        })
        .collect()
}

fn cached_cookies_lines(json: &serde_json::Value) -> Vec<String> {
    let arr = json
        .get("cookies")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    if arr.is_empty() {
        return vec!["(no cookies)".to_string()];
    }
    arr.iter()
        .map(|c| {
            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let value = c.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let domain = c.get("domain").and_then(|v| v.as_str()).unwrap_or("");
            if domain.is_empty() {
                format!("{name} = {value}")
            } else {
                format!("{name} = {value}    domain={domain}")
            }
        })
        .collect()
}

fn cached_timing_lines(json: &serde_json::Value) -> Vec<String> {
    let obj = match json.as_object() {
        Some(o) => o,
        None => return vec!["(no timing)".to_string()],
    };
    let mut out: Vec<String> = Vec::new();
    let push = |out: &mut Vec<String>, label: &str, key: &str| {
        if let Some(ms) = obj.get(key).and_then(|v| v.as_u64()) {
            out.push(format!("{label:<20} {ms}ms"));
        }
    };
    push(&mut out, "Total", "total_ms");
    push(&mut out, "TTFB", "ttfb_ms");
    push(&mut out, "DNS", "dns_ms");
    push(&mut out, "Connect", "connect_ms");
    push(&mut out, "TLS", "tls_ms");
    if let Some(reused) = obj.get("connection_reused").and_then(|v| v.as_bool()) {
        out.push(format!("{:<20} {reused}", "Conn reused"));
    }
    if out.is_empty() {
        out.push("(no timing data)".to_string());
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn render_headers_region(
    frame: &mut Frame,
    inner: Rect,
    focused: bool,
    parsed: &ParsedView,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let _ = pane_focused;
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if parsed.headers.is_empty() && pane.block_edit.is_none() {
        let muted = Style::default().fg(crate::ui::palette::muted());
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("(no headers)", muted))),
            inner,
        );
        return;
    }
    let key_w: u16 = parsed
        .headers
        .iter()
        .map(|(k, _)| k.chars().count() as u16)
        .max()
        .unwrap_or(8)
        .max(8);
    let cursor_row = if focused { pane.block_row } else { usize::MAX };
    let cursor_col = if focused { pane.block_col } else { usize::MAX };
    let edit_row_col = pane.block_edit.as_ref().and_then(|e| match e.field {
        EditField::HttpHeaderKey(r) => Some((r, 0usize)),
        EditField::HttpHeaderValue(r) => Some((r, 1usize)),
        _ => None,
    });
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(parsed.headers.len());
    for (i, (k, v)) in parsed.headers.iter().enumerate() {
        let mut key_text = k.clone();
        let mut value_text = v.clone();
        if let Some((row, col)) = edit_row_col {
            if row == i {
                let buf = pane
                    .block_edit
                    .as_ref()
                    .map(|e| e.current_text())
                    .unwrap_or_default();
                if col == 0 {
                    key_text = buf;
                } else {
                    value_text = buf;
                }
            }
        }
        let key_focused = cursor_row == i && cursor_col == 0;
        let value_focused = cursor_row == i && cursor_col == 1;
        let key_style = field_style(key_focused);
        let value_style = field_style(value_focused);
        let padded_key = format!("{key_text:<width$}", width = key_w as usize);
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(padded_key, key_style),
            Span::raw("  "),
            Span::styled(value_text, value_style),
        ]));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    // Place the terminal caret at the edited field's cursor column,
    // accounting for the leading padding and the key column width.
    if let Some(edit) = pane.block_edit.as_ref() {
        let (row, col) = match edit.field {
            EditField::HttpHeaderKey(r) => (r, 0usize),
            EditField::HttpHeaderValue(r) => (r, 1usize),
            _ => return,
        };
        if row >= parsed.headers.len() {
            return;
        }
        let row_y = inner.y.saturating_add(row as u16);
        if row_y >= inner.y + inner.height {
            return;
        }
        let leading = 2u16;
        let (_doc_row, doc_col) = edit_cursor_row_col(edit);
        let cell_col = doc_col as u16;
        let base_x = if col == 0 {
            inner.x + leading
        } else {
            inner.x + leading + key_w + 2
        };
        let cx = base_x.saturating_add(cell_col);
        if cx < inner.x + inner.width {
            frame.set_cursor_position((cx, row_y));
        }
        // Visual selection overlay over the edited field's cell.
        if let Some(overlay) = visual_overlay {
            let cell_w = if col == 0 {
                key_w
            } else {
                inner.width.saturating_sub(leading + key_w + 2)
            };
            let cell_area = ratatui::layout::Rect {
                x: base_x,
                y: row_y,
                width: cell_w,
                height: 1,
            };
            crate::ui::overlay_visual_selection(
                frame,
                cell_area,
                &edit.doc,
                0,
                overlay,
            );
        }
    }
}

