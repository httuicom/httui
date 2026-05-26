//! Centered popup asking the user to confirm `git push -u <remote>
//! <branch>` after a regular push found no upstream. Mirrors the
//! shape of `connection_delete_confirm`: bordered Box, two prompt
//! lines, a hint row.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::git::GitSetUpstreamConfirmState;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &GitSetUpstreamConfirmState) {
    let width = 56.min(editor_area.width.saturating_sub(4));
    let height = 7.min(editor_area.height.saturating_sub(2));
    let x = editor_area.x + (editor_area.width.saturating_sub(width)) / 2;
    let y = editor_area.y + (editor_area.height.saturating_sub(height)) / 2;
    let area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::LightYellow))
        .title(Span::styled(
            " Set upstream? ",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = vec![
        Line::from(Span::styled(
            format!("Push branch '{}' with upstream '{}'?", state.branch, state.remote),
            Style::default().fg(Color::White),
        )),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled(
                "git push -u ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{} {}", state.remote, state.branch),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(Span::raw("")),
        Line::from(vec![
            Span::styled("[y/Enter] ", Style::default().fg(Color::Green)),
            Span::styled("confirm   ", Style::default().fg(Color::Gray)),
            Span::styled("[n/Esc] ", Style::default().fg(Color::Red)),
            Span::styled("cancel", Style::default().fg(Color::Gray)),
        ]),
    ];
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_text(state: &GitSetUpstreamConfirmState, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, f.area(), state)).unwrap();
        let buffer = term.backend().buffer();
        let mut out = String::new();
        for y in 0..h {
            for x in 0..w {
                out.push_str(buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn renders_remote_and_branch_in_the_command_line() {
        let state = GitSetUpstreamConfirmState {
            remote: "origin".into(),
            branch: "feature/x".into(),
        };
        let text = render_to_text(&state, 70, 12);
        assert!(text.contains("origin"));
        assert!(text.contains("feature/x"));
        assert!(text.contains("git push -u"));
        assert!(text.contains("[y/Enter]"));
        assert!(text.contains("[n/Esc]"));
    }
}
