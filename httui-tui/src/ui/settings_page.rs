//! Settings page renderer.
//!
//! Three-section fullscreen modal mirroring the
//! `ConnectionsPage`/`EnvsPage` layout: tab strip at the top,
//! per-section body underneath, fixed footer hint.
//!
//! Takes a pre-resolved [`KeymapRowView`] slice so it stays `App`-free
//! and unit-testable against a `TestBackend`.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::{SettingsPageState, SettingsSection};
use crate::input::apply::settings_page::{EditorRowView, KeymapRowView, ThemeRowView};
use crate::ui::palette;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    state: &SettingsPageState,
    keymap_rows: &[KeymapRowView],
    theme_rows: &[ThemeRowView],
    editor_rows: &[EditorRowView],
) {
    let bg_style = Style::default()
        .bg(palette::popup_bg())
        .fg(palette::foreground());

    // Hard-fill the panel area so the editor underneath doesn't
    // bleed through cells the inner widgets don't explicitly write.
    {
        let buf = frame.buffer_mut();
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let outer = Block::default()
        .borders(Borders::ALL)
        .title(" Settings · Alt+, ")
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(palette::border())
                .bg(palette::popup_bg()),
        );
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // tab strip
            Constraint::Min(1),    // body
            Constraint::Length(1), // footer hint
        ])
        .split(inner);
    render_tab_strip(frame, chunks[0], state.section);
    render_body(
        frame,
        chunks[1],
        state,
        keymap_rows,
        theme_rows,
        editor_rows,
    );
    render_footer(frame, chunks[2], state);
}

fn render_tab_strip(frame: &mut Frame, area: Rect, active: SettingsSection) {
    let mut spans: Vec<Span> = Vec::new();
    for (idx, section) in SettingsSection::ALL.iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw("  "));
        }
        let label = format!(" {} ", section.label());
        let style = if *section == active {
            Style::default()
                .fg(palette::accent())
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(palette::muted())
        };
        spans.push(Span::styled(label, style));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_body(
    frame: &mut Frame,
    area: Rect,
    state: &SettingsPageState,
    keymap_rows: &[KeymapRowView],
    theme_rows: &[ThemeRowView],
    editor_rows: &[EditorRowView],
) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(palette::border()));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    match state.section {
        SettingsSection::Keymaps => render_keymaps(frame, inner, state, keymap_rows),
        SettingsSection::Theme => render_theme(frame, inner, state, theme_rows),
        SettingsSection::Editor => render_editor(frame, inner, state, editor_rows),
    }
}

