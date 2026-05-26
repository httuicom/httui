//! Connection form modal (V3 P3, polished 2026-05-23). Aproxima a
//! UX do desktop:
//! - Driver tabs no topo (PostgreSQL / MySQL / SQLite) com highlight
//!   no selecionado e atalhos `space`/`←→` pra cyclar.
//! - HOST + PORT na mesma linha (layout horizontal); USERNAME +
//!   PASSWORD idem.
//! - Connection-string preview no rodapé, calculada do estado em
//!   tempo real.
//! - Cancel / Create CTA explícitos abaixo do preview.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{ConnectionFormFocus, ConnectionFormState, DRIVER_OPTIONS};

const POPUP_WIDTH: u16 = 62;
const POPUP_HEIGHT: u16 = 22;

/// Renders the form and returns the position of the terminal
/// cursor for the focused field (or `None` when focus lands on
/// the driver tabs / readonly toggle, which have no caret).
/// `render_root` calls `frame.set_cursor_position` with the
/// returned coord — the terminal then handles native blinking.
pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &ConnectionFormState,
) -> Option<(u16, u16)> {
    let popup = centered_rect(editor_area, POPUP_WIDTH, POPUP_HEIGHT);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard-fill so editor / Connections page underneath don't bleed.
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

    let title = match (state.is_session_override, state.editing.as_deref()) {
        (true, Some(name)) => format!(" Session override (TEMP) · {name} "),
        (true, None) => " Session override (TEMP) ".to_string(),
        (false, Some(name)) => format!(" Edit connection · {name} "),
        (false, None) => " New connection ".to_string(),
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(Style::default().fg(Color::LightYellow).bg(Color::Black));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    // Vertical layout: name | driver tabs | host+port | database |
    // user+pass | readonly | description | preview | error | footer.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // name label
            Constraint::Length(1), // name input
            Constraint::Length(1), // blank
            Constraint::Length(1), // driver tabs
            Constraint::Length(1), // blank
            Constraint::Length(1), // host/port labels
            Constraint::Length(1), // host/port inputs
            Constraint::Length(1), // blank
            Constraint::Length(1), // database label
            Constraint::Length(1), // database input
            Constraint::Length(1), // blank
            Constraint::Length(1), // user/pass labels
            Constraint::Length(1), // user/pass inputs
            Constraint::Length(1), // readonly
            Constraint::Length(1), // description label
            Constraint::Length(1), // description input
            Constraint::Min(0),    // preview + error + footer
        ])
        .split(inner);

    let pad = |area: Rect, indent: u16| Rect {
        x: area.x.saturating_add(indent),
        y: area.y,
        width: area.width.saturating_sub(indent),
        height: area.height,
    };

    let name_focused = matches!(state.focus, ConnectionFormFocus::Name);
    frame.render_widget(label_widget("Name", name_focused), pad(rows[0], 2));
    frame.render_widget(
        input_widget(state.name.as_str(), false, "(required)"),
        pad(rows[1], 2),
    );
    frame.render_widget(driver_tabs(state), pad(rows[3], 2));

    // Host + Port row (label + input pairs).
    let host_focused = matches!(state.focus, ConnectionFormFocus::Host);
    let port_focused = matches!(state.focus, ConnectionFormFocus::Port);
    let lr = horizontal_split(pad(rows[5], 2), 3);
    frame.render_widget(label_widget("Host", host_focused), lr.0);
    frame.render_widget(label_widget("Port", port_focused), lr.1);
    let lr = horizontal_split(pad(rows[6], 2), 3);
    frame.render_widget(input_widget(state.host.as_str(), false, "localhost"), lr.0);
    frame.render_widget(input_widget(state.port.as_str(), false, "5432"), lr.1);

    let db_focused = matches!(state.focus, ConnectionFormFocus::Database);
    frame.render_widget(label_widget("Database", db_focused), pad(rows[8], 2));
    frame.render_widget(
        input_widget(
            state.database_name.as_str(),
            false,
            placeholder_for_database(state),
        ),
        pad(rows[9], 2),
    );

    let user_focused = matches!(state.focus, ConnectionFormFocus::Username);
    let pass_focused = matches!(state.focus, ConnectionFormFocus::Password);
    let lr = horizontal_split(pad(rows[11], 2), 3);
    frame.render_widget(label_widget("Username", user_focused), lr.0);
    frame.render_widget(label_widget("Password", pass_focused), lr.1);
    let lr = horizontal_split(pad(rows[12], 2), 3);
    frame.render_widget(input_widget(state.username.as_str(), false, ""), lr.0);
    frame.render_widget(input_widget(state.password.as_str(), true, ""), lr.1);

    frame.render_widget(readonly_widget(state), pad(rows[13], 2));
    let desc_focused = matches!(state.focus, ConnectionFormFocus::Description);
    frame.render_widget(label_widget("Description", desc_focused), pad(rows[14], 2));
    frame.render_widget(
        input_widget(state.description.as_str(), false, ""),
        pad(rows[15], 2),
    );

    // Bottom: connection-string preview + error + footer (cancel/create hint).
    let bottom = rows[16];
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("  {}", connection_string_preview(state)),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )));
    if let Some(err) = state.error.as_deref() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  error: {err}"),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            " Esc ",
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel    ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            " Enter ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" create", Style::default().fg(Color::White)),
        Span::styled(
            "      Tab/Shift-Tab next/prev",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ),
    ]));
    let para = Paragraph::new(lines)
        .style(bg_style)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, bottom);

    // Terminal cursor for the focused field. `render_root` calls
    // `frame.set_cursor_position` with this; the terminal handles
    // native blink. Cursor lives at the input row's column, offset
    // by the prefix (2 cells) + the LineEdit's char-col.
    let focused_input = match state.focus {
        ConnectionFormFocus::Name => Some((pad(rows[1], 2), &state.name)),
        ConnectionFormFocus::Host => {
            let lr = horizontal_split(pad(rows[6], 2), 3);
            Some((lr.0, &state.host))
        }
        ConnectionFormFocus::Port => {
            let lr = horizontal_split(pad(rows[6], 2), 3);
            Some((lr.1, &state.port))
        }
        ConnectionFormFocus::Database => Some((pad(rows[9], 2), &state.database_name)),
        ConnectionFormFocus::Username => {
            let lr = horizontal_split(pad(rows[12], 2), 3);
            Some((lr.0, &state.username))
        }
        ConnectionFormFocus::Password => {
            let lr = horizontal_split(pad(rows[12], 2), 3);
            Some((lr.1, &state.password))
        }
        ConnectionFormFocus::Description => Some((pad(rows[15], 2), &state.description)),
        ConnectionFormFocus::Driver | ConnectionFormFocus::Readonly => None,
    };
    focused_input.map(|(area, edit)| {
        let col = (edit.cursor_col() as u16).saturating_add(2); // "  " prefix
        let x = area
            .x
            .saturating_add(col)
            .min(area.x + area.width.saturating_sub(1));
        (x, area.y)
    })
}

