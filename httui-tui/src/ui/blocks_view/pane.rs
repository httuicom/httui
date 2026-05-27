use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{region_label, BlockMeta, EditField, FileBlocks, RegionEdit};
use crate::pane::Pane;

use super::BlocksRenderCtx;

pub(super) fn render_leaf(
    frame: &mut Frame,
    area: Rect,
    leaf: &Pane,
    focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
    running: Option<&str>,
    ctx: &mut BlocksRenderCtx<'_>,
) {
    let border_color = if focused {
        crate::ui::palette::accent()
    } else {
        crate::ui::palette::muted()
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let Some(target) = leaf.block_selected else {
        paint_empty_state(frame, inner);
        return;
    };
    let Some(ws) = ctx.workspace else {
        paint_empty_state(frame, inner);
        return;
    };
    let Some(file) = ws.index.files.get(target.file_idx) else {
        paint_missing(frame, inner);
        return;
    };
    let Some(block) = file.blocks.get(target.block_idx) else {
        paint_missing(frame, inner);
        return;
    };

    render_block(
        frame,
        inner,
        file,
        block,
        leaf,
        focused,
        visual_overlay,
        running,
        ctx,
    );
}

fn paint_empty_state(frame: &mut Frame, area: Rect) {
    if area.height < 2 {
        return;
    }
    let muted = Style::default().fg(crate::ui::palette::muted());
    let mid = area.y + area.height / 2;
    let lines = [
        ("Select a block from the sidebar", true),
        ("Enter on a block opens it here · Tab cycles regions", false),
    ];
    for (offset, (text, bold)) in lines.iter().enumerate() {
        let y = mid.saturating_add(offset as u16);
        if y >= area.y + area.height {
            break;
        }
        let style = if *bold {
            muted.add_modifier(Modifier::BOLD)
        } else {
            muted
        };
        let line = Line::from(Span::styled(text.to_string(), style));
        let row = Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(line), row);
    }
}

fn paint_missing(frame: &mut Frame, area: Rect) {
    if area.height < 1 {
        return;
    }
    let muted = Style::default().fg(crate::ui::palette::muted());
    let line = Line::from(Span::styled(
        "(block missing — vault changed?)".to_string(),
        muted,
    ));
    frame.render_widget(Paragraph::new(line), area);
}

#[allow(clippy::too_many_arguments)]
fn render_block(
    frame: &mut Frame,
    area: Rect,
    file: &FileBlocks,
    block: &BlockMeta,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
    running: Option<&str>,
    ctx: &mut BlocksRenderCtx<'_>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    let dirty = pane.block_draft.is_some();
    render_header(frame, chunks[0], file, block, dirty, running);

    let parsed = load_view(ctx.vault, file, block, pane);
    let region = pane.block_region;
    if block.block_type == "http" {
        render_http_regions(
            frame,
            chunks[1],
            region,
            &parsed,
            &block.block_type,
            pane,
            pane_focused,
            visual_overlay,
            running,
        );
    } else if block.block_type.starts_with("db") {
        render_db_regions(
            frame,
            chunks[1],
            region,
            &parsed,
            &block.block_type,
            pane,
            pane_focused,
            visual_overlay,
            file,
            block,
            ctx,
        );
    } else {
        render_fallback(frame, chunks[1], &parsed.raw);
    }
}

