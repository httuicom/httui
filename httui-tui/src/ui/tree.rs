//! File-tree sidebar renderer. Two-column layout: indent guides per
//! depth + entry name. Folders are prefixed with `▾`/`▸` (expanded /
//! collapsed); files with two spaces.
//!
//! When `focused`, the title bar uses a brighter color and the
//! selected row is highlighted boldly so the user can see which pane
//! has keyboard focus.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::tree::FileTree;

const SIDEBAR_WIDTH: u16 = 30;

pub fn width() -> u16 {
    SIDEBAR_WIDTH
}

pub fn render(frame: &mut Frame, area: Rect, tree: &FileTree, focused: bool) {
    let (border_color, title_style) = if focused {
        (
            crate::ui::palette::popup_border_accent(),
            Style::default()
                .fg(crate::ui::palette::popup_border_accent())
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (
            crate::ui::palette::muted(),
            Style::default().fg(Color::Gray),
        )
    };
    let title = if tree.block_index.is_some() {
        " Blocks "
    } else {
        " Files "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(title, title_style));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let block_mode = tree.block_index.is_some();
    let items: Vec<ListItem> = tree
        .entries
        .iter()
        .map(|node| {
            let indent = "  ".repeat(node.depth);
            if let Some(meta) = node.block.as_ref() {
                let badge = block_badge(&meta.block_type);
                let badge_bg = if meta.block_type == "http" {
                    crate::ui::palette::accent()
                } else {
                    crate::ui::palette::popup_border_accent()
                };
                let line = Line::from(vec![
                    Span::raw(indent),
                    Span::raw("  "),
                    Span::styled(
                        badge,
                        Style::default()
                            .bg(badge_bg)
                            .fg(crate::ui::palette::popup_bg())
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(meta.label.clone(), Style::default()),
                ]);
                return ListItem::new(line);
            }
            let expandable = node.is_dir || block_mode;
            let icon = if expandable {
                if tree.expanded.contains(&node.path) {
                    "▾ "
                } else {
                    "▸ "
                }
            } else {
                "  "
            };
            let name_style = if node.is_dir {
                Style::default().fg(Color::LightCyan)
            } else {
                Style::default()
            };
            let line = Line::from(vec![
                Span::raw(indent),
                Span::styled(icon, Style::default().fg(crate::ui::palette::muted())),
                Span::styled(node.name.clone(), name_style),
            ]);
            ListItem::new(line)
        })
        .collect();

    let highlight_style = if focused {
        Style::default()
            .bg(Color::Yellow)
            .fg(crate::ui::palette::popup_bg())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .bg(crate::ui::palette::muted())
            .fg(crate::ui::palette::foreground())
    };

    let list = List::new(items)
        .highlight_style(highlight_style)
        .highlight_symbol("");
    let mut state = ListState::default();
    if !tree.entries.is_empty() {
        state.select(Some(tree.selected.min(tree.entries.len() - 1)));
    }
    frame.render_stateful_widget(list, inner, &mut state);
}

fn block_badge(block_type: &str) -> String {
    if block_type == "http" {
        " HTTP ".into()
    } else if block_type.starts_with("db") {
        " DB ".into()
    } else {
        format!(" {block_type} ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{FileTree, TreeNode};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

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

    fn fixture() -> FileTree {
        let mut tree = FileTree {
            entries: vec![
                TreeNode {
                    name: "notes".into(),
                    path: "notes".into(),
                    is_dir: true,
                    depth: 0,
                    block: None,
                },
                TreeNode {
                    name: "alpha.md".into(),
                    path: "notes/alpha.md".into(),
                    is_dir: false,
                    depth: 1,
                    block: None,
                },
                TreeNode {
                    name: "beta.md".into(),
                    path: "notes/beta.md".into(),
                    is_dir: false,
                    depth: 1,
                    block: None,
                },
            ],
            ..FileTree::default()
        };
        tree.expanded.insert("notes".into());
        tree
    }

    #[test]
    fn width_is_a_stable_constant() {
        assert_eq!(width(), 30);
    }

    #[test]
    fn renders_entries_with_files_block_title_and_dir_icon() {
        let mut t = term(30, 8);
        let tree = fixture();
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("Files"));
        assert!(text.contains("notes"));
        assert!(text.contains("alpha.md"));
        assert!(text.contains("beta.md"));
        // Expanded folder shows the down-pointing triangle.
        assert!(text.contains("▾"));
    }

    #[test]
    fn collapsed_dir_uses_right_pointing_icon() {
        let mut t = term(30, 8);
        let mut tree = fixture();
        tree.expanded.clear();
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("▸"), "collapsed icon missing: {text}");
    }

    #[test]
    fn empty_tree_renders_without_panic() {
        let mut t = term(30, 6);
        let tree = FileTree::default();
        t.draw(|f| render(f, Rect::new(0, 0, 30, 6), &tree, false))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("Files"));
    }

    #[test]
    fn focused_and_unfocused_both_render() {
        // Smoke test for both branches of the focus styling — neither
        // should panic and both must keep painting the title.
        let tree = fixture();
        for focused in [false, true] {
            let mut t = term(30, 8);
            t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, focused))
                .unwrap();
            assert!(buffer_text(&t).contains("Files"));
        }
    }

    #[test]
    fn selected_index_is_clamped_to_last_entry() {
        // Defensive: an out-of-range selection must not panic; the
        // renderer clamps to the last entry.
        let mut t = term(30, 8);
        let mut tree = fixture();
        tree.selected = 999;
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, true))
            .unwrap();
        assert!(buffer_text(&t).contains("beta.md"));
    }
}
