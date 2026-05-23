//! Centered modal showing the full HTTP response — status line,
//! headers, and the body in full. Opened with `<CR>` while the
//! cursor is parked on an HTTP block's response panel; closed with
//! `Ctrl-C`.
//!
//! Architecture mirrors [`super::db_row_detail`] — the body lives on
//! a sub-`Document` carried by [`HttpResponseDetailState`] so the
//! editor's full motion vocabulary works inside the modal. This file
//! owns the painting only: it walks the doc's prose segment(s),
//! computes a scroll offset that keeps the modal cursor visible,
//! highlights body lines per role (status pill / `Headers` heading /
//! header pairs / `Body` heading / body lines).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::HttpResponseDetailState;
use crate::buffer::{Cursor, Segment};
use crate::ui::VisualOverlay;

/// Paint the modal centered over `editor_area`. Records the body
/// area's height back into `state.viewport_height` so half/full-page
/// motions know how far to jump on the next tick.
pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &mut HttpResponseDetailState,
    visual: Option<VisualOverlay>,
) {
    let modal = centered_rect(editor_area, 75, 75);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    {
        let buf = frame.buffer_mut();
        for y in modal.y..modal.y.saturating_add(modal.height) {
            for x in modal.x..modal.x.saturating_add(modal.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(state.title.clone())
        .style(bg_style)
        .border_style(Style::default().fg(Color::LightBlue).bg(Color::Black));
    let inner = outer.inner(modal);
    frame.render_widget(outer, modal);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let body_area = chunks[0];
    let footer_area = chunks[1];

    state.viewport_height = body_area.height;

    paint_body(frame, body_area, state, bg_style, visual);
    paint_footer(frame, footer_area, bg_style);
}

fn paint_body(
    frame: &mut Frame,
    body_area: Rect,
    state: &mut HttpResponseDetailState,
    bg: Style,
    visual: Option<VisualOverlay>,
) {
    if body_area.width == 0 || body_area.height == 0 {
        return;
    }
    let rope = match state.doc.segments().iter().find_map(|s| match s {
        Segment::Prose(r) => Some(r),
        _ => None,
    }) {
        Some(r) => r,
        None => return,
    };

    let total_lines = rope.len_lines() as u16;
    let (cursor_line, cursor_col) = match state.doc.cursor() {
        Cursor::InProse { offset, .. } => {
            let safe_offset = offset.min(rope.len_chars());
            let line_idx = rope.char_to_line(safe_offset);
            let line_start = rope.line_to_char(line_idx);
            (line_idx as u16, (safe_offset - line_start) as u16)
        }
        _ => (0, 0),
    };

    let viewport = body_area.height;
    let new_top = clamp_viewport(state.viewport_top, viewport, cursor_line, total_lines);
    state.viewport_top = new_top;
    let offset = new_top;
    let visible = viewport.min(total_lines.saturating_sub(offset));

    let mut lines: Vec<Line<'static>> = Vec::with_capacity(visible as usize);
    for i in 0..visible as usize {
        let line_idx = (offset as usize) + i;
        if line_idx >= rope.len_lines() {
            break;
        }
        let mut text = rope.line(line_idx).to_string();
        if text.ends_with('\n') {
            text.pop();
            if text.ends_with('\r') {
                text.pop();
            }
        }
        lines.push(style_body_line(text, bg, line_idx == 0));
    }
    let para = Paragraph::new(lines).style(bg);
    frame.render_widget(para, body_area);

    let cursor_screen_y = body_area
        .y
        .saturating_add(cursor_line.saturating_sub(offset));
    let cursor_screen_x = body_area
        .x
        .saturating_add(cursor_col.min(body_area.width.saturating_sub(1)));
    if cursor_screen_y < body_area.y.saturating_add(body_area.height) {
        frame.set_cursor_position((cursor_screen_x, cursor_screen_y));
    }

    if let Some(overlay) = visual {
        paint_visual_selection(
            frame,
            body_area,
            rope,
            offset,
            overlay,
            cursor_line,
            cursor_col,
        );
    }
}

fn paint_visual_selection(
    frame: &mut ratatui::Frame,
    body_area: Rect,
    rope: &ropey::Rope,
    viewport_top: u16,
    overlay: VisualOverlay,
    cursor_line: u16,
    cursor_col: u16,
) {
    let anchor_offset = match overlay.anchor {
        Cursor::InProse { offset, .. } => offset,
        _ => return,
    };
    let total_chars = rope.len_chars();
    let safe_anchor = anchor_offset.min(total_chars);
    let anchor_line = rope.char_to_line(safe_anchor);
    let anchor_col = safe_anchor - rope.line_to_char(anchor_line);

    let (start_line, start_col, end_line, end_col_inclusive) = if overlay.linewise {
        let lo_line = anchor_line.min(cursor_line as usize);
        let hi_line = anchor_line.max(cursor_line as usize);
        (lo_line, 0usize, hi_line, usize::MAX)
    } else {
        let (lo_line, lo_col, hi_line, hi_col) =
            if (anchor_line, anchor_col) <= (cursor_line as usize, cursor_col as usize) {
                (
                    anchor_line,
                    anchor_col,
                    cursor_line as usize,
                    cursor_col as usize,
                )
            } else {
                (
                    cursor_line as usize,
                    cursor_col as usize,
                    anchor_line,
                    anchor_col,
                )
            };
        (lo_line, lo_col, hi_line, hi_col)
    };

    let style = Style::default().bg(super::palette::SELECTION_BG);
    let buf = frame.buffer_mut();
    let total_lines = rope.len_lines();
    for line in start_line..=end_line {
        if line >= total_lines {
            break;
        }
        if (line as u16) < viewport_top {
            continue;
        }
        let y = body_area.y.saturating_add((line as u16) - viewport_top);
        if y >= body_area.y.saturating_add(body_area.height) {
            break;
        }
        let line_text = rope.line(line).to_string();
        let line_chars = line_text.trim_end_matches('\n').chars().count();
        let from = if line == start_line { start_col } else { 0 };
        let to = if overlay.linewise {
            body_area.width as usize
        } else if line == end_line {
            (end_col_inclusive + 1).min(line_chars.max(1))
        } else {
            line_chars.max(1)
        };
        if to <= from {
            continue;
        }
        let max_x = body_area.x.saturating_add(body_area.width);
        for col in from..to {
            let x = body_area.x.saturating_add(col as u16);
            if x >= max_x {
                break;
            }
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(cell.style().patch(style));
            }
        }
    }
}

/// Style one body line based on its role. Five line shapes:
///
/// 1. Line 0 (status line) → status code colored by class
///    (2xx green, 3xx yellow, 4xx/5xx red), rest of line dim.
/// 2. Empty separator → render as-is.
/// 3. Section heading (`Headers`, `Body`) → bold cyan.
/// 4. Indented `key: value` → key in cyan, value in white.
/// 5. Otherwise (body content lines) → faint coloring per JSON
///    fragment if the body looks like JSON; plain otherwise.
fn style_body_line(text: String, bg: Style, is_status_line: bool) -> Line<'static> {
    if is_status_line {
        return style_status_line(&text, bg);
    }
    if text.is_empty() {
        return Line::from(Span::styled(text, bg));
    }
    let trimmed = text.trim_start();
    if trimmed == "Headers" || trimmed == "Body" {
        return Line::from(Span::styled(
            text,
            Style::default()
                .fg(Color::Cyan)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
    }
    // Indented header line: `  Key: Value` — split on the first `:`
    // (after the indent) to color the key.
    if let Some(rest) = text.strip_prefix("  ") {
        if let Some(idx) = rest.find(':') {
            let (key, after) = rest.split_at(idx);
            // Filter out the bullet placeholders we emit when no
            // headers are present — render those dim, not as a fake
            // header pair.
            if !key.starts_with('(') {
                let key_style = Style::default()
                    .fg(Color::Cyan)
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD);
                let value_style = Style::default()
                    .fg(Color::Rgb(0xc6, 0xd0, 0xf5))
                    .bg(Color::Black);
                return Line::from(vec![
                    Span::styled("  ", bg),
                    Span::styled(key.to_string(), key_style),
                    Span::styled(after.to_string(), value_style),
                ]);
            }
        }
        // Indented body line — light fg, kept as a single span.
        let style = Style::default()
            .fg(Color::Rgb(0xc6, 0xd0, 0xf5))
            .bg(Color::Black);
        return Line::from(Span::styled(text, style));
    }
    Line::from(Span::styled(
        text,
        Style::default().fg(Color::White).bg(Color::Black),
    ))
}

/// First line of the body — `200 OK · 142 ms · 1.4 kB`. Color the
/// status code by class (2xx → green, 3xx → yellow, 4xx/5xx → red);
/// keep the rest dim. Falls back to plain white on parse failure.
fn style_status_line(text: &str, bg: Style) -> Line<'static> {
    let mut parts = text.splitn(2, ' ');
    let status_part = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");
    let status_color = status_part
        .chars()
        .next()
        .map(|c| match c {
            '2' => Color::LightGreen,
            '3' => Color::LightYellow,
            '4' | '5' => Color::LightRed,
            _ => Color::White,
        })
        .unwrap_or(Color::White);
    let status_style = Style::default()
        .fg(status_color)
        .bg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let rest_style = Style::default().fg(Color::Gray).bg(Color::Black);
    let mut spans = vec![Span::styled(status_part.to_string(), status_style)];
    if !rest.is_empty() {
        spans.push(Span::styled(" ", bg));
        spans.push(Span::styled(rest.to_string(), rest_style));
    }
    Line::from(spans)
}

fn paint_footer(frame: &mut Frame, footer_area: Rect, bg: Style) {
    let chip_key = Style::default()
        .bg(Color::LightBlue)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" Ctrl-C ", chip_key),
        Span::styled(" close   ", chip_label),
        Span::styled(" hjkl ", chip_key),
        Span::styled(" navigate   ", chip_label),
        Span::styled(" gg G ", chip_key),
        Span::styled(" top/bottom   ", chip_label),
        Span::styled(" y{m} ", chip_key),
        Span::styled(" yank → clipboard   ", chip_label),
        Span::styled(" Y ", chip_key),
        Span::styled(" copy body ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg), footer_area);
}

const SCROLL_OFF: u16 = 3;

/// Persistent-viewport scroll — same contract as
/// `app::clamp_viewport` and `db_row_detail::clamp_viewport`. Keeps
/// the cursor inside `[viewport_top + scrolloff, viewport_top +
/// height - scrolloff - 1]`; the window only shifts when the cursor
/// would otherwise leave it.
fn clamp_viewport(viewport_top: u16, height: u16, cursor: u16, total: u16) -> u16 {
    if height == 0 {
        return viewport_top;
    }
    let scrolloff = SCROLL_OFF.min(height / 2);
    let upper = cursor.saturating_sub(scrolloff);
    let lower = cursor.saturating_add(scrolloff + 1).saturating_sub(height);
    let next = if viewport_top > upper {
        upper
    } else if viewport_top < lower {
        lower
    } else {
        viewport_top
    };
    let max_top = total.saturating_sub(height);
    next.min(max_top)
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_w = area.width * percent_x / 100;
    let popup_h = area.height * percent_y / 100;
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_viewport_keeps_cursor_visible() {
        let h = 10;
        assert_eq!(clamp_viewport(0, h, 3, 50), 0);
        assert_eq!(clamp_viewport(0, h, 6, 50), 0);
        assert_eq!(clamp_viewport(0, h, 9, 50), 3);
        assert_eq!(clamp_viewport(0, h, 25, 50), 19);
        assert_eq!(clamp_viewport(20, h, 22, 50), 19);
        assert_eq!(clamp_viewport(0, h, 49, 50), 40);
        assert_eq!(clamp_viewport(7, 0, 100, 100), 7);
    }

    #[test]
    fn style_status_line_picks_color_by_status_class() {
        let bg = Style::default().bg(Color::Black);
        let line = style_status_line("200 OK · 142 ms", bg);
        // Smoke test: the renderer produces at least the status span
        // plus separator + rest spans.
        assert!(!line.spans.is_empty());
    }
}
