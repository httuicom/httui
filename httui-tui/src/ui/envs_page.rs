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
    EnvDeleteConfirmState, EnvFormState, EnvsPageState, EnvsPaneFocus, VarDeleteConfirmState,
    VarFormFocus, VarFormState,
};

const POPUP_WIDTH: u16 = 76;
const POPUP_HEIGHT: u16 = 30;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &EnvsPageState) -> Option<(u16, u16)> {
    let area = centered(editor_area, POPUP_WIDTH, POPUP_HEIGHT);
    let bg = Style::default().bg(Color::Black).fg(Color::White);
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
        .title(title)
        .style(bg)
        .border_style(Style::default().fg(Color::LightMagenta).bg(Color::Black));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(24), Constraint::Length(1), Constraint::Min(0)])
        .split(rows[0]);
    render_env_list(frame, body[0], state);
    fill_col(frame, body[1], "│", Color::DarkGray);
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
    fill_row(frame, vars_split[1], "─", Color::DarkGray);
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
            c.set_style(Style::default().fg(fg).bg(Color::Black));
        }
    }
}

fn render_env_list(frame: &mut Frame, area: Rect, state: &EnvsPageState) {
    let focused = state.focus == EnvsPaneFocus::Envs;
    if state.envs.is_empty() {
        let p = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(" no envs yet", Style::default().fg(Color::DarkGray))),
            Line::from(""),
            Line::from(Span::styled(
                " press n to add",
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::ITALIC),
            )),
        ])
        .style(Style::default().bg(Color::Black))
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
                Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
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
                Span::styled(marker, Style::default().fg(Color::LightGreen)),
                Span::raw(" "),
                Span::styled(name_truncated, style_name),
                Span::raw(" ".repeat(padding)),
                Span::styled(
                    shortcut,
                    Style::default()
                        .fg(Color::LightMagenta)
                        .add_modifier(Modifier::DIM),
                ),
            ]))
        })
        .collect();
    let highlight = if focused {
        Style::default()
            .bg(crate::ui::palette::SELECTION_BG)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let list = List::new(items)
        .style(Style::default().bg(Color::Black))
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
            Line::from(Span::styled(format!("  {hint}"), Style::default().fg(Color::DarkGray))),
        ])
        .style(Style::default().bg(Color::Black));
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
                    Style::default().fg(Color::White),
                ),
                Span::styled(
                    if v.is_secret { "🔒 " } else { "  " },
                    Style::default().fg(Color::LightYellow),
                ),
                Span::styled(value_display, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();
    let highlight = if focused {
        Style::default()
            .bg(crate::ui::palette::SELECTION_BG)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let list = List::new(items)
        .style(Style::default().bg(Color::Black))
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
            .fg(Color::DarkGray)
            .bg(Color::Black)
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
            c.set_style(Style::default().fg(fg).bg(Color::Black));
        }
    }
}

pub(super) fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(2));
    let h = h.min(area.height.saturating_sub(2));
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect { x, y, width: w, height: h }
}

// ---- forms + confirms ----

pub fn render_env_form(frame: &mut Frame, editor_area: Rect, state: &EnvFormState) -> Option<(u16, u16)> {
    let area = centered(editor_area, 50, 9);
    let bg = Style::default().bg(Color::Black).fg(Color::White);
    fill(frame, area, bg);
    let title = if state.editing.is_some() { " Rename env " } else { " New env " };
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg)
        .border_style(Style::default().fg(Color::LightYellow).bg(Color::Black));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let mut lines = vec![
        Line::from(Span::styled(
            "Name",
            Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            if state.name.as_str().is_empty() {
                "(required)".to_string()
            } else {
                state.name.as_str().to_string()
            },
            if state.name.as_str().is_empty() {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)
            } else {
                Style::default().fg(Color::White)
            },
        )),
        Line::from(""),
    ];
    if let Some(err) = state.error.as_deref() {
        lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        " Enter save · Esc cancel",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
    )));
    let p = Paragraph::new(lines).style(bg).wrap(Wrap { trim: false });
    let inner_pad = Rect { x: inner.x + 1, y: inner.y, width: inner.width.saturating_sub(1), height: inner.height };
    frame.render_widget(p, inner_pad);
    let cx = inner_pad.x + state.name.cursor_col() as u16;
    Some((cx, inner_pad.y + 1))
}

