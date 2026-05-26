//! Commit-form sub-renderer for the git side panel. Split out of
//! `ui::git_panel` to keep that file under the size gate. Paints the
//! draft / placeholder line, optional error, amend toggle and key
//! hints (Enter = commit, Ctrl+⏎ = sync, Ctrl+A = amend).

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::git::{template::commit_template, GitPanel};

pub(super) fn render_commit_form(
    frame: &mut Frame,
    area: Rect,
    panel: &GitPanel,
    focused: bool,
    commit_tpl: &str,
) -> Option<(u16, u16)> {
    if area.height < 2 {
        return None;
    }
    let border_color = if focused {
        Color::LightYellow
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(" message ", Style::default().fg(Color::Gray)));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let draft = panel.commit_message.as_str();
    let placeholder = panel
        .status
        .as_ref()
        .map(|s| commit_template(s, commit_tpl))
        .unwrap_or_default();
    let (text, style) = if draft.is_empty() {
        if placeholder.is_empty() {
            (
                "type a commit message…".to_string(),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            (placeholder, Style::default().fg(Color::DarkGray))
        }
    } else {
        (draft.to_string(), Style::default().fg(Color::White))
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(Span::styled(text, style)));
    if let Some(err) = panel.commit_error.as_ref() {
        lines.push(Line::from(Span::styled(
            err.lines().next().unwrap_or("").to_string(),
            Style::default().fg(Color::Red),
        )));
    }
    lines.push(amend_toggle_line(panel.amend));
    lines.push(key_hint_line());

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);

    if focused && inner.width > 0 && inner.height > 0 {
        let cursor_col = panel.commit_message.cursor_col() as u16;
        let x = inner.x + cursor_col.min(inner.width.saturating_sub(1));
        Some((x, inner.y))
    } else {
        None
    }
}

fn amend_toggle_line(amend: bool) -> Line<'static> {
    let (marker, style) = if amend {
        (
            "[x] amend last",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("[ ] amend last", Style::default().fg(Color::DarkGray))
    };
    Line::from(vec![
        Span::styled(marker.to_string(), style),
        Span::styled("   [Ctrl+A]", Style::default().fg(Color::DarkGray)),
    ])
}

fn key_hint_line() -> Line<'static> {
    Line::from(vec![
        Span::styled(
            "[Enter] ",
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("commit  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            "[Ctrl+⏎] ",
            Style::default()
                .fg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("sync", Style::default().fg(Color::DarkGray)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn amend_toggle_renders_checked_marker_when_active() {
        let line = amend_toggle_line(true);
        let raw = span_text(&line);
        assert!(raw.starts_with("[x] amend last"));
        assert!(raw.contains("[Ctrl+A]"));
    }

    #[test]
    fn amend_toggle_renders_unchecked_marker_when_inactive() {
        let line = amend_toggle_line(false);
        let raw = span_text(&line);
        assert!(raw.starts_with("[ ] amend last"));
        assert!(raw.contains("[Ctrl+A]"));
    }

    #[test]
    fn key_hint_line_shows_commit_and_sync_chords() {
        let line = key_hint_line();
        let raw = span_text(&line);
        assert!(raw.contains("[Enter]"));
        assert!(raw.contains("commit"));
        assert!(raw.contains("[Ctrl+⏎]"));
        assert!(raw.contains("sync"));
    }
}
