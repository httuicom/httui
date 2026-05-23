//! Compact popup that lists DB export formats (CSV / JSON / Markdown
//! / INSERT) so the user can copy a SELECT result to the clipboard
//! without leaving the editor. Triggered by the `gx` chord on a DB
//! block that has rows; navigated with `j`/`k` (or arrows / Ctrl-n/p);
//! `Enter` copies, `Esc`/`Ctrl-C` close.
//!
//! Visual: a small bordered box anchored above the focused block (or
//! below when there's no headroom), 4 rows of body + 1 row of footer
//! chrome — same rendering style as `connection_picker` so users see
//! a consistent popup family.
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::DbExportPickerState;
use crate::ui::BlockAnchor;

pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &DbExportPickerState,
    anchor: Option<BlockAnchor>,
) {
    let popup = compute_popup_rect(editor_area, state, anchor);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard-fill so the editor underneath doesn't bleed through any
    // transparent borders. Same trick as `connection_picker` and
    // `quickopen`.
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

    let title = format!(" Export · {}/{} ", state.selected + 1, state.formats.len());
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(Style::default().fg(Color::LightBlue).bg(Color::Black));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let body_area = chunks[0];
    let footer_area = chunks[1];

    let items: Vec<ListItem> = state
        .formats
        .iter()
        .map(|f| {
            ListItem::new(Line::from(vec![Span::styled(
                f.label().to_string(),
                Style::default().bg(Color::Black).fg(Color::White),
            )]))
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
        state.selected.min(state.formats.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, body_area, &mut list_state);

    let chip_key = Style::default()
        .bg(Color::LightBlue)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" jk ", chip_key),
        Span::styled(" navigate   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" copy   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

/// Compute the popup rect. Width is fixed at 26 (the longest label
/// "INSERT statements" is 17 chars + chrome); height fits the 4
/// formats + footer + borders. Anchoring rule mirrors
/// `connection_picker::compute_popup_rect`: align with the block's
/// left edge, prefer the slot above; fall back to below or centered
/// when there's no room.
fn compute_popup_rect(
    area: Rect,
    state: &DbExportPickerState,
    anchor: Option<BlockAnchor>,
) -> Rect {
    const WIDTH: u16 = 26;
    let n_items = state.formats.len() as u16;
    let height = n_items + 3; // top border + footer + bottom border

    let width = WIDTH.min(area.width.saturating_sub(2));
    let (x, y) = match anchor {
        Some(a) => {
            let max_x = area.x + area.width.saturating_sub(width);
            let x = (area.x + 2).min(max_x);
            let above_y = a.screen_top.checked_sub(height);
            let below_y = a.screen_top.saturating_add(a.height);
            let fits_below = below_y.saturating_add(height) <= area.y.saturating_add(area.height);
            let y = match (above_y, fits_below) {
                (Some(top), _) if top >= area.y => top,
                (_, true) => below_y,
                (Some(top), false) => top,
                (None, false) => area.y,
            };
            (x, y)
        }
        None => {
            let x = area.x + (area.width.saturating_sub(width)) / 2;
            let y = area.y + 3u16.min(area.height.saturating_sub(height));
            (x, y)
        }
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}