fn render_header(
    frame: &mut Frame,
    area: Rect,
    file: &FileBlocks,
    block: &BlockMeta,
    dirty: bool,
    running: Option<&str>,
) {
    if area.width == 0 {
        return;
    }
    let badge = badge_text(&block.block_type);
    let badge_bg = if block.block_type == "http" {
        crate::ui::palette::accent()
    } else {
        crate::ui::palette::popup_border_accent()
    };
    let muted = Style::default().fg(crate::ui::palette::muted());
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(
            badge,
            Style::default()
                .bg(badge_bg)
                .fg(crate::ui::palette::popup_bg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            block.label(),
            Style::default()
                .fg(crate::ui::palette::accent())
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if dirty {
        spans.push(Span::styled(
            " *",
            Style::default()
                .fg(crate::ui::palette::amber())
                .add_modifier(Modifier::BOLD),
        ));
    }
    // `running` no longer surfaces on the block header — it lives on
    // the `[4] Response` / `[3] Result` region title now so the
    // progress sits next to where the bytes will land.
    let _ = running;
    spans.push(Span::styled("  ·  ", muted));
    spans.push(Span::styled(file.display.clone(), muted));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn render_http_regions(
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
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Min(3),
            Constraint::Min(3),
        ])
        .split(area);
    let editing_url = pane_focused
        && pane
            .block_edit
            .as_ref()
            .map(|e| matches!(e.field, EditField::HttpUrl))
            .unwrap_or(false);
    let method = parsed.method.as_deref().unwrap_or("GET").to_string();
    let url_value = pane
        .block_edit
        .as_ref()
        .filter(|e| matches!(e.field, EditField::HttpUrl))
        .map(|e| e.current_text())
        .unwrap_or_else(|| parsed.url.clone().unwrap_or_default());
    render_request_region(
        frame,
        chunks[0],
        block_type,
        region == 0,
        editing_url,
        &method,
        &url_value,
        pane.block_edit
            .as_deref()
            .filter(|_| editing_url),
        visual_overlay,
    );
    render_headers_region(
        frame,
        chunks[1],
        block_type,
        region == 1,
        parsed,
        pane,
        pane_focused,
        visual_overlay,
    );
    render_multiline_region(
        frame,
        chunks[2],
        block_type,
        2,
        region == 2,
        &parsed.body,
        "(no body)",
        pane,
        pane_focused,
        |f| matches!(f, EditField::HttpBody),
        visual_overlay,
    );
    render_http_response_region(
        frame,
        chunks[3],
        block_type,
        region == 3,
        parsed,
        pane,
        running,
    );
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
    let block_widget = region_block(block_type, 3, focused, false);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    let active = pane.response_subtab.min(4);
    paint_response_tabs(frame, chunks[0], active, focused);
    if chunks[1].height == 0 {
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
        chunks[1],
    );
}

fn paint_response_tabs(frame: &mut Frame, area: Rect, active: usize, focused: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    const LABELS: [&str; 5] = ["Body", "Headers", "Cookies", "Timing", "History"];
    let muted = Style::default().fg(crate::ui::palette::muted());
    let on = Style::default()
        .fg(if focused {
            crate::ui::palette::accent()
        } else {
            crate::ui::palette::foreground()
        })
        .add_modifier(Modifier::BOLD);
    let mut spans: Vec<Span> = Vec::with_capacity(LABELS.len() * 2);
    for (i, label) in LABELS.iter().enumerate() {
        if i == active {
            spans.push(Span::styled(format!("▸ {label}"), on));
        } else {
            spans.push(Span::styled(format!("  {label}"), muted));
        }
        if i + 1 < LABELS.len() {
            spans.push(Span::styled("  ", muted));
        }
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
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

/// Convert a `RegionEdit` sub-Document cursor into a visual `(row,
/// col)` pair counted from the start of the buffer text. Works for
/// single-line and multi-line fields since the doc is prose-only.
fn edit_cursor_row_col(edit: &RegionEdit) -> (usize, usize) {
    let offset = match edit.doc.cursor() {
        crate::buffer::Cursor::InProse { offset, .. } => offset,
        crate::buffer::Cursor::InBlock { offset, .. } => offset,
        crate::buffer::Cursor::InBlockResult { .. } => 0,
    };
    let text = edit.current_text();
    let mut row = 0usize;
    let mut col = 0usize;
    for ch in text.chars().take(offset) {
        if ch == '\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (row, col)
}

#[allow(clippy::too_many_arguments)]
fn render_request_region(
    frame: &mut Frame,
    area: Rect,
    block_type: &str,
    focused: bool,
    editing: bool,
    method: &str,
    url: &str,
    edit: Option<&RegionEdit>,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let block_widget = region_block(block_type, 0, focused, editing);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let prefix = format!(" {method}  ");
    let prefix_w = prefix.chars().count() as u16;
    let value_style = if focused {
        Style::default()
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::UNDERLINED)
    } else {
        Style::default()
    };
    let line = Line::from(vec![
        Span::raw(prefix),
        Span::styled(url.to_string(), value_style),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
    if let Some(edit) = edit {
        let (_row, col) = edit_cursor_row_col(edit);
        let cx = inner.x.saturating_add(prefix_w).saturating_add(col as u16);
        if cx < inner.x + inner.width && inner.height > 0 {
            frame.set_cursor_position((cx, inner.y));
        }
        // Paint the visual selection background on top of the URL
        // text. The text area starts at `inner.x + prefix_w`, so we
        // offset the overlay rect to match.
        if let Some(overlay) = visual_overlay {
            let text_area = ratatui::layout::Rect {
                x: inner.x.saturating_add(prefix_w),
                y: inner.y,
                width: inner.width.saturating_sub(prefix_w),
                height: inner.height,
            };
            crate::ui::overlay_visual_selection(
                frame,
                text_area,
                &edit.doc,
                0,
                overlay,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_headers_region(
    frame: &mut Frame,
    area: Rect,
    block_type: &str,
    focused: bool,
    parsed: &ParsedView,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let editing = pane_focused
        && focused
        && pane
            .block_edit
            .as_ref()
            .map(|e| {
                matches!(
                    e.field,
                    EditField::HttpHeaderKey(_) | EditField::HttpHeaderValue(_)
                )
            })
            .unwrap_or(false);
    let block_widget = region_block(block_type, 1, focused, editing);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
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

fn field_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::UNDERLINED)
    } else {
        Style::default()
    }
}

/// Title-bearing border for a region. When `editing` is true, the title
/// gets a trailing `EDIT` chip so it's obvious from any pane width that
/// the keystrokes are flowing into the buffer, not the doc.
fn region_block(block_type: &str, index: usize, focused: bool, editing: bool) -> Block<'static> {
    let (border_color, title_color) = if focused {
        (crate::ui::palette::accent(), crate::ui::palette::accent())
    } else {
        (crate::ui::palette::muted(), crate::ui::palette::muted())
    };
    let title_style = if focused {
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(title_color)
    };
    let label = format!(" [{}] {} ", index + 1, region_label(block_type, index));
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(label, title_style));
    if editing {
        let edit_chip = Span::styled(
            " EDIT ",
            Style::default()
                .bg(crate::ui::palette::amber())
                .fg(crate::ui::palette::popup_bg())
                .add_modifier(Modifier::BOLD),
        );
        block = block.title_top(Line::from(edit_chip).right_aligned());
    }
    block
}

#[allow(clippy::too_many_arguments)]
fn render_db_regions(
    frame: &mut Frame,
    area: Rect,
    region: usize,
    parsed: &ParsedView,
    block_type: &str,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
    file: &FileBlocks,
    block: &BlockMeta,
    ctx: &mut BlocksRenderCtx<'_>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Min(3),
        ])
        .split(area);
    let conn_label = parsed
        .connection
        .as_deref()
        .map(|raw| {
            let name = ctx
                .connection_names
                .get(raw)
                .cloned()
                .unwrap_or_else(|| raw.to_string());
            format!("{name}")
        })
        .unwrap_or_else(|| "(no connection)".to_string());
    render_region(frame, chunks[0], 0, block_type, region == 0, &[conn_label]);
    let query_caret = render_multiline_region(
        frame,
        chunks[1],
        block_type,
        1,
        region == 1,
        &parsed.body,
        "(empty query)",
        pane,
        pane_focused,
        |f| matches!(f, EditField::DbQuery),
        visual_overlay,
    );
    if let Some(cell) = query_caret {
        *ctx.popup_cursor_cell = Some(cell);
    }
    render_db_result_region(
        frame,
        chunks[2],
        block_type,
        region == 2,
        file,
        block,
        pane,
        ctx,
    );
}

/// `[3] Result` — delegates to `ui::blocks::result_tabs` +
/// `ui::blocks::db_table::build_result_table`. Carries the result
/// panel's full tab bar (Result / Messages / Plan / Stats), the
/// real result table widget (header bold + zebra + numeric align +
/// scroll viewport), and the error banner branch.
#[allow(clippy::too_many_arguments)]
fn render_db_result_region(
    frame: &mut Frame,
    area: Rect,
    block_type: &str,
    focused: bool,
    file: &FileBlocks,
    block: &BlockMeta,
    pane: &Pane,
    ctx: &mut BlocksRenderCtx<'_>,
) {
    let block_widget = region_block(block_type, 2, focused, false);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    // Prefer the pane's loaded document — that's where `cached_result`
    // lives after `run_focused_block`. Falls back to disk for a fresh
    // pane that hasn't loaded the file yet (no cached_result).
    let block_node = match block_node_from_pane(pane, file, block)
        .or_else(|| load_block_node(ctx.vault, file, block))
    {
        Some(b) => b,
        None => {
            render_region(
                frame,
                area,
                2,
                block_type,
                focused,
                &["(no result — press r to run)".to_string()],
            );
            return;
        }
    };
    let key = block_node_id(file, block);
    let viewport_key: usize = key.0 as usize;
    let tab = ctx
        .result_tabs
        .get(&key)
        .copied()
        .unwrap_or(crate::app::ResultPanelTab::Result);
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(0),
        ])
        .split(inner);
    crate::ui::blocks::result_tabs::render_result_tab_bar_for(
        frame,
        chunks[0],
        tab,
        None,
        block_type,
    );
    if chunks[1].height == 0 {
        return;
    }
    use crate::app::ResultPanelTab;
    match tab {
        ResultPanelTab::Result => {
            ctx.result_viewport_top.entry(viewport_key).or_insert(0);
            let selected_row = if focused { Some(pane.block_row) } else { None };
            if let Some((table, viewport_selected)) =
                crate::ui::blocks::db_table::build_result_table(
                    &block_node,
                    selected_row,
                    ctx.result_viewport_top.get_mut(&viewport_key),
                )
            {
                let mut state = ratatui::widgets::TableState::default();
                state.select(viewport_selected);
                let table = table.row_highlight_style(
                    Style::default()
                        .bg(crate::ui::palette::selection_bg())
                        .add_modifier(Modifier::BOLD),
                );
                frame.render_stateful_widget(table, chunks[1], &mut state);
            } else if let Some(lines) =
                crate::ui::blocks::result_tabs::build_error_lines(&block_node)
            {
                frame.render_widget(Paragraph::new(lines), chunks[1]);
            } else {
                frame.render_widget(
                    Paragraph::new("(no result — press r to run)"),
                    chunks[1],
                );
            }
        }
        ResultPanelTab::Messages => {
            let lines = crate::ui::blocks::result_tabs::build_messages_lines(&block_node);
            frame.render_widget(Paragraph::new(lines), chunks[1]);
        }
        ResultPanelTab::Plan => {
            let lines = crate::ui::blocks::result_tabs::build_plan_lines(&block_node);
            frame.render_widget(Paragraph::new(lines), chunks[1]);
        }
        ResultPanelTab::Stats => {
            let lines = crate::ui::blocks::result_tabs::build_stats_lines(&block_node);
            frame.render_widget(Paragraph::new(lines), chunks[1]);
        }
        ResultPanelTab::Raw => {
            // DB blocks don't expose Raw — fall back to Stats.
            let lines = crate::ui::blocks::result_tabs::build_stats_lines(&block_node);
            frame.render_widget(Paragraph::new(lines), chunks[1]);
        }
    }
}

/// Stable id per (file, block) used to key into `App::result_tabs`
/// and `App::result_viewport_top`. Hash combines display path +
/// block line_start so two blocks with the same alias in different
/// files don't share viewport state.
fn block_node_id(file: &FileBlocks, block: &BlockMeta) -> crate::buffer::block::BlockId {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    file.display.hash(&mut h);
    block.line_start.hash(&mut h);
    crate::buffer::block::BlockId(h.finish())
}

/// Try to extract the matching `BlockNode` from the pane's loaded
/// document. Returns `None` when the pane isn't on this file (e.g.
/// fresh tab) — caller falls back to a disk read for static info.
fn block_node_from_pane(
    pane: &Pane,
    file: &FileBlocks,
    block: &BlockMeta,
) -> Option<crate::buffer::block::BlockNode> {
    let pane_path = pane.document_path.as_ref()?;
    let pane_rel = if pane_path.is_absolute() {
        pane_path.strip_prefix(pane_path.ancestors().last()?).ok()?
    } else {
        pane_path.as_path()
    };
    let target_rel = file.path.as_path();
    let matches = pane_path.ends_with(target_rel)
        || pane_rel.ends_with(target_rel)
        || pane_path == target_rel;
    if !matches {
        return None;
    }
    let doc = pane.document.as_ref()?;
    for seg in doc.segments() {
        if let crate::buffer::Segment::Block(b) = seg {
            if b.block_type == block.block_type && b.alias == block.alias {
                return Some(b.clone());
            }
        }
    }
    None
}

/// Read the file from disk, find the matching block segment, return
/// its `BlockNode`. No `cached_result` (results only live in the
/// in-memory pane Document — disk has just the raw fence). Falls
/// back to `None` if the file can't be read or no matching block
/// exists.
fn load_block_node(
    vault: &Path,
    file: &FileBlocks,
    block: &BlockMeta,
) -> Option<crate::buffer::block::BlockNode> {
    use crate::buffer::Segment;
    let doc = crate::document_loader::load_document(vault, &file.path).ok()?;
    let mut found: Option<crate::buffer::block::BlockNode> = None;
    let mut next_line = 0usize;
    for seg in doc.segments() {
        match seg {
            Segment::Prose(rope) => {
                next_line += rope.len_lines().max(1);
            }
            Segment::Block(b) => {
                if next_line == block.line_start && b.block_type == block.block_type {
                    found = Some(b.clone());
                    break;
                }
                next_line += b.raw.lines().count().max(1);
            }
        }
    }
    found
}

/// Render a region whose value is a multi-line string (HTTP body / DB
/// query). When the pane is in EDIT for the matching field, paints the
/// `MultilineBuffer` contents in place of the disk value and places the
/// terminal caret at the buffer's (row, col).
#[allow(clippy::too_many_arguments)]
fn render_multiline_region(
    frame: &mut Frame,
    area: Rect,
    block_type: &str,
    index: usize,
    focused: bool,
    fallback: &str,
    placeholder: &str,
    pane: &Pane,
    pane_focused: bool,
    field_matches: impl Fn(&EditField) -> bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) -> Option<(u16, u16)> {
    let editing = pane_focused
        && focused
        && pane
            .block_edit
            .as_ref()
            .map(|e| field_matches(&e.field))
            .unwrap_or(false);
    let block_widget = region_block(block_type, index, focused, editing);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return None;
    }
    let active_edit = pane
        .block_edit
        .as_ref()
        .filter(|e| field_matches(&e.field));
    let (text, caret): (String, Option<(u16, u16)>) = if let Some(edit) = active_edit {
        let text = edit.current_text();
        let (row, col) = edit_cursor_row_col(edit);
        let cy = inner.y.saturating_add(row as u16);
        let cx = inner.x.saturating_add(col as u16);
        (text, Some((cx, cy)))
    } else if fallback.is_empty() {
        (placeholder.to_string(), None)
    } else {
        (fallback.to_string(), None)
    };
    // SQL blocks pick up the same syntax highlighter the DOC view
    // uses (`ui::sql_highlight::highlight`). Anything else paints
    // plain so URL/headers/HTTP body still render verbatim.
    let rendered: Vec<Line<'static>> = if block_type.starts_with("db") {
        crate::ui::sql_highlight::highlight(&text)
            .into_iter()
            .map(Line::from)
            .collect()
    } else {
        text.split('\n')
            .map(|l| Line::from(Span::raw(l.to_string())))
            .collect()
    };
    frame.render_widget(Paragraph::new(rendered), inner);
    if let Some((cx, cy)) = caret {
        if cx < inner.x + inner.width && cy < inner.y + inner.height {
            frame.set_cursor_position((cx, cy));
        }
    }
    // Visual-mode selection overlay over the multi-line text. Only
    // paints while EDIT is active and the engine is in Visual /
    // VisualLine on the sub-doc.
    if let (Some(edit), Some(overlay)) = (active_edit, visual_overlay) {
        crate::ui::overlay_visual_selection(frame, inner, &edit.doc, 0, overlay);
    }
    caret
}

