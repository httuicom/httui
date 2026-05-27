//! Compact popup for the DB block settings modal — limit + timeout
//! in a single form. Triggered by the `gs` chord on a DB block.
//! Tab/BackTab cycle the focused input; Enter saves both; Esc cancels.
//!
//! Visual: small bordered box, ~44 cols wide and 7 rows tall (top
//! border + 2× (label row + input row) + footer + bottom border).
//! The focused input row has a yellow caret sigil and a subtle
//! highlight on its label so users see which field receives typing.
//!
//! This is the user-pinned UX from `project_tui_block_settings_modal.md`
//! — one popup with multiple fields, NOT chord-per-field.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::DbSettingsState;
use crate::ui::BlockAnchor;

/// Per-field overhead inside the popup: 1 row for the label + 1
/// row for the input.
const ROWS_PER_FIELD: u16 = 2;
/// Chrome rows: top border + footer + bottom border.
const CHROME_ROWS: u16 = 3;
const POPUP_MIN_WIDTH: u16 = 44;

pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &DbSettingsState,
    anchor: Option<BlockAnchor>,
) {
    let popup = compute_popup_rect(editor_area, state, anchor);
    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    // Hard-fill so editor content underneath doesn't bleed through.
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

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Block settings ")
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::popup_border_accent())
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    // Inner layout: 2 rows per field (label + input) plus 1 row for
    // the footer. Walk the state's `fields` vector — each entry
    // becomes a (label, input) pair, the focused one rendered with
    // the active caret style.
    let mut constraints: Vec<Constraint> = Vec::new();
    for _ in &state.fields {
        constraints.push(Constraint::Length(1)); // label
        constraints.push(Constraint::Length(1)); // input
    }
    constraints.push(Constraint::Length(1)); // footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, field) in state.fields.iter().enumerate() {
        let label_idx = i * 2;
        render_field(
            frame,
            chunks[label_idx],
            chunks[label_idx + 1],
            field.label,
            &field.input,
            state.focus == i,
            bg_style,
        );
    }

    let footer_idx = state.fields.len() * 2;
    let chip_key = Style::default()
        .bg(crate::ui::palette::popup_border_accent())
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    // Tab chip only makes sense on multi-field modals; for the
    // single-field variant we drop it.
    let footer = if state.fields.len() > 1 {
        Line::from(vec![
            Span::styled(" Tab ", chip_key),
            Span::styled(" next   ", chip_label),
            Span::styled(" Enter ", chip_key),
            Span::styled(" save   ", chip_label),
            Span::styled(" Esc ", chip_key),
            Span::styled(" cancel ", chip_label),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Enter ", chip_key),
            Span::styled(" save   ", chip_label),
            Span::styled(" Esc ", chip_key),
            Span::styled(" cancel ", chip_label),
        ])
    };
    frame.render_widget(Paragraph::new(footer).style(bg_style), chunks[footer_idx]);
}

/// Paint one labeled field — label row above, input row below. The
/// focused input gets a yellow caret sigil and a brighter label fg
/// so the user sees which buffer typing lands in.
fn render_field(
    frame: &mut Frame,
    label_area: Rect,
    input_area: Rect,
    label_text: &str,
    input: &crate::vim::lineedit::LineEdit,
    focused: bool,
    bg_style: Style,
) {
    let label_fg = if focused {
        crate::ui::palette::popup_border_accent()
    } else {
        Color::Gray
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            label_text.to_string(),
            bg_style.fg(label_fg).add_modifier(Modifier::BOLD),
        ))),
        label_area,
    );

    let buf_str = input.as_str();
    let cursor_col = input.cursor_col();
    let (before, after) = split_at_byte_offset_by_chars(buf_str, cursor_col);
    let line: Line<'static> = if focused {
        Line::from(vec![
            Span::styled(" ".to_string(), bg_style),
            Span::styled(before.to_string(), bg_style),
            Span::styled(
                "▏",
                Style::default()
                    .bg(crate::ui::palette::popup_bg())
                    .fg(crate::ui::palette::popup_border_accent()),
            ),
            Span::styled(after.to_string(), bg_style),
        ])
    } else {
        // Unfocused: still show buffer content (so user sees both
        // values at once) but no caret sigil. Dim it slightly so
        // the eye knows where typing is going.
        Line::from(vec![
            Span::styled(" ".to_string(), bg_style),
            Span::styled(
                buf_str.to_string(),
                bg_style.fg(crate::ui::palette::muted()),
            ),
        ])
    };
    frame.render_widget(Paragraph::new(line), input_area);
}

/// Find the byte offset that sits `n_chars` characters into `s` and
/// split there. Same helper as `ui::fence_edit` — could be promoted
/// to a shared utility once a third caller appears.
fn split_at_byte_offset_by_chars(s: &str, n_chars: usize) -> (&str, &str) {
    let byte = s
        .char_indices()
        .nth(n_chars)
        .map(|(b, _)| b)
        .unwrap_or(s.len());
    s.split_at(byte)
}

fn compute_popup_rect(area: Rect, state: &DbSettingsState, anchor: Option<BlockAnchor>) -> Rect {
    let width = POPUP_MIN_WIDTH.min(area.width.saturating_sub(2));
    // Height = N fields × 2 rows + chrome (top border, footer,
    // bottom border). Saturating clamp so a 0-field modal still
    // computes a sane height (defensive — opener always pushes ≥1
    // field).
    let height = (state.fields.len() as u16)
        .saturating_mul(ROWS_PER_FIELD)
        .saturating_add(CHROME_ROWS);

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
