use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_header(
    frame: &mut Frame,
    area: Rect,
    block: &BlockMeta,
    parsed: &ParsedView,
    dirty: bool,
    running: Option<&str>,
    ctx: &mut BlocksRenderCtx<'_>,
    pane: &Pane,
    pane_focused: bool,
    region: usize,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    if area.width == 0 {
        return;
    }
    let focused = pane_focused && region == 0;
    let inner = region_frame(frame, area, focused);
    if inner.width == 0 {
        return;
    }
    let is_http = block.block_type == "http";
    let method = parsed.method.as_deref().unwrap_or("GET");
    let url_edit = pane
        .block_edit
        .as_ref()
        .filter(|e| pane_focused && matches!(e.field, EditField::HttpUrl));

    // Identity reads alias → method → target, all as text (no chips).
    let mut left: Vec<Span<'static>> = Vec::new();
    if let Some(alias) = block.alias.as_deref().filter(|a| !a.is_empty()) {
        left.push(Span::styled(
            alias.to_string(),
            Style::default()
                .fg(crate::ui::palette::foreground())
                .add_modifier(Modifier::BOLD),
        ));
        left.push(Span::raw("  "));
    }
    let (method_label, method_color) = if is_http {
        (method.to_string(), method_chip_color(method))
    } else {
        ("SQL".to_string(), crate::ui::palette::popup_border_accent())
    };
    left.push(Span::styled(
        method_label,
        Style::default()
            .fg(method_color)
            .add_modifier(Modifier::BOLD),
    ));
    left.push(Span::raw("  "));
    let url_start_x = inner.x
        + left
            .iter()
            .map(|s| s.content.chars().count())
            .sum::<usize>() as u16;
    if is_http {
        if let Some(edit) = url_edit {
            // Verbatim while editing so the caret maps 1:1 to bytes.
            left.push(Span::styled(
                edit.current_text(),
                Style::default()
                    .fg(crate::ui::palette::foreground())
                    .add_modifier(Modifier::UNDERLINED),
            ));
        } else {
            let url = parsed.url.clone().unwrap_or_default();
            left.extend(refs_spans(&url, false));
        }
    } else {
        let conn = parsed
            .connection
            .as_deref()
            .map(|raw| {
                ctx.connection_names
                    .get(raw)
                    .cloned()
                    .unwrap_or_else(|| raw.to_string())
            })
            .unwrap_or_else(|| "(no connection)".to_string());
        left.push(Span::styled(
            conn,
            Style::default().fg(crate::ui::palette::muted()),
        ));
    }
    if dirty {
        left.push(Span::styled(
            "  •",
            Style::default()
                .fg(crate::ui::palette::amber())
                .add_modifier(Modifier::BOLD),
        ));
    }

    let mut right: Vec<Span<'static>> = Vec::new();
    if let Some(label) = running {
        right.push(Span::styled(
            label.to_string(),
            Style::default()
                .fg(crate::ui::palette::amber())
                .add_modifier(Modifier::BOLD),
        ));
        right.push(Span::raw("  "));
    } else if let Some((badge, badge_color, latency)) = run_summary(parsed, is_http) {
        right.push(Span::styled(
            badge,
            Style::default()
                .fg(badge_color)
                .add_modifier(Modifier::BOLD),
        ));
        if let Some(latency) = latency {
            right.push(Span::styled(
                " · ",
                Style::default().fg(crate::ui::palette::muted()),
            ));
            right.push(Span::styled(
                latency,
                Style::default().fg(crate::ui::palette::muted()),
            ));
        }
        right.push(Span::raw("  "));
    }
    right.push(Span::styled(
        "▶ Run",
        Style::default()
            .fg(crate::ui::palette::accent())
            .add_modifier(Modifier::BOLD),
    ));
    right.push(Span::raw(" "));

    let left_w: usize = left.iter().map(|s| s.content.chars().count()).sum();
    let right_w: usize = right.iter().map(|s| s.content.chars().count()).sum();
    let total = inner.width as usize;
    let mut spans = left;
    if total > left_w + right_w {
        spans.push(Span::raw(" ".repeat(total - left_w - right_w)));
        spans.extend(right);
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), inner);

    if let Some(edit) = url_edit {
        let (_row, col) = edit_cursor_row_col(edit);
        let cx = url_start_x.saturating_add(col as u16);
        if cx < inner.x + inner.width {
            frame.set_cursor_position((cx, inner.y));
            // Publish the caret cell so the completion popup anchors under
            // this pane's URL field (not the editor area's left edge in
            // multi-pane layouts).
            *ctx.popup_cursor_cell = Some((cx, inner.y));
        }
        if let Some(overlay) = visual_overlay {
            let text_area = Rect {
                x: url_start_x,
                y: inner.y,
                width: inner.width.saturating_sub(url_start_x - inner.x),
                height: 1,
            };
            crate::ui::overlay_visual_selection(frame, text_area, &edit.doc, 0, overlay);
        }
    }
}

fn method_chip_color(method: &str) -> ratatui::style::Color {
    crate::ui::blocks::http_panel::method_color(method)
}

fn run_summary(
    parsed: &ParsedView,
    is_http: bool,
) -> Option<(String, ratatui::style::Color, Option<String>)> {
    let json = parsed.cached_json.as_ref()?;
    let obj = json.as_object()?;
    let latency = obj
        .get("elapsed_ms")
        .or_else(|| obj.get("total_ms"))
        .and_then(|v| v.as_u64())
        .map(|ms| format!("{ms}ms"));
    if is_http {
        let status = obj.get("status").and_then(|s| s.as_i64())?;
        let color = match status {
            200..=299 => crate::ui::palette::success(),
            300..=399 => crate::ui::palette::accent(),
            _ => crate::ui::palette::amber(),
        };
        Some((status.to_string(), color, latency))
    } else {
        let rows = obj
            .get("results")
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .and_then(|r| r.get("rows"))
            .and_then(|r| r.as_array())
            .map(|r| r.len());
        let label = match rows {
            Some(1) => "1 row".to_string(),
            Some(n) => format!("{n} rows"),
            None => "done".to_string(),
        };
        Some((label, crate::ui::palette::popup_border_accent(), latency))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::palette;

    #[test]
    fn run_summary_colors_status_by_class() {
        let mut pv = ParsedView::empty();
        pv.cached_json = Some(serde_json::json!({"status": 201, "elapsed_ms": 318}));
        let (badge, color, latency) = run_summary(&pv, true).expect("http summary");
        assert_eq!(badge, "201");
        assert_eq!(color, palette::success());
        assert_eq!(latency.as_deref(), Some("318ms"));

        pv.cached_json = Some(serde_json::json!({"status": 500}));
        let (_, color, _) = run_summary(&pv, true).expect("http summary");
        assert_eq!(color, palette::amber());
    }

    #[test]
    fn run_summary_counts_db_rows() {
        let mut pv = ParsedView::empty();
        pv.cached_json = Some(serde_json::json!({
            "results": [{"rows": [{"id": 1}, {"id": 2}, {"id": 3}]}]
        }));
        let (badge, _, _) = run_summary(&pv, false).expect("db summary");
        assert_eq!(badge, "3 rows");
    }

    #[test]
    fn run_summary_none_before_run() {
        let pv = ParsedView::empty();
        assert!(run_summary(&pv, true).is_none());
    }

    #[test]
    fn method_chip_color_matches_doc_view() {
        assert_eq!(
            method_chip_color("POST"),
            crate::ui::blocks::http_panel::method_color("POST")
        );
    }
}
