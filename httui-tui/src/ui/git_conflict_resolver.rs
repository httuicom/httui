//! 3-way conflict resolver — file list on the left + base / ours /
//! theirs side-by-side. `1`/`2`/`3` pick the chosen version.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::git::GitConflictResolverState;

pub fn render(frame: &mut Frame, area: Rect, state: &GitConflictResolverState) {
    let outer = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(30)])
        .split(area);
    render_file_list(frame, outer[0], state);
    render_versions(frame, outer[1], state);
}

fn render_file_list(frame: &mut Frame, area: Rect, state: &GitConflictResolverState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::LightYellow))
        .title(Span::styled(
            format!(" Conflicts ({}) ", state.files.len()),
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem<'static>> = state
        .files
        .iter()
        .map(|f| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    "! ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw(f.clone()),
            ]))
        })
        .collect();
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );
    let mut s = ListState::default();
    if !state.files.is_empty() {
        s.select(Some(state.selected_file.min(state.files.len() - 1)));
    }
    frame.render_stateful_widget(list, inner, &mut s);
}

fn render_versions(frame: &mut Frame, area: Rect, state: &GitConflictResolverState) {
    let outer_block = Block::default().borders(Borders::ALL).border_style(
        Style::default().fg(Color::DarkGray),
    );
    let inner = outer_block.inner(area);
    frame.render_widget(outer_block, area);

    let hints_height = 1u16;
    let body_height = inner.height.saturating_sub(hints_height);
    let body_area = Rect::new(inner.x, inner.y, inner.width, body_height);
    let hints_area = Rect::new(inner.x, inner.y + body_height, inner.width, hints_height);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(body_area);

    let (base_body, ours_body, theirs_body) = body_strings(state);
    render_column(frame, cols[0], "1 — base", &base_body, Color::Cyan);
    render_column(frame, cols[1], "2 — ours", &ours_body, Color::Yellow);
    render_column(frame, cols[2], "3 — theirs", &theirs_body, Color::Green);

    let hint = Line::from(vec![
        Span::styled(" [1] ", Style::default().fg(Color::Cyan)),
        Span::raw("base   "),
        Span::styled("[2] ", Style::default().fg(Color::Yellow)),
        Span::raw("ours   "),
        Span::styled("[3] ", Style::default().fg(Color::Green)),
        Span::raw("theirs   "),
        Span::styled("[j/k] ", Style::default().fg(Color::Gray)),
        Span::raw("file   "),
        Span::styled("[Esc] ", Style::default().fg(Color::Gray)),
        Span::raw("close"),
    ]);
    frame.render_widget(Paragraph::new(hint), hints_area);
}

fn render_column(frame: &mut Frame, area: Rect, title: &str, body: &str, color: Color) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let lines: Vec<Line<'static>> = body
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn body_strings(state: &GitConflictResolverState) -> (String, String, String) {
    match (state.versions.as_ref(), state.error.as_ref()) {
        (Some(v), _) => (v.base.clone(), v.ours.clone(), v.theirs.clone()),
        (None, Some(err)) => (err.clone(), err.clone(), err.clone()),
        (None, None) => (
            "loading…".to_string(),
            "loading…".to_string(),
            "loading…".to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::git::conflict::ConflictVersions;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_text(state: &GitConflictResolverState, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, f.area(), state)).unwrap();
        let buf = term.backend().buffer();
        let mut out = String::new();
        for y in 0..h {
            for x in 0..w {
                out.push_str(buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn renders_file_list_and_loading_placeholders() {
        let state = GitConflictResolverState::new(vec!["a.md".into(), "b.md".into()]);
        let text = render_to_text(&state, 130, 14);
        assert!(text.contains("a.md"));
        assert!(text.contains("b.md"));
        assert!(text.contains("loading"));
        assert!(text.contains("base"));
        assert!(text.contains("ours"));
        assert!(text.contains("theirs"));
    }

    #[test]
    fn renders_each_version_body_in_its_column() {
        let mut state = GitConflictResolverState::new(vec!["a.md".into()]);
        state.versions = Some(ConflictVersions {
            base: "BASEBODY".into(),
            ours: "OURSBODY".into(),
            theirs: "THEIRSBODY".into(),
        });
        let text = render_to_text(&state, 150, 14);
        assert!(text.contains("BASEBODY"));
        assert!(text.contains("OURSBODY"));
        assert!(text.contains("THEIRSBODY"));
    }

    #[test]
    fn paints_hint_row_with_keyboard_shortcuts() {
        let state = GitConflictResolverState::new(vec!["a.md".into()]);
        let text = render_to_text(&state, 150, 14);
        assert!(text.contains("[1]"));
        assert!(text.contains("[2]"));
        assert!(text.contains("[3]"));
        assert!(text.contains("[Esc]"));
    }
}
