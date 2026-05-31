//! V4 P2-P4 (2026-05-23): Vars + Envs page. Master-detail: envs
//! sidebar + vars table. Focus alterna com Tab.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{
    EnvFormState, EnvsPageState, EnvsPaneFocus, VarFormFocus, VarFormState,
};

const POPUP_WIDTH: u16 = 76;
const POPUP_HEIGHT: u16 = 30;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &EnvsPageState) -> Option<(u16, u16)> {
    let area = centered(editor_area, POPUP_WIDTH, POPUP_HEIGHT);
    let bg = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());
    fill(frame, area, bg);
    let title = if state.envs.is_empty() {
        " Variables · no envs ".to_string()
    } else {
        format!(
            " Variables · {} envs · {} vars ",
            state.envs.len(),
            state.vars.len()
        )
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title)
        .style(bg)
        .border_style(
            Style::default()
                .fg(crate::ui::palette::border())
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(rows[0]);
    render_env_list(frame, body[0], state);
    fill_col(frame, body[1], "│", crate::ui::palette::muted());
    // V4 P7: vars area = vars list (altura proporcional ao count) +
    // divisor + used-in panel. Sem gap morto quando há poucas vars.
    let vars_h = (state.vars.len() as u16 + 1).clamp(3, body[2].height.saturating_sub(8));
    let vars_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(vars_h),
            Constraint::Length(1), // divisor horizontal
            Constraint::Min(3),    // used-in panel
        ])
        .split(body[2]);
    render_var_table(frame, vars_split[0], state);
    fill_row(frame, vars_split[1], "─", crate::ui::palette::muted());
    super::var_uses_panel::render_var_uses_panel(frame, vars_split[2], state);
    render_hint(frame, rows[1], state.focus);
    None
}

/// Preenche uma linha horizontal com `sym` na cor `fg`.
pub(super) fn fill_row(frame: &mut Frame, area: Rect, sym: &str, fg: Color) {
    let buf = frame.buffer_mut();
    if area.height == 0 {
        return;
    }
    let y = area.y;
    for x in area.x..area.x.saturating_add(area.width) {
        if let Some(c) = buf.cell_mut((x, y)) {
            c.set_symbol(sym);
            c.set_style(Style::default().fg(fg).bg(crate::ui::palette::popup_bg()));
        }
    }
}

fn render_env_list(frame: &mut Frame, area: Rect, state: &EnvsPageState) {
    let focused = state.focus == EnvsPaneFocus::Envs;
    if state.envs.is_empty() {
        let p = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                " no envs yet",
                Style::default().fg(crate::ui::palette::muted()),
            )),
            Line::from(""),
            Line::from(Span::styled(
                " press n to add",
                Style::default()
                    .fg(crate::ui::palette::popup_border_accent())
                    .add_modifier(Modifier::ITALIC),
            )),
        ])
        .style(Style::default().bg(crate::ui::palette::popup_bg()))
        .wrap(Wrap { trim: false });
        frame.render_widget(p, area);
        return;
    }
    let items: Vec<ListItem> = state
        .envs
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let active = state.active.as_deref() == Some(e.name.as_str());
            let marker = if active { "●" } else { " " };
            let style_name = if active {
                Style::default()
                    .fg(crate::ui::palette::success())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(crate::ui::palette::foreground())
            };
            // Atalho numérico no fim da linha (só pros 9 primeiros);
            // largura = 24 cols - 1 highlight symbol - 4 chrome
            // (` ● ` + atalho ` N`) = 17 cols disponíveis pro nome.
            let name_truncated = truncate(&e.name, 17);
            let shortcut = if i < 9 {
                format!("{} ", i + 1)
            } else {
                "  ".to_string()
            };
            let padding = 17usize.saturating_sub(name_truncated.chars().count());
            ListItem::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(marker, Style::default().fg(crate::ui::palette::success())),
                Span::raw(" "),
                Span::styled(name_truncated, style_name),
                Span::raw(" ".repeat(padding)),
                Span::styled(
                    shortcut,
                    Style::default()
                        .fg(crate::ui::palette::accent())
                        .add_modifier(Modifier::DIM),
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
    ls.select(Some(state.selected_env));
    frame.render_stateful_widget(list, area, &mut ls);
}

