//! Compact popup for inline fence-metadata edits (alias today;
//! limit / timeout once those slices land). Triggered by `ga` (and
//! future `gl` / `gt`); a single-line text input centered horizontally
//! over the block, anchored above the block when there's headroom and
//! below otherwise — same anchoring rule as `connection_picker`.
//!
//! Visual: small bordered box, ~40 cols wide and 4 rows tall (top
//! border + input row + footer hints + bottom border). The buffer
//! lives on `App.fence_edit.input` (a `LineEdit`); we just paint
//! whatever it carries plus a block-cursor sigil at the caret.
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::ui::BlockAnchor;
use crate::vim::lineedit::LineEdit;

/// Outer box minimums: the input row is one cell, plus 2 borders + 1
/// footer = 4 chrome rows. Width is clamped so the popup never
/// overflows the editor area.
const POPUP_HEIGHT: u16 = 4;
const POPUP_MIN_WIDTH: u16 = 36;

pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    kind_label: &str,
    input: &LineEdit,
    anchor: Option<BlockAnchor>,
) {
    let popup = compute_popup_rect(editor_area, anchor);
    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    // Hard-fill the popup before painting so editor content under it
    // doesn't bleed through (matches connection_picker / quickopen).
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

    let title = format!(" Edit {} ", kind_label);
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::popup_border_accent())
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    let input_area = chunks[0];
    let footer_area = chunks[1];

    // Render the buffer with a block-cursor sigil at the caret. We
    // can't show the real terminal cursor here (that lives where the
    // editor put it); ▏ is a thin vertical bar that reads as "your
    // typing position" without overlapping the previous char.
    let buf_str = input.as_str();
    let cursor_col = input.cursor_col();
    let (before, after) = split_at_byte_offset_by_chars(buf_str, cursor_col);
    let line = Line::from(vec![
        Span::styled(before.to_string(), bg_style),
        Span::styled(
            "▏",
            Style::default()
                .bg(crate::ui::palette::popup_bg())
                .fg(crate::ui::palette::popup_border_accent()),
        ),
        Span::styled(after.to_string(), bg_style),
    ]);
    frame.render_widget(Paragraph::new(line), input_area);

    let chip_key = Style::default()
        .bg(crate::ui::palette::popup_border_accent())
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" Enter ", chip_key),
        Span::styled(" save   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" cancel ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

/// Find the byte offset that sits `n_chars` characters into `s` and
/// split there. Used to put the cursor sigil between two grapheme
/// clusters when the buffer carries multibyte text (CJK alias names,
/// etc.). For ASCII it's the same as `split_at(n)`.
fn split_at_byte_offset_by_chars(s: &str, n_chars: usize) -> (&str, &str) {
    let byte = s
        .char_indices()
        .nth(n_chars)
        .map(|(b, _)| b)
        .unwrap_or(s.len());
    s.split_at(byte)
}

/// Compute the popup rect. Width fits the longest reasonable input
/// (clamped between `POPUP_MIN_WIDTH` and the editor width); height
/// is a fixed `POPUP_HEIGHT` (input + footer + 2 borders).
///
/// Anchoring rule mirrors the connection picker: snap to the block's
/// left edge, prefer the slot directly *above* the block; if the
/// block sits too close to the editor top, drop below; if neither
/// fits cleanly, fall back to a horizontally-centered slot near the
/// editor top.
fn compute_popup_rect(area: Rect, anchor: Option<BlockAnchor>) -> Rect {
    let width = POPUP_MIN_WIDTH.min(area.width.saturating_sub(2));
    let height = POPUP_HEIGHT;

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
