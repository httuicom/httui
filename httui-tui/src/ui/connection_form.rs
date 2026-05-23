//! V3 P3 (2026-05-23): create-connection form modal. Centered
//! popup with 9 focusable fields: name / driver dial / host / port
//! / database / username / password / readonly toggle /
//! description. Submits via `Enter`, cancels via `Esc`.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{ConnectionFormFocus, ConnectionFormState, DRIVER_OPTIONS};

const POPUP_WIDTH: u16 = 60;
const POPUP_HEIGHT: u16 = 24;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &ConnectionFormState) {
    let popup = centered_rect(editor_area, POPUP_WIDTH, POPUP_HEIGHT);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard-fill so editor content underneath doesn't bleed through.
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
        .title(" New connection ")
        .style(bg_style)
        .border_style(Style::default().fg(Color::LightYellow).bg(Color::Black));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(text_row(
        "name",
        state.name.as_str(),
        matches!(state.focus, ConnectionFormFocus::Name),
        false,
    ));
    lines.push(driver_row(state));
    lines.push(text_row(
        "host",
        state.host.as_str(),
        matches!(state.focus, ConnectionFormFocus::Host),
        false,
    ));
    lines.push(text_row(
        "port",
        state.port.as_str(),
        matches!(state.focus, ConnectionFormFocus::Port),
        false,
    ));
    lines.push(text_row(
        "database",
        state.database_name.as_str(),
        matches!(state.focus, ConnectionFormFocus::Database),
        false,
    ));
    lines.push(text_row(
        "username",
        state.username.as_str(),
        matches!(state.focus, ConnectionFormFocus::Username),
        false,
    ));
    lines.push(text_row(
        "password",
        state.password.as_str(),
        matches!(state.focus, ConnectionFormFocus::Password),
        true,
    ));
    lines.push(toggle_row(state));
    lines.push(text_row(
        "description",
        state.description.as_str(),
        matches!(state.focus, ConnectionFormFocus::Description),
        false,
    ));
    lines.push(Line::from(""));
    if let Some(err) = state.error.as_deref() {
        lines.push(Line::from(Span::styled(
            format!("  ✗ {err}"),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        "  Tab next · Shift-Tab prev · Enter save · Esc cancel",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )));

    let para = Paragraph::new(lines).style(bg_style).wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
}

fn text_row(label: &str, value: &str, focused: bool, mask: bool) -> Line<'static> {
    let display: String = if mask && !value.is_empty() {
        "•".repeat(value.chars().count())
    } else {
        value.to_string()
    };
    let marker = if focused { "▌ " } else { "  " };
    let label_style = if focused {
        Style::default()
            .fg(Color::LightYellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let value_style = if focused {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::White)
    };
    let (val_text, val_style_final) = if display.is_empty() {
        ("—".to_string(), Style::default().fg(Color::DarkGray))
    } else {
        (display, value_style)
    };
    Line::from(vec![
        Span::raw(marker),
        Span::styled(format!("{label:<13}"), label_style),
        Span::styled(val_text, val_style_final),
    ])
}

fn driver_row(state: &ConnectionFormState) -> Line<'static> {
    let focused = matches!(state.focus, ConnectionFormFocus::Driver);
    let marker = if focused { "▌ " } else { "  " };
    let label_style = if focused {
        Style::default()
            .fg(Color::LightYellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let mut spans = vec![
        Span::raw(marker),
        Span::styled(format!("{:<13}", "driver"), label_style),
    ];
    for (i, name) in DRIVER_OPTIONS.iter().enumerate() {
        if i == state.driver_idx {
            spans.push(Span::styled(
                format!("[{name}]"),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {name} "),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if i + 1 < DRIVER_OPTIONS.len() {
            spans.push(Span::raw(" "));
        }
    }
    Line::from(spans)
}

fn toggle_row(state: &ConnectionFormState) -> Line<'static> {
    let focused = matches!(state.focus, ConnectionFormFocus::Readonly);
    let marker = if focused { "▌ " } else { "  " };
    let label_style = if focused {
        Style::default()
            .fg(Color::LightYellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let chip_style = if focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::LightYellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    Line::from(vec![
        Span::raw(marker),
        Span::styled(format!("{:<13}", "readonly"), label_style),
        Span::styled(
            if state.is_readonly {
                "[x] yes "
            } else {
                "[ ] no  "
            },
            chip_style,
        ),
        Span::styled(
            "  (space to toggle)",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ),
    ])
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width.saturating_sub(2));
    let h = height.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}
