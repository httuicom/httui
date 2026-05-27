//! directory navigator. Lists the current directory's
//! children with `..` always on top; vault roots are marked so the
//! user knows which entries activate vs which just descend.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::{VaultOpenEntryKind, VaultOpenPickerState};

const MAX_VISIBLE_ROWS: usize = 16;

fn collapse_home(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            if let Some(rest) = path.strip_prefix(&home) {
                return format!("~{rest}");
            }
        }
    }
    path.to_string()
}

pub fn render(frame: &mut Frame, editor_area: Rect, state: &VaultOpenPickerState) {
    let popup = compute_popup_rect(editor_area, state);
    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    {
        let buf = frame.buffer_mut();
        for y in popup.y..popup.y.saturating_add(popup.height) {
            for x in popup.x..popup.x.saturating_add(popup.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let cwd_display = collapse_home(&state.cwd.display().to_string());
    let title = format!(" Open vault · {cwd_display} ");
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::border())
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let body_area = chunks[0];
    let footer_area = chunks[1];

    let items: Vec<ListItem> = state
        .entries
        .iter()
        .map(|e| {
            let (marker, marker_style, suffix) = match e.kind {
                VaultOpenEntryKind::Parent => (
                    "↑ ",
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(crate::ui::palette::muted()),
                    "",
                ),
                VaultOpenEntryKind::Directory => (
                    "/ ",
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(Color::Gray),
                    "/",
                ),
                VaultOpenEntryKind::Vault => (
                    "● ",
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(Color::LightMagenta),
                    "  [vault]",
                ),
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, marker_style),
                Span::styled(
                    e.name.clone(),
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(crate::ui::palette::foreground()),
                ),
                Span::styled(
                    suffix.to_string(),
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(crate::ui::palette::muted()),
                ),
            ]))
        })
        .collect();
    let list = List::new(items).style(bg_style).highlight_style(
        Style::default()
            .bg(super::palette::selection_bg())
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::BOLD),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(
        state.selected.min(state.entries.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, body_area, &mut list_state);

    let chip_key = Style::default()
        .bg(Color::LightMagenta)
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" jk ", chip_key),
        Span::styled(" navigate   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" descend   ", chip_label),
        Span::styled(" o ", chip_key),
        Span::styled(" open as vault   ", chip_label),
        Span::styled(" Bksp ", chip_key),
        Span::styled(" up   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

fn compute_popup_rect(area: Rect, state: &VaultOpenPickerState) -> Rect {
    const PADDING: u16 = 10;
    let longest = state
        .entries
        .iter()
        .map(|e| e.name.chars().count())
        .max()
        .unwrap_or(30) as u16;
    let width = (longest + PADDING).clamp(50, area.width.saturating_sub(2));
    let visible = state.entries.len().min(MAX_VISIBLE_ROWS) as u16;
    let height = visible.max(1) + 3;

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + 3u16.min(area.height.saturating_sub(height));
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{VaultOpenEntry, VaultOpenEntryKind};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn dump(terminal: &Terminal<TestBackend>) -> String {
        let buf = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if let Some(cell) = buf.cell((x, y)) {
                    out.push_str(cell.symbol());
                }
            }
            out.push('\n');
        }
        out
    }

    fn state(entries: &[(&str, VaultOpenEntryKind)]) -> VaultOpenPickerState {
        VaultOpenPickerState {
            cwd: std::path::PathBuf::from("/tmp"),
            entries: entries
                .iter()
                .map(|(n, k)| VaultOpenEntry {
                    name: (*n).to_string(),
                    kind: *k,
                })
                .collect(),
            selected: 0,
        }
    }

    #[test]
    fn collapse_home_strips_home_prefix() {
        std::env::set_var("HOME", "/Users/test");
        assert_eq!(collapse_home("/Users/test/projects"), "~/projects");
    }

    #[test]
    fn compute_popup_rect_fits_within_area() {
        let s = state(&[
            ("..", VaultOpenEntryKind::Parent),
            ("project-a", VaultOpenEntryKind::Vault),
        ]);
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        let popup = compute_popup_rect(area, &s);
        assert!(popup.width <= area.width.saturating_sub(2));
        assert!(popup.width >= 50, "minimum clamp must hold");
    }

    #[test]
    fn render_paints_title_entries_and_footer() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state(&[
            ("..", VaultOpenEntryKind::Parent),
            ("docs", VaultOpenEntryKind::Directory),
            ("notes-vault", VaultOpenEntryKind::Vault),
        ]);
        terminal
            .draw(|f| {
                render(f, f.area(), &s);
            })
            .unwrap();
        let painted = dump(&terminal);
        assert!(painted.contains("Open vault"));
        assert!(painted.contains("docs"));
        assert!(painted.contains("notes-vault"));
        assert!(painted.contains("vault"), "marker suffix present");
        assert!(painted.contains("navigate"));
    }

    #[test]
    fn render_handles_empty_entries_list() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        let s = state(&[]);
        terminal
            .draw(|f| {
                render(f, f.area(), &s);
            })
            .unwrap();
        let painted = dump(&terminal);
        assert!(painted.contains("Open vault"));
    }
}
