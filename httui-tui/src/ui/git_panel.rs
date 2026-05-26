//! Git side-panel renderer — bordered column to the right of the
//! editor. Header carries branch + ahead/behind; body splits into a
//! metrics line (+N -M), staged group, unstaged group, and (when no
//! repo) a friendly error message.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use httui_core::git::log::CommitInfo;
use httui_core::git::status::{FileChange, GitStatus};

use crate::git::GitPanel;

const PANEL_WIDTH: u16 = 42;
/// Message box = border (2) + draft line (1).
const MESSAGE_BOX_HEIGHT: u16 = 3;

pub fn width() -> u16 {
    PANEL_WIDTH
}

/// Render the panel and, when `focused`, place the terminal cursor
/// at the commit-message caret. `commit_tpl` comes from the user's
/// `[ui].git_commit_template` (shared with the desktop); the
/// placeholder text under an empty draft is computed from it.
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
        (Color::DarkGray, Style::default().fg(Color::Gray))
    };

    let title = format!(" Git — {} ", header_label(panel));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(title, title_style));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Desktop SCM order: status → message box → meta (amend / hints)
    // → history. The bordered "message" box wraps the draft line
    // only; amend + key hints sit on the panel below as separate
    // affordances.
    let meta_height = super::git_panel_form::meta_height(panel);
    let status_rows = status_body_rows(panel);
    let history_rows = history_body_rows(panel);
    let fixed = MESSAGE_BOX_HEIGHT + meta_height;
    let status_height = (status_rows.len() as u16).min(
        inner.height.saturating_sub(fixed + history_rows.len() as u16),
    );
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(status_height),
            Constraint::Length(MESSAGE_BOX_HEIGHT),
            Constraint::Length(meta_height),
            Constraint::Min(0),
        ])
        .split(inner);
    let status_area = split[0];
    let message_area = split[1];
    let meta_area = split[2];
    let history_area = split[3];

    render_row_list(frame, status_area, &status_rows, selected_row(panel, &status_rows), focused);

    let cursor = super::git_panel_form::render_message_box(
        frame, message_area, panel, focused, commit_tpl,
    );

    super::git_panel_form::render_meta(frame, meta_area, panel);

    render_row_list(frame, history_area, &history_rows, None, false);

    cursor
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
        Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    };
    frame.render_stateful_widget(List::new(items).highlight_style(highlight), area, &mut state);
}

fn header_label(panel: &GitPanel) -> String {
    match (&panel.status, &panel.status_error) {
        (Some(status), _) => {
            let mut s = status.branch.as_deref().unwrap_or("detached").to_string();
            if status.ahead != 0 || status.behind != 0 {
                s.push_str(&format!(" ↑{} ↓{}", status.ahead, status.behind));
            }
            let n = status.changed.len();
            if n > 0 {
                s.push_str(&format!(" · {n} change{}", if n == 1 { "" } else { "s" }));
            }
            s
        }
        (None, Some(_)) => "no repo".to_string(),
        (None, None) => "loading…".to_string(),
    }
}

/// Logical body rows. Decoupled from `ListItem` so [`selected_row`]
/// can walk the structure without re-parsing rendered text.
#[derive(Debug, Clone)]
pub(super) enum BodyRow {
    /// Caps section label with a count suffix: `UNSTAGED (3)`.
    Section { label: &'static str, count: usize },
    File(FileChange),
    Separator,
    /// `HISTORY` section header with the `View all` affordance on
    /// the right (the chord to open it is `Ctrl+L`).
    HistoryHeader,
    /// One row from `recent_commits` — `<short_sha> <subject>`.
    Commit(CommitInfo),
    Clean,
    Loading,
    Error(String),
}

/// Combined rows — kept for the existing test surface. The
/// production renderer uses [`status_body_rows`] and
/// [`history_body_rows`] separately so the commit form can sit
/// between them (desktop SCM ordering).
#[cfg(test)]
pub(super) fn body_rows(panel: &GitPanel) -> Vec<BodyRow> {
    let mut out = status_body_rows(panel);
    let history = history_body_rows(panel);
    if !out.is_empty() && !history.is_empty() {
        out.push(BodyRow::Separator);
    }
    out.extend(history);
    out
}

pub(super) fn status_body_rows(panel: &GitPanel) -> Vec<BodyRow> {
    match (&panel.status, &panel.status_error) {
        (Some(status), _) => status_rows(status),
        (None, Some(err)) => vec![BodyRow::Error(
            err.lines().next().unwrap_or("").to_string(),
        )],
        (None, None) => vec![BodyRow::Loading],
    }
}

pub(super) fn history_body_rows(panel: &GitPanel) -> Vec<BodyRow> {
    if panel.recent_commits.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(panel.recent_commits.len() + 1);
    out.push(BodyRow::HistoryHeader);
    for c in &panel.recent_commits {
        out.push(BodyRow::Commit(c.clone()));
    }
    out
}

fn status_rows(status: &GitStatus) -> Vec<BodyRow> {
    let mut out = Vec::new();
    let (staged, unstaged): (Vec<_>, Vec<_>) =
        status.changed.iter().partition(|c| c.staged && !c.untracked);
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
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" ({count})"),
                Style::default().fg(Color::DarkGray),
            ),
        ])),
        BodyRow::File(change) => {
            let glyph = status_glyph(change);
            let glyph_color = status_color(change);
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{glyph} "),
                    Style::default().fg(glyph_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(change.path.clone()),
            ]))
        }
        BodyRow::Separator => ListItem::new(Line::from(Span::raw(""))),
        BodyRow::HistoryHeader => ListItem::new(Line::from(vec![
            Span::styled(
                "HISTORY",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  [Ctrl+L] view all",
                Style::default().fg(Color::DarkGray),
            ),
        ])),
        BodyRow::Commit(c) => ListItem::new(Line::from(vec![
            Span::styled(
                format!("{} ", c.short_sha),
                Style::default().fg(Color::DarkGray),
            ),
            Span::raw(c.subject.clone()),
        ])),
        BodyRow::Clean => ListItem::new(Line::from(Span::styled(
            "Working tree clean.",
            Style::default().fg(Color::Green),
        ))),
        BodyRow::Loading => ListItem::new(Line::from(Span::styled(
            "Loading git status…",
            Style::default().fg(Color::DarkGray),
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
            BodyRow::HistoryHeader => "HISTORY".into(),
            BodyRow::Commit(c) => format!("{} {}", c.short_sha, c.subject),
            BodyRow::Clean => "Working tree clean.".into(),
            BodyRow::Loading => "Loading git status…".into(),
            BodyRow::Error(msg) => msg.clone(),
        }
    }

    fn labels(rows: &[BodyRow]) -> Vec<String> {
        rows.iter().map(row_label).collect()
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
    fn body_includes_history_section_after_changes() {
        let panel = GitPanel {
            status: Some(status("main", 0, 0, vec![])),
            recent_commits: vec![CommitInfo {
                sha: "abc".into(),
                short_sha: "abc1234".into(),
                author_name: "A".into(),
                author_email: "a@b".into(),
                timestamp: 0,
                subject: "seed".into(),
            }],
            ..GitPanel::default()
        };
        let rows = body_rows(&panel);
        let raw = labels(&rows);
        assert!(raw.iter().any(|s| s == "HISTORY"));
        assert!(raw.iter().any(|s| s == "abc1234 seed"));
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
            status: Some(status("main", 0, 0, vec![change("x.md", ".M", false, false)])),
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
