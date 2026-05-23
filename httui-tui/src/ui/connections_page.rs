//! Connections page (V3, 2026-05-23 + polish 2026-05-23). Master-
//! detail popup listing every entry in `<vault>/connections.toml`.
//! Polish pass aproxima a UX do desktop: chip colorido por driver,
//! detail agrupado em sections, popup denso (não fullscreen).
//! Triggered by `gC` / `Alt+P`. `n` opens the create form; `e`/`D`
//! land in P3/P4.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{ConnectionDetail, ConnectionsPageState};

const POPUP_WIDTH: u16 = 64;
const POPUP_HEIGHT: u16 = 20;
const SIDEBAR_COLS: u16 = 22;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &ConnectionsPageState) {
    let area = centered_rect(editor_area);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard-fill so editor content underneath doesn't bleed through.
    {
        let buf = frame.buffer_mut();
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let title = format!(" Connections · {} ", state.connections.len());
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(Style::default().fg(Color::LightBlue).bg(Color::Black));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Vertical: body | hint footer.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    // Body: sidebar | divider | detail.
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(SIDEBAR_COLS),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(rows[0]);

    render_sidebar(frame, body[0], state, bg_style);
    render_divider(frame, body[1], bg_style);
    render_detail(frame, body[2], state, bg_style);
    render_hint(frame, rows[1], bg_style);
}

fn render_sidebar(frame: &mut Frame, area: Rect, state: &ConnectionsPageState, bg: Style) {
    if state.connections.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                " no conns yet",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                " press n to add",
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::ITALIC),
            )),
        ])
        .style(bg)
        .wrap(Wrap { trim: false });
        frame.render_widget(empty, area);
        return;
    }

    let items: Vec<ListItem> = state
        .connections
        .iter()
        .map(|c| {
            let (chip_label, chip_color) = driver_chip(&c.driver);
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!(" {chip_label} "),
                    Style::default()
                        .bg(chip_color)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(c.name.clone(), Style::default().fg(Color::White)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .style(bg)
        .highlight_style(
            Style::default()
                .bg(crate::ui::palette::SELECTION_BG)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▌");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));
    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_divider(frame: &mut Frame, area: Rect, _bg: Style) {
    let buf = frame.buffer_mut();
    for y in area.y..area.y.saturating_add(area.height) {
        if let Some(cell) = buf.cell_mut((area.x, y)) {
            cell.set_symbol("│");
            cell.set_style(Style::default().fg(Color::DarkGray).bg(Color::Black));
        }
    }
}

fn render_detail(frame: &mut Frame, area: Rect, state: &ConnectionsPageState, bg: Style) {
    let Some(detail) = state.connections.get(state.selected) else {
        let empty = Paragraph::new("  (no connection selected)")
            .style(bg.fg(Color::DarkGray));
        frame.render_widget(empty, area);
        return;
    };

    let lines = detail_lines(detail);
    let para = Paragraph::new(lines).style(bg).wrap(Wrap { trim: false });
    let inner = Rect {
        x: area.x.saturating_add(2),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };
    frame.render_widget(para, inner);
}

fn driver_chip(driver: &str) -> (&'static str, Color) {
    match driver {
        "postgres" => ("PG", Color::Cyan),
        "mysql" => ("MY", Color::LightYellow),
        "sqlite" => ("SL", Color::LightGreen),
        _ => ("??", Color::DarkGray),
    }
}

fn detail_lines(c: &ConnectionDetail) -> Vec<Line<'static>> {
    let label = |text: &str| {
        Span::styled(
            format!("{text:<10}"),
            Style::default().fg(Color::DarkGray),
        )
    };
    let value = |text: String| Span::styled(text, Style::default().fg(Color::White));
    let none = || Span::styled("—", Style::default().fg(Color::DarkGray));
    let opt = |s: &Option<String>| -> Span<'static> {
        s.as_ref().map(|v| value(v.clone())).unwrap_or_else(none)
    };
    let opt_port = |p: Option<u16>| -> Span<'static> {
        p.map(|v| value(v.to_string())).unwrap_or_else(none)
    };
    let (chip_label, chip_color) = driver_chip(&c.driver);

    let mut lines = vec![
        // Header row: name + driver chip.
        Line::from(vec![
            Span::styled(
                c.name.clone(),
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!(" {chip_label} "),
                Style::default()
                    .bg(chip_color)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", c.driver),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(""),
        // Section: Connection.
        section_header("Connection"),
        Line::from(vec![label("host"), opt(&c.host)]),
        Line::from(vec![label("port"), opt_port(c.port)]),
        Line::from(vec![label("database"), opt(&c.database_name)]),
        Line::from(""),
        // Section: Auth.
        section_header("Auth"),
        Line::from(vec![label("username"), opt(&c.username)]),
        Line::from(vec![
            label("password"),
            if c.has_password {
                Span::styled("•••• (keychain)", Style::default().fg(Color::White))
            } else {
                none()
            },
        ]),
        Line::from(""),
        // Section: Options.
        section_header("Options"),
        Line::from(vec![label("ssl_mode"), opt(&c.ssl_mode)]),
        Line::from(vec![
            label("readonly"),
            if c.is_readonly {
                Span::styled(
                    "yes",
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("no", Style::default().fg(Color::White))
            },
        ]),
    ];
    if let Some(desc) = c.description.as_deref() {
        lines.push(Line::from(""));
        lines.push(section_header("Description"));
        lines.push(Line::from(Span::styled(
            desc.to_string(),
            Style::default().fg(Color::White),
        )));
    }
    lines
}

fn section_header(title: &str) -> Line<'static> {
    Line::from(vec![Span::styled(
        format!("── {title} "),
        Style::default()
            .fg(Color::LightBlue)
            .add_modifier(Modifier::BOLD),
    )])
}

fn centered_rect(area: Rect) -> Rect {
    let w = POPUP_WIDTH.min(area.width.saturating_sub(2));
    let h = POPUP_HEIGHT.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn render_hint(frame: &mut Frame, area: Rect, _bg: Style) {
    let hint = " j/k nav · n new · e edit · t test · D del · Esc close ";
    let para = Paragraph::new(Span::styled(
        hint,
        Style::default()
            .fg(Color::DarkGray)
            .bg(Color::Black)
            .add_modifier(Modifier::ITALIC),
    ));
    frame.render_widget(para, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn detail(name: &str, driver: &str) -> ConnectionDetail {
        ConnectionDetail {
            name: name.into(),
            driver: driver.into(),
            host: Some("localhost".into()),
            port: Some(5432),
            database_name: Some("mydb".into()),
            username: Some("user".into()),
            has_password: true,
            ssl_mode: None,
            is_readonly: false,
            description: None,
        }
    }

    fn render_page(state: &ConnectionsPageState, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(f, Rect::new(0, 0, w, h), state);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..h)
            .map(|y| {
                let line: String = (0..w)
                    .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                    .collect();
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn render_empty_state_shows_hint() {
        let state = ConnectionsPageState {
            connections: Vec::new(),
            selected: 0,
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("Connections · 0"));
        assert!(text.contains("no conns yet"));
        assert!(text.contains("press n to add"));
    }

    #[test]
    fn render_populated_list_paints_entries_and_chips() {
        let state = ConnectionsPageState {
            connections: vec![
                detail("Test", "sqlite"),
                detail("old-htui", "postgres"),
            ],
            selected: 0,
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("Connections · 2"));
        assert!(text.contains("Test"));
        assert!(text.contains("old-htui"));
        // Chip labels (SL/PG) appear in the sidebar.
        assert!(text.contains("SL"), "sqlite chip missing: {text}");
        assert!(text.contains("PG"), "postgres chip missing: {text}");
    }

    #[test]
    fn render_detail_pane_groups_into_sections() {
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("Connection"));
        assert!(text.contains("Auth"));
        assert!(text.contains("Options"));
        // Specific value lines.
        assert!(text.contains("localhost"));
        assert!(text.contains("mydb"));
        assert!(text.contains("user"));
        assert!(text.contains("•••• (keychain)"));
    }

    #[test]
    fn render_includes_description_section_when_present() {
        let mut d = detail("Test", "sqlite");
        d.description = Some("local dev db".into());
        let state = ConnectionsPageState {
            connections: vec![d],
            selected: 0,
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("Description"));
        assert!(text.contains("local dev db"));
    }

    #[test]
    fn render_omits_description_when_absent() {
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
        };
        let text = render_page(&state, 80, 24);
        assert!(!text.contains("Description"));
    }

    #[test]
    fn render_readonly_yes_when_flagged() {
        let mut d = detail("Test", "sqlite");
        d.is_readonly = true;
        let state = ConnectionsPageState {
            connections: vec![d],
            selected: 0,
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("yes"));
    }

    #[test]
    fn render_footer_hint_lists_chords() {
        let state = ConnectionsPageState {
            connections: vec![detail("Test", "sqlite")],
            selected: 0,
        };
        let text = render_page(&state, 80, 24);
        assert!(text.contains("j/k nav"));
        assert!(text.contains("Esc close"));
    }

    #[test]
    fn driver_chip_unknown_driver_falls_back() {
        let (label, _) = driver_chip("oracle");
        assert_eq!(label, "??");
    }

    #[test]
    fn render_smoke_does_not_panic_on_small_area() {
        let state = ConnectionsPageState {
            connections: vec![detail("a", "sqlite")],
            selected: 0,
        };
        let _ = render_page(&state, 30, 10);
    }
}
