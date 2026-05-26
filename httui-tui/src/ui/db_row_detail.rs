//! Centered modal showing the full contents of a single DB result
//! row. Opened with `<CR>` while the cursor is parked on a row in
//! the result table; closed with `Esc`/`q`.
//!
//! The body lives in its own `Document` (see `app::DbRowDetailState`)
//! so vim motions navigate the modal exactly like they do the
//! editor — `parse_db_row_detail` filters `parse_normal` down to
//! motion actions and the dispatch routes them at `state.doc`. This
//! file owns the painting only: it walks the doc's prose segment(s),
//! computes a scroll offset that keeps the modal cursor visible,
//! highlights the cursor's line, and places the terminal cursor at
//! the column too so word motions feel right.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::DbRowDetailState;
use crate::buffer::{Cursor, Segment};
use crate::ui::VisualOverlay;

/// Paint the modal centered over `editor_area`. The renderer also
/// records the body area's height back into `state.viewport_height`
/// so half/full-page motions know how far to jump on the next tick.
/// `visual` is `Some` when `Mode::Visual`/`VisualLine` is active over
/// the modal — paints a selection bg between the anchor and the
/// modal cursor.
pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &mut DbRowDetailState,
    visual: Option<VisualOverlay>,
) {
    let modal = centered_rect(editor_area, 70, 70);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard fill: paint every cell so anything painted earlier (the
    // editor underneath) doesn't bleed through. Same trick as
    // `quickopen::render`.
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

    // Stash for next dispatch: half/full-page motions read this.
    state.viewport_height = body_area.height;

    paint_body(frame, body_area, state, bg_style, visual);
    paint_footer(frame, footer_area, bg_style);
}

/// Walk the modal's body `Document` (a single Prose run) and paint
/// the slice that fits inside `body_area`, with a row highlight on
/// the cursor's line and the terminal cursor placed at the cursor's
/// column inside that line. When `visual` is set, also overlays the
/// selection bg between the anchor and the cursor — same look as
/// the editor's visual mode.
fn paint_body(
    frame: &mut Frame,
    body_area: Rect,
    state: &mut DbRowDetailState,
    bg: Style,
    visual: Option<VisualOverlay>,
) {
    if body_area.width == 0 || body_area.height == 0 {
        return;
    }
    // The modal builds the body via `Document::from_markdown(plain)`,
    // which yields a single Prose segment. Defensively walk for the
    // first one; if there's somehow no prose, paint an empty body.
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
        // The modal only ever holds an InProse cursor (its doc has a
        // single prose segment), but the enum is exhaustive so we
        // need a fallthrough.
        _ => (0, 0),
    };

    let viewport = body_area.height;
    // Persistent viewport: only adjust when the cursor would scroll
    // off-screen (mirrors `app::clamp_viewport`). Inside the visible
    // window the cursor moves freely with no scroll — scrolling
    // happens at the edges, with a small `scrolloff` buffer so the
    // cursor never paints on the very last line.
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
        // Strip trailing newline so it doesn't render as an extra
        // empty space at end-of-line.
        if text.ends_with('\n') {
            text.pop();
            if text.ends_with('\r') {
                text.pop();
            }
        }
        lines.push(style_body_line(text, bg));
    }
    // No `wrap` here — the editor doesn't wrap horizontally either,
    // so motions and visible columns line up. Long values get
    // clipped at the modal edge; word-motions still walk past them.
    let para = Paragraph::new(lines).style(bg);
    frame.render_widget(para, body_area);

    // Place the terminal cursor at the column inside the highlighted
    // line, clamped to the body area so a long-value column doesn't
    // overflow into the border.
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

/// Paint a bg highlight under the active visual selection inside the
/// modal body. Mirrors `ui::overlay_visual_selection` but in modal
/// coordinates: anchor + cursor are both `Cursor::InProse` against
/// the modal's single-prose `Document`, so we can compute (line,
/// col) ranges directly from the rope.
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
        // The modal doc only ever has Prose segments — defensive bail.
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

/// Single-line key hint at the bottom of the modal. Reads as a
/// reminder, not a comprehensive map — the modal accepts everything
/// `parse_normal` does, so listing every motion would be noise.
/// Paint a single body line with role-specific colors so the user
/// can scan a row at a glance. Three line shapes:
///
/// 1. Empty separator → render as-is.
/// 2. Indented (starts with two spaces) → value line. The whole line
///    gets the value color (bright fg).
/// 3. Otherwise → header line of the form `{name}  ({type})`. The
///    `{name}` paints in cyan-bold and the `({type})` chunk in dim
///    gray. Lines without a `(type)` suffix still color the name —
///    the suffix is optional.
///
/// The body text is built by `dispatch::build_db_row_body_text`, so
/// the layout is fully predictable: a header always lives on column
/// 0, a value always on column 2.
fn style_body_line(text: String, bg: Style) -> Line<'static> {
    if text.is_empty() {
        return Line::from(Span::styled(text, bg));
    }
    if text.starts_with("  ") {
        // Value line — keep it as a single span in the value color.
        return Line::from(Span::styled(
            text,
            Style::default()
                .fg(Color::Rgb(0xc6, 0xd0, 0xf5))
                .bg(Color::Black),
        ));
    }
    // Header line. Look for the `  (` separator — when present,
    // split into name + type so the type can fade into the bg.
    let name_style = Style::default()
        .fg(Color::Cyan)
        .bg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let type_style = Style::default().fg(Color::DarkGray).bg(Color::Black);
    if let Some(idx) = text.find("  (") {
        let (name, ty) = text.split_at(idx);
        return Line::from(vec![
            Span::styled(name.to_string(), name_style),
            Span::styled(ty.to_string(), type_style),
        ]);
    }
    Line::from(Span::styled(text, name_style))
}

