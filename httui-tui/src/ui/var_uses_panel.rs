//! V4 P7 (2026-05-23): "Used in N" panel rendered no rodapé da pane
//! de vars. Lista até 4 entries do `grep_var_uses`; `+N more` se
//! exceder. Extraído de `ui/envs_page.rs` pra respeitar size limit.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::EnvsPageState;

use super::envs_page::truncate;

const MAX_ROWS: usize = 4;

pub(super) fn render_var_uses_panel(frame: &mut Frame, area: Rect, state: &EnvsPageState) {
    let header_label = if state.vars.get(state.selected_var).is_some() {
        format!(" Used in {}", state.var_uses.len())
    } else {
        " Used in —".to_string()
    };
    let header = Line::from(Span::styled(
        header_label,
        Style::default()
            .fg(Color::LightMagenta)
            .add_modifier(Modifier::BOLD),
    ));
    let mut lines: Vec<Line<'static>> = vec![header];

    if state.vars.get(state.selected_var).is_none() {
        lines.push(Line::from(Span::styled(
            "  (no variable selected)",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));
    } else if state.var_uses.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no references in this vault)",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));
    } else {
        for u in state.var_uses.iter().take(MAX_ROWS) {
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {}:{}  ", truncate(&u.file_path, 22), u.line),
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    truncate(&u.snippet, 36),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        if state.var_uses.len() > MAX_ROWS {
            lines.push(Line::from(Span::styled(
                format!(" +{} more", state.var_uses.len() - MAX_ROWS),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            )));
        }
    }

    let p = Paragraph::new(lines)
        .style(Style::default().bg(Color::Black))
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::VarRow;
    use httui_core::var_uses::VarUseEntry;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render(state: &EnvsPageState, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_var_uses_panel(f, Rect::new(0, 0, w, h), state);
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

    fn state_with(vars: Vec<&str>, uses: Vec<VarUseEntry>) -> EnvsPageState {
        EnvsPageState {
            vars: vars
                .into_iter()
                .map(|k| VarRow {
                    key: k.into(),
                    value: "v".into(),
                    is_secret: false,
                })
                .collect(),
            var_uses: uses,
            ..Default::default()
        }
    }

    fn entry(path: &str, line: usize, snippet: &str) -> VarUseEntry {
        VarUseEntry {
            file_path: path.into(),
            line,
            snippet: snippet.into(),
        }
    }

    #[test]
    fn shows_empty_state_with_no_var_selected() {
        let s = state_with(Vec::new(), Vec::new());
        let text = render(&s, 60, 7);
        assert!(text.contains("Used in —"));
        assert!(text.contains("no variable selected"));
    }

    #[test]
    fn shows_zero_references_message() {
        let s = state_with(vec!["API"], Vec::new());
        let text = render(&s, 60, 7);
        assert!(text.contains("Used in 0"));
        assert!(text.contains("no references in this vault"));
    }

    #[test]
    fn lists_entries() {
        let s = state_with(
            vec!["API"],
            vec![
                entry("nota.md", 12, "url: {{API}}/users"),
                entry("other.md", 3, "body: {{API.body}}"),
            ],
        );
        let text = render(&s, 80, 7);
        assert!(text.contains("Used in 2"));
        assert!(text.contains("nota.md:12"));
        assert!(text.contains("other.md:3"));
    }

    #[test]
    fn caps_at_4_with_more_marker() {
        let entries: Vec<VarUseEntry> = (1..=7)
            .map(|i| entry("a.md", i, &format!("hit {i}")))
            .collect();
        let s = state_with(vec!["API"], entries);
        let text = render(&s, 80, 7);
        assert!(text.contains("Used in 7"));
        assert!(text.contains("+3 more"));
    }
}
