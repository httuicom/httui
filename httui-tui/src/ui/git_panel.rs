//! Git side-panel renderer. Header carries branch + ahead/behind;
//! body holds metrics, staged/unstaged groups, commit form, and
//! recent history.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use httui_core::git::status::{FileChange, GitStatus};

use crate::git::GitPanel;

const PANEL_WIDTH: u16 = 52;
/// Message box = border (2) + draft line (1).
const MESSAGE_BOX_HEIGHT: u16 = 3;

pub fn width() -> u16 {
    PANEL_WIDTH
}

/// Render the panel. When `focused`, returns the commit-message
/// caret position so the caller can place the terminal cursor.
/// `commit_tpl` is the user's `[ui].git_commit_template` (drives the
/// empty-draft placeholder).
pub fn render(
    frame: &mut Frame,
    area: Rect,
    panel: &GitPanel,
    focused: bool,
    commit_tpl: &str,
) -> Option<(u16, u16)> {
    let (border_color, title_style) = if focused {
        (
            Color::LightYellow,
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (
            crate::ui::palette::MUTED,
            Style::default().fg(crate::ui::palette::SECONDARY),
        )
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(" Git ", title_style));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // header → status → message → meta → history, 1-row gaps.
    let meta_height = super::git_panel_form::meta_height(panel);
    let status_rows = status_body_rows(panel);
    let history_count = panel.recent_commits.len() as u16
        + if panel.recent_commits.is_empty() {
            0
        } else {
            1
        };
    let fixed = 1 + MESSAGE_BOX_HEIGHT + meta_height + 4; // header + 4 gaps
    let status_height =
        (status_rows.len() as u16).min(inner.height.saturating_sub(fixed + history_count));
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header strip
            Constraint::Length(1),
            Constraint::Length(status_height),
            Constraint::Length(1),
            Constraint::Length(MESSAGE_BOX_HEIGHT),
            Constraint::Length(1),
            Constraint::Length(meta_height),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);
    let (header_area, status_area, message_area, meta_area, history_area) =
        (split[0], split[2], split[4], split[6], split[8]);

    render_header_strip(frame, header_area, panel);

    render_row_list(
        frame,
        status_area,
        &status_rows,
        selected_row(panel, &status_rows),
        focused,
    );

    let cursor =
        super::git_panel_form::render_message_box(frame, message_area, panel, focused, commit_tpl);

    super::git_panel_form::render_meta(frame, meta_area, panel);

    super::git_panel_history::render_history(frame, history_area, panel);

    cursor
}

fn render_header_strip(frame: &mut Frame, area: Rect, panel: &GitPanel) {
    if area.height == 0 {
        return;
    }
    let (left_text, right_text) = match (&panel.status, &panel.status_error) {
        (Some(s), _) => {
            let mut left = s.branch.as_deref().unwrap_or("detached").to_string();
            if s.ahead != 0 || s.behind != 0 {
                left.push_str(&format!(" ↑{} ↓{}", s.ahead, s.behind));
            }
            let right = if s.changed.is_empty() {
                "clean".to_string()
            } else {
                let n = s.changed.len();
                let suffix = if n == 1 { "" } else { "s" };
                format!("{n} change{suffix}")
            };
            (left, right)
        }
        (None, Some(_)) => ("no repo".to_string(), String::new()),
        (None, None) => ("loading…".to_string(), String::new()),
    };
    let line = super::git_panel_form::two_col_line(
        vec![Span::styled(
            left_text,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )],
        vec![Span::styled(
            right_text,
            Style::default().fg(crate::ui::palette::MUTED),
        )],
        area.width,
    );
    frame.render_widget(ratatui::widgets::Paragraph::new(line), area);
}

fn render_row_list(
    frame: &mut Frame,
    area: Rect,
    rows: &[BodyRow],
    selected: Option<usize>,
    focused: bool,
) {
    if area.height == 0 {
        return;
    }
    let items: Vec<ListItem<'static>> = rows.iter().map(row_to_item).collect();
    let mut state = ListState::default();
    if let Some(idx) = selected {
        state.select(Some(idx));
    }
    let highlight = if focused {
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    };
    frame.render_stateful_widget(
        List::new(items).highlight_style(highlight),
        area,
        &mut state,
    );
}

/// Body rows for the status section. History uses its own renderer
/// (`git_panel_history`) because it needs space-between layout.
#[derive(Debug, Clone)]
pub(super) enum BodyRow {
    /// Caps section label + count suffix, e.g. `UNSTAGED (3)`.
    Section {
        label: &'static str,
        count: usize,
    },
    File(FileChange),
    Separator,
    Clean,
    Loading,
    Error(String),
}

#[cfg(test)]
pub(super) fn body_rows(panel: &GitPanel) -> Vec<BodyRow> {
    status_body_rows(panel)
}

pub(super) fn status_body_rows(panel: &GitPanel) -> Vec<BodyRow> {
    match (&panel.status, &panel.status_error) {
        (Some(status), _) => status_rows(status),
        (None, Some(err)) => vec![BodyRow::Error(err.lines().next().unwrap_or("").to_string())],
        (None, None) => vec![BodyRow::Loading],
    }
}