fn render_region(
    frame: &mut Frame,
    area: Rect,
    index: usize,
    block_type: &str,
    focused: bool,
    lines: &[String],
) {
    let (border_color, title_color) = if focused {
        (crate::ui::palette::accent(), crate::ui::palette::accent())
    } else {
        (crate::ui::palette::muted(), crate::ui::palette::muted())
    };
    let title_style = if focused {
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(title_color)
    };
    let block_widget = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            format!(" [{}] {} ", index + 1, region_label(block_type, index)),
            title_style,
        ));
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let rendered: Vec<Line<'static>> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let style = if focused && i == 0 {
                Style::default()
                    .fg(crate::ui::palette::foreground())
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default()
            };
            Line::from(Span::styled(l.clone(), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(rendered).wrap(Wrap { trim: false }), inner);
}

fn render_fallback(frame: &mut Frame, area: Rect, raw: &str) {
    let lines: Vec<Line<'static>> = raw
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        area,
    );
}

struct ParsedView {
    method: Option<String>,
    url: Option<String>,
    headers: Vec<(String, String)>,
    body: String,
    connection: Option<String>,
    cached: String,
    /// Raw JSON output of the last run (HTTP / DB executor). Sub-tab
    /// renderers slice off body / headers / cookies / timing from this.
    cached_json: Option<serde_json::Value>,
    raw: String,
}