fn render_var_table(frame: &mut Frame, area: Rect, state: &EnvsPageState) {
    let focused = state.focus == EnvsPaneFocus::Vars;
    if state.vars.is_empty() {
        let hint = if state.envs.is_empty() {
            "(create an env first)"
        } else {
            "(no vars · press n to add)"
        };
        let p = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                format!("  {hint}"),
                Style::default().fg(crate::ui::palette::muted()),
            )),
        ])
        .style(Style::default().bg(crate::ui::palette::popup_bg()));
        frame.render_widget(p, area);
        return;
    }
    let items: Vec<ListItem> = state
        .vars
        .iter()
        .map(|v| {
            let value_display = if v.is_secret {
                if v.value.is_empty() {
                    "•••• (keychain)".to_string()
                } else {
                    "•".repeat(v.value.chars().count().min(20))
                }
            } else {
                v.value.clone()
            };
            ListItem::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("{:<20}", truncate(&v.key, 20)),
                    Style::default().fg(crate::ui::palette::foreground()),
                ),
                Span::styled(
                    if v.is_secret { "🔒 " } else { "  " },
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

fn render_hint(frame: &mut Frame, area: Rect, focus: EnvsPaneFocus) {
    let body = match focus {
        EnvsPaneFocus::Envs => " Tab vars · j/k · 1-9 activate · a activate · n new · e edit · c clone · D delete · Esc ",
        EnvsPaneFocus::Vars => " Tab envs · j/k · n new var · e edit · D delete · Esc ",
    };
    let p = Paragraph::new(Span::styled(
        body,
        Style::default()
            .fg(crate::ui::palette::muted())
            .bg(crate::ui::palette::popup_bg())
            .add_modifier(Modifier::ITALIC),
    ));
    frame.render_widget(p, area);
}

pub(super) fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n.saturating_sub(1)).collect::<String>() + "…"
    }
}

pub(super) fn fill(frame: &mut Frame, area: Rect, style: Style) {
    let buf = frame.buffer_mut();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(c) = buf.cell_mut((x, y)) {
                c.set_symbol(" ");
                c.set_style(style);
            }
        }
    }
}

fn fill_col(frame: &mut Frame, area: Rect, sym: &str, fg: Color) {
    let buf = frame.buffer_mut();
    for y in area.y..area.y.saturating_add(area.height) {
        if let Some(c) = buf.cell_mut((area.x, y)) {
            c.set_symbol(sym);
            c.set_style(Style::default().fg(fg).bg(crate::ui::palette::popup_bg()));
        }
    }
}

pub(super) fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

// ---- forms + confirms ----

pub fn render_env_form(
    frame: &mut Frame,
    editor_area: Rect,
    state: &EnvFormState,
) -> Option<(u16, u16)> {
    let area = centered(editor_area, 50, 9);
    let bg = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());
    fill(frame, area, bg);
    let title = if state.editing.is_some() {
        " Rename env "
    } else {
        " New env "
    };
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
    let mut lines = vec![
        Line::from(Span::styled(
            "Name",
            Style::default()
                .fg(crate::ui::palette::popup_border_accent())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            if state.name.as_str().is_empty() {
                "(required)".to_string()
            } else {
                state.name.as_str().to_string()
            },
            if state.name.as_str().is_empty() {
                Style::default()
                    .fg(crate::ui::palette::muted())
                    .add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(crate::ui::palette::foreground())
            },
        )),
        Line::from(""),
    ];
    if let Some(err) = state.error.as_deref() {
        lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        " Enter save · Esc cancel",
        Style::default()
            .fg(crate::ui::palette::muted())
            .add_modifier(Modifier::ITALIC),
    )));
    let p = Paragraph::new(lines).style(bg).wrap(Wrap { trim: false });
    let inner_pad = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(1),
        height: inner.height,
    };
    frame.render_widget(p, inner_pad);
    let cx = inner_pad.x + state.name.cursor_col() as u16;
    Some((cx, inner_pad.y + 1))
}

