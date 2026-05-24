//! Compact popup that lists registered vaults so the user can swap
//! workspace without restarting the binary. Triggered by `Alt+W`
//! (configurable); navigated with `j`/`k` or arrows; `Enter`
//! activates, `Esc`/`Ctrl-C` close.
//!
//! Visual: a wider variant of `environment_picker` because vault
//! paths are longer than env names. Same border / footer chrome so
//! the picker family looks consistent.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::VaultPickerState;

const MAX_VISIBLE_ROWS: usize = 12;

/// `/Users/joao/foo` → `~/foo` when `$HOME` matches. Leaves anything
/// else untouched. Cosmetic only — the picker still operates on the
/// canonical absolute path stored in `state.entries`.
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

pub fn render(frame: &mut Frame, editor_area: Rect, state: &VaultPickerState) {
    let popup = compute_popup_rect(editor_area, state);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

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

    let title = format!(
        " Pick vault · {}/{} ",
        state.selected + 1,
        state.entries.len()
    );
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::BORDER)
                .bg(Color::Black),
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
        .map(|path| {
            let is_active = state.active.as_deref() == Some(path.as_str());
            let marker = if is_active { "● " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default().bg(Color::Black).fg(Color::LightMagenta),
                ),
                Span::styled(
                    collapse_home(path),
                    Style::default().bg(Color::Black).fg(Color::White),
                ),
            ]))
        })
        .collect();
    let list = List::new(items).style(bg_style).highlight_style(
        Style::default()
            .bg(super::palette::SELECTION_BG)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(
        state.selected.min(state.entries.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, body_area, &mut list_state);

    let chip_key = Style::default()
        .bg(Color::LightMagenta)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" jk ", chip_key),
        Span::styled(" navigate   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" switch   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

/// Width fits the longest path (clamped 40..area.width-2). Height
/// is `min(entries, MAX_VISIBLE_ROWS) + chrome`. Centered horizontally,
/// dropped 3 rows below the top.
fn compute_popup_rect(area: Rect, state: &VaultPickerState) -> Rect {
    const PADDING: u16 = 6;
    let longest = state
        .entries
        .iter()
        .map(|p| collapse_home(p).chars().count())
        .max()
        .unwrap_or(30) as u16;
    let width = (longest + PADDING).clamp(40, area.width.saturating_sub(2));
    let visible = state.entries.len().min(MAX_VISIBLE_ROWS) as u16;
    let height = visible + 3;

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

    #[test]
    fn collapse_home_swaps_only_leading_match() {
        std::env::set_var("HOME", "/Users/test");
        assert_eq!(collapse_home("/Users/test/notes"), "~/notes");
        // Different leading path stays as-is.
        assert_eq!(collapse_home("/opt/data"), "/opt/data");
        // Substring match in the middle isn't collapsed.
        assert_eq!(
            collapse_home("/var/log/Users/test/x"),
            "/var/log/Users/test/x"
        );
    }

    #[test]
    fn collapse_home_passes_through_when_home_empty() {
        std::env::set_var("HOME", "");
        assert_eq!(collapse_home("/whatever/path"), "/whatever/path");
    }

    #[test]
    fn compute_popup_rect_fits_long_paths_within_area() {
        let state = VaultPickerState {
            entries: vec!["/very/long/path/to/vault".into()],
            selected: 0,
            active: None,
        };
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 30,
        };
        let popup = compute_popup_rect(area, &state);
        assert!(popup.width <= area.width.saturating_sub(2));
        assert!(popup.width >= 40, "minimum clamp must hold");
    }
}