fn render_editor(frame: &mut Frame, area: Rect, state: &SettingsPageState, rows: &[EditorRowView]) {
    if rows.is_empty() {
        return;
    }
    let label_width = rows
        .iter()
        .map(|r| r.label.chars().count())
        .max()
        .unwrap_or(20);
    let value_width = rows
        .iter()
        .map(|r| r.value.chars().count())
        .max()
        .unwrap_or(8)
        .max(8);
    let cursor = state.editor_cursor.min(rows.len().saturating_sub(1));

    let mut lines: Vec<Line> = Vec::with_capacity(rows.len() + 2);
    lines.push(Line::from(""));
    for (idx, row) in rows.iter().enumerate() {
        let is_selected = idx == cursor;
        let prefix = if is_selected { "▶ " } else { "  " };
        let label_pad = " ".repeat(label_width.saturating_sub(row.label.chars().count()));
        let value_pad = " ".repeat(value_width.saturating_sub(row.value.chars().count()));

        let row_style = if is_selected {
            Style::default()
                .bg(palette::selection_bg())
                .fg(palette::accent())
        } else {
            Style::default()
        };
        let label_style = if is_selected {
            row_style.add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::secondary())
        };
        let value_style = Style::default()
            .fg(palette::amber())
            .add_modifier(Modifier::BOLD);

        let mut spans: Vec<Span> = vec![
            Span::styled(format!("  {prefix}"), row_style),
            Span::styled(row.label.clone(), label_style),
            Span::styled(label_pad, row_style),
            Span::raw("  "),
            Span::styled(row.value.clone(), value_style),
            Span::styled(value_pad, row_style),
        ];
        if !row.hint.is_empty() {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                row.hint.clone(),
                Style::default().fg(palette::muted()),
            ));
        }
        lines.push(Line::from(spans));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Press Enter on a row to toggle its value.",
        Style::default().fg(palette::muted()),
    )));
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_theme(frame: &mut Frame, area: Rect, state: &SettingsPageState, rows: &[ThemeRowView]) {
    if rows.is_empty() {
        return;
    }
    let cursor = state.theme_cursor.min(rows.len().saturating_sub(1));
    let mut lines: Vec<Line> = Vec::with_capacity(rows.len() + 2);
    lines.push(Line::from(""));
    for (idx, row) in rows.iter().enumerate() {
        let is_selected = idx == cursor;
        let prefix = if is_selected { "▶ " } else { "  " };
        let suffix = if row.is_active { "  (active)" } else { "" };
        let row_style = if is_selected {
            Style::default()
                .bg(palette::selection_bg())
                .fg(palette::accent())
        } else {
            Style::default()
        };
        let label_style = if is_selected {
            row_style.add_modifier(Modifier::BOLD)
        } else if row.is_active {
            Style::default().fg(palette::accent())
        } else {
            Style::default().fg(palette::secondary())
        };
        let active_style = Style::default().fg(palette::amber());
        lines.push(Line::from(vec![
            Span::styled(format!("  {prefix}"), row_style),
            Span::styled(row.name.clone(), label_style),
            Span::styled(suffix, active_style),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Per-color overrides go in `~/.config/httui/config.toml` under `[palette]`.",
        Style::default().fg(palette::muted()),
    )));
    frame.render_widget(Paragraph::new(lines), area);
}

fn render_keymaps(
    frame: &mut Frame,
    area: Rect,
    state: &SettingsPageState,
    rows: &[KeymapRowView],
) {
    if rows.is_empty() {
        return;
    }
    let name_width = rows
        .iter()
        .map(|r| r.display.chars().count())
        .max()
        .unwrap_or(20);
    let chord_width = rows
        .iter()
        .map(|r| r.chord.chars().count())
        .max()
        .unwrap_or(10)
        .max(10);

    let capture_target: Option<&str> = state.capture.as_ref().map(|c| c.action_name.as_str());
    let cursor = state.keymap_cursor.min(rows.len().saturating_sub(1));

    let visible = area.height as usize;
    let scroll = scroll_offset(cursor, visible, rows.len());

    let mut lines: Vec<Line> = Vec::with_capacity(rows.len() + 2);
    for (idx, row) in rows.iter().enumerate().skip(scroll).take(visible) {
        let is_selected = idx == cursor;
        let prefix = if is_selected { "▶ " } else { "  " };
        let name_pad = " ".repeat(name_width.saturating_sub(row.display.chars().count()));
        let chord_disp = if Some(row.name.as_str()) == capture_target {
            "press the new chord (Esc to cancel)".to_string()
        } else {
            row.chord.clone()
        };
        let chord_pad = " ".repeat(chord_width.saturating_sub(chord_disp.chars().count()));

        let row_style = if is_selected {
            Style::default()
                .bg(palette::selection_bg())
                .fg(palette::accent())
        } else {
            Style::default()
        };
        let label_style = if is_selected {
            row_style.add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::secondary())
        };
        let chord_style = if Some(row.name.as_str()) == capture_target {
            Style::default()
                .fg(palette::amber())
                .add_modifier(Modifier::BOLD)
        } else if is_selected {
            row_style
        } else {
            Style::default()
        };

        let mut spans: Vec<Span> = vec![
            Span::styled(prefix.to_string(), row_style),
            Span::styled(row.display.clone(), label_style),
            Span::styled(name_pad, row_style),
            Span::raw("  "),
            Span::styled(chord_disp, chord_style),
            Span::styled(chord_pad, row_style),
        ];
        if let Some(other) = row.conflict_with.as_deref() {
            spans.push(Span::raw("  "));
            spans.push(Span::styled(
                format!("⚠ also bound to `{other}`"),
                Style::default().fg(palette::amber()),
            ));
        }
        lines.push(Line::from(spans));

        if is_selected {
            if let Some(cap) = state.capture.as_ref() {
                if let Some(bad) = cap.last_invalid.as_deref() {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            format!("✗ `{bad}` cannot be a binding"),
                            Style::default().fg(ratatui::style::Color::Red),
                        ),
                    ]));
                }
            }
        }
    }
    frame.render_widget(Paragraph::new(lines), area);
}

