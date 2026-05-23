//! Fullscreen Connections page (V3, 2026-05-23). Master-detail
//! layout listing every entry in `<vault>/connections.toml` with a
//! detail pane for the highlighted row. Triggered by `gC` / `Alt+P`.
//! P3 will add `n` (new) / `e` (edit); P4 adds `D` (delete) and
//! `t` (test).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{ConnectionDetail, ConnectionsPageState};

const SIDEBAR_PERCENT: u16 = 35;
/// Percentage of the available editor area covered by the popup;
/// the remaining border lets the editor underneath stay visible
/// (matches the chrome of the other modals — Help, BlockHistory).
const POPUP_PERCENT: u16 = 85;

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

    let title = format!(
        " Connections · {}/{} ",
        if state.connections.is_empty() {
            0
        } else {
            state.selected + 1
        },
        state.connections.len()
    );
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(Style::default().fg(Color::LightBlue).bg(Color::Black));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Split: sidebar (lista) | detail.
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(SIDEBAR_PERCENT),
            Constraint::Min(0),
        ])
        .split(inner);

    render_sidebar(frame, columns[0], state, bg_style);
    render_detail(frame, columns[1], state, bg_style);
    render_hint(frame, area);
}

fn render_sidebar(frame: &mut Frame, area: Rect, state: &ConnectionsPageState, bg: Style) {
    if state.connections.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "  no connections in this vault yet",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  press n to create one",
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
            let driver_chip = format!(" [{}]", c.driver);
            let line = Line::from(vec![
                Span::raw("  "),
                Span::styled(c.name.clone(), Style::default().fg(Color::White)),
                Span::styled(driver_chip, Style::default().fg(Color::DarkGray)),
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
        .highlight_symbol("▌ ");

    let mut list_state = ListState::default();
    list_state.select(Some(state.selected));
    frame.render_stateful_widget(list, area, &mut list_state);
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
    // Indent 2 cells from the divider so the column doesn't crowd
    // the right edge of the sidebar.
    let inner = Rect {
        x: area.x.saturating_add(2),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    };
    frame.render_widget(para, inner);
}

fn detail_lines(c: &ConnectionDetail) -> Vec<Line<'static>> {
    let label = |text: &str| {
        Span::styled(
            format!("{text:<14}"),
            Style::default().fg(Color::DarkGray),
        )
    };
    let value = |text: String| Span::styled(text, Style::default().fg(Color::White));
    let none = || Span::styled("—", Style::default().fg(Color::DarkGray));

    let opt = |s: &Option<String>| -> Span<'static> {
        s.as_ref()
            .map(|v| value(v.clone()))
            .unwrap_or_else(none)
    };
    let opt_port = |p: Option<u16>| -> Span<'static> {
        p.map(|v| value(v.to_string())).unwrap_or_else(none)
    };

    let mut lines = vec![
        Line::from(vec![
            Span::raw(""),
            Span::styled(
                c.name.clone(),
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            format!("{} connection", c.driver),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![label("host"), opt(&c.host)]),
        Line::from(vec![label("port"), opt_port(c.port)]),
        Line::from(vec![label("database"), opt(&c.database_name)]),
        Line::from(vec![label("username"), opt(&c.username)]),
        Line::from(vec![
            label("password"),
            if c.has_password {
                Span::styled("•••• (keychain)", Style::default().fg(Color::White))
            } else {
                none()
            },
        ]),
        Line::from(vec![label("ssl_mode"), opt(&c.ssl_mode)]),
        Line::from(vec![
            label("readonly"),
            value(if c.is_readonly { "yes".into() } else { "no".into() }),
        ]),
    ];
    if let Some(desc) = c.description.as_deref() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "description",
            Style::default().fg(Color::DarkGray),
        )));
        lines.push(Line::from(Span::styled(
            desc.to_string(),
            Style::default().fg(Color::White),
        )));
    }
    lines
}

fn centered_rect(area: Rect) -> Rect {
    let w = (area.width as u32 * POPUP_PERCENT as u32 / 100) as u16;
    let h = (area.height as u32 * POPUP_PERCENT as u32 / 100) as u16;
    let w = w.max(40);
    let h = h.max(12);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn render_hint(frame: &mut Frame, area: Rect) {
    // Render a single-line hint on the bottom border row, overwriting
    // the bottom edge with the chord vocabulary.
    if area.height < 2 {
        return;
    }
    let hint = " j/k navigate · Esc close · n new (P3) · e edit (P3) · D delete (P4) ";
    let hint_x = area
        .x
        .saturating_add(area.width.saturating_sub(hint.chars().count() as u16))
        .saturating_sub(2);
    let hint_y = area.y + area.height - 1;
    let para = Paragraph::new(Span::styled(
        hint,
        Style::default()
            .fg(Color::DarkGray)
            .bg(Color::Black)
            .add_modifier(Modifier::ITALIC),
    ));
    let hint_area = Rect {
        x: hint_x,
        y: hint_y,
        width: hint.chars().count() as u16,
        height: 1,
    };
    frame.render_widget(para, hint_area);
}
