//! Git side-panel renderer — bordered column to the right of the
//! editor. Lote A is the visual scaffold: header (branch /
//! ahead-behind) + body that surfaces either a "not a git repo" hint
//! or a placeholder line. File list + commit form land in Lote B/C.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::git::GitPanel;

const PANEL_WIDTH: u16 = 36;

pub fn width() -> u16 {
    PANEL_WIDTH
}

pub fn render(frame: &mut Frame, area: Rect, panel: &GitPanel, focused: bool) {
    let (border_color, title_style) = if focused {
        (
            Color::LightYellow,
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (Color::DarkGray, Style::default().fg(Color::Gray))
    };

    let title = format!(" Git — {} ", header_label(panel));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(title, title_style));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = body_lines(panel);
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

fn header_label(panel: &GitPanel) -> String {
    match (&panel.status, &panel.status_error) {
        (Some(status), _) => {
            let branch = status.branch.as_deref().unwrap_or("detached");
            if status.ahead == 0 && status.behind == 0 {
                branch.to_string()
            } else {
                format!("{branch} ↑{} ↓{}", status.ahead, status.behind)
            }
        }
        (None, Some(_)) => "no repo".to_string(),
        (None, None) => "loading…".to_string(),
    }
}

fn body_lines(panel: &GitPanel) -> Vec<Line<'static>> {
    match (&panel.status, &panel.status_error) {
        (Some(status), _) if status.clean => vec![Line::from(Span::styled(
            "Working tree clean.",
            Style::default().fg(Color::Green),
        ))],
        (Some(status), _) => vec![Line::from(Span::raw(format!(
            "{} changed file{}",
            status.changed.len(),
            if status.changed.len() == 1 { "" } else { "s" },
        )))],
        (None, Some(err)) => {
            let trimmed = err.lines().next().unwrap_or("").to_string();
            vec![Line::from(Span::styled(
                trimmed,
                Style::default().fg(Color::Red),
            ))]
        }
        (None, None) => vec![Line::from(Span::styled(
            "Loading git status…",
            Style::default().fg(Color::DarkGray),
        ))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::git::status::{FileChange, GitStatus};

    fn status(branch: &str, ahead: u32, behind: u32, changed: Vec<FileChange>) -> GitStatus {
        let clean = changed.is_empty();
        GitStatus {
            branch: Some(branch.to_string()),
            upstream: None,
            ahead,
            behind,
            changed,
            clean,
        }
    }

    fn panel_with_status(s: GitStatus) -> GitPanel {
        GitPanel {
            status: Some(s),
            ..GitPanel::default()
        }
    }

    fn panel_with_error(err: &str) -> GitPanel {
        GitPanel {
            status_error: Some(err.to_string()),
            ..GitPanel::default()
        }
    }

    fn lines_to_string(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn header_uses_branch_when_synced() {
        let panel = panel_with_status(status("main", 0, 0, vec![]));
        assert_eq!(header_label(&panel), "main");
    }

    #[test]
    fn header_includes_ahead_behind_arrows_when_diverged() {
        let panel = panel_with_status(status("feature", 2, 1, vec![]));
        assert_eq!(header_label(&panel), "feature ↑2 ↓1");
    }

    #[test]
    fn header_falls_back_to_detached_for_no_branch() {
        let mut s = status("ignored", 0, 0, vec![]);
        s.branch = None;
        let panel = panel_with_status(s);
        assert_eq!(header_label(&panel), "detached");
    }

    #[test]
    fn header_says_no_repo_on_status_error() {
        let panel = panel_with_error("fatal: not a git repository");
        assert_eq!(header_label(&panel), "no repo");
    }

    #[test]
    fn body_reports_clean_tree() {
        let panel = panel_with_status(status("main", 0, 0, vec![]));
        let lines = body_lines(&panel);
        assert_eq!(lines.len(), 1);
        assert!(lines_to_string(&lines).contains("clean"));
    }

    #[test]
    fn body_reports_changed_count_plural_and_singular() {
        let one = FileChange {
            path: "a.md".into(),
            status: "??".into(),
            staged: false,
            untracked: true,
        };
        let two = FileChange {
            path: "b.md".into(),
            status: "??".into(),
            staged: false,
            untracked: true,
        };
        let panel = panel_with_status(status("main", 0, 0, vec![one.clone()]));
        assert_eq!(lines_to_string(&body_lines(&panel)), "1 changed file");

        let panel = panel_with_status(status("main", 0, 0, vec![one, two]));
        assert_eq!(lines_to_string(&body_lines(&panel)), "2 changed files");
    }

    #[test]
    fn body_surfaces_first_line_of_status_error() {
        let panel = panel_with_error("fatal: not a git repository\nextra noise");
        assert_eq!(
            lines_to_string(&body_lines(&panel)),
            "fatal: not a git repository"
        );
    }

    #[test]
    fn body_shows_loading_placeholder_for_fresh_panel() {
        let panel = GitPanel::default();
        assert!(lines_to_string(&body_lines(&panel)).starts_with("Loading"));
    }

    #[test]
    fn width_is_panel_constant() {
        assert_eq!(width(), PANEL_WIDTH);
    }
}
