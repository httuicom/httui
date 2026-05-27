//! V3 P4 (2026-05-23): destructive confirm modal for deleting a
//! connection. Small centered popup: "Delete connection X? (y/N)".
//! Painted on top of the Connections page; `y`/`Enter` confirms,
//! `n`/`Esc` reopens the page unchanged.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::ConnectionDeleteConfirmState;

const POPUP_WIDTH: u16 = 50;
const POPUP_HEIGHT: u16 = 8;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &ConnectionDeleteConfirmState) {
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
        .title(" Delete connection ")
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(Color::LightRed)
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Delete "),
            Span::styled(
                format!("\"{}\"", state.name),
                Style::default()
                    .fg(crate::ui::palette::popup_border_accent())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("?"),
        ]),
        Line::from(Span::styled(
            "  This rewrites connections.toml. Cannot be undone.",
            Style::default()
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                " y / Enter ",
                Style::default()
                    .fg(crate::ui::palette::popup_bg())
                    .bg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" delete    "),
            Span::styled(
                " n / Esc ",
                Style::default()
                    .fg(crate::ui::palette::foreground())
                    .bg(crate::ui::palette::muted())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" cancel"),
        ]),
    ];
    let para = Paragraph::new(lines)
        .style(bg_style)
        .wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
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

    fn render_to_text(state: &ConnectionDeleteConfirmState, w: u16, h: u16) -> String {
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
    fn render_shows_name_and_keys() {
        let state = ConnectionDeleteConfirmState {
            name: "old-htui".into(),
        };
        let text = render_to_text(&state, 80, 14);
        assert!(text.contains("Delete connection"));
        assert!(text.contains("\"old-htui\""));
        assert!(text.contains("y / Enter"));
        assert!(text.contains("n / Esc"));
        assert!(text.contains("rewrites connections.toml"));
    }

    #[test]
    fn render_smoke_small_area() {
        let state = ConnectionDeleteConfirmState { name: "x".into() };
        let _ = render_to_text(&state, 30, 8);
    }
}
