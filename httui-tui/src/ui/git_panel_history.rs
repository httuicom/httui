//! History section of the git panel — pinta o cabeçalho
//! `HISTORY [Ctrl+L] view all` (space-between) e cada commit
//! recente como `<sha> <initials>  <subject>  <Xt ago>`. Split out
//! of `ui::git_panel` to keep that file under the size gate and to
//! own the time-ago / author-initials formatters.

use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use httui_core::git::log::CommitInfo;

use crate::git::GitPanel;

use super::git_panel_form::two_col_line;

pub(super) fn render_history(frame: &mut Frame, area: Rect, panel: &GitPanel) {
    if area.height == 0 || panel.recent_commits.is_empty() {
        return;
    }
    let now = unix_now();
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(panel.recent_commits.len() + 1);
    lines.push(history_header_line(area.width));
    let usable = (area.height as usize).saturating_sub(1);
    for commit in panel.recent_commits.iter().take(usable) {
        lines.push(commit_line(commit, area.width, now));
    }
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn history_header_line(width: u16) -> Line<'static> {
    two_col_line(
        vec![Span::styled(
            "HISTORY",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )],
        vec![
            Span::styled(
                "[Ctrl+L] ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "view all",
                Style::default().fg(Color::Gray),
            ),
        ],
        width,
    )
}

fn commit_line(c: &CommitInfo, width: u16, now: i64) -> Line<'static> {
    let initials = author_initials(&c.author_name);
    let ago = time_ago(now, c.timestamp);
    let sha = format!("{} ", c.short_sha);
    let initials_col = format!("{initials:<2} ");
    let left = vec![
        Span::styled(sha, Style::default().fg(Color::DarkGray)),
        Span::styled(initials_col, Style::default().fg(Color::DarkGray)),
        Span::raw(c.subject.clone()),
    ];
    let right = vec![Span::styled(ago, Style::default().fg(Color::DarkGray))];
    two_col_line(left, right, width)
}

/// First letter of each whitespace-separated word in `name`, up to
/// two characters, upper-cased. Empty string falls back to `?`.
fn author_initials(name: &str) -> String {
    let initials: String = name
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    if initials.is_empty() {
        "?".to_string()
    } else {
        initials
    }
}

/// Relative time label. `< 60s` → `Xs`, `< 60m` → `Xm`, `< 24h` →
/// `Xh`, `< 30d` → `Xd`, otherwise `Xw` (weeks). Suffix `ago` is
/// appended by the caller-friendly format.
fn time_ago(now: i64, then: i64) -> String {
    let diff = now.saturating_sub(then).max(0);
    if diff < 60 {
        format!("{diff}s ago")
    } else if diff < 3_600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86_400 {
        format!("{}h ago", diff / 3_600)
    } else if diff < 30 * 86_400 {
        format!("{}d ago", diff / 86_400)
    } else {
        format!("{}w ago", diff / (7 * 86_400))
    }
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
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
    fn author_initials_two_word_name() {
        assert_eq!(author_initials("Joao Silva"), "JS");
    }

    #[test]
    fn author_initials_single_word_name() {
        assert_eq!(author_initials("Test"), "T");
    }

    #[test]
    fn author_initials_empty_falls_back() {
        assert_eq!(author_initials(""), "?");
        assert_eq!(author_initials("   "), "?");
    }

    #[test]
    fn author_initials_caps_extra_words_at_two() {
        assert_eq!(author_initials("a b c d"), "AB");
    }

    #[test]
    fn time_ago_seconds() {
        assert_eq!(time_ago(100, 90), "10s ago");
        assert_eq!(time_ago(100, 100), "0s ago");
    }

    #[test]
    fn time_ago_minutes_hours_days_weeks() {
        assert_eq!(time_ago(120 + 100, 100), "2m ago");
        assert_eq!(time_ago(2 * 3_600 + 100, 100), "2h ago");
        assert_eq!(time_ago(3 * 86_400 + 100, 100), "3d ago");
        assert_eq!(time_ago(60 * 86_400 + 100, 100), "8w ago");
    }

    #[test]
    fn time_ago_clamps_negative_to_zero() {
        // Clock skew shouldn't crash — `then` in the future → 0s.
        assert_eq!(time_ago(0, 100), "0s ago");
    }

    #[test]
    fn history_header_renders_caps_left_and_view_all_right() {
        let line = history_header_line(40);
        let raw = span_text(&line);
        assert!(raw.starts_with("HISTORY"));
        assert!(raw.ends_with("view all"));
    }

    #[test]
    fn commit_line_lays_out_columns() {
        let c = CommitInfo {
            sha: "abc".into(),
            short_sha: "abc1234".into(),
            author_name: "Joao Silva".into(),
            author_email: "j@s".into(),
            timestamp: 0,
            subject: "wip".into(),
        };
        let line = commit_line(&c, 40, 60);
        let raw = span_text(&line);
        assert!(raw.starts_with("abc1234 JS "));
        assert!(raw.contains("wip"));
        assert!(raw.ends_with("ago"));
    }
}