fn scroll_offset(cursor: usize, visible: usize, total: usize) -> usize {
    if total <= visible {
        return 0;
    }
    let margin = (visible / 4).max(1);
    let max_scroll = total.saturating_sub(visible);
    if cursor < margin {
        0
    } else if cursor + margin >= total {
        max_scroll
    } else {
        cursor.saturating_sub(visible / 2).min(max_scroll)
    }
}

fn render_footer(frame: &mut Frame, area: Rect, state: &SettingsPageState) {
    let mut spans: Vec<Span> = vec![
        chip(" Tab "),
        Span::styled(" section  ", Style::default().fg(palette::muted())),
        chip(" j/k "),
        Span::styled(" move  ", Style::default().fg(palette::muted())),
        chip(" Enter "),
        Span::styled(" rebind  ", Style::default().fg(palette::muted())),
        chip(" r "),
        Span::styled(" reset  ", Style::default().fg(palette::muted())),
        chip(" Esc "),
        Span::styled(" close", Style::default().fg(palette::muted())),
    ];
    if let Some(err) = state.last_error.as_deref() {
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            format!("⚠ {err}"),
            Style::default().fg(ratatui::style::Color::Red),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn chip(label: &'static str) -> Span<'static> {
    Span::styled(
        label,
        Style::default()
            .fg(palette::accent())
            .add_modifier(Modifier::BOLD),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::CaptureState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::PathBuf;

    fn term(w: u16, h: u16) -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(w, h)).unwrap()
    }

    fn buffer_text(t: &Terminal<TestBackend>) -> String {
        let buf = t.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf.cell((x, y)).unwrap().symbol());
            }
            out.push('\n');
        }
        out
    }

    fn fresh_state() -> SettingsPageState {
        SettingsPageState::new(PathBuf::from("/tmp/cfg.toml"))
    }

    fn fake_rows() -> Vec<KeymapRowView> {
        vec![
            KeymapRowView {
                name: "copy".into(),
                display: "copy".into(),
                chord: "ctrl+c".into(),
                conflict_with: None,
            },
            KeymapRowView {
                name: "cut".into(),
                display: "cut".into(),
                chord: "ctrl+x".into(),
                conflict_with: Some("delete_back".into()),
            },
            KeymapRowView {
                name: "editor.toggle_mode".into(),
                display: "[ vim ↔ standard toggle ]".into(),
                chord: "alt+m".into(),
                conflict_with: None,
            },
        ]
    }

    fn fake_editor() -> Vec<EditorRowView> {
        vec![
            EditorRowView {
                label: "Editor mode".into(),
                value: "Standard".into(),
                hint: "Standard: arrows + Ctrl-Z/Y/C/V/S. Vim: full modal engine.".into(),
            },
            EditorRowView {
                label: "Mouse enabled".into(),
                value: "OFF".into(),
                hint: String::new(),
            },
        ]
    }

    fn fake_themes() -> Vec<ThemeRowView> {
        vec![
            ThemeRowView {
                name: "default-dark".into(),
                is_active: true,
            },
            ThemeRowView {
                name: "default-light".into(),
                is_active: false,
            },
            ThemeRowView {
                name: "terminal-native".into(),
                is_active: false,
            },
        ]
    }

    #[test]
    fn renders_section_tabs_with_active_marked() {
        let mut t = term(80, 12);
        let state = fresh_state();
        t.draw(|f| {
            render(
                f,
                Rect::new(0, 0, 80, 12),
                &state,
                &fake_rows(),
                &fake_themes(),
                &fake_editor(),
            )
        })
        .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("Keymaps"));
        assert!(text.contains("Theme"));
        assert!(text.contains("Editor"));
    }

    #[test]
    fn renders_keymap_rows_with_chord_column() {
        let mut t = term(80, 12);
        let state = fresh_state();
        t.draw(|f| {
            render(
                f,
                Rect::new(0, 0, 80, 12),
                &state,
                &fake_rows(),
                &fake_themes(),
                &fake_editor(),
            )
        })
        .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("copy"));
        assert!(text.contains("ctrl+c"));
        assert!(text.contains("alt+m"));
    }

    #[test]
    fn renders_conflict_warning_on_a_row_with_duplicate_chord() {
        let mut t = term(80, 12);
        let state = fresh_state();
        t.draw(|f| {
            render(
                f,
                Rect::new(0, 0, 80, 12),
                &state,
                &fake_rows(),
                &fake_themes(),
                &fake_editor(),
            )
        })
        .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("also bound to"));
        assert!(text.contains("delete_back"));
    }

    #[test]
    fn renders_capture_prompt_on_selected_row_in_capture_mode() {
        let mut t = term(80, 12);
        let mut state = fresh_state();
        state.keymap_cursor = 0; // copy
        state.capture = Some(CaptureState {
            action_name: "copy".into(),
            conflict_with: None,
            last_invalid: None,
        });
        t.draw(|f| {
            render(
                f,
                Rect::new(0, 0, 80, 12),
                &state,
                &fake_rows(),
                &fake_themes(),
                &fake_editor(),
            )
        })
        .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("press the new chord"));
        assert!(text.contains("Esc to cancel"));
    }

    #[test]
    fn renders_invalid_capture_hint_under_selected_row() {
        let mut t = term(80, 12);
        let mut state = fresh_state();
        state.keymap_cursor = 0;
        state.capture = Some(CaptureState {
            action_name: "copy".into(),
            conflict_with: None,
            last_invalid: Some("KeyEvent { Null }".into()),
        });
        t.draw(|f| {
            render(
                f,
                Rect::new(0, 0, 80, 12),
                &state,
                &fake_rows(),
                &fake_themes(),
                &fake_editor(),
            )
        })
        .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("cannot be a binding"));
    }

    #[test]
    fn renders_footer_hints_and_last_error() {
        // Wider buffer so the chord-hint chain + the error message
        // both fit on the single-row footer.
        let mut t = term(120, 12);
        let mut state = fresh_state();
        state.last_error = Some("disk full".into());
        t.draw(|f| {
            render(
                f,
                Rect::new(0, 0, 120, 12),
                &state,
                &fake_rows(),
                &fake_themes(),
                &fake_editor(),
            )
        })
        .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("rebind"));
        assert!(text.contains("reset"));
        assert!(text.contains("close"));
        assert!(text.contains("disk full"), "missing error: {text}");
    }

    #[test]
    fn theme_section_lists_presets_with_active_marker() {
        let mut t = term(100, 14);
        let mut state = fresh_state();
        state.section = SettingsSection::Theme;
        t.draw(|f| {
            render(
                f,
                Rect::new(0, 0, 100, 14),
                &state,
                &fake_rows(),
                &fake_themes(),
                &fake_editor(),
            )
        })
        .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("default-dark"));
        assert!(text.contains("default-light"));
        assert!(text.contains("terminal-native"));
        assert!(text.contains("(active)"));
        assert!(text.contains("config.toml"));
    }

    #[test]
    fn theme_section_highlights_cursor_row() {
        let mut t = term(100, 14);
        let mut state = fresh_state();
        state.section = SettingsSection::Theme;
        state.theme_cursor = 1; // default-light
        t.draw(|f| {
            render(
                f,
                Rect::new(0, 0, 100, 14),
                &state,
                &fake_rows(),
                &fake_themes(),
                &fake_editor(),
            )
        })
        .unwrap();
        let buf = t.backend().buffer().clone();
        let mut found = false;
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = buf.cell((x, y)).unwrap();
                if cell.symbol() == "▶" {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "cursor caret missing from theme list");
    }

    #[test]
    fn editor_section_lists_rows_with_current_values() {
        let mut t = term(120, 14);
        let mut state = fresh_state();
        state.section = SettingsSection::Editor;
        t.draw(|f| {
            render(
                f,
                Rect::new(0, 0, 120, 14),
                &state,
                &fake_rows(),
                &fake_themes(),
                &fake_editor(),
            )
        })
        .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("Editor mode"));
        assert!(text.contains("Mouse enabled"));
        assert!(text.contains("Standard"));
        assert!(text.contains("OFF"));
        assert!(text.contains("toggle"));
    }

    #[test]
    fn empty_rows_render_is_a_noop_not_panic() {
        let mut t = term(80, 12);
        let state = fresh_state();
        // Defensive: a future bug could pass an empty slice; the
        // renderer must not panic.
        t.draw(|f| render(f, Rect::new(0, 0, 80, 12), &state, &[], &[], &[]))
            .unwrap();
    }

    #[test]
    fn scroll_offset_clamps_when_cursor_near_end() {
        // Pure helper test — visible window can't run off the end.
        assert_eq!(scroll_offset(0, 5, 20), 0);
        assert_eq!(scroll_offset(19, 5, 20), 15);
        // Tiny lists fit in the window — no scroll.
        assert_eq!(scroll_offset(2, 10, 5), 0);
    }
}
