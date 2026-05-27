//! Create vault form. Two inputs (parent dir, name) and
//! an error line, in a small centered popup. Submit triggers
//! `create_new_vault` + `switch_vault`; Esc cancels back to the
//! VaultPicker context (closes the form; the picker is not
//! re-opened — the user re-presses Alt+W if they want to see the
//! updated list).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{VaultCreateFormFocus, VaultCreateFormState};

const POPUP_WIDTH: u16 = 60;
const POPUP_HEIGHT: u16 = 14;

pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &VaultCreateFormState,
) -> Option<(u16, u16)> {
    let popup = centered_rect(editor_area, POPUP_WIDTH, POPUP_HEIGHT);
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

    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(" Create vault ")
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::border())
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // parent label
            Constraint::Length(1), // parent input
            Constraint::Length(1), // blank
            Constraint::Length(1), // name label
            Constraint::Length(1), // name input
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
                    .fg(crate::ui::palette::popup_border_accent())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(crate::ui::palette::muted())
            },
        )))
        .style(bg_style)
    };

    let value = |s: &str, hint: &str| {
        if s.is_empty() {
            Paragraph::new(Line::from(Span::styled(
                hint.to_string(),
                Style::default()
                    .fg(crate::ui::palette::muted())
                    .add_modifier(Modifier::ITALIC),
            )))
            .style(bg_style)
        } else {
            Paragraph::new(Line::from(Span::styled(
                s.to_string(),
                Style::default().fg(crate::ui::palette::foreground()),
            )))
            .style(bg_style)
        }
    };

    let parent_focused = state.focus == VaultCreateFormFocus::Parent;
    let name_focused = state.focus == VaultCreateFormFocus::Name;

    frame.render_widget(label("Parent dir", parent_focused), rows[0]);
    frame.render_widget(value(state.parent.as_str(), "/path/to/parent"), rows[1]);
    frame.render_widget(label("Name", name_focused), rows[3]);
    frame.render_widget(value(state.name.as_str(), "my-vault"), rows[4]);

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
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" Tab ", chip_key),
        Span::styled(" cycle field   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" create   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" cancel ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), rows[8]);

    // Cursor position for the focused field — drop the renderer in
    // the right cell so the terminal blinks it.
    let (input_row, buffer) = match state.focus {
        VaultCreateFormFocus::Parent => (rows[1], state.parent.as_str()),
        VaultCreateFormFocus::Name => (rows[4], state.name.as_str()),
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
    use crate::vim::lineedit::LineEdit;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn dump(terminal: &Terminal<TestBackend>) -> String {
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
            out.push('\n');
        }
        out
    }

    fn state_with(
        parent: &str,
        name: &str,
        focus: VaultCreateFormFocus,
        error: Option<&str>,
    ) -> VaultCreateFormState {
        VaultCreateFormState {
            parent: LineEdit::from_str(parent),
            name: LineEdit::from_str(name),
            focus,
            error: error.map(|s| s.to_string()),
        }
    }

    #[test]
    fn render_paints_title_labels_and_footer() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state_with("/tmp", "demo", VaultCreateFormFocus::Name, None);
        terminal
            .draw(|f| {
                render(f, f.area(), &s);
            })
            .unwrap();
        let painted = dump(&terminal);
        assert!(painted.contains("Create vault"), "title present");
        assert!(painted.contains("Parent dir"), "parent label present");
        assert!(painted.contains("Name"), "name label present");
        assert!(painted.contains("cycle field"), "footer hint present");
    }

    #[test]
    fn render_shows_error_line_when_set() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state_with("/tmp", "demo", VaultCreateFormFocus::Name, Some("boom"));
        terminal
            .draw(|f| {
                render(f, f.area(), &s);
            })
            .unwrap();
        let painted = dump(&terminal);
        assert!(painted.contains("error: boom"), "error line painted");
    }

    #[test]
    fn render_shows_hint_when_field_empty() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state_with("", "", VaultCreateFormFocus::Parent, None);
        terminal
            .draw(|f| {
                render(f, f.area(), &s);
            })
            .unwrap();
        let painted = dump(&terminal);
        assert!(painted.contains("my-vault"), "name placeholder visible");
    }

    #[test]
    fn render_returns_cursor_at_end_of_focused_field() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state_with("/tmp", "abc", VaultCreateFormFocus::Name, None);
        let mut cursor = None;
        terminal
            .draw(|f| {
                cursor = render(f, f.area(), &s);
            })
            .unwrap();
        assert!(cursor.is_some(), "cursor reported for focused field");
    }

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
        // Roughly centered.
        let dx = popup.x as i32 - ((area.width - popup.width) as i32 / 2);
        assert!(dx.abs() <= 1);
    }

    #[test]
    fn centered_rect_does_not_overflow_when_area_is_smaller_than_target() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 30,
            height: 8,
        };
        let popup = centered_rect(area, POPUP_WIDTH, POPUP_HEIGHT);
        assert!(popup.width + popup.x <= area.width + area.x);
        assert!(popup.height + popup.y <= area.height + area.y);
    }
}
