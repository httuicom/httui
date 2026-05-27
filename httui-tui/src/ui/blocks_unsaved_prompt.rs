//! "You have unsaved blocks" guard modal painted when the user tries
//! to leave BLOCKS view (`Alt+M`) while any pane carries a draft.
//! Tiny widget: title, file list, three labelled chip buttons.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{BlocksUnsavedPromptFocus, BlocksUnsavedPromptState};

const POPUP_WIDTH: u16 = 64;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &BlocksUnsavedPromptState) {
    let popup = compute_popup_rect(editor_area, state.dirty.len());
    let bg = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    // Hard fill so the BLOCKS pane behind doesn't bleed through.
    {
        let buf = frame.buffer_mut();
        for y in popup.y..popup.y.saturating_add(popup.height) {
            for x in popup.x..popup.x.saturating_add(popup.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg);
                }
            }
        }
    }

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Unsaved blocks ")
        .style(bg)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::amber())
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),                                  // heading
            Constraint::Min(1),                                     // file list
            Constraint::Length(1),                                  // blank
            Constraint::Length(1),                                  // chip row
        ])
        .split(inner);

    let heading = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "Save changes before switching view?",
            Style::default()
                .fg(crate::ui::palette::foreground())
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(heading), chunks[0]);

    let muted = Style::default().fg(crate::ui::palette::muted());
    let files: Vec<Line<'static>> = state
        .dirty
        .iter()
        .map(|p| {
            Line::from(vec![
                Span::raw("   • "),
                Span::styled(p.display().to_string(), muted),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(files).wrap(Wrap { trim: false }), chunks[1]);

    let chips = [
        (BlocksUnsavedPromptFocus::Save, "[s] Save"),
        (BlocksUnsavedPromptFocus::Discard, "[d] Discard"),
        (BlocksUnsavedPromptFocus::Cancel, "[esc] Cancel"),
    ];
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    for (focus, label) in chips {
        let style = if focus == state.focus {
            Style::default()
                .bg(crate::ui::palette::accent())
                .fg(crate::ui::palette::popup_bg())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .bg(crate::ui::palette::muted())
                .fg(crate::ui::palette::popup_bg())
        };
        spans.push(Span::styled(format!(" {label} "), style));
        spans.push(Span::raw("  "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), chunks[3]);
}

fn compute_popup_rect(editor_area: Rect, file_count: usize) -> Rect {
    let list_height = (file_count as u16).clamp(1, 6);
    let height: u16 = 2 /* borders */ + 1 /* heading */ + list_height + 1 /* blank */ + 1 /* chips */;
    let width = POPUP_WIDTH
        .min(editor_area.width.saturating_sub(2))
        .max(28);
    let x = editor_area
        .x
        .saturating_add(editor_area.width.saturating_sub(width) / 2);
    let y = editor_area
        .y
        .saturating_add(editor_area.height.saturating_sub(height) / 2);
    Rect {
        x,
        y,
        width,
        height,
    }
}
