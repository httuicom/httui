//! Tab picker (`gb`). Centered popup listing every open tab by
//! its focused-leaf path; j/k or arrows navigate; Enter switches
//! `tabs.active`; Esc closes.
//!
//! Same chrome as the env / template pickers — blue border to match
//! `Mode::TabPicker`'s LightBlue bg, distinguishing "switch buffer"
//! from `gE`'s "switch env" (magenta) and `gN`'s "create new"
//! (green).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::TabPickerState;

const MAX_VISIBLE_ROWS: usize = 14;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &TabPickerState, active: usize) {
    if state.entries.is_empty() {
        return;
    }

    let popup = compute_popup_rect(editor_area, state);
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

    let title = format!(
        " Pick tab · {}/{} ",
        state.selected + 1,
        state.entries.len()
    );
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(Color::LightBlue)
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

    // Build list items: `●` for the active tab, `*` suffix when
    // dirty, plain otherwise.
    let items: Vec<ListItem> = state
        .entries
        .iter()
        .map(|e| {
            let active_marker = if e.idx == active { "● " } else { "  " };
            let dirty_marker = if e.dirty { " *" } else { "" };
            ListItem::new(Line::from(vec![
                Span::styled(
                    active_marker,
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(Color::LightBlue),
                ),
                Span::styled(
                    e.label.clone(),
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(crate::ui::palette::foreground()),
                ),
                Span::styled(
                    dirty_marker,
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(crate::ui::palette::popup_border_accent()),
                ),
            ]))
        })
        .collect();
    let list = List::new(items).style(bg_style).highlight_style(
        Style::default()
            .bg(super::palette::selection_bg())
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::BOLD),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(
        state.selected.min(state.entries.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, body_area, &mut list_state);

    let chip_key = Style::default()
        .bg(Color::LightBlue)
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" jk ", chip_key),
        Span::styled(" navigate   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" switch   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

fn compute_popup_rect(area: Rect, state: &TabPickerState) -> Rect {
    const PADDING: u16 = 8; // borders + spacing + markers
    let longest = state
        .entries
        .iter()
        .map(|e| e.label.chars().count())
        .max()
        .unwrap_or(20) as u16;
    let width = (longest + PADDING).clamp(40, area.width.saturating_sub(2));
    let visible = state.entries.len().min(MAX_VISIBLE_ROWS) as u16;
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
