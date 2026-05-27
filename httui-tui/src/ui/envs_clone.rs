//! V4 P5 (2026-05-23): clone-env form renderer. Extraído de
//! `ui/envs_page.rs` pra respeitar size limit do DoD. Reusa
//! `truncate`/`fill`/`centered` do `envs_page` (pub(super)).

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{EnvCloneFormFocus, EnvCloneFormState};

use super::envs_page::{centered, fill, truncate};

/// Clone-env form. Layout = nome (1 linha) + checklist de vars
/// (scroll); focus visible no campo ativo. Retorna posição do cursor
/// quando foco está em Name (terminal-native blink).
pub fn render_env_clone_form(
    frame: &mut Frame,
    editor_area: Rect,
    state: &EnvCloneFormState,
) -> Option<(u16, u16)> {
    let area = centered(editor_area, 64, 22);
    let bg = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());
    fill(frame, area, bg);
    let title = format!(" Clone env · from {} ", state.source);
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title)
        .style(bg)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::popup_border_accent())
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let inner_pad = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(1),
        height: inner.height,
    };

    let name_focused = state.focus == EnvCloneFormFocus::Name;
    let vars_focused = state.focus == EnvCloneFormFocus::Vars;

    let name_label = Line::from(Span::styled(
        "New env name",
        if name_focused {
            Style::default()
                .fg(crate::ui::palette::popup_border_accent())
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(crate::ui::palette::muted())
        },
    ));
    let name_value = if state.name.as_str().is_empty() {
        Line::from(Span::styled(
            "(required)",
            Style::default()
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::ITALIC),
        ))
    } else {
        Line::from(Span::styled(
            state.name.as_str().to_string(),
            Style::default().fg(crate::ui::palette::foreground()),
        ))
    };

    let total = state.vars.len();
    let checked = state.vars.iter().filter(|v| v.checked).count();
    let vars_header = Line::from(vec![
        Span::styled(
            "Copy variables",
            if vars_focused {
                Style::default()
                    .fg(crate::ui::palette::popup_border_accent())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(crate::ui::palette::muted())
            },
        ),
        Span::styled(
            format!("  {checked}/{total} selected"),
            Style::default().fg(crate::ui::palette::muted()),
        ),
    ]);

    let header_lines = vec![name_label, name_value, Line::from(""), vars_header];
    let header_area = Rect {
        x: inner_pad.x,
        y: inner_pad.y,
        width: inner_pad.width,
        height: 4,
    };
    let header_p = Paragraph::new(header_lines)
        .style(bg)
        .wrap(Wrap { trim: false });
    frame.render_widget(header_p, header_area);

    let footer_h: u16 = if state.error.is_some() { 3 } else { 2 };
    let list_y = inner_pad.y.saturating_add(4);
    let list_h = inner_pad.height.saturating_sub(4).saturating_sub(footer_h);
    let list_area = Rect {
        x: inner_pad.x,
        y: list_y,
        width: inner_pad.width,
        height: list_h,
    };
    render_clone_var_list(frame, list_area, state, vars_focused);

    let mut footer_lines: Vec<Line<'static>> = Vec::new();
    if let Some(err) = state.error.as_deref() {
        footer_lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )));
    }
    footer_lines.push(Line::from(Span::styled(
        " Tab next · space toggle · a all/none · Enter clone · Esc cancel",
        Style::default()
            .fg(crate::ui::palette::muted())
            .add_modifier(Modifier::ITALIC),
    )));
    let footer_area = Rect {
        x: inner_pad.x,
        y: list_y.saturating_add(list_h),
        width: inner_pad.width,
        height: footer_h,
    };
    let footer_p = Paragraph::new(footer_lines)
        .style(bg)
        .wrap(Wrap { trim: false });
    frame.render_widget(footer_p, footer_area);

    if name_focused {
        Some((
            inner_pad.x + state.name.cursor_col() as u16,
            inner_pad.y + 1,
        ))
    } else {
        None
    }
}

