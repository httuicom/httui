//! Centered confirm modal that fires before TUI runs any DB write
//! (INSERT/UPDATE/DELETE/CREATE/DROP/etc — see `is_writing_query`).
//! The `reason` string differentiates unscoped destructive
//! (UPDATE/DELETE without WHERE) from other writes. Tiny vocab —
//! `y`/`Enter` runs anyway, `n`/`Esc`/`Ctrl-C` cancels — so this
//! widget is also tiny: red border, two text lines (warning +
//! reason), one footer with the two key chips.
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::DbConfirmRunState;

/// Fixed popup width — the warning text plus the reason should
/// always fit. Height auto-grows to whatever the text needs.
const POPUP_WIDTH: u16 = 56;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &DbConfirmRunState) {
    let popup = compute_popup_rect(editor_area);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard-fill so editor content doesn't bleed through. Same trick
    // as `connection_picker`/`completion_popup`.
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
        .title(" Confirm write ")
        .style(bg_style)
        .border_style(Style::default().fg(Color::LightRed).bg(Color::Black));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // warning
            Constraint::Length(1), // reason
            Constraint::Min(0),
            Constraint::Length(1), // footer
        ])
        .split(inner);

    let warn_style = Style::default()
        .bg(Color::Black)
        .fg(Color::LightRed)
        .add_modifier(Modifier::BOLD);
    let reason_style = Style::default().bg(Color::Black).fg(Color::White);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(" ⚠  Run anyway? ", warn_style))),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!(" {} ", state.reason),
            reason_style,
        ))),
        chunks[1],
    );

    let chip_yes = Style::default()
        .bg(Color::LightRed)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let chip_no = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" y ", chip_yes),
        Span::styled(" run anyway   ", chip_label),
        Span::styled(" n ", chip_no),
        Span::styled(" cancel ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer), chunks[3]);
}

/// Center the popup horizontally + vertically. Width clamps to the
/// editor area so a narrow split doesn't push the popup off-screen.
fn compute_popup_rect(editor_area: Rect) -> Rect {
    let height: u16 = 5; // 2-row body + footer + 2 borders
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
