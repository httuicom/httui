//! Centered branch-picker popup. Lists local + remote-tracking
//! branches with the current one marked; Enter checks out the
//! highlighted entry.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::git::GitBranchPickerState;

pub fn render(frame: &mut Frame, editor_area: Rect, state: &GitBranchPickerState) {
    let width = 50.min(editor_area.width.saturating_sub(4));
    let max_rows = (state.branches.len() as u16 + 4).max(6);
    let height = max_rows.min(editor_area.height.saturating_sub(2)).max(6);
    let x = editor_area.x + (editor_area.width.saturating_sub(width)) / 2;
    let y = editor_area.y + (editor_area.height.saturating_sub(height)) / 2;
    let area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::LightYellow))
        .title(Span::styled(
            " branches ",
            Style::default()
                .fg(Color::LightYellow)
                .add_modifier(Modifier::BOLD),
        ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let error_height = if state.error.is_some() { 1u16 } else { 0 };
    let list_height = inner.height.saturating_sub(error_height);
    let list_area = Rect::new(inner.x, inner.y, inner.width, list_height);

    let items: Vec<ListItem<'static>> = state
        .branches
        .iter()
        .map(|b| {
            let marker = if b.current { "* " } else { "  " };
            let glyph_color = if b.current {
                Color::LightGreen
            } else {
                Color::Gray
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    marker,
                    Style::default()
                        .fg(glyph_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(b.name.clone()),
            ]))
        })
        .collect();
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );
    let mut list_state = ListState::default();
    if !state.branches.is_empty() {
        list_state.select(Some(state.selected.min(state.branches.len() - 1)));
    }
    frame.render_stateful_widget(list, list_area, &mut list_state);

    if let Some(err) = state.error.as_ref() {
        let err_area = Rect::new(
            inner.x,
            inner.y + list_height,
            inner.width,
            error_height,
        );
        let paragraph = Paragraph::new(Line::from(Span::styled(
            err.clone(),
            Style::default().fg(Color::Red),
        )));
        frame.render_widget(paragraph, err_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::git::status::BranchInfo;
    use ratatui::{backend::TestBackend, Terminal};

    fn state_with(branches: &[(&str, bool, bool)]) -> GitBranchPickerState {
        let bs: Vec<BranchInfo> = branches
            .iter()
            .map(|(name, current, remote)| BranchInfo {
                name: (*name).to_string(),
                current: *current,
                remote: *remote,
            })
            .collect();
        GitBranchPickerState::new(bs)
    }

    fn render_to_text(state: &GitBranchPickerState, w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, f.area(), state)).unwrap();
        let buf = term.backend().buffer();
        let mut out = String::new();
        for y in 0..h {
            for x in 0..w {
                out.push_str(buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn renders_branches_with_current_marker() {
        let state = state_with(&[
            ("main", true, false),
            ("feature/x", false, false),
            ("origin/main", false, true),
        ]);
        let text = render_to_text(&state, 60, 10);
        assert!(text.contains("main"));
        assert!(text.contains("feature/x"));
        assert!(text.contains("origin/main"));
        assert!(text.contains("* "), "current marker painted: {text:?}");
    }

    #[test]
    fn renders_error_at_bottom_when_present() {
        let mut state = state_with(&[("main", true, false)]);
        state.error = Some("error: switch failed".into());
        let text = render_to_text(&state, 60, 10);
        assert!(text.contains("error: switch failed"));
    }

    #[test]
    fn move_cursor_wraps_around() {
        let mut s = state_with(&[
            ("a", true, false),
            ("b", false, false),
            ("c", false, false),
        ]);
        // `new()` set selected to the current branch — index 0 here.
        assert_eq!(s.selected, 0);
        s.move_cursor(-1);
        assert_eq!(s.selected, 2, "wraps backward");
        s.move_cursor(1);
        assert_eq!(s.selected, 0, "wraps forward");
    }

    #[test]
    fn new_selects_current_branch_if_present() {
        let s = state_with(&[
            ("a", false, false),
            ("b", true, false),
            ("c", false, false),
        ]);
        assert_eq!(s.selected, 1);
    }

    #[test]
    fn new_defaults_to_first_when_no_current() {
        let s = state_with(&[("a", false, false), ("b", false, false)]);
        assert_eq!(s.selected, 0);
    }
}
