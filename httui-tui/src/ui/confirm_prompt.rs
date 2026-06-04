//! Generic centered y/n confirm modal. Replaces the per-flow render
//! files (`db_confirm_run.rs`, `connection_delete_confirm.rs`, the
//! `render_*_delete_confirm` fns in `envs_page.rs`) — each call site
//! provides only the title + body string via [`ConfirmPromptState`].
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::ConfirmPromptState;

/// Fixed popup width — short title + one-line body is the common case.
const POPUP_WIDTH: u16 = 60;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &ConfirmPromptState) {
    let popup = compute_popup_rect(editor_area);
    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    // Hard-fill so editor content doesn't bleed through (same trick as
    // `db_confirm_run` / `connection_picker`).
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
        .title(format!(" {} ", state.title))
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(Color::LightRed)
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // body line
            Constraint::Min(0),
            Constraint::Length(1), // footer
        ])
        .split(inner);

    let body_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {} ", state.body),
            body_style,
        ))),
        chunks[0],
    );

    let chip_yes = Style::default()
        .bg(Color::LightRed)
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_no = Style::default()
        .bg(crate::ui::palette::muted())
        .fg(crate::ui::palette::foreground())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" y ", chip_yes),
        Span::styled(" confirm   ", chip_label),
        Span::styled(" n ", chip_no),
        Span::styled(" cancel ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer), chunks[2]);
}

/// Center the popup horizontally + vertically; clamp width to the
/// editor area so a narrow split doesn't push the popup off-screen.
fn compute_popup_rect(editor_area: Rect) -> Rect {
    let height: u16 = 5; // body + spacer + footer + 2 borders
    let width = POPUP_WIDTH.min(editor_area.width.saturating_sub(2)).max(20);
    let x = editor_area
        .x
        .saturating_add((editor_area.width.saturating_sub(width)) / 2);
    let y = editor_area
        .y
        .saturating_add((editor_area.height.saturating_sub(height)) / 2);
    Rect {
        x,
        y,
        width,
        height,
    }
}
