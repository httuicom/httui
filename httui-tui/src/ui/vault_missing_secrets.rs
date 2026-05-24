//! first-run secrets modal. Lists every missing
//! `{{keychain:...}}` reference in the active vault with an inline
//! input. Browse with jk/Tab; Enter (or `e`) starts editing the
//! focused row; Enter again saves to the keychain; `s` skips
//! (leaves the row in `pending_secrets` so the status-bar badge
//! surfaces it later); Esc closes.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::VaultMissingSecretsState;

const MAX_VISIBLE_ROWS: usize = 12;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &VaultMissingSecretsState) -> Option<(u16, u16)> {
    let popup = compute_popup_rect(editor_area, state);
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

    let pending = state.items.iter().filter(|r| !r.saved).count();
    let title = format!(" Pending secrets · {pending}/{} ", state.items.len());
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::BORDER)
                .bg(Color::Black),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // list
            Constraint::Length(1), // blank
            Constraint::Length(1), // value label
            Constraint::Length(1), // value input
            Constraint::Length(1), // footer
        ])
        .split(inner);
    let list_area = chunks[0];
    let value_label_area = chunks[2];
    let value_input_area = chunks[3];
    let footer_area = chunks[4];

    let items: Vec<ListItem> = state
        .items
        .iter()
        .map(|row| {
            let marker = if row.saved { "✓ " } else { "○ " };
            let marker_style = if row.saved {
                Style::default().bg(Color::Black).fg(Color::Green)
            } else {
                Style::default().bg(Color::Black).fg(Color::LightYellow)
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, marker_style),
                Span::styled(
                    row.label.clone(),
                    Style::default().bg(Color::Black).fg(Color::White),
                ),
            ]))
        })
        .collect();
    let list = List::new(items).style(bg_style).highlight_style(
        Style::default()
            .bg(super::palette::SELECTION_BG)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(
        state.selected.min(state.items.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, list_area, &mut list_state);

    let (label_text, label_style) = if state.editing {
        (
            "Value (Enter saves, Esc cancels)",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (
            "Value (Enter / e to edit, s to skip)",
            Style::default().fg(Color::DarkGray),
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(label_text.to_string(), label_style))).style(bg_style),
        value_label_area,
    );

    let focused_value = state
        .items
        .get(state.selected)
        .map(|r| r.value.as_str())
        .unwrap_or("");
    let value_widget = if focused_value.is_empty() && !state.editing {
        Paragraph::new(Line::from(Span::styled(
            "(empty — press Enter to type)".to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )))
        .style(bg_style)
    } else if state.editing {
        // Show as masked dots so the value never paints in clear on
        // a shared screen — same posture as the env var form.
        let masked = "•".repeat(focused_value.chars().count());
        Paragraph::new(Line::from(Span::styled(
            masked,
            Style::default().fg(Color::White),
        )))
        .style(bg_style)
    } else {
        Paragraph::new(Line::from(Span::styled(
            "•".repeat(focused_value.chars().count()),
            Style::default().fg(Color::DarkGray),
        )))
        .style(bg_style)
    };
    frame.render_widget(value_widget, value_input_area);

    let chip_key = Style::default()
        .bg(Color::LightMagenta)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = if state.editing {
        Line::from(vec![
            Span::styled(" Enter ", chip_key),
            Span::styled(" save   ", chip_label),
            Span::styled(" Esc ", chip_key),
            Span::styled(" cancel ", chip_label),
        ])
    } else {
        Line::from(vec![
            Span::styled(" jk ", chip_key),
            Span::styled(" navigate   ", chip_label),
            Span::styled(" Enter ", chip_key),
            Span::styled(" edit   ", chip_label),
            Span::styled(" s ", chip_key),
            Span::styled(" skip   ", chip_label),
            Span::styled(" Esc ", chip_key),
            Span::styled(" close ", chip_label),
        ])
    };
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);

    if state.editing {
        let buffer = focused_value;
        let x = value_input_area.x + buffer.chars().count() as u16;
        let y = value_input_area.y;
        Some((
            x.min(value_input_area.x + value_input_area.width.saturating_sub(1)),
            y,
        ))
    } else {
        None
    }
}

fn compute_popup_rect(area: Rect, state: &VaultMissingSecretsState) -> Rect {
    const PADDING: u16 = 6;
    let longest = state
        .items
        .iter()
        .map(|r| r.label.chars().count())
        .max()
        .unwrap_or(30) as u16;
    let width = (longest + PADDING).clamp(50, area.width.saturating_sub(2));
    let list_rows = state.items.len().min(MAX_VISIBLE_ROWS) as u16;
    // list + blank + label + input + footer + borders = list + 6
    let height = list_rows.max(1) + 6;

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + 3u16.min(area.height.saturating_sub(height));
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::MissingSecretRow;
    use httui_core::vault_config::missing_secrets::MissingKind;
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

    fn row(label: &str) -> MissingSecretRow {
        MissingSecretRow {
            keychain_key: format!("env:test:{label}"),
            label: label.to_string(),
            kind: MissingKind::Env,
            value: crate::vim::lineedit::LineEdit::new(),
            saved: false,
        }
    }

    fn row_with_value(label: &str, val: &str) -> MissingSecretRow {
        let mut r = row(label);
        r.value = crate::vim::lineedit::LineEdit::from_str(val);
        r
    }

    #[test]
    fn render_paints_title_and_footer_in_browse_mode() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = VaultMissingSecretsState {
            items: vec![row("API_TOKEN"), row("DB_PWD")],
            selected: 0,
            editing: false,
        };
        terminal.draw(|f| {
            render(f, f.area(), &state);
        }).unwrap();
        let painted = dump(&terminal);
        assert!(painted.contains("Pending secrets"));
        assert!(painted.contains("API_TOKEN"));
        assert!(painted.contains("DB_PWD"));
        assert!(painted.contains("navigate"));
    }

    #[test]
    fn render_in_editing_mode_shows_save_footer_and_cursor() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = VaultMissingSecretsState {
            items: vec![row_with_value("TOKEN", "abc")],
            selected: 0,
            editing: true,
        };
        let mut cursor = None;
        terminal.draw(|f| {
            cursor = render(f, f.area(), &state);
        }).unwrap();
        assert!(cursor.is_some(), "cursor reported in editing mode");
        let painted = dump(&terminal);
        assert!(painted.contains("save"));
        assert!(painted.contains("•"), "value masked with bullets");
    }

    #[test]
    fn render_shows_empty_hint_when_value_empty_and_not_editing() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = VaultMissingSecretsState {
            items: vec![row("TOKEN")],
            selected: 0,
            editing: false,
        };
        terminal.draw(|f| {
            render(f, f.area(), &state);
        }).unwrap();
        assert!(dump(&terminal).contains("(empty"));
    }

    #[test]
    fn render_marks_saved_rows_with_check() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut saved_row = row("TOKEN");
        saved_row.saved = true;
        let state = VaultMissingSecretsState {
            items: vec![saved_row],
            selected: 0,
            editing: false,
        };
        terminal.draw(|f| {
            render(f, f.area(), &state);
        }).unwrap();
        let painted = dump(&terminal);
        assert!(painted.contains("✓"), "saved marker present");
    }

    #[test]
    fn compute_popup_rect_fits_within_area() {
        let state = VaultMissingSecretsState {
            items: vec![row("token-a"), row("token-b")],
            selected: 0,
            editing: false,
        };
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        let popup = compute_popup_rect(area, &state);
        assert!(popup.width <= area.width.saturating_sub(2));
        assert!(popup.width >= 50);
    }
}