fn load_view(
    vault_path: &Path,
    file: &FileBlocks,
    block: &BlockMeta,
    pane: &Pane,
) -> ParsedView {
    // Pull cached run output from the pane's loaded Document if it
    // matches the block. Populated by `run_focused_block` after a
    // successful execution; renderer turns it into the "[4] Response"
    // body string. Run output never sits on disk (it lives in SQLite
    // cache + in-memory `BlockNode.cached_result`), so this is the
    // only path the renderer can surface it from.
    let cached_from_pane = pane.document.as_ref().and_then(|doc| {
        for seg in doc.segments() {
            if let crate::buffer::Segment::Block(b) = seg {
                if b.block_type == block.block_type && b.alias == block.alias {
                    return b
                        .cached_result
                        .as_ref()
                        .map(|v| (serialize_cached_result(v), v.clone()));
                }
            }
        }
        None
    });
    // Per-pane draft wins over disk so committed edits are reflected
    // before save (otherwise the renderer would still show stale
    // values after Esc). Draft and disk parse share the same
    // ParsedBlock shape, so the view mapping is identical.
    if let Some(draft) = pane.block_draft.as_ref() {
        if draft.block_line_start == block.line_start && draft.block.block_type == block.block_type
        {
            let raw = httui_core::blocks::serialize_block(&draft.block);
            let mut view = parsed_to_view(&draft.block, raw);
            if let Some((c, json)) = cached_from_pane {
                view.cached = c;
                view.cached_json = Some(json);
            }
            return view;
        }
    }
    let Ok(raw) = httui_core::fs::read_note(
        &vault_path.to_string_lossy(),
        &file.path.to_string_lossy(),
    ) else {
        return ParsedView::empty();
    };
    let parsed = httui_core::blocks::parse_blocks(&raw);
    let Some(p) = parsed.iter().find(|p| {
        p.line_start == block.line_start && p.block_type == block.block_type
    }) else {
        return ParsedView::empty();
    };
    let lines: Vec<&str> = raw.lines().collect();
    let end = p.line_end.min(lines.len().saturating_sub(1));
    let start = p.line_start.min(end);
    let raw_block = lines[start..=end].join("\n");
    let mut view = parsed_to_view(p, raw_block);
    if let Some((c, json)) = cached_from_pane {
        view.cached = c;
        view.cached_json = Some(json);
    }
    view
}

