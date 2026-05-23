//! Compact popup that lists DB connections so the user can swap a
//! block's `connection=` without leaving the editor. Triggered by
//! `:conn`; navigated with `j`/`k` or arrows; `Enter` applies, the
//! usual `Esc`/`Ctrl-C` close.
//!
//! Visual: a small bordered box, ~40 cols wide and as tall as it
//! needs to be (one row per connection + chrome), centered
//! horizontally and floated near the top of the editor area. We
//! deliberately don't anchor to the source block — block bounds
//! aren't easily reachable from this layer and "near the top" is
//! never wrong.
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::ConnectionPickerState;
use crate::ui::BlockAnchor;

/// Maximum body rows shown at once. The list scrolls past this via
/// `ListState`'s built-in selection-aware offset.
const MAX_VISIBLE_ROWS: usize = 12;

pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &ConnectionPickerState,
    anchor: Option<BlockAnchor>,
) {
    let popup = compute_popup_rect(editor_area, state, anchor);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard-fill the popup area before painting so editor content
    // underneath doesn't bleed through (same trick as `quickopen`).
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
        " Pick connection · {}/{} ",
        state.selected + 1,
        state.connections.len()
    );
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

    // Build list items: name in white, kind in dim gray as suffix.
    let items: Vec<ListItem> = state
        .connections
        .iter()
        .map(|c| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    c.name.clone(),
                    Style::default().bg(Color::Black).fg(Color::White),
                ),
                Span::styled(
                    format!("  ({})", c.kind),
                    Style::default().bg(Color::Black).fg(Color::DarkGray),
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
        state
            .selected
            .min(state.connections.len().saturating_sub(1)),
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
        Span::styled(" pick   ", chip_label),
        Span::styled(" D ", chip_key),
        Span::styled(" delete   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

/// Compute the popup rect. Width fits the longest connection label
/// (clamped between 30 and the editor width); height is
/// `min(connections, MAX_VISIBLE_ROWS) + chrome`.
///
/// Anchoring rule: when `anchor` is `Some` (we know the focused
/// block's screen rect), the popup snaps to the block's left edge
/// and prefers the slot directly *above* the block. If there's not
/// enough headroom above, it flips to the slot just below. When
/// `anchor` is `None` (block off-screen / no doc), it falls back to
/// floating ~3 rows below the editor top, horizontally centered.
fn compute_popup_rect(
    area: Rect,
    state: &ConnectionPickerState,
    anchor: Option<BlockAnchor>,
) -> Rect {
    const PADDING: u16 = 4; // borders + spacing
    let longest = state
        .connections
        .iter()
        .map(|c| c.name.chars().count() + c.kind.chars().count() + 4) // " (kind)"
        .max()
        .unwrap_or(20) as u16;
    let width = (longest + PADDING).clamp(30, area.width.saturating_sub(2));
    let visible = state.connections.len().min(MAX_VISIBLE_ROWS) as u16;
    let height = visible + 3; // top border + footer + bottom border

    let (x, y) = match anchor {
        Some(a) => {
            // Horizontal: align with the block's left edge, but
            // shift left if it would overflow the right of the
            // editor area.
            let max_x = area.x + area.width.saturating_sub(width);
            let x = (area.x + 2).min(max_x); // +2 = past the block's left border
                                             // Vertical: prefer above the block. If the block sits
                                             // too close to the top of the editor, drop the popup
                                             // below the block instead.
            let above_y = a.screen_top.checked_sub(height);
            let below_y = a.screen_top.saturating_add(a.height);
            let fits_below = below_y.saturating_add(height) <= area.y.saturating_add(area.height);
            let y = match (above_y, fits_below) {
                (Some(top), _) if top >= area.y => top,
                (_, true) => below_y,
                (Some(top), false) => top, // fall back even if it clips
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
