//! Floating popup that lists SQL completion candidates inside a DB
//! block body. Lives independently of `Mode::Insert` (the user keeps
//! typing to filter) so the dispatcher hijacks a small set of keys
//! (`Tab`/`Enter`/`Esc`/`Ctrl-n`/`Ctrl-p`) and routes them here while
//! the popup is open.
//!
//! Anchored below the focused block — if there isn't enough room
//! below, fall back above. Same precedence as `connection_picker`,
//! but we keep the popup short (≤8 rows) so it doesn't dwarf the
//! editor while the user is typing a single keyword.
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::CompletionPopupState;
use crate::ui::BlockAnchor;

/// At most this many rows in the popup body. Larger result sets
/// scroll via `ListState`'s selection-aware offset — the user
/// typically refines by typing more characters before scrolling.
const MAX_VISIBLE_ROWS: usize = 8;
/// Max popup width in cells. Long labels truncate visually; the
/// `detail` chip on the right shrinks first.
const POPUP_WIDTH: u16 = 36;

pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &CompletionPopupState,
    anchor: Option<BlockAnchor>,
) {
    let popup = compute_popup_rect(editor_area, state, anchor);
    let bg_style = Style::default().bg(Color::Black).fg(Color::White);

    // Hard-fill so editor content doesn't bleed through. Same trick
    // as `connection_picker`/`quickopen` — `Clear` widget on the area
    // would also work but we already paint background style anyway.
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

    let title = if state.prefix.is_empty() {
        " complete ".to_string()
    } else {
        format!(" complete · {} ", state.prefix)
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(Style::default().fg(Color::LightCyan).bg(Color::Black));
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let kind_style = Style::default().bg(Color::Black).fg(Color::DarkGray);
    let label_style = Style::default().bg(Color::Black).fg(Color::White);
    let items: Vec<ListItem> = state
        .items
        .iter()
        .map(|item| {
            // Prefer the source-specific `detail` when set — that
            // disambiguates env vars from block aliases (`env` vs
            // `db-postgres · cached`), columns from keywords (`int4`
            // vs `keyword`), and so on. Fall back to the kind label
            // for sources that don't carry their own detail.
            let suffix = item
                .detail
                .clone()
                .unwrap_or_else(|| item.kind.label().to_string());
            ListItem::new(Line::from(vec![
                Span::styled(item.label.clone(), label_style),
                Span::styled(format!("  {suffix}"), kind_style),
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
        state.selected.min(state.items.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, inner, &mut list_state);
}

/// Compute the popup rect: anchor it just below the cursor's
/// current screen line so it tracks where the user is typing — same
/// pattern as `lang-sql` autocomplete on the desktop. Falls back
/// above the cursor when there's no room below; centered fallback
/// when the block is off-screen.
///
/// Block geometry: the block paints full-width in `editor_area`,
/// with a 1-cell border. The cursor's prefix starts at
/// `(block.x + 1 + anchor_offset, block.y + 1 + anchor_line)` —
/// `+1` accounts for the border on each side. Popup top sits one
/// row below that, x-aligned to the prefix start so the dropdown
/// "drops from" the word being completed.
fn compute_popup_rect(
    editor_area: Rect,
    state: &CompletionPopupState,
    anchor: Option<BlockAnchor>,
) -> Rect {
    let body_rows = state.items.len().clamp(1, MAX_VISIBLE_ROWS) as u16;
    let popup_height = body_rows.saturating_add(2);
    let width = POPUP_WIDTH.min(editor_area.width.saturating_sub(2)).max(20);
    let editor_right = editor_area.x.saturating_add(editor_area.width);
    let editor_bottom = editor_area.y.saturating_add(editor_area.height);

    if let Some(anchor) = anchor {
        // Cursor cell inside the block (1-cell border on every side).
        // `anchor_line` and `anchor_offset` are body-relative; clamp
        // x so the popup never spills past the editor's right edge
        // (slides left), and so wide labels stay readable.
        let cursor_x = editor_area
            .x
            .saturating_add(1)
            .saturating_add(state.anchor_offset as u16);
        // +2 = chrome header (1) + footer/status row (1). DB blocks
        // render with a top chrome (driver chip + RW + connection)
        // BEFORE the body lines start, so `screen_top + 1` would
        // land ON the cursor row (overlap). +2 anchors the popup
        // one cell BELOW the cursor — IDE-style.
        let cursor_y = anchor
            .screen_top
            .saturating_add(2)
            .saturating_add(state.anchor_line as u16);

        // Right-edge clamp: shift the popup left until it fits.
        let popup_x = cursor_x.min(editor_right.saturating_sub(width));

        // Always render below the cursor, clipping the popup height
        // if there isn't full room left. Going above tends to cover
        // the prose/headers the user is referencing — keeping it
        // strictly below + truncated is the desktop's behaviour and
        // what the user expects (issue 2026-05-23).
        let below_top = cursor_y.saturating_add(1);
        let avail = editor_bottom.saturating_sub(below_top);
        // Need at least the border (top + bottom = 2) + 1 row.
        if avail >= 3 {
            let h = popup_height.min(avail);
            return Rect {
                x: popup_x,
                y: below_top,
                width,
                height: h,
            };
        }
    }

    // No anchor or no room above/below — center on the editor area.
    let x = editor_area
        .x
        .saturating_add((editor_area.width.saturating_sub(width)) / 2);
    let y = editor_area
        .y
        .saturating_add((editor_area.height.saturating_sub(popup_height)) / 2);
    Rect {
        x,
        y,
        width,
        height: popup_height,
    }
}
