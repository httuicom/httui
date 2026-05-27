//! Content-search modal renderer. Centered
//! overlay with a `?` prompt at the top and a result list below;
//! each row shows the file path and the FTS5-generated snippet
//! with `<mark>…</mark>` tags rewritten as a colored span. Cursor
//! lands inside the prompt — typing flows into the LineEdit which
//! re-queries on every change.
//!
//! Visual is wider than QuickOpen (90×70) since snippets need
//! breathing room; vault-relative path on its own row, snippet
//! indented underneath. Same hard-fill anti-bleed trick as the
//! other overlays.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::ContentSearchState;

const MAX_VISIBLE_ROWS: usize = 14;

/// Render the modal centered over `editor_area`. Returns the
/// prompt's `(x, y)` so the caller can place the terminal cursor
/// inside the query input.
pub fn render(frame: &mut Frame, editor_area: Rect, state: &ContentSearchState) -> (u16, u16) {
    let modal = centered_rect(editor_area, 90, 70);
    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    // Hard-fill so editor content underneath doesn't bleed.
    {
        let buf = frame.buffer_mut();
        for y in modal.y..modal.y.saturating_add(modal.height) {
            for x in modal.x..modal.x.saturating_add(modal.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let title = format!(
        " Find content · {} {} ",
        state.results.len(),
        if state.results.len() == 1 {
            "match"
        } else {
            "matches"
        }
    );
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .bg(crate::ui::palette::popup_bg())
                .fg(crate::ui::palette::success()),
        );
    let inner = outer.inner(modal);
    frame.render_widget(outer, modal);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // prompt
            Constraint::Length(1), // separator/blank
            Constraint::Min(1),    // results
            Constraint::Length(1), // footer
        ])
        .split(inner);
    let prompt_area = chunks[0];
    let _separator_area = chunks[1];
    let list_area = chunks[2];
    let footer_area = chunks[3];

    // Prompt line. `?` instead of `>` to differentiate from
    // QuickOpen at a glance.
    let prompt_line = Line::from(vec![
        Span::styled(
            "? ",
            Style::default()
                .bg(crate::ui::palette::popup_bg())
                .fg(crate::ui::palette::success()),
        ),
        Span::styled(state.query.as_str().to_string(), bg_style),
    ]);
    frame.render_widget(Paragraph::new(prompt_line).style(bg_style), prompt_area);

    // Result list. Each entry: `<path>` on the row, snippet
    // shown one row below with the marked region highlighted.
    // We collapse the two visual rows into one ListItem with a
    // multi-line `Line` array — Ratatui ListItem supports that.
    let items: Vec<ListItem> = if state.building {
        vec![ListItem::new(Line::from(Span::styled(
            "  indexing vault…",
            Style::default()
                .bg(crate::ui::palette::popup_bg())
                .fg(crate::ui::palette::popup_border_accent())
                .add_modifier(Modifier::ITALIC | Modifier::BOLD),
        )))]
    } else if state.query.as_str().is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  type to search vault contents",
            Style::default()
                .bg(crate::ui::palette::popup_bg())
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::ITALIC),
        )))]
    } else if state.results.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "  no matches",
            Style::default()
                .bg(crate::ui::palette::popup_bg())
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::ITALIC),
        )))]
    } else {
        state
            .results
            .iter()
            .take(MAX_VISIBLE_ROWS)
            .map(|r| {
                let path_line = Line::from(Span::styled(
                    r.file_path.clone(),
                    bg_style.fg(Color::LightCyan),
                ));
                let snippet_line = Line::from(highlight_snippet(&r.snippet, bg_style));
                ListItem::new(vec![path_line, snippet_line])
            })
            .collect()
    };
    let list = List::new(items)
        .style(bg_style)
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(40, 60, 40))
                .fg(crate::ui::palette::foreground())
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▸ ");
    let mut list_state = ListState::default();
    if !state.results.is_empty() {
        list_state.select(Some(state.selected.min(state.results.len() - 1)));
    }
    frame.render_stateful_widget(list, list_area, &mut list_state);

    let chip_key = Style::default()
        .bg(crate::ui::palette::success())
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" ↑↓ ", chip_key),
        Span::styled(" navigate   ", chip_label),
        Span::styled(" Enter ", chip_key),
        Span::styled(" open   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);

    // Cursor: 2 chars of `? ` prefix + chars-before-cursor.
    let col = (state.query.cursor_col() as u16).saturating_add(2);
    let x = prompt_area
        .x
        .saturating_add(col.min(prompt_area.width.saturating_sub(1)));
    (x + 1, prompt_area.y + 1) // shift inside the modal border
}

