//! Clone vault form. URL + parent dir, submit clones
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

pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &VaultCloneFormState,
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
        .title(" Clone vault ")
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

    let url_focused = state.focus == VaultCloneFormFocus::Url;
    let parent_focused = state.focus == VaultCloneFormFocus::Parent;

    frame.render_widget(label("Git URL", url_focused), rows[0]);
    frame.render_widget(
        value(state.url.as_str(), "https://github.com/org/repo.git"),
        rows[1],
    );
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
        .fg(crate::ui::palette::popup_bg())
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
        url: &str,
        parent: &str,
        focus: VaultCloneFormFocus,
        error: Option<&str>,
    ) -> VaultCloneFormState {
        VaultCloneFormState {
            url: LineEdit::from_str(url),
            parent: LineEdit::from_str(parent),
            focus,
            error: error.map(|s| s.to_string()),
        }
    }

    #[test]
    fn render_paints_title_labels_and_footer() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state_with("https://x.git", "/tmp", VaultCloneFormFocus::Url, None);
        terminal
            .draw(|f| {
                render(f, f.area(), &s);
            })
            .unwrap();
        let painted = dump(&terminal);
        assert!(painted.contains("Clone vault"));
        assert!(painted.contains("Git URL"));
        assert!(painted.contains("Parent dir"));
        assert!(painted.contains("cycle field"));
    }

    #[test]
    fn render_shows_error_when_set() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state_with("", "/tmp", VaultCloneFormFocus::Url, Some("nope"));
        terminal
            .draw(|f| {
                render(f, f.area(), &s);
            })
            .unwrap();
        assert!(dump(&terminal).contains("error: nope"));
    }

    #[test]
    fn render_shows_hint_for_empty_fields() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state_with("", "", VaultCloneFormFocus::Parent, None);
        terminal
            .draw(|f| {
                render(f, f.area(), &s);
            })
            .unwrap();
        let painted = dump(&terminal);
        assert!(
            painted.contains("https://github.com"),
            "url placeholder visible"
        );
    }

    #[test]
    fn render_returns_cursor_for_focused_field() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state_with("g", "/tmp", VaultCloneFormFocus::Url, None);
        let mut cursor = None;
        terminal
            .draw(|f| {
                cursor = render(f, f.area(), &s);
            })
            .unwrap();
        assert!(cursor.is_some());
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
    }
}