/// Best-effort serialization of a cached HTTP/DB result for the
/// `[4] Response` / `[3] Result` panel. The first line carries the
/// summary chip (`200 OK · 937ms · 2.1kb`) — the user reads that as
/// the in-panel "status line" without the global footer needing to
/// surface block-specific data. The body / rows follow underneath.
fn serialize_cached_result(v: &serde_json::Value) -> String {
    if let Some(obj) = v.as_object() {
        if let Some(status) = obj.get("status").and_then(|s| s.as_i64()) {
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
            let mut out = summary;
            if let Some(body) = obj.get("body") {
                let body_str = match body {
                    serde_json::Value::String(s) => s.clone(),
                    _ => serde_json::to_string_pretty(body)
                        .unwrap_or_else(|_| body.to_string()),
                };
                out.push('\n');
                out.push_str(&body_str);
            }
            return out;
        }
        if let Some(results) = obj.get("results").and_then(|r| r.as_array()) {
            if let Some(first) = results.first() {
                return serde_json::to_string_pretty(first)
                    .unwrap_or_else(|_| first.to_string());
            }
        }
    }
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

fn format_bytes(n: u64) -> String {
    if n < 1024 {
        format!("{n}b")
    } else if n < 1024 * 1024 {
        format!("{:.1}kb", n as f64 / 1024.0)
    } else {
        format!("{:.1}mb", n as f64 / (1024.0 * 1024.0))
    }
}

fn parsed_to_view(p: &httui_core::blocks::parser::ParsedBlock, raw: String) -> ParsedView {
    let method = p
        .params
        .get("method")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let url = p
        .params
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let headers = p
        .params
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|h| {
                    let k = h
                        .get("key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let v = h
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    (k, v)
                })
                .collect()
        })
        .unwrap_or_default();
    let body = p
        .params
        .get("body")
        .and_then(|v| v.as_str())
        .or_else(|| p.params.get("query").and_then(|v| v.as_str()))
        .unwrap_or("")
        .trim_end_matches('\n')
        .to_string();
    let connection = p
        .params
        .get("connection")
        .or_else(|| p.params.get("connection_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    ParsedView {
        method,
        url,
        headers,
        body,
        connection,
        cached: String::new(),
        cached_json: None,
        raw,
    }
}