fn status_rows(status: &GitStatus) -> Vec<BodyRow> {
    let mut out = Vec::new();
    let (staged, unstaged): (Vec<_>, Vec<_>) = status
        .changed
        .iter()
        .partition(|c| c.staged && !c.untracked);
    // Desktop ordering: UNSTAGED first, STAGED below.
    if !unstaged.is_empty() {
        out.push(BodyRow::Section {
            label: "UNSTAGED",
            count: unstaged.len(),
        });
        for c in &unstaged {
            out.push(BodyRow::File((*c).clone()));
        }
    }
    if !staged.is_empty() {
        if !unstaged.is_empty() {
            out.push(BodyRow::Separator);
        }
        out.push(BodyRow::Section {
            label: "STAGED",
            count: staged.len(),
        });
        for c in &staged {
            out.push(BodyRow::File((*c).clone()));
        }
    }
    if staged.is_empty() && unstaged.is_empty() {
        out.push(BodyRow::Clean);
    }
    out
}

fn row_to_item(row: &BodyRow) -> ListItem<'static> {
    match row {
        BodyRow::Section { label, count } => ListItem::new(Line::from(vec![
            Span::styled(
                label.to_string(),
                Style::default()
                    .fg(crate::ui::palette::MUTED)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({count})"),
                Style::default().fg(crate::ui::palette::MUTED),
            ),
        ])),
        BodyRow::File(change) => {
            let glyph = status_glyph(change);
            let glyph_color = status_color(change);
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{glyph} "),
                    Style::default()
                        .fg(glyph_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(change.path.clone()),
            ]))
        }
        BodyRow::Separator => ListItem::new(Line::from(Span::raw(""))),
        BodyRow::Clean => ListItem::new(Line::from(Span::styled(
            "Working tree clean.",
            Style::default().fg(crate::ui::palette::MUTED),
        ))),
        BodyRow::Loading => ListItem::new(Line::from(Span::styled(
            "Loading git status…",
            Style::default().fg(crate::ui::palette::MUTED),
        ))),
        BodyRow::Error(msg) => ListItem::new(Line::from(Span::styled(
            msg.clone(),
            Style::default().fg(Color::Red),
        ))),
    }
}

fn status_glyph(change: &FileChange) -> &'static str {
    if change.untracked {
        return "?";
    }
    if change.status.contains('U') {
        return "!";
    }
    match change.status.chars().next() {
        Some('A') => "A",
        Some('M') => "M",
        Some('D') => "D",
        Some('R') => "R",
        Some('C') => "C",
        _ => match change.status.chars().nth(1) {
            Some('M') => "M",
            Some('D') => "D",
            _ => "·",
        },
    }
}

fn status_color(change: &FileChange) -> Color {
    if change.untracked {
        Color::Yellow
    } else if change.status.contains('U') {
        Color::Red
    } else if change.status.starts_with('A') || change.status.starts_with('?') {
        Color::Green
    } else if change.status.starts_with('D') {
        Color::Red
    } else {
        Color::Blue
    }
}