pub fn render_var_form(
    frame: &mut Frame,
    editor_area: Rect,
    state: &VarFormState,
) -> Option<(u16, u16)> {
    let area = centered(editor_area, 60, 16);
    let bg = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());
    fill(frame, area, bg);
    let title = if state.editing.is_some() {
        format!(" Edit var · {} ", state.env_name)
    } else {
        format!(" New var in {} ", state.env_name)
    };
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

    let label = |t: &str, focused: bool| -> Line<'static> {
        Line::from(Span::styled(
            t.to_string(),
            if focused {
                Style::default()
                    .fg(crate::ui::palette::popup_border_accent())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(crate::ui::palette::muted())
            },
        ))
    };
    let value = |s: &str, hint: &str| -> Line<'static> {
        if s.is_empty() {
            Line::from(Span::styled(
                hint.to_string(),
                Style::default()
                    .fg(crate::ui::palette::muted())
                    .add_modifier(Modifier::ITALIC),
            ))
        } else {
            Line::from(Span::styled(
                s.to_string(),
                Style::default().fg(crate::ui::palette::foreground()),
            ))
        }
    };

    let key_focused = state.focus == VarFormFocus::Key;
    let val_focused = state.focus == VarFormFocus::Value;
    let sec_focused = state.focus == VarFormFocus::Secret;

    let mut lines = vec![
        label("Key", key_focused),
        value(state.key.as_str(), "(required)"),
        Line::from(""),
        label("Value", val_focused),
    ];
    // value (mask if secret)
    let v_text = if state.is_secret && !state.value.as_str().is_empty() {
        "•".repeat(state.value.as_str().chars().count())
    } else {
        state.value.as_str().to_string()
    };
    lines.push(value(&v_text, ""));
    lines.push(Line::from(""));
    let chip = if state.is_secret {
        " [x] secret "
    } else {
        " [ ] secret "
    };
    let chip_style = if sec_focused {
        Style::default()
            .fg(crate::ui::palette::popup_bg())
            .bg(crate::ui::palette::popup_border_accent())
            .add_modifier(Modifier::BOLD)
    } else if state.is_secret {
        Style::default()
            .fg(crate::ui::palette::popup_border_accent())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(crate::ui::palette::foreground())
    };
    lines.push(Line::from(vec![
        Span::styled(chip, chip_style),
        Span::styled(
            "  (space toggles)",
            Style::default()
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::ITALIC),
        ),
    ]));
    if let Some(err) = state.error.as_deref() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Tab next · Enter save · Esc cancel",
        Style::default()
            .fg(crate::ui::palette::muted())
            .add_modifier(Modifier::ITALIC),
    )));
    let p = Paragraph::new(lines).style(bg).wrap(Wrap { trim: false });
    let inner_pad = Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(1),
        height: inner.height,
    };
    frame.render_widget(p, inner_pad);

    match state.focus {
        VarFormFocus::Key => Some((inner_pad.x + state.key.cursor_col() as u16, inner_pad.y + 1)),
        VarFormFocus::Value => Some((
            inner_pad.x + state.value.cursor_col() as u16,
            inner_pad.y + 4,
        )),
        VarFormFocus::Secret => None,
    }
}

// V4 P5: render_env_clone_form e render_clone_var_list moved to
// `ui/envs_clone.rs` (size limit). Env/var delete confirms moved to the
// generic `ui/confirm_prompt.rs`.

