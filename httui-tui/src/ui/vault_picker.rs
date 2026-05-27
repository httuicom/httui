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
                .fg(crate::ui::palette::border())
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),    // list
            Constraint::Length(1), // verbs footer (n/c/o/s)
            Constraint::Length(1), // nav footer (jk/Enter/Esc)
        ])
        .split(inner);
    let body_area = chunks[0];
    let verbs_area = chunks[1];
    let nav_area = chunks[2];

    let items: Vec<ListItem> = state
        .entries
        .iter()
        .map(|path| {
            let is_active = state.active.as_deref() == Some(path.as_str());
            let marker = if is_active { "● " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(Color::LightMagenta),
                ),
                Span::styled(
                    collapse_home(path),
                    Style::default()
                        .bg(crate::ui::palette::popup_bg())
                        .fg(crate::ui::palette::foreground()),
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

    // Minimal hint footer: bold accent key, dim label, `·` separator.
    // No coloured background blocks — they read as noise once you
    // have 4+ chips on screen.
    let key_style = Style::default()
        .fg(crate::ui::palette::accent())
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(crate::ui::palette::muted());
    let sep_style = Style::default().fg(crate::ui::palette::muted());
    let hint = |pairs: &[(&str, &str)]| {
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(pairs.len() * 4);
        for (i, (key, label)) in pairs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("  ·  ".to_string(), sep_style));
            }
            spans.push(Span::styled((*key).to_string(), key_style));
            spans.push(Span::raw(" "));
            spans.push(Span::styled((*label).to_string(), label_style));
        }
        Line::from(spans)
    };

    frame.render_widget(
        Paragraph::new(hint(&[("n", "new"), ("c", "clone"), ("o", "open")])).style(bg_style),
        verbs_area,
    );
    frame.render_widget(
        Paragraph::new(hint(&[
            ("jk", "navigate"),
            ("Enter", "switch"),
            ("Esc", "close"),
        ]))
        .style(bg_style),
        nav_area,
    );
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
    // Width clamps to 50 minimum so the verbs footer
    // (` n  new  c  clone  o  open  s  secrets `) doesn't wrap.
    let width = (longest + PADDING).clamp(50, area.width.saturating_sub(2));
    let visible = state.entries.len().min(MAX_VISIBLE_ROWS) as u16;
    // list rows + verbs footer + nav footer + 2 borders.
    let height = visible + 4;

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
        assert!(popup.width >= 50, "minimum clamp must hold");
    }

    fn render_with(state: &VaultPickerState) {
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render(
                    f,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 80,
                        height: 24,
                    },
                    state,
                );
            })
            .unwrap();
    }

    #[test]
    fn render_smoke_empty_state() {
        let state = VaultPickerState {
            entries: vec![],
            selected: 0,
            active: None,
        };
        render_with(&state);
    }

    #[test]
    fn render_smoke_with_entries_and_selection() {
        let state = VaultPickerState {
            entries: vec![
                "/home/u/vault-a".into(),
                "/home/u/vault-b".into(),
                "/home/u/vault-c".into(),
            ],
            selected: 1,
            active: Some("/home/u/vault-b".into()),
        };
        render_with(&state);
    }

    #[test]
    fn render_smoke_long_list_overflow() {
        let entries: Vec<String> = (0..40).map(|i| format!("/v/path-{i}")).collect();
        let state = VaultPickerState {
            entries,
            selected: 25,
            active: None,
        };
        render_with(&state);
    }

    #[test]
    fn collapse_home_replaces_user_dir_with_tilde() {
        let home = std::env::var("HOME").unwrap_or_default();
        if home.is_empty() {
            return;
        }
        let p = format!("{home}/foo/bar");
        let s = collapse_home(&p);
        assert!(s.starts_with("~/"), "got: {s}");
    }

    #[test]
    fn collapse_home_preserves_non_home_paths() {
        let s = collapse_home("/etc/foo");
        assert_eq!(s, "/etc/foo");
    }
}