fn paint_footer(frame: &mut Frame, footer_area: Rect, bg: Style) {
    // Black text on a bright bg matches the status-bar mode chip
    // (`status::render` uses the same `fg(Black) + bg(bright)`
    // combo). Avoiding `DarkGray` + `White` because some terminal
    // palettes render those as nearly-equal light grays — the chip
    // background shows but the label inside disappears.
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
        Span::styled(" copy row as JSON ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg), footer_area);
}

/// How tall a buffer of "scroll-off" rows we keep above and below
/// the cursor before snapping the viewport. Matches the editor's
/// `app::SCROLL_OFF`. Capped at half the viewport so very small
/// modals still center the cursor sanely.
const SCROLL_OFF: u16 = 3;

/// Persistent-viewport scroll: nudge `viewport_top` only enough to
/// keep the cursor inside `[viewport_top + scrolloff, viewport_top
/// + height - scrolloff - 1]`. Mirrors `app::clamp_viewport` so the
/// modal scrolls exactly like an editor pane — the cursor moves
/// freely inside the visible window, and the window only shifts
/// when the cursor would otherwise leave it. Result is also capped
/// at `total - height` so we never paint past the end.
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
    fn clamp_viewport_holds_until_cursor_leaves() {
        // viewport_top=0, height=10, scrolloff=3 → cursor stays
        // free in [3, 6]; outside that the window scrolls.
        let h = 10;
        // Cursor inside the comfort band: no scroll.
        assert_eq!(clamp_viewport(0, h, 3, 50), 0);
        assert_eq!(clamp_viewport(0, h, 6, 50), 0);
        // Cursor below the lower scroll-off: window inches down so
        // the cursor stays `scrolloff` rows above the bottom.
        assert_eq!(clamp_viewport(0, h, 7, 50), 1);
        assert_eq!(clamp_viewport(0, h, 9, 50), 3);
        // Cursor jumps to 25, viewport_top was 0 → snap to keep
        // cursor inside (offset = cursor + scrolloff + 1 - height).
        assert_eq!(clamp_viewport(0, h, 25, 50), 19);
        // Going up past the upper scroll-off pulls the window up
        // just enough to keep the cursor `scrolloff` rows below the
        // top — viewport_top = cursor - scrolloff.
        assert_eq!(clamp_viewport(20, h, 22, 50), 19);
        assert_eq!(clamp_viewport(20, h, 5, 50), 2);
        // Cursor at the very last line clamps at total - height.
        assert_eq!(clamp_viewport(0, h, 49, 50), 40);
        // Defensive: zero height keeps viewport_top untouched.
        assert_eq!(clamp_viewport(7, 0, 100, 100), 7);
    }

    fn render_with_state(state: &mut DbRowDetailState, visual: Option<VisualOverlay>) {
        let backend = ratatui::backend::TestBackend::new(60, 20);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(
                    f,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 60,
                        height: 20,
                    },
                    state,
                    visual,
                );
            })
            .unwrap();
    }

    fn state_with(text: &str) -> DbRowDetailState {
        DbRowDetailState {
            segment_idx: 0,
            row: 0,
            title: "row".into(),
            doc: crate::buffer::Document::from_markdown(text).unwrap(),
            viewport_height: 4,
            viewport_top: 0,
        }
    }

    #[test]
    fn render_smoke_no_visual_overlay() {
        let mut state = state_with("line1\nline2\nline3");
        render_with_state(&mut state, None);
    }

    #[test]
    fn render_smoke_with_charwise_visual_overlay() {
        let mut state = state_with("alpha beta\n");
        let cur = state.doc.cursor();
        render_with_state(
            &mut state,
            Some(VisualOverlay {
                anchor: cur,
                linewise: false,
            }),
        );
    }

    #[test]
    fn render_smoke_with_linewise_visual_overlay() {
        let mut state = state_with("alpha\nbeta\n");
        let cur = state.doc.cursor();
        render_with_state(
            &mut state,
            Some(VisualOverlay {
                anchor: cur,
                linewise: true,
            }),
        );
    }

    #[test]
    fn render_smoke_long_document_triggers_viewport_clamp() {
        let body: String = (0..50).map(|i| format!("line{i}\n")).collect();
        let mut state = state_with(&body);
        // Position cursor far down to exercise viewport scrolling.
        state.doc.set_cursor(Cursor::InProse {
            segment_idx: 0,
            offset: body.find("line40").unwrap_or(0),
        });
        render_with_state(&mut state, None);
    }
}