#[allow(dead_code)]
fn confirm_popup(frame: &mut Frame, editor_area: Rect, title: &str, prompt: &str) {
    let area = centered(editor_area, 56, 7);
    let bg = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());
    fill(frame, area, bg);
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .title(title.to_string())
        .style(bg)
        .border_style(
            Style::default()
                .fg(Color::LightRed)
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {prompt}"),
            Style::default()
                .fg(crate::ui::palette::popup_border_accent())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                " y / Enter ",
                Style::default()
                    .fg(crate::ui::palette::popup_bg())
                    .bg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " delete    ",
                Style::default().fg(crate::ui::palette::muted()),
            ),
            Span::styled(
                " n / Esc ",
                Style::default()
                    .fg(crate::ui::palette::foreground())
                    .bg(crate::ui::palette::muted())
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " cancel",
                Style::default().fg(crate::ui::palette::foreground()),
            ),
        ]),
    ];
    let p = Paragraph::new(lines).style(bg).wrap(Wrap { trim: false });
    frame.render_widget(p, inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn hint_envs_focus_mentions_used_in_keys() {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_hint(f, Rect::new(0, 0, 80, 1), EnvsPaneFocus::Envs);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let line: String = (0..80)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect();
        assert!(line.contains("1-9 activate"), "hint envs sem 1-9: {line:?}");
    }

    #[test]
    fn hint_envs_focus_mentions_clone() {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_hint(f, Rect::new(0, 0, 80, 1), EnvsPaneFocus::Envs);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let line: String = (0..80)
            .map(|x| buf.cell((x, 0)).unwrap().symbol().to_string())
            .collect();
        assert!(
            line.contains("c clone"),
            "hint envs deve conter 'c clone': {line:?}"
        );
    }

    use crate::app::{EnvSummary, VarRow};

    fn term(w: u16, h: u16) -> Terminal<TestBackend> {
        Terminal::new(TestBackend::new(w, h)).unwrap()
    }

    fn dump(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer().clone();
        let mut out = String::new();
        for y in 0..buf.area().height {
            for x in 0..buf.area().width {
                if let Some(c) = buf.cell((x, y)) {
                    out.push_str(c.symbol());
                }
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn render_envs_page_empty_shows_no_envs_yet() {
        let mut t = term(100, 40);
        let state = EnvsPageState::default();
        t.draw(|f| {
            render(f, Rect::new(0, 0, 100, 40), &state);
        })
        .unwrap();
        let frame = dump(&t);
        assert!(
            frame.contains("no envs"),
            "expected 'no envs' hint, got\n{frame}"
        );
        assert!(frame.contains("Variables"), "expected title chrome");
    }

    #[test]
    fn render_envs_page_with_data_lists_envs_and_vars() {
        let mut t = term(100, 40);
        let state = EnvsPageState {
            envs: vec![
                EnvSummary { name: "dev".into() },
                EnvSummary {
                    name: "prod".into(),
                },
            ],
            active: Some("dev".into()),
            selected_env: 0,
            vars: vec![
                VarRow {
                    key: "API_KEY".into(),
                    value: "abc".into(),
                    is_secret: false,
                },
                VarRow {
                    key: "SECRET".into(),
                    value: "xyz".into(),
                    is_secret: true,
                },
            ],
            selected_var: 0,
            focus: EnvsPaneFocus::Envs,
            var_uses: Vec::new(),
        };
        t.draw(|f| {
            render(f, Rect::new(0, 0, 100, 40), &state);
        })
        .unwrap();
        let frame = dump(&t);
        assert!(frame.contains("dev"), "expected env 'dev' in output");
        assert!(frame.contains("API_KEY"), "expected var key in output");
        // Secret var value should be masked.
        assert!(!frame.contains("xyz"), "secret value leaked: {frame}");
    }

    #[test]
    fn render_envs_page_with_no_envs_var_message() {
        let mut t = term(100, 40);
        let state = EnvsPageState {
            envs: Vec::new(),
            vars: Vec::new(),
            ..EnvsPageState::default()
        };
        t.draw(|f| {
            render(f, Rect::new(0, 0, 100, 40), &state);
        })
        .unwrap();
        let frame = dump(&t);
        assert!(
            frame.contains("create an env first"),
            "expected env-first hint"
        );
    }

    #[test]
    fn render_envs_page_with_envs_but_no_vars_shows_press_n_hint() {
        let mut t = term(100, 40);
        let state = EnvsPageState {
            envs: vec![EnvSummary { name: "dev".into() }],
            active: None,
            vars: Vec::new(),
            focus: EnvsPaneFocus::Vars,
            ..EnvsPageState::default()
        };
        t.draw(|f| {
            render(f, Rect::new(0, 0, 100, 40), &state);
        })
        .unwrap();
        let frame = dump(&t);
        assert!(frame.contains("press n to add"), "expected vars-empty hint");
    }

    #[test]
    fn render_env_form_new_and_edit_titles() {
        let mut t = term(100, 30);
        let mut state = EnvFormState::default();
        t.draw(|f| {
            render_env_form(f, Rect::new(0, 0, 100, 30), &state);
        })
        .unwrap();
        let frame = dump(&t);
        assert!(
            frame.contains("New env"),
            "expected new-env title, got\n{frame}"
        );
        assert!(frame.contains("required"), "expected placeholder hint");

        state.editing = Some("dev".into());
        state.name = crate::vim::lineedit::LineEdit::from_str("dev".to_string());
        state.error = Some("name taken".into());
        let mut t = term(100, 30);
        t.draw(|f| {
            render_env_form(f, Rect::new(0, 0, 100, 30), &state);
        })
        .unwrap();
        let frame = dump(&t);
        assert!(frame.contains("Rename env"), "expected rename title");
        assert!(frame.contains("name taken"), "expected error in frame");
    }

    #[test]
    fn render_var_form_new_with_secret_focus_and_value() {
        let mut t = term(100, 30);
        let state = VarFormState {
            env_name: "dev".into(),
            key: crate::vim::lineedit::LineEdit::from_str("API_KEY"),
            value: crate::vim::lineedit::LineEdit::from_str("abc"),
            is_secret: true,
            focus: VarFormFocus::Secret,
            editing: None,
            error: Some("oops".into()),
        };
        t.draw(|f| {
            let cursor = render_var_form(f, Rect::new(0, 0, 100, 30), &state);
            assert!(cursor.is_none(), "Secret focus has no cursor");
        })
        .unwrap();
        let frame = dump(&t);
        assert!(frame.contains("New var in dev"), "expected new-var title");
        assert!(frame.contains("API_KEY"), "expected key");
        assert!(!frame.contains("abc"), "secret value should be masked");
        assert!(frame.contains("oops"), "expected error message");
        assert!(frame.contains("[x] secret"), "expected toggled chip");
    }

    #[test]
    fn render_var_form_edit_focus_key_returns_cursor() {
        let mut t = term(100, 30);
        let state = VarFormState {
            env_name: "dev".into(),
            key: crate::vim::lineedit::LineEdit::from_str("API"),
            value: crate::vim::lineedit::LineEdit::new(),
            is_secret: false,
            focus: VarFormFocus::Key,
            editing: Some("API".into()),
            error: None,
        };
        t.draw(|f| {
            let cursor = render_var_form(f, Rect::new(0, 0, 100, 30), &state);
            assert!(cursor.is_some(), "Key focus must return cursor pos");
        })
        .unwrap();
        let frame = dump(&t);
        assert!(frame.contains("Edit var · dev"), "expected edit title");
        assert!(frame.contains("[ ] secret"), "expected un-toggled chip");
    }

    #[test]
    fn render_var_form_focus_value_returns_cursor() {
        let mut t = term(100, 30);
        let state = VarFormState {
            env_name: "dev".into(),
            key: crate::vim::lineedit::LineEdit::new(),
            value: crate::vim::lineedit::LineEdit::from_str("x"),
            focus: VarFormFocus::Value,
            ..VarFormState::default()
        };
        t.draw(|f| {
            let cursor = render_var_form(f, Rect::new(0, 0, 100, 30), &state);
            assert!(cursor.is_some(), "Value focus returns cursor");
        })
        .unwrap();
    }


    #[test]
    fn truncate_under_and_over_limit() {
        assert_eq!(truncate("abc", 5), "abc");
        assert_eq!(truncate("abcdef", 4).chars().count(), 4);
        assert!(truncate("abcdef", 4).ends_with('…'));
    }

    #[test]
    fn centered_clamps_to_area() {
        let r = centered(Rect::new(0, 0, 30, 10), 100, 100);
        assert!(r.width <= 28);
        assert!(r.height <= 8);
    }

    #[test]
    fn fill_row_with_zero_height_is_noop() {
        let mut t = term(20, 5);
        t.draw(|f| {
            fill_row(f, Rect::new(0, 0, 20, 0), "─", crate::ui::palette::muted());
        })
        .unwrap();
    }

    #[test]
    fn render_envs_page_truncates_long_env_names_and_marks_active() {
        let mut t = term(100, 40);
        let state = EnvsPageState {
            envs: vec![
                EnvSummary {
                    name: "very-long-env-name-that-exceeds-17-chars".into(),
                },
                EnvSummary {
                    name: "short".into(),
                },
            ],
            active: Some("short".into()),
            ..EnvsPageState::default()
        };
        t.draw(|f| {
            render(f, Rect::new(0, 0, 100, 40), &state);
        })
        .unwrap();
        let frame = dump(&t);
        // truncated name appears with the ellipsis
        assert!(
            frame.contains("…"),
            "expected truncation marker, got\n{frame}"
        );
        // active env shows the marker
        assert!(frame.contains("●"), "expected active marker");
    }
}
