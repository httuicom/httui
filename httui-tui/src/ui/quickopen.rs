//! Quick-open modal renderer. Centered overlay with `>` prompt at the
//! top and a scrollable result list below. The selected row is
//! highlighted; cursor lands inside the prompt.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::vim::quickopen::QuickOpen;

const MAX_VISIBLE_ROWS: usize = 12;

/// Render the modal centered over `editor_area`. Returns the prompt's
/// `(x, y)` so the caller can place the terminal cursor at the input
/// position.
///
/// We layer three things to keep the underlying editor from bleeding
/// through (some terminals don't fully honor a bare `Clear`):
/// 1. `Clear` widget to reset cells.
/// 2. An explicit background `Block` with `bg = Black` filling the
///    entire modal area.
/// 3. The bordered `Block` with title.
///
/// Inner widgets carry the same bg style so their unwritten cells stay
/// black instead of inheriting whatever Clear left behind.
pub fn render(frame: &mut Frame, editor_area: Rect, qo: &QuickOpen) -> (u16, u16) {
    let modal = centered_rect(editor_area, 80, 60);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard fill: write a styled space into every cell of the modal
    // area, bypassing widget-level abstractions that only `set_style`
    // (preserving stale chars from the editor underneath).
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

    // Bordered frame with title.
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Open file ")
        .style(bg_style);
    let inner = outer.inner(modal);
    frame.render_widget(outer, modal);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);
    let prompt_area = chunks[0];
    let list_area = chunks[1];

    // Prompt line — styled bg so trailing cells past the typed query
    // stay black instead of bleeding.
    let prompt_line = Line::from(vec![
        Span::styled("> ", Style::default().bg(Color::Black).fg(Color::LightCyan)),
        Span::styled(qo.query.as_str().to_string(), bg_style),
    ]);
    frame.render_widget(Paragraph::new(prompt_line).style(bg_style), prompt_area);

    // Result list.
    let visible = MAX_VISIBLE_ROWS.min(qo.filtered.len());
    let items: Vec<ListItem> = if qo.filtered.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  no matches",
            Style::default()
                .bg(Color::Black)
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )))]
    } else {
        qo.filtered
            .iter()
            .take(visible.max(MAX_VISIBLE_ROWS))
            .filter_map(|i| qo.all_files.get(*i))
            .map(|path| ListItem::new(Line::from(Span::styled(path.clone(), bg_style))))
            .collect()
    };
    let list = List::new(items)
        .style(bg_style)
        .highlight_style(
            Style::default()
                .bg(super::palette::SELECTION_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    let mut state = ListState::default();
    if !qo.filtered.is_empty() {
        state.select(Some(qo.selected.min(qo.filtered.len() - 1)));
    }
    frame.render_stateful_widget(list, list_area, &mut state);

    // Cursor: 2 chars of `> ` prefix + chars-before-cursor, clamped.
    let col = (qo.query.cursor_col() as u16).saturating_add(2);
    let x = prompt_area
        .x
        .saturating_add(col.min(prompt_area.width.saturating_sub(1)));
    (x, prompt_area.y)
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
