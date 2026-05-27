//! Full-screen git log page — list of commits on the left, diff for
//! the selected commit on the right. Diff body is fetched lazily by
//! the apply layer before render; we just project the cached string.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::git::GitLogPageState;

pub fn render(frame: &mut Frame, area: Rect, state: &GitLogPageState) {
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(38), Constraint::Min(20)])
        .split(area);
    render_list(frame, split[0], state);
    render_diff(frame, split[1], state);
}

fn render_list(frame: &mut Frame, area: Rect, state: &GitLogPageState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(crate::ui::palette::popup_border_accent()))
        .title(Span::styled(
            format!(" Commits ({}) ", state.commits.len()),
            Style::default()
                .fg(crate::ui::palette::popup_border_accent())
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem<'static>> = state
        .commits
        .iter()
        .map(|c| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", c.short_sha),
                    Style::default().fg(crate::ui::palette::muted()),
                ),
                Span::raw(c.subject.clone()),
            ]))
        })
        .collect();
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(crate::ui::palette::popup_bg())
            .add_modifier(Modifier::BOLD),
    );
    let mut s = ListState::default();
    if !state.commits.is_empty() {
        s.select(Some(state.selected.min(state.commits.len() - 1)));
    }
    frame.render_stateful_widget(list, inner, &mut s);
}

fn render_diff(frame: &mut Frame, area: Rect, state: &GitLogPageState) {
    let title = state
        .commits
        .get(state.selected)
        .map(|c| format!(" {} — {} ", c.short_sha, c.subject))
        .unwrap_or_else(|| " diff ".to_string());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(crate::ui::palette::muted()))
        .title(Span::styled(title, Style::default().fg(Color::Gray)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let body = match (state.diff.as_ref(), state.error.as_ref()) {
        (Some(b), _) => b.clone(),
        (None, Some(err)) => format!("error: {err}"),
        (None, None) => "loading diff…".to_string(),
    };

    let lines: Vec<Line<'static>> = body
        .lines()
        .skip(state.diff_scroll as usize)
        .map(|raw| {
            let style = if raw.starts_with('+') && !raw.starts_with("+++") {
                Style::default().fg(Color::Green)
            } else if raw.starts_with('-') && !raw.starts_with("---") {
                Style::default().fg(Color::Red)
            } else if raw.starts_with("@@") {
                Style::default().fg(crate::ui::palette::popup_key_label())
            } else if raw.starts_with("diff --git")
                || raw.starts_with("commit ")
                || raw.starts_with("Author:")
                || raw.starts_with("Date:")
            {
                Style::default().fg(crate::ui::palette::popup_border_accent())
            } else {
                Style::default()
            };
            Line::from(Span::styled(raw.to_string(), style))
        })
        .collect();
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::git::log::CommitInfo;
    use ratatui::{backend::TestBackend, Terminal};

    fn commit(short: &str, sha: &str, subject: &str) -> CommitInfo {
        CommitInfo {
            sha: sha.into(),
            short_sha: short.into(),
            author_name: "A".into(),
            author_email: "a@b".into(),
            timestamp: 0,
            subject: subject.into(),
        }
    }

    fn render_to_text(state: &GitLogPageState, w: u16, h: u16) -> String {
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
    fn renders_commit_list_and_diff_placeholder() {
        let state = GitLogPageState::new(vec![commit("abc1234", "abc1234deadbeef", "first")]);
        let text = render_to_text(&state, 100, 12);
        assert!(text.contains("abc1234"));
        assert!(text.contains("first"));
        assert!(text.contains("loading diff"));
    }

    #[test]
    fn renders_diff_body_and_highlights_additions_deletions() {
        let mut state = GitLogPageState::new(vec![commit("abc1234", "abc1234deadbeef", "first")]);
        state.diff =
            Some("commit abc1234\ndiff --git a/x b/x\n@@ -1 +1 @@\n-old\n+new\n".to_string());
        let text = render_to_text(&state, 100, 12);
        assert!(text.contains("+new"));
        assert!(text.contains("-old"));
        assert!(text.contains("@@ -1 +1 @@"));
    }

    #[test]
    fn renders_error_when_diff_load_failed() {
        let mut state = GitLogPageState::new(vec![commit("abc1234", "abc1234deadbeef", "first")]);
        state.error = Some("boom".into());
        let text = render_to_text(&state, 100, 12);
        assert!(text.contains("error: boom"));
    }
}