pub fn render_var_form(frame: &mut Frame, editor_area: Rect, state: &VarFormState) -> Option<(u16, u16)> {
    let area = centered(editor_area, 60, 16);
    let bg = Style::default().bg(Color::Black).fg(Color::White);
    fill(frame, area, bg);
    let title = if state.editing.is_some() {
        format!(" Edit var · {} ", state.env_name)
    } else {
        format!(" New var in {} ", state.env_name)
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg)
        .border_style(Style::default().fg(Color::LightYellow).bg(Color::Black));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let label = |t: &str, focused: bool| -> Line<'static> {
        Line::from(Span::styled(
            t.to_string(),
            if focused {
                Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ))
    };
    let value = |s: &str, hint: &str| -> Line<'static> {
        if s.is_empty() {
            Line::from(Span::styled(
                hint.to_string(),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            ))
        } else {
            Line::from(Span::styled(s.to_string(), Style::default().fg(Color::White)))
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
    let chip = if state.is_secret { " [x] secret " } else { " [ ] secret " };
    let chip_style = if sec_focused {
        Style::default().fg(Color::Black).bg(Color::LightYellow).add_modifier(Modifier::BOLD)
    } else if state.is_secret {
        Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    lines.push(Line::from(vec![
        Span::styled(chip, chip_style),
        Span::styled(
            "  (space toggles)",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        ),
    ]));
    if let Some(err) = state.error.as_deref() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("error: {err}"),
            Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " Tab next · Enter save · Esc cancel",
        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
    )));
    let p = Paragraph::new(lines).style(bg).wrap(Wrap { trim: false });
    let inner_pad = Rect { x: inner.x + 1, y: inner.y, width: inner.width.saturating_sub(1), height: inner.height };
    frame.render_widget(p, inner_pad);

    match state.focus {
        VarFormFocus::Key => Some((inner_pad.x + state.key.cursor_col() as u16, inner_pad.y + 1)),
        VarFormFocus::Value => Some((inner_pad.x + state.value.cursor_col() as u16, inner_pad.y + 4)),
        VarFormFocus::Secret => None,
    }
}

pub fn render_env_delete_confirm(frame: &mut Frame, area: Rect, state: &EnvDeleteConfirmState) {
    confirm_popup(frame, area, &format!(" Delete env "), &format!("Delete env \"{}\"?", state.name));
}

pub fn render_var_delete_confirm(frame: &mut Frame, area: Rect, state: &VarDeleteConfirmState) {
    confirm_popup(
        frame,
        area,
        &format!(" Delete var "),
        &format!("Delete \"{}\" from {}?", state.key, state.env_name),
    );
}

// V4 P5: render_env_clone_form e render_clone_var_list moved to
// `ui/envs_clone.rs` (size limit).

fn confirm_popup(frame: &mut Frame, editor_area: Rect, title: &str, prompt: &str) {
    let area = centered(editor_area, 56, 7);
    let bg = Style::default().bg(Color::Black).fg(Color::White);
    fill(frame, area, bg);
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title.to_string())
        .style(bg)
        .border_style(Style::default().fg(Color::LightRed).bg(Color::Black));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {prompt}"),
            Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                " y / Enter ",
                Style::default().fg(Color::Black).bg(Color::LightRed).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" delete    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                " n / Esc ",
                Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" cancel", Style::default().fg(Color::White)),
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
        assert!(line.contains("c clone"), "hint envs deve conter 'c clone': {line:?}");
    }
}
