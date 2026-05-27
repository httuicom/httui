//! Top tab bar. Renders one row showing every open tab; the active
//! one stands out via reversed colors.
//!
//! Layout note: the bar is only drawn when more than one tab is open;
//! a single-tab session keeps the editor full-height.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    symbols,
    text::Line,
    widgets::Tabs,
    Frame,
};

use crate::app::TabBar;

pub fn render(frame: &mut Frame, area: Rect, tabs: &TabBar) {
    let titles: Vec<Line<'static>> = tabs
        .tabs
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let leaf = tab.active_leaf();
            let name = leaf
                .document_path
                .as_ref()
                .map(|p| {
                    p.file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| p.display().to_string())
                })
                .unwrap_or_else(|| "(no name)".into());
            // Trailing `*` when the focused doc has unsaved edits —
            // mirrors the `tab_picker` markers and the dirty marker
            // already painted in the status bar's file segment, so
            // the user has consistent dirty signaling everywhere.
            let dirty = leaf.document.as_ref().is_some_and(|d| d.is_dirty());
            let dirty_marker = if dirty { "*" } else { "" };
            Line::from(format!(" {} {}{} ", i + 1, name, dirty_marker))
        })
        .collect();

    let widget = Tabs::new(titles)
        .select(tabs.active())
        .style(Style::default().fg(crate::ui::palette::muted()))
        .highlight_style(
            Style::default()
                .fg(crate::ui::palette::popup_bg())
                .bg(Color::LightCyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(symbols::line::VERTICAL);
    frame.render_widget(widget, area);
}
