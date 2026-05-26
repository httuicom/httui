//! Painters for the bootstrap empty-state.
//!
//! Cards screen is custom; Create/Clone/Open sub-screens delegate to
//! the existing `crate::ui::vault_*` renderers so the visual language
//! stays in sync with the in-app `Alt+W` flow.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use super::state::{BootstrapState, CardChoice, Screen};

pub fn render(frame: &mut Frame, state: &BootstrapState) -> Option<(u16, u16)> {
    let area = frame.area();
    // Background wash so terminals with translucent themes get a
    // consistent canvas.
    let bg = Style::default().bg(Color::Black);
    frame.render_widget(Block::default().style(bg), area);

    match &state.screen {
        Screen::Cards => {
            render_cards(frame, area, state.selected_card);
            None
        }
        Screen::Create(form) => crate::ui::vault_create_form::render(frame, area, form),
        Screen::Clone(form) => crate::ui::vault_clone_form::render(frame, area, form),
        Screen::Open(picker) => {
            crate::ui::vault_open_picker::render(frame, area, picker);
            None
        }
    }
}

const CARD_WIDTH: u16 = 28;
const CARD_HEIGHT: u16 = 9;
const CARD_GAP: u16 = 3;

fn render_cards(frame: &mut Frame, area: Rect, selected: CardChoice) {
    // Vertical stack: title, subtitle, spacer, cards row, spacer, footer.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // top padding
            Constraint::Length(2), // title
            Constraint::Length(1), // subtitle
            Constraint::Length(1), // spacer
            Constraint::Length(CARD_HEIGHT),
            Constraint::Min(1),    // flexible spacer
            Constraint::Length(1), // footer
            Constraint::Length(1), // bottom padding
        ])
        .split(area);

    let title = Paragraph::new(Line::from(vec![Span::styled(
        "Welcome to httui",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )]))
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(title, rows[1]);

    let subtitle = Paragraph::new(Line::from(vec![Span::styled(
        "Pick how you'd like to start your first vault",
        Style::default().fg(Color::Gray),
    )]))
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(subtitle, rows[2]);

    let cards_row = center_cards_row(rows[4]);
    let card_areas = split_cards(cards_row);
    for (i, choice) in CardChoice::ALL.iter().enumerate() {
        render_card(frame, card_areas[i], *choice, *choice == selected);
    }

    let chip_key = Style::default()
        .bg(Color::LightMagenta)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" ←→ ", chip_key),
        Span::styled(" select   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" open   ", chip_label),
        Span::styled(" o/g/n ", chip_key),
        Span::styled(" shortcut   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" quit ", chip_label),
    ]);
    frame.render_widget(
        Paragraph::new(footer).alignment(ratatui::layout::Alignment::Center),
        rows[6],
    );
}

fn split_cards(row: Rect) -> [Rect; 3] {
    let n_cards = CardChoice::ALL.len() as u16;
    let total_w = CARD_WIDTH * n_cards + CARD_GAP * (n_cards - 1);
    let start_x = row.x + row.width.saturating_sub(total_w) / 2;
    let mut out = [Rect::default(); 3];
    for (i, slot) in out.iter_mut().enumerate() {
        let x = start_x + (CARD_WIDTH + CARD_GAP) * (i as u16);
        *slot = Rect {
            x,
            y: row.y,
            width: CARD_WIDTH.min(row.width.saturating_sub(x - row.x)),
            height: CARD_HEIGHT.min(row.height),
        };
    }
    out
}

fn center_cards_row(area: Rect) -> Rect {
    // Use the full available row; split_cards handles the centering.
    area
}

fn render_card(frame: &mut Frame, area: Rect, choice: CardChoice, selected: bool) {
    let (title, shortcut, lines): (&str, &str, &[&str]) = match choice {
        CardChoice::Open => (
            " Open ",
            "o",
            &["Use an existing", "directory of", "markdown notes"],
        ),
        CardChoice::Clone => (
            " Clone ",
            "g",
            &[
                "Clone a git repo",
                "into a parent dir",
                "(GitHub, GitLab, …)",
            ],
        ),
        CardChoice::Create => (
            " Create ",
            "n",
            &["Scaffold a new", "vault with git", "in a parent dir"],
        ),
    };
    let border_style = if selected {
        Style::default()
            .fg(Color::LightMagenta)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let title_style = if selected {
        Style::default()
            .bg(Color::LightMagenta)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Gray)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(title.to_string(), title_style));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let body_lines = lines
        .iter()
        .map(|s| {
            Line::from(Span::styled(
                (*s).to_string(),
                Style::default().fg(Color::White),
            ))
        })
        .collect::<Vec<_>>();
    let body = Paragraph::new(body_lines)
        .alignment(ratatui::layout::Alignment::Center)
        .wrap(Wrap { trim: true });

    // Reserve the last inner row for the shortcut chip.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    frame.render_widget(body, chunks[0]);

    let chip = Line::from(vec![Span::styled(
        format!(" {shortcut} "),
        Style::default()
            .bg(Color::LightMagenta)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    )]);
    frame.render_widget(
        Paragraph::new(chip).alignment(ratatui::layout::Alignment::Center),
        chunks[1],
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn dump_backend(terminal: &Terminal<TestBackend>) -> String {
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

    #[test]
    fn cards_screen_renders_three_card_titles() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = BootstrapState::new();
        terminal
            .draw(|f| {
                render(f, &state);
            })
            .unwrap();
        let painted = dump_backend(&terminal);
        assert!(painted.contains("Open"), "Open card present");
        assert!(painted.contains("Clone"), "Clone card present");
        assert!(painted.contains("Create"), "Create card present");
        assert!(painted.contains("Welcome"), "welcome title present");
    }

    #[test]
    fn cards_screen_marks_default_selection() {
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        let state = BootstrapState::new();
        terminal
            .draw(|f| {
                render(f, &state);
            })
            .unwrap();
        // The buffer reflects styling — assert the selected card title
        // cell carries the highlight style (bg=LightMagenta).
        let buf = terminal.backend().buffer();
        let mut found_highlighted_open = false;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    if cell.symbol() == "O" && cell.bg == Color::LightMagenta {
                        found_highlighted_open = true;
                    }
                }
            }
        }
        assert!(
            found_highlighted_open,
            "default selection (Open) should render with highlight bg"
        );
    }
}