/// Convert FTS5's `<mark>…</mark>` snippet into styled spans —
/// marked region in green, rest in dim gray for readability. CR/LF
/// are folded to single spaces so the snippet stays on one line in
/// the list.
fn highlight_snippet(snippet: &str, bg_style: Style) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let cleaned: String = snippet.replace(['\r', '\n'], " ");
    let mark_open = "<mark>";
    let mark_close = "</mark>";
    let mut rest: &str = &cleaned;
    spans.push(Span::styled("  ".to_string(), bg_style));
    while !rest.is_empty() {
        match rest.find(mark_open) {
            Some(i) => {
                if i > 0 {
                    spans.push(Span::styled(
                        rest[..i].to_string(),
                        bg_style.fg(crate::ui::palette::muted()),
                    ));
                }
                let after_open = &rest[i + mark_open.len()..];
                match after_open.find(mark_close) {
                    Some(j) => {
                        spans.push(Span::styled(
                            after_open[..j].to_string(),
                            bg_style
                                .fg(crate::ui::palette::popup_bg())
                                .bg(crate::ui::palette::success())
                                .add_modifier(Modifier::BOLD),
                        ));
                        rest = &after_open[j + mark_close.len()..];
                    }
                    None => {
                        // Unmatched <mark> — render the rest as
                        // plain so the user still sees the text.
                        spans.push(Span::styled(
                            after_open.to_string(),
                            bg_style.fg(crate::ui::palette::muted()),
                        ));
                        break;
                    }
                }
            }
            None => {
                spans.push(Span::styled(
                    rest.to_string(),
                    bg_style.fg(crate::ui::palette::muted()),
                ));
                break;
            }
        }
    }
    // We need owned strings — make sure all spans carry `'static` content.
    // Already done via `to_string()` above; the function signature
    // `Vec<Span<'static>>` enforces it.
    spans
}

fn centered_rect(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let popup_w = area.width * percent_x / 100;
    let popup_h = area.height * percent_y / 100;
    let x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_h)) / 2;
    Rect {
        x,
        y,
        width: popup_w,
        height: popup_h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_snippet_marks_strip_html_and_styles() {
        let bg = Style::default()
            .bg(crate::ui::palette::popup_bg())
            .fg(crate::ui::palette::foreground());
        let spans = highlight_snippet("foo <mark>bar</mark> baz", bg);
        // Leading 2-space indent + before + marked + after = 4 spans.
        assert_eq!(spans.len(), 4);
        // The "bar" span is the marked one — bg green, bold.
        assert_eq!(spans[2].content.as_ref(), "bar");
    }

    #[test]
    fn highlight_snippet_handles_unmatched_open() {
        let bg = Style::default().bg(crate::ui::palette::popup_bg());
        let spans = highlight_snippet("a <mark>b", bg);
        // Just the indent + "a " + "b" — unmatched mark falls
        // through as plain text, no panic.
        assert!(spans.iter().any(|s| s.content.contains('b')));
    }

    #[test]
    fn highlight_snippet_collapses_newlines() {
        let bg = Style::default().bg(crate::ui::palette::popup_bg());
        let spans = highlight_snippet("a\nb\r\nc", bg);
        // No span content carries a `\n`.
        for s in &spans {
            assert!(!s.content.contains('\n'));
            assert!(!s.content.contains('\r'));
        }
    }
}