impl ParsedView {
    fn empty() -> Self {
        Self {
            method: None,
            url: None,
            headers: Vec::new(),
            body: String::new(),
            connection: None,
            cached: String::new(),
            cached_json: None,
            raw: String::new(),
        }
    }
}

pub(super) fn paint_picker_overlay(frame: &mut Frame, area: Rect, n: usize) {
    if area.width < 5 || area.height < 3 {
        return;
    }
    let letter = if (1..=26).contains(&n) {
        (b'a' + (n - 1) as u8) as char
    } else {
        '?'
    };
    let label = format!("[ {letter} ]");
    let cy = area.y + area.height / 2;
    let label_w = label.chars().count() as u16;
    let cx = area.x + area.width.saturating_sub(label_w) / 2;
    let style = Style::default()
        .bg(crate::ui::palette::popup_border_accent())
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let row = Rect {
        x: cx,
        y: cy,
        width: label_w.min(area.width),
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(label, style))),
        row,
    );
}

fn badge_text(block_type: &str) -> String {
    if block_type == "http" {
        " HTTP ".into()
    } else if block_type.starts_with("db") {
        " DB ".into()
    } else {
        format!(" {block_type} ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_text_classifies_kinds() {
        assert_eq!(badge_text("http"), " HTTP ");
        assert_eq!(badge_text("db-postgres"), " DB ");
        assert_eq!(badge_text("custom"), " custom ");
    }
}
