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
    file: &FileBlocks,
    block: &BlockMeta,
    ctx: &BlocksRenderCtx<'_>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Min(3)])
        .split(area);

    // Request region: accent rail + a Headers│Body tab row. The active
    // tab follows the focused region (1=Headers, 2=Body); when focus is
    // on the URL (0) or Response (3) the region shows Headers, dimmed.
    let req_focused = pane_focused && (region == 1 || region == 2);
    let active_cell = if region == 2 { 1 } else { 0 };
    let inner = region_frame(frame, chunks[0], req_focused);
    if inner.width > 0 && inner.height > 0 {
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);
        render_region_tabs(
            frame,
            parts[0],
            "Request",
            &["Headers".to_string(), "Body".to_string()],
            active_cell,
            req_focused,
        );
        if parts[1].height > 0 {
            if active_cell == 1 {
                render_multiline_region(
                    frame,
                    parts[1],
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
                    parts[1],
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
        pane_focused && region == 3,
        file,
        block,
        pane,
        ctx,
        running,
    );
}

/// `[3] RESPONSE` — tab strip (Body / Headers / Cookies / Timing / Raw)
/// over the shared `ResultPanelTab`; content is delegated to the DOC
/// view's `render_http_response_panel` so highlighting is identical.
#[allow(clippy::too_many_arguments)]
fn render_http_response_region(
    frame: &mut Frame,
    area: Rect,
    focused: bool,
    file: &FileBlocks,
    block: &BlockMeta,
    pane: &Pane,
    ctx: &BlocksRenderCtx<'_>,
    running: Option<&str>,
) {
    let inner = region_frame(frame, area, focused);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    // Shared, BlockId-keyed selection — same map the DOC view cycles, so
    // a sub-tab choice carries across views/panes for the same block.
    let tab = ctx
        .result_tabs
        .get(&block_node_id(file, block))
        .copied()
        .unwrap_or(crate::app::ResultPanelTab::Result);
    let variants = crate::app::ResultPanelTab::variants_for("http");
    let labels: Vec<String> = variants
        .iter()
        .map(|t| t.label_for("http").to_string())
        .collect();
    let active_cell = variants.iter().position(|t| *t == tab).unwrap_or(0);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    render_region_tabs(frame, chunks[0], "Response", &labels, active_cell, focused);
    if chunks[1].height == 0 {
        return;
    }
    if let Some(label) = running {
        frame.render_widget(Paragraph::new(label.to_string()), chunks[1]);
        return;
    }
    // Results live only in the in-memory pane Document; disk has none.
    match block_node_from_pane(pane, file, block)
        .or_else(|| load_block_node(ctx.vault, file, block))
    {
        Some(b) if b.cached_result.is_some() => {
            crate::ui::blocks::http_panel::response::render_http_response_panel(
                frame,
                chunks[1],
                &b,
                tab,
            );
        }
        _ => {
            frame.render_widget(
                Paragraph::new("(no response — press r to run)"),
                chunks[1],
            );
        }
    }
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
        let mut lines = vec![Line::from(Span::styled("  (no headers)", muted))];
        if focused {
            lines.push(add_header_hint());
        }
        frame.render_widget(Paragraph::new(lines), inner);
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
        let mut row_spans = vec![
            Span::raw("  "),
            Span::styled(padded_key, key_style),
            Span::raw("  "),
        ];
        // The cell under edit shows the raw buffer (caret tracked
        // below); idle cells chip `{{ref}}` like the DOC view.
        if edit_row_col == Some((i, 1)) {
            row_spans.push(Span::styled(value_text, value_style));
        } else {
            row_spans.extend(refs_spans(&value_text, value_focused));
        }
        lines.push(Line::from(row_spans));
    }
    if focused {
        lines.push(add_header_hint());
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

/// Dim affordance shown under the headers when the Request region is
/// focused — surfaces the otherwise-invisible "press `o` to add a row".
fn add_header_hint() -> Line<'static> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "＋ add header",
            Style::default().fg(crate::ui::palette::muted()),
        ),
        Span::raw("   "),
        Span::styled(
            "o",
            Style::default()
                .fg(crate::ui::palette::accent())
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

