//! Compact popup that lists environments so the user can swap the
//! active one without leaving the editor. Triggered by `gE`;
//! navigated with `j`/`k` or arrows; `Enter` activates, `Esc`/
//! `Ctrl-C` close.
//!
//! Visual: a small bordered box (~30 cols), centered horizontally
//! and floated near the top. Envs are global state — there's no
//! source block to anchor to, so we always center.
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::EnvironmentPickerState;

/// Maximum body rows shown at once. The list scrolls past this via
/// `ListState`'s built-in selection-aware offset.
const MAX_VISIBLE_ROWS: usize = 12;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &EnvironmentPickerState) {
    let popup = compute_popup_rect(editor_area, state);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard-fill the popup area before painting so editor content
    // underneath doesn't bleed through.
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
        " Pick environment · {}/{} ",
        state.selected + 1,
        state.entries.len()
    );
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(Style::default().fg(Color::LightMagenta).bg(Color::Black));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let body_area = chunks[0];
    let footer_area = chunks[1];

    // Build list items: name in white; the active env gets a leading
    // `●` glyph in magenta (matches the status-bar chip color).
    let items: Vec<ListItem> = state
        .entries
        .iter()
        .map(|e| {
            let is_active = state.active_id.as_deref() == Some(e.id.as_str());
            let marker = if is_active { "● " } else { "  " };
            ListItem::new(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default().bg(Color::Black).fg(Color::LightMagenta),
                ),
                Span::styled(
                    e.name.clone(),
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
        Span::styled(" 1-9 ", chip_key),
        Span::styled(" pick   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" activate   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

/// Width fits the longest env name (clamped between 30 and the
/// editor width); height is `min(envs, MAX_VISIBLE_ROWS) + chrome`.
/// Centered horizontally; vertically dropped 3 rows below the editor
/// top so the popup feels like an overlay, not a modal taking over.
fn compute_popup_rect(area: Rect, state: &EnvironmentPickerState) -> Rect {
    const PADDING: u16 = 6; // borders + spacing + marker glyph
    let longest = state
        .entries
        .iter()
        .map(|e| e.name.chars().count())
        .max()
        .unwrap_or(20) as u16;
    let width = (longest + PADDING).clamp(30, area.width.saturating_sub(2));
    let visible = state.entries.len().min(MAX_VISIBLE_ROWS) as u16;
    let height = visible + 3; // top border + footer + bottom border

    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + 3u16.min(area.height.saturating_sub(height));
    Rect {
        x,
        y,
        width,
        height,
    }
}