fn label_widget(label: &str, focused: bool) -> Paragraph<'static> {
    let style = if focused {
        Style::default()
            .fg(Color::LightYellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Paragraph::new(Line::from(Span::styled(label.to_string(), style)))
}

fn input_widget(value: &str, mask: bool, placeholder: &str) -> Paragraph<'static> {
    let display: String = if mask && !value.is_empty() {
        "•".repeat(value.chars().count())
    } else {
        value.to_string()
    };
    let (text, style) = if display.is_empty() {
        (
            placeholder.to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )
    } else {
        (display, Style::default().fg(Color::White))
    };
    // Focus is signaled by the cursor glyph + label highlight only —
    // no background tint on the input itself (kept the row visually
    // clean against the popup's black background).
    // Prefix is always two blank cells so the column where the
    // terminal cursor lands is consistent across rows. The cursor
    // itself comes from `frame.set_cursor_position` (returned by
    // `render`); native terminal blink, no software glyph.
    Paragraph::new(Line::from(vec![Span::raw("  "), Span::styled(text, style)]))
}

fn driver_tabs(state: &ConnectionFormState) -> Paragraph<'static> {
    let focused = matches!(state.focus, ConnectionFormFocus::Driver);
    let driver_label = |i: usize, name: &str| -> Span<'static> {
        let pretty = match name {
            "postgres" => "PostgreSQL",
            "mysql" => "MySQL",
            "sqlite" => "SQLite",
            _ => name,
        };
        if i == state.driver_idx {
            Span::styled(
                format!(" {pretty} "),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(format!(" {pretty} "), Style::default().fg(Color::DarkGray))
        }
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    if focused {
        spans.push(Span::styled("▍ ", Style::default().fg(Color::LightYellow)));
    } else {
        spans.push(Span::raw("  "));
    }
    for (i, name) in DRIVER_OPTIONS.iter().enumerate() {
        spans.push(driver_label(i, name));
        if i + 1 < DRIVER_OPTIONS.len() {
            spans.push(Span::styled("│", Style::default().fg(Color::DarkGray)));
        }
    }
    if focused {
        spans.push(Span::styled(
            "   ←/→ or space",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ));
    }
    Paragraph::new(Line::from(spans))
}