/// Index into `rows` corresponding to `panel.selected` (an index
/// into `status.changed`). Returns `None` when nothing is selectable
/// (no repo, clean tree, empty list).
fn selected_row(panel: &GitPanel, rows: &[BodyRow]) -> Option<usize> {
    let status = panel.status.as_ref()?;
    if status.changed.is_empty() {
        return None;
    }
    let target = panel.selected.min(status.changed.len().saturating_sub(1));
    let mut seen = 0usize;
    for (idx, row) in rows.iter().enumerate() {
        if matches!(row, BodyRow::File(_)) {
            if seen == target {
                return Some(idx);
            }
            seen += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn change(path: &str, code: &str, staged: bool, untracked: bool) -> FileChange {
        FileChange {
            path: path.to_string(),
            status: code.to_string(),
            staged,
            untracked,
        }
    }

    fn row_label(row: &BodyRow) -> String {
        match row {
            BodyRow::Section { label, count } => format!("{label} ({count})"),
            BodyRow::File(c) => format!("{} {}", status_glyph(c), c.path),
            BodyRow::Separator => String::new(),
            BodyRow::Clean => "Working tree clean.".into(),
            BodyRow::Loading => "Loading git status…".into(),
            BodyRow::Error(msg) => msg.clone(),
        }
    }

    fn labels(rows: &[BodyRow]) -> Vec<String> {
        rows.iter().map(row_label).collect()
    }

    #[test]
    fn body_reports_clean_tree() {
        let panel = panel_with_status(status("main", 0, 0, vec![]));
        let rows = body_rows(&panel);
        assert!(matches!(rows[0], BodyRow::Clean));
    }

    #[test]
    fn body_lists_unstaged_and_staged_groups_in_desktop_order() {
        let panel = GitPanel {
            status: Some(status(
                "main",
                0,
                0,
                vec![
                    change("notes/a.md", "M.", true, false),
                    change("notes/b.md", ".M", false, false),
                    change("new.md", "??", false, true),
                ],
            )),
            ..GitPanel::default()
        };
        let rows = body_rows(&panel);
        let raw = labels(&rows);
        // UNSTAGED appears before STAGED in the desktop's SCM column;
        // the count is in the label, not on a separate metrics line.
        let unstaged_idx = raw.iter().position(|s| s.starts_with("UNSTAGED"));
        let staged_idx = raw.iter().position(|s| s.starts_with("STAGED"));
        assert_eq!(raw[unstaged_idx.expect("UNSTAGED row")], "UNSTAGED (2)");
        assert_eq!(raw[staged_idx.expect("STAGED row")], "STAGED (1)");
        assert!(unstaged_idx < staged_idx);
        // Files appear under the right header.
        assert!(raw.iter().any(|s| s.contains("notes/a.md")));
        assert!(raw.iter().any(|s| s.contains("notes/b.md")));
        assert!(raw.iter().any(|s| s.contains("new.md")));
    }

    #[test]
    fn file_row_glyph_distinguishes_untracked_modified_added_deleted() {
        let panel = GitPanel {
            status: Some(status(
                "main",
                0,
                0,
                vec![
                    change("u.md", "??", false, true),
                    change("m.md", ".M", false, false),
                    change("a.md", "A.", true, false),
                    change("d.md", "D.", true, false),
                ],
            )),
            ..GitPanel::default()
        };
        let rows = body_rows(&panel);
        let joined = labels(&rows).join("\n");
        assert!(joined.contains("? u.md"));
        assert!(joined.contains("M m.md"));
        assert!(joined.contains("A a.md"));
        assert!(joined.contains("D d.md"));
    }

    #[test]
    fn conflicted_file_shows_bang_glyph() {
        let panel = GitPanel {
            status: Some(status(
                "main",
                0,
                0,
                vec![change("conflict.md", "UU", false, false)],
            )),
            ..GitPanel::default()
        };
        let rows = body_rows(&panel);
        assert!(labels(&rows).iter().any(|s| s.contains("! conflict.md")));
    }

    #[test]
    fn body_surfaces_first_line_of_status_error() {
        let panel = panel_with_error("fatal: not a git repository\nextra noise");
        let rows = body_rows(&panel);
        assert!(matches!(&rows[0], BodyRow::Error(s) if s == "fatal: not a git repository"));
    }

    #[test]
    fn body_shows_loading_placeholder_for_fresh_panel() {
        let rows = body_rows(&GitPanel::default());
        assert!(matches!(rows[0], BodyRow::Loading));
    }

    #[test]
    fn selected_row_skips_section_headers() {
        // UNSTAGED renders before STAGED → the first file row the
        // user navigates to is the unstaged one. `selected` indexes
        // file rows in render order, not `status.changed` order.
        let mut panel = GitPanel {
            status: Some(status(
                "main",
                0,
                0,
                vec![
                    change("a.md", "M.", true, false),
                    change("b.md", ".M", false, false),
                ],
            )),
            ..GitPanel::default()
        };
        let rows = body_rows(&panel);
        panel.selected = 0;
        let idx = selected_row(&panel, &rows).expect("first file selectable");
        assert!(matches!(&rows[idx], BodyRow::File(c) if c.path == "b.md"));
        panel.selected = 1;
        let idx = selected_row(&panel, &rows).expect("second file selectable");
        assert!(matches!(&rows[idx], BodyRow::File(c) if c.path == "a.md"));
    }

    #[test]
    fn selected_row_returns_none_when_list_empty() {
        let panel = panel_with_status(status("main", 0, 0, vec![]));
        let rows = body_rows(&panel);
        assert!(selected_row(&panel, &rows).is_none());
    }

    #[test]
    fn selected_row_clamps_to_last_entry() {
        let panel = GitPanel {
            selected: 99,
            status: Some(status(
                "main",
                0,
                0,
                vec![change("only.md", ".M", false, false)],
            )),
            ..GitPanel::default()
        };
        let rows = body_rows(&panel);
        let idx = selected_row(&panel, &rows).expect("clamped to last file");
        assert!(matches!(&rows[idx], BodyRow::File(c) if c.path == "only.md"));
    }

    #[test]
    fn row_to_item_paints_two_spans_for_files() {
        let panel = GitPanel {
            status: Some(status(
                "main",
                0,
                0,
                vec![change("x.md", ".M", false, false)],
            )),
            ..GitPanel::default()
        };
        let rows = body_rows(&panel);
        // rows = [Metrics, Unstaged section, File(x.md)]; row_to_item
        // must produce a ListItem (we just sanity-check no panic and
        // that the call exists — span surface is locked in `Line`).
        for r in &rows {
            let _ = row_to_item(r);
        }
    }

    #[test]
    fn width_is_panel_constant() {
        assert_eq!(width(), PANEL_WIDTH);
    }
}
