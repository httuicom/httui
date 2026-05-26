//! Commit-form sub-renderer for the git side panel. Split out of
//! `ui::git_panel` to keep that file under the size gate.
//!
//! The form is composed of two sibling rects:
//!
//! - [`render_message_box`]: bordered "message" container with ONLY
//!   the draft / placeholder line. Returns the cursor position when
//!   focused.
//! - [`render_meta`]: amend toggle + key hints + optional error
//!   message painted directly on the panel below the box (no
//!   border, so they read as panel-level affordances, not part of
//!   the message itself).

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::git::{template::commit_template, GitPanel};

/// Height the meta block needs: (optional error) + amend toggle +
/// files-staged/commit hint + sync button. Callers reserve this
/// much above the history region.
pub(super) fn meta_height(panel: &GitPanel) -> u16 {
    let base = 3; // amend + commit hint + sync button
    if panel.commit_error.is_some() {
        base + 1
    } else {
        base
    }
}

/// Build a `Line` with the right-hand spans pushed to `width`
/// (filling the gap with spaces). Mirrors the desktop's
/// `justify-content: space-between` rows in the SCM column.
pub(super) fn two_col_line(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    width: u16,
) -> Line<'static> {
    let left_w: usize = left
        .iter()
        .map(|s| s.content.chars().count())
        .sum();
    let right_w: usize = right
        .iter()
        .map(|s| s.content.chars().count())
        .sum();
    let total = width as usize;
    let pad = total.saturating_sub(left_w + right_w);
    let mut spans = left;
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    spans.extend(right);
    Line::from(spans)
}

pub(super) fn render_message_box(
    frame: &mut Frame,
    area: Rect,
    panel: &GitPanel,
    focused: bool,
    commit_tpl: &str,
) -> Option<(u16, u16)> {
    if area.height < 3 {
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
    let paragraph = Paragraph::new(Line::from(Span::styled(text, style)));
    frame.render_widget(paragraph, inner);

    if focused && inner.width > 0 && inner.height > 0 {
        let cursor_col = panel.commit_message.cursor_col() as u16;
        let x = inner.x + cursor_col.min(inner.width.saturating_sub(1));
        Some((x, inner.y))
    } else {
        None
    }
}

pub(super) fn render_meta(frame: &mut Frame, area: Rect, panel: &GitPanel) {
    if area.height == 0 {
        return;
    }
    let width = area.width;
    let mut lines: Vec<Line<'static>> = Vec::new();
    if let Some(err) = panel.commit_error.as_ref() {
        lines.push(Line::from(Span::styled(
            err.lines().next().unwrap_or("").to_string(),
            Style::default().fg(Color::Red),
        )));
    }
    lines.push(amend_toggle_line(panel.amend, width));
    lines.push(commit_hint_line(panel, width));
    lines.push(sync_button_line(width));
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}

fn amend_toggle_line(amend: bool, width: u16) -> Line<'static> {
    let (marker, style) = if amend {
        (
            "[x] amend last",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        ("[ ] amend last", Style::default().fg(Color::Gray))
    };
    two_col_line(
        vec![Span::styled(marker.to_string(), style)],
        vec![Span::styled("[Ctrl+A]", Style::default().fg(Color::DarkGray))],
        width,
    )
}

fn commit_hint_line(panel: &GitPanel, width: u16) -> Line<'static> {
    let n = panel
        .status
        .as_ref()
        .map(|s| s.changed.len())
        .unwrap_or(0);
    let suffix = if n == 1 { "" } else { "s" };
    two_col_line(
        vec![Span::styled(
            format!("{n} file{suffix} staged"),
            Style::default().fg(Color::DarkGray),
        )],
        vec![
            Span::styled(
                "[Enter] ",
                Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD),
            ),
            Span::styled("commit", Style::default().fg(Color::Gray)),
        ],
        width,
    )
}

fn sync_button_line(width: u16) -> Line<'static> {
    // Sync renders as a visual button: inverted bg + spaced label,
    // mirroring the desktop's Sync CTA. Centered when there's room;
    // left-aligned otherwise.
    let label = "  ⏵ Sync  ";
    let chord = "[Ctrl+⏎]";
    let label_w = label.chars().count();
    let chord_w = chord.chars().count();
    let pad = (width as usize).saturating_sub(label_w + chord_w);
    let mut spans = vec![Span::styled(
        label.to_string(),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Gray)
            .add_modifier(Modifier::BOLD),
    )];
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    spans.push(Span::styled(
        chord.to_string(),
        Style::default().fg(Color::DarkGray),
    ));
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vim::lineedit::LineEdit;

    fn span_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn amend_toggle_renders_checked_marker_when_active() {
        let line = amend_toggle_line(true, 40);
        let raw = span_text(&line);
        assert!(raw.starts_with("[x] amend last"));
        assert!(raw.ends_with("[Ctrl+A]"));
    }

    #[test]
    fn amend_toggle_renders_unchecked_marker_when_inactive() {
        let line = amend_toggle_line(false, 40);
        let raw = span_text(&line);
        assert!(raw.starts_with("[ ] amend last"));
        assert!(raw.ends_with("[Ctrl+A]"));
    }

    #[test]
    fn commit_hint_line_pluralises_and_right_aligns_enter() {
        let panel = GitPanel::default();
        let line = commit_hint_line(&panel, 40);
        let raw = span_text(&line);
        assert!(raw.starts_with("0 files staged"));
        assert!(raw.ends_with("commit"));
    }

    #[test]
    fn sync_button_line_uses_inverted_bg_for_label() {
        let line = sync_button_line(40);
        // First span is the inverted-bg label.
        assert_eq!(line.spans[0].style.bg, Some(Color::Gray));
        let raw = span_text(&line);
        assert!(raw.contains("⏵ Sync"));
        assert!(raw.ends_with("[Ctrl+⏎]"));
    }

    #[test]
    fn two_col_line_fills_gap_to_width() {
        let line = two_col_line(
            vec![Span::raw("a")],
            vec![Span::raw("b")],
            6,
        );
        let raw = span_text(&line);
        // a + 4 spaces + b
        assert_eq!(raw, "a    b");
    }

    #[test]
    fn two_col_line_no_gap_when_already_wider() {
        let line = two_col_line(
            vec![Span::raw("abcd")],
            vec![Span::raw("efgh")],
            4,
        );
        let raw = span_text(&line);
        assert_eq!(raw, "abcdefgh");
    }

    #[test]
    fn meta_height_grows_when_error_is_present() {
        // amend + commit-hint + sync = 3 base lines.
        assert_eq!(meta_height(&GitPanel::default()), 3);
        let panel = GitPanel {
            commit_error: Some("boom".into()),
            ..GitPanel::default()
        };
        assert_eq!(meta_height(&panel), 4);
    }

    #[test]
    fn message_box_short_circuits_when_too_short() {
        // Area height < 3 → no cursor returned, no panic.
        let panel = GitPanel {
            commit_message: LineEdit::from_str("hello"),
            ..GitPanel::default()
        };
        let backend = ratatui::backend::TestBackend::new(40, 2);
        let mut term = ratatui::Terminal::new(backend).unwrap();
        let _ = term
            .draw(|f| {
                let _ = render_message_box(f, f.area(), &panel, true, "");
            })
            .unwrap();
    }
}