fn readonly_widget(state: &ConnectionFormState) -> Paragraph<'static> {
    let focused = matches!(state.focus, ConnectionFormFocus::Readonly);
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
    } else if state.is_readonly {
        Style::default()
            .fg(Color::LightYellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let marker = if focused { "▍ " } else { "  " };
    let marker_style = if focused {
        Style::default().fg(Color::LightYellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let chip_text = if state.is_readonly {
        " [x] read-only "
    } else {
        " [ ] read-only "
    };
    Paragraph::new(Line::from(vec![
        Span::styled(marker, marker_style),
        Span::styled(chip_text, chip_style),
        Span::styled(
            "   (space toggles)",
            label_style.add_modifier(Modifier::ITALIC),
        ),
    ]))
}

fn placeholder_for_database(state: &ConnectionFormState) -> &'static str {
    match DRIVER_OPTIONS.get(state.driver_idx).copied().unwrap_or("") {
        "sqlite" => "/path/to/file.db",
        "postgres" => "mydb",
        "mysql" => "mydb",
        _ => "",
    }
}

/// Real-time URL preview, mirroring the desktop's
/// `postgres://user@host:port/database` line.
fn connection_string_preview(state: &ConnectionFormState) -> String {
    let driver = DRIVER_OPTIONS
        .get(state.driver_idx)
        .copied()
        .unwrap_or("postgres");
    if driver == "sqlite" {
        let db = state.database_name.as_str().trim();
        return format!("sqlite://{}", if db.is_empty() { "<path>" } else { db });
    }
    let user = state.username.as_str().trim();
    let host = state.host.as_str().trim();
    let port = state.port.as_str().trim();
    let db = state.database_name.as_str().trim();
    let user_part = if user.is_empty() {
        String::new()
    } else {
        format!("{user}@")
    };
    let host_part = if host.is_empty() {
        "<host>".to_string()
    } else {
        host.to_string()
    };
    let port_part = if port.is_empty() {
        String::new()
    } else {
        format!(":{port}")
    };
    let db_part = if db.is_empty() {
        "<database>".to_string()
    } else {
        db.to_string()
    };
    format!("{driver}://{user_part}{host_part}{port_part}/{db_part}")
}

fn horizontal_split(area: Rect, gap: u16) -> (Rect, Rect) {
    let half = area.width.saturating_sub(gap) / 2;
    let left = Rect {
        x: area.x,
        y: area.y,
        width: half,
        height: area.height,
    };
    let right = Rect {
        x: area.x.saturating_add(half + gap),
        y: area.y,
        width: area.width.saturating_sub(half + gap),
        height: area.height,
    };
    (left, right)
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_form(state: &ConnectionFormState, w: u16, h: u16) -> (String, Option<(u16, u16)>) {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut cursor = None;
        terminal
            .draw(|f| {
                cursor = render(f, Rect::new(0, 0, w, h), state);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let text: String = (0..h)
            .map(|y| {
                let line: String = (0..w)
                    .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                    .collect();
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        (text, cursor)
    }

    #[test]
    fn render_paints_title_and_driver_tabs() {
        let state = ConnectionFormState::new();
        let (text, _) = render_form(&state, 80, 28);
        assert!(text.contains("New connection"), "title missing: {text}");
        assert!(text.contains("PostgreSQL"));
        assert!(text.contains("MySQL"));
        assert!(text.contains("SQLite"));
    }

    #[test]
    fn render_shows_connection_string_preview_for_postgres_default() {
        let state = ConnectionFormState::new();
        let (text, _) = render_form(&state, 80, 28);
        // Default driver is postgres; without host/db the preview uses
        // <host>/<database> placeholders.
        assert!(text.contains("postgres://"), "preview missing: {text}");
    }

    #[test]
    fn render_preview_swaps_to_sqlite_when_driver_changes() {
        let mut state = ConnectionFormState::new();
        state.driver_idx = 2; // sqlite
        state.database_name = crate::vim::lineedit::LineEdit::from_str("/tmp/x.db");
        let (text, _) = render_form(&state, 80, 28);
        assert!(text.contains("sqlite:///tmp/x.db"), "preview wrong: {text}");
    }

    #[test]
    fn render_returns_cursor_position_when_text_field_focused() {
        let mut state = ConnectionFormState::new();
        state.focus = ConnectionFormFocus::Name;
        // Insert two chars so the cursor advances to column 2.
        state.name.insert_char('a');
        state.name.insert_char('b');
        let (_, cursor) = render_form(&state, 80, 28);
        let (cx, _cy) = cursor.expect("cursor pos should be returned");
        // Popup is centered (62 wide in 80 → starts at x=9); inside the
        // popup the form has a 1-cell border + 2-cell indent + 2-cell
        // input prefix + 2-char text → cx = 9 + 1 + 2 + 2 + 2 = 16.
        assert_eq!(cx, 16);
    }

    #[test]
    fn render_returns_none_cursor_for_driver_and_readonly_fields() {
        let mut state = ConnectionFormState::new();
        state.focus = ConnectionFormFocus::Driver;
        let (_, cursor) = render_form(&state, 80, 28);
        assert!(cursor.is_none(), "driver tab focus has no caret");
        state.focus = ConnectionFormFocus::Readonly;
        let (_, cursor) = render_form(&state, 80, 28);
        assert!(cursor.is_none(), "readonly toggle has no caret");
    }

    #[test]
    fn render_masks_password_field() {
        let mut state = ConnectionFormState::new();
        state.password = crate::vim::lineedit::LineEdit::from_str("hunter2");
        let (text, _) = render_form(&state, 80, 28);
        // 7 chars → 7 bullets, and the raw value must NOT appear.
        assert!(
            text.contains("•••••••"),
            "password should be masked: {text}"
        );
        assert!(!text.contains("hunter2"), "raw password leaked: {text}");
    }

    #[test]
    fn render_displays_inline_error_when_set() {
        let mut state = ConnectionFormState::new();
        state.error = Some("name is required".into());
        let (text, _) = render_form(&state, 80, 28);
        assert!(
            text.contains("error: name is required"),
            "error not shown: {text}"
        );
    }

    #[test]
    fn render_smoke_does_not_panic_on_small_area() {
        let state = ConnectionFormState::new();
        let _ = render_form(&state, 40, 12);
    }
}
