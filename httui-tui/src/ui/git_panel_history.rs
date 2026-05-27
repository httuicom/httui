//! History section of the git panel. Renders the `HISTORY [Ctrl+L]
//! view all` header (space-between) and each recent commit as
//! `<sha> <initials>  <subject>  <Xt ago>`. Owns the time-ago /
//! author-initials formatters.

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
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::BOLD),
        )],
        vec![
            Span::styled(
                "[Ctrl+L] ",
                Style::default().fg(crate::ui::palette::muted()),
            ),
            Span::styled("view all", Style::default().fg(Color::Gray)),
        ],
        width,
    )
}

fn commit_line(c: &CommitInfo, width: u16, now: i64) -> Line<'static> {
    let initials = author_initials(&c.author_name);
    let ago = time_ago(now, c.timestamp);
    let sha = format!("{} ", c.short_sha);
    let initials_col = format!("{initials:<2} ");
    // Reserve: sha + initials + ago + 1-cell gap before ago.
    let fixed = sha.chars().count() + initials_col.chars().count() + ago.chars().count() + 1;
    let max_subject = (width as usize).saturating_sub(fixed);
    let subject = truncate_with_ellipsis(&c.subject, max_subject);
    let left = vec![
        Span::styled(sha, Style::default().fg(crate::ui::palette::muted())),
        Span::styled(
            initials_col,
            Style::default().fg(crate::ui::palette::muted()),
        ),
        Span::styled(
            subject,
            Style::default().fg(crate::ui::palette::secondary()),
        ),
    ];
    let right = vec![Span::styled(
        ago,
        Style::default().fg(crate::ui::palette::muted()),
    )];
    two_col_line(left, right, width)
}

/// Cap `s` at `max` characters, ending with `…` when truncated.
/// `max == 0` returns an empty string. UTF-8 safe (operates over
/// `chars()` so accented letters stay intact).
fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max == 1 {
        return "…".to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
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
    fn truncate_keeps_short_strings_intact() {
        assert_eq!(truncate_with_ellipsis("hi", 10), "hi");
        assert_eq!(truncate_with_ellipsis("hi", 2), "hi");
    }

    #[test]
    fn truncate_adds_ellipsis_when_over_limit() {
        assert_eq!(truncate_with_ellipsis("abcdef", 4), "abc…");
        assert_eq!(truncate_with_ellipsis("", 4), "");
        assert_eq!(truncate_with_ellipsis("abc", 0), "");
        assert_eq!(truncate_with_ellipsis("abc", 1), "…");
    }

    #[test]
    fn commit_line_truncates_long_subject_keeping_ago_visible() {
        let c = CommitInfo {
            sha: "abc".into(),
            short_sha: "abc1234".into(),
            author_name: "J".into(),
            author_email: "j@e".into(),
            timestamp: 0,
            subject: "this is a very long commit subject that overflows".into(),
        };
        let line = commit_line(&c, 30, 60);
        let raw = span_text(&line);
        assert!(raw.ends_with("ago"), "ago is visible: {raw:?}");
        assert!(raw.contains("…"), "subject truncated: {raw:?}");
        assert_eq!(raw.chars().count(), 30, "fits panel width exactly: {raw:?}");
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