fn render_clone_var_list(frame: &mut Frame, area: Rect, state: &EnvCloneFormState, focused: bool) {
    if state.vars.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            "  (source env has no vars)",
            Style::default()
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::ITALIC),
        )))
        .style(Style::default().bg(crate::ui::palette::popup_bg()));
        frame.render_widget(p, area);
        return;
    }
    let items: Vec<ListItem> = state
        .vars
        .iter()
        .map(|v| {
            let mark = if v.checked { "[x] " } else { "[ ] " };
            let secret = if v.is_secret { "🔒 " } else { "  " };
            let value_display = if v.is_secret {
                "••••".to_string()
            } else {
                truncate(&v.value, 24)
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    mark.to_string(),
                    Style::default()
                        .fg(if v.checked {
                            crate::ui::palette::success()
                        } else {
                            crate::ui::palette::muted()
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{:<20}", truncate(&v.key, 20)),
                    Style::default().fg(crate::ui::palette::foreground()),
                ),
                Span::styled(
                    secret,
                    Style::default().fg(crate::ui::palette::popup_border_accent()),
                ),
                Span::styled(
                    value_display,
                    Style::default().fg(crate::ui::palette::muted()),
                ),
            ]))
        })
        .collect();
    let highlight = if focused {
        Style::default()
            .bg(crate::ui::palette::selection_bg())
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let list = List::new(items)
        .style(Style::default().bg(crate::ui::palette::popup_bg()))
        .highlight_style(highlight)
        .highlight_symbol(if focused { "▌" } else { " " });
    let mut ls = ListState::default();
    ls.select(Some(state.selected_var));
    frame.render_stateful_widget(list, area, &mut ls);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::CloneVarRow;
    use crate::vim::lineedit::LineEdit;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn render_clone(state: &EnvCloneFormState, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_env_clone_form(f, Rect::new(0, 0, w, h), state);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..h)
            .map(|y| {
                let line: String = (0..w)
                    .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                    .collect();
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn state_with_vars(
        source: &str,
        name: &str,
        vars: Vec<(&str, &str, bool, bool)>,
    ) -> EnvCloneFormState {
        EnvCloneFormState {
            source: source.into(),
            name: LineEdit::from_str(name),
            vars: vars
                .into_iter()
                .map(|(k, v, secret, checked)| CloneVarRow {
                    key: k.into(),
                    value: v.into(),
                    is_secret: secret,
                    checked,
                })
                .collect(),
            selected_var: 0,
            focus: EnvCloneFormFocus::Name,
            error: None,
        }
    }

    #[test]
    fn clone_render_shows_source_and_target_name() {
        let s = state_with_vars(
            "staging",
            "staging-copy",
            vec![("API_URL", "https://example.com", false, true)],
        );
        let text = render_clone(&s, 80, 22);
        assert!(text.contains("Clone env"));
        assert!(text.contains("staging"));
        assert!(text.contains("staging-copy"));
        assert!(text.contains("New env name"));
        assert!(text.contains("Copy variables"));
    }

    #[test]
    fn clone_render_shows_check_counts() {
        let s = state_with_vars(
            "src",
            "dst",
            vec![
                ("A", "1", false, true),
                ("B", "2", false, false),
                ("C", "3", false, true),
            ],
        );
        let text = render_clone(&s, 80, 22);
        assert!(text.contains("2/3 selected"));
        assert!(text.contains("[x] "));
        assert!(text.contains("[ ] "));
    }

    #[test]
    fn clone_render_empty_vars_shows_hint() {
        let s = state_with_vars("src", "dst-copy", Vec::new());
        let text = render_clone(&s, 70, 22);
        assert!(text.contains("source env has no vars"));
        assert!(text.contains("0/0 selected"));
    }

    #[test]
    fn clone_render_required_placeholder_when_name_empty() {
        let s = state_with_vars("src", "", vec![("K", "V", false, true)]);
        let text = render_clone(&s, 70, 22);
        assert!(text.contains("(required)"));
    }

    #[test]
    fn clone_render_secret_value_is_masked() {
        let s = state_with_vars(
            "src",
            "dst",
            vec![("TOKEN", "super-secret-value", true, true)],
        );
        let text = render_clone(&s, 80, 22);
        assert!(!text.contains("super-secret-value"));
        assert!(text.contains("TOKEN"));
    }

    #[test]
    fn clone_render_error_line_shown() {
        let mut s = state_with_vars("src", "dst", vec![("K", "V", false, true)]);
        s.error = Some("name conflict".into());
        let text = render_clone(&s, 70, 22);
        assert!(text.contains("error: name conflict"));
    }

    #[test]
    fn clone_render_footer_hint_present() {
        let s = state_with_vars("src", "dst", vec![("K", "V", false, true)]);
        let text = render_clone(&s, 90, 22);
        assert!(text.contains("Tab next"));
        assert!(text.contains("space toggle"));
        assert!(text.contains("all/none"));
        assert!(text.contains("Enter clone"));
    }

    #[test]
    fn clone_render_smoke_small_area() {
        let s = state_with_vars("s", "d", vec![("X", "Y", false, true)]);
        let _ = render_clone(&s, 30, 10);
    }
}
