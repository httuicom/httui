//! Block-template picker (`gN`). Centered popup listing the
//! `BlockTemplate::ALL` set. j/k or arrows navigate; Enter splices
//! the selected template at the cursor and re-parses; Esc closes.
//!
//! Same chrome as `environment_picker` — green border instead of
//! magenta to distinguish "create new" from "switch active".

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{BlockTemplate, BlockTemplatePickerState};

const MAX_VISIBLE_ROWS: usize = 12;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &BlockTemplatePickerState) {
    let templates = BlockTemplate::ALL;
    let popup = compute_popup_rect(editor_area, templates);
    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    {
        let buf = frame.buffer_mut();
        for y in popup.y..popup.y.saturating_add(popup.height) {
            for x in popup.x..popup.x.saturating_add(popup.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let title = format!(" New block · {}/{} ", state.selected + 1, templates.len());
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::success())
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let body_area = chunks[0];
    let footer_area = chunks[1];

    let items: Vec<ListItem> = templates
        .iter()
        .map(|t| {
            ListItem::new(Line::from(vec![Span::styled(
                t.label,
                Style::default()
                    .bg(crate::ui::palette::popup_bg())
                    .fg(crate::ui::palette::foreground()),
            )]))
        })
        .collect();
    let list = List::new(items).style(bg_style).highlight_style(
        Style::default()
            .bg(super::palette::selection_bg())
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::BOLD),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(state.selected.min(templates.len().saturating_sub(1))));
    frame.render_stateful_widget(list, body_area, &mut list_state);

    let chip_key = Style::default()
        .bg(crate::ui::palette::success())
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" jk ", chip_key),
        Span::styled(" navigate   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" insert   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

fn compute_popup_rect(area: Rect, templates: &[BlockTemplate]) -> Rect {
    const PADDING: u16 = 6;
    let longest = templates
        .iter()
        .map(|t| t.label.chars().count())
        .max()
        .unwrap_or(20) as u16;
    let width = (longest + PADDING).clamp(28, area.width.saturating_sub(2));
    let visible = templates.len().min(MAX_VISIBLE_ROWS) as u16;
    let height = visible + 3;
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + 3u16.min(area.height.saturating_sub(height));
    Rect {
        x,
        y,
        width,
        height,
    }
}
