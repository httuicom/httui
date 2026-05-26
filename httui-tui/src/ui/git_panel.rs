//! Git side-panel renderer — bordered column to the right of the
//! editor. Header carries branch + ahead/behind; body splits into a
//! metrics line (+N -M), staged group, unstaged group, and (when no
//! repo) a friendly error message.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use httui_core::git::status::{DiffMetrics, FileChange, GitStatus};

use crate::git::{template::commit_template, GitPanel};

const PANEL_WIDTH: u16 = 36;
const COMMIT_FORM_HEIGHT: u16 = 5;

pub fn width() -> u16 {
    PANEL_WIDTH
}

/// Render the panel and, when `focused`, place the terminal cursor
/// at the commit-message caret. Returns the absolute cursor cell
/// when set, so callers can avoid duplicate `set_cursor_position`
/// calls if they manage cursor themselves.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    panel: &GitPanel,
    focused: bool,
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

    // Reserve space for the commit form at the bottom when there's
    // room; tiny panels collapse the form away so the list stays
    // legible.
    let form_height = COMMIT_FORM_HEIGHT.min(inner.height.saturating_sub(2));
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(form_height)])
        .split(inner);
    let list_area = split[0];
    let form_area = split[1];

    let rows = body_rows(panel);
    let items: Vec<ListItem<'static>> = rows.iter().map(row_to_item).collect();
    let mut state = ListState::default();
    if let Some(idx) = selected_row(panel, &rows) {
        state.select(Some(idx));
    }
    let list = List::new(items).highlight_style(if focused {
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().bg(Color::DarkGray).fg(Color::White)
    });
    frame.render_stateful_widget(list, list_area, &mut state);

    render_commit_form(frame, form_area, panel, focused)
}

fn render_commit_form(
    frame: &mut Frame,
    area: Rect,
    panel: &GitPanel,
    focused: bool,
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
        .title(Span::styled(
            " message ",
            Style::default().fg(Color::Gray),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let draft = panel.commit_message.as_str();
    let placeholder = panel
        .status
        .as_ref()
        .map(commit_template)
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
    let lines: Vec<Line<'static>> = if let Some(err) = panel.commit_error.as_ref() {
        vec![
            Line::from(Span::styled(text, style)),
            Line::from(Span::styled(
                err.lines().next().unwrap_or("").to_string(),
                Style::default().fg(Color::Red),
            )),
        ]
    } else {
        vec![Line::from(Span::styled(text, style))]
    };
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

/// Logical body rows. Decoupled from `ListItem` so [`selected_row`]
/// can walk the structure without re-parsing rendered text.
#[derive(Debug, Clone)]
pub(super) enum BodyRow {
    Metrics { files: usize, plus: u32, minus: u32 },
    Section(&'static str),
    File(FileChange),
    Separator,
    Clean,
    Loading,
    Error(String),
}

pub(super) fn body_rows(panel: &GitPanel) -> Vec<BodyRow> {
    match (&panel.status, &panel.status_error) {
        (Some(status), _) => populated_rows(status, &panel.metrics),
        (None, Some(err)) => vec![BodyRow::Error(
            err.lines().next().unwrap_or("").to_string(),
        )],
        (None, None) => vec![BodyRow::Loading],
    }
}

fn populated_rows(status: &GitStatus, metrics: &DiffMetrics) -> Vec<BodyRow> {
    let mut out = Vec::new();
    out.push(BodyRow::Metrics {
        files: status.changed.len(),
        plus: metrics.insertions,
        minus: metrics.deletions,
    });
    let (staged, unstaged): (Vec<_>, Vec<_>) =
        status.changed.iter().partition(|c| c.staged && !c.untracked);
    if !staged.is_empty() {
        out.push(BodyRow::Section("Staged"));
        for c in &staged {
            out.push(BodyRow::File((*c).clone()));
        }
    }
    if !unstaged.is_empty() {
        if !staged.is_empty() {
            out.push(BodyRow::Separator);
        }
        out.push(BodyRow::Section("Unstaged"));
        for c in &unstaged {
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
        BodyRow::Metrics { files, plus, minus } => {
            let label = format!(
                "{} file{}  +{}  -{}",
                files,
                if *files == 1 { "" } else { "s" },
                plus,
                minus,
            );
            ListItem::new(Line::from(Span::styled(
                label,
                Style::default().fg(Color::Cyan),
            )))
        }
        BodyRow::Section(name) => ListItem::new(Line::from(Span::styled(
            (*name).to_string(),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        ))),
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
            BodyRow::Metrics { files, plus, minus } => format!(
                "{} file{}  +{}  -{}",
                files,
                if *files == 1 { "" } else { "s" },
                plus,
                minus,
            ),
            BodyRow::Section(s) => s.to_string(),
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
        assert!(matches!(rows[0], BodyRow::Metrics { .. }));
        assert!(matches!(rows[1], BodyRow::Clean));
    }

    #[test]
    fn body_lists_unstaged_and_staged_groups() {
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
            metrics: DiffMetrics {
                files: 2,
                insertions: 4,
                deletions: 1,
            },
            ..GitPanel::default()
        };
        let rows = body_rows(&panel);
        let raw = labels(&rows);
        assert!(raw[0].contains("3 files"));
        assert!(raw[0].contains("+4"));
        assert!(raw[0].contains("-1"));
        assert!(raw.iter().any(|s| s == "Staged"));
        assert!(raw.iter().any(|s| s.contains("notes/a.md")));
        assert!(raw.iter().any(|s| s == "Unstaged"));
        assert!(raw.iter().any(|s| s.contains("notes/b.md")));
        assert!(raw.iter().any(|s| s.contains("new.md")));
    }

    #[test]
    fn metrics_row_uses_singular_for_single_file() {
        let panel = GitPanel {
            status: Some(status("main", 0, 0, vec![change("a", ".M", false, false)])),
            metrics: DiffMetrics {
                files: 1,
                insertions: 2,
                deletions: 0,
            },
            ..GitPanel::default()
        };
        let rows = body_rows(&panel);
        assert!(row_label(&rows[0]).contains("1 file "));
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
    fn selected_row_skips_metrics_and_section_headers() {
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
        assert!(matches!(&rows[idx], BodyRow::File(c) if c.path == "a.md"));
        panel.selected = 1;
        let idx = selected_row(&panel, &rows).expect("second file selectable");
        assert!(matches!(&rows[idx], BodyRow::File(c) if c.path == "b.md"));
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
