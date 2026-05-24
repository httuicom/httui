//! V10 slice 5 — Clone vault form. URL + parent dir, submit clones
//! the repo via `httui_core::git::clone::git_clone` and switches
//! the active workspace in-place.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{VaultCloneFormFocus, VaultCloneFormState};

const POPUP_WIDTH: u16 = 64;
const POPUP_HEIGHT: u16 = 14;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &VaultCloneFormState) -> Option<(u16, u16)> {
    let popup = centered_rect(editor_area, POPUP_WIDTH, POPUP_HEIGHT);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

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

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(" Clone vault ")
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::BORDER)
                .bg(Color::Black),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // url label
            Constraint::Length(1), // url input
            Constraint::Length(1), // blank
            Constraint::Length(1), // parent label
            Constraint::Length(1), // parent input
            Constraint::Length(1), // blank
            Constraint::Length(1), // error
            Constraint::Min(0),    // spacer
            Constraint::Length(1), // footer
        ])
        .split(inner);

    let label = |text: &str, focused: bool| {
        Paragraph::new(Line::from(Span::styled(
            text.to_string(),
            if focused {
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        )))
        .style(bg_style)
    };

    let value = |s: &str, hint: &str| {
        if s.is_empty() {
            Paragraph::new(Line::from(Span::styled(
                hint.to_string(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )))
            .style(bg_style)
        } else {
            Paragraph::new(Line::from(Span::styled(
                s.to_string(),
                Style::default().fg(Color::White),
            )))
            .style(bg_style)
        }
    };

    let url_focused = state.focus == VaultCloneFormFocus::Url;
    let parent_focused = state.focus == VaultCloneFormFocus::Parent;

    frame.render_widget(label("Git URL", url_focused), rows[0]);
    frame.render_widget(value(state.url.as_str(), "https://github.com/org/repo.git"), rows[1]);
    frame.render_widget(label("Parent dir", parent_focused), rows[3]);
    frame.render_widget(value(state.parent.as_str(), "/path/to/parent"), rows[4]);

    if let Some(err) = state.error.as_deref() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("error: {err}"),
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            )))
            .style(bg_style),
            rows[6],
        );
    }

    let chip_key = Style::default()
        .bg(Color::LightMagenta)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" Tab ", chip_key),
        Span::styled(" cycle field   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" clone   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" cancel ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), rows[8]);

    let (input_row, buffer) = match state.focus {
        VaultCloneFormFocus::Url => (rows[1], state.url.as_str()),
        VaultCloneFormFocus::Parent => (rows[4], state.parent.as_str()),
    };
    let x = input_row.x + buffer.chars().count() as u16;
    let y = input_row.y;
    Some((x.min(input_row.x + input_row.width.saturating_sub(1)), y))
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    let x = area.x + area.width.saturating_sub(w) / 2;
    let y = area.y + area.height.saturating_sub(h) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_clamps_within_area() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let popup = centered_rect(area, POPUP_WIDTH, POPUP_HEIGHT);
        assert!(popup.width <= area.width.saturating_sub(2));
        assert!(popup.height <= area.height.saturating_sub(2));
    }
}
