//! File-tree sidebar renderer. A right divider separates it from the
//! editor panes; rows show files/dirs and (in BLOCKS mode) the
//! executable blocks under each file. The divider + the selected row
//! brighten to the accent when the sidebar holds keyboard focus.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use std::collections::HashSet;

use crate::tree::FileTree;

const SIDEBAR_WIDTH: u16 = 30;

pub fn width() -> u16 {
    SIDEBAR_WIDTH
}

/// `dirty_files` holds vault-relative paths of files that have at least
/// one block in draft (committed-but-unsaved in some pane). Matching
/// file rows get a `●` dot. Empty outside BLOCKS view.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    tree: &FileTree,
    focused: bool,
    dirty_files: &HashSet<String>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    // No box — a single right divider separates the sidebar from the
    // panes; it brightens to the accent when the sidebar holds focus.
    let divider_color = if focused {
        crate::ui::palette::accent()
    } else {
        crate::ui::palette::border()
    };
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(divider_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    // Top line: the quick-jump hint; the tree list fills the rest.
    let rows = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(0),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " Ctrl+P",
                Style::default().fg(crate::ui::palette::accent()),
            ),
            Span::styled("  Jump…", Style::default().fg(crate::ui::palette::muted())),
        ])),
        rows[0],
    );
    let list_area = rows[1];

    let block_mode = tree.block_index.is_some();
    let items: Vec<ListItem> = tree
        .entries
        .iter()
        .map(|node| {
            let indent = "  ".repeat(node.depth);
            if let Some(meta) = node.block.as_ref() {
                // Block row: dim type tag + bold alias. No status badge —
                // run status reads off the block's own header.
                return ListItem::new(Line::from(vec![
                    Span::raw(indent),
                    Span::raw("  "),
                    Span::styled(
                        format!("{:<4}", block_type_label(&meta.block_type)),
                        Style::default().fg(crate::ui::palette::muted()),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        meta.label.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]));
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
            let mut spans = vec![
                Span::raw(indent),
                Span::styled(icon, Style::default().fg(crate::ui::palette::muted())),
                Span::styled(node.name.clone(), name_style),
            ];
            if !node.is_dir && dirty_files.contains(&node.path) {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    "●",
                    Style::default().fg(crate::ui::palette::amber()),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let highlight_style = if focused {
        Style::default()
            .bg(crate::ui::palette::accent())
            .fg(crate::ui::palette::popup_bg())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .bg(crate::ui::palette::selection_bg())
            .fg(crate::ui::palette::foreground())
    };

    let list = List::new(items)
        .highlight_style(highlight_style)
        .highlight_symbol("");
    let mut state = ListState::default();
    if !tree.entries.is_empty() {
        state.select(Some(tree.selected.min(tree.entries.len() - 1)));
    }
    frame.render_stateful_widget(list, list_area, &mut state);
}

/// Compact lowercase type tag for a block row (`http` / `db`).
fn block_type_label(block_type: &str) -> &str {
    if block_type == "http" {
        "http"
    } else if block_type.starts_with("db") {
        "db"
    } else {
        block_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::{FileTree, TreeBlockMeta, TreeNode};
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
    fn renders_files_and_dir_icon() {
        let mut t = term(30, 8);
        let tree = fixture();
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("notes"));
        assert!(text.contains("alpha.md"));
        assert!(text.contains("beta.md"));
        // Expanded folder shows the down-pointing triangle.
        assert!(text.contains("▾"));
    }

    #[test]
    fn shows_jump_hint() {
        let mut t = term(30, 8);
        let tree = fixture();
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        assert!(buffer_text(&t).contains("Jump"));
    }

    #[test]
    fn collapsed_dir_uses_right_pointing_icon() {
        let mut t = term(30, 8);
        let mut tree = fixture();
        tree.expanded.clear();
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        assert!(buffer_text(&t).contains("▸"), "collapsed icon missing");
    }

    #[test]
    fn empty_tree_renders_without_panic() {
        let mut t = term(30, 6);
        let tree = FileTree::default();
        t.draw(|f| render(f, Rect::new(0, 0, 30, 6), &tree, false, &HashSet::new()))
            .unwrap();
        assert!(buffer_text(&t).contains("Jump"));
    }

    #[test]
    fn focused_and_unfocused_both_render() {
        // Smoke test for both branches of the focus styling.
        let tree = fixture();
        for focused in [false, true] {
            let mut t = term(30, 8);
            t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, focused, &HashSet::new()))
                .unwrap();
            assert!(buffer_text(&t).contains("alpha.md"));
        }
    }

    #[test]
    fn selected_index_is_clamped_to_last_entry() {
        // Defensive: an out-of-range selection must not panic.
        let mut t = term(30, 8);
        let mut tree = fixture();
        tree.selected = 999;
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, true, &HashSet::new()))
            .unwrap();
        assert!(buffer_text(&t).contains("beta.md"));
    }

    fn block_row(block_type: &str, label: &str) -> FileTree {
        FileTree {
            entries: vec![
                TreeNode {
                    name: "api.md".into(),
                    path: "api.md".into(),
                    is_dir: false,
                    depth: 0,
                    block: None,
                },
                TreeNode {
                    name: label.into(),
                    path: "api.md".into(),
                    is_dir: false,
                    depth: 1,
                    block: Some(TreeBlockMeta {
                        file_idx: 0,
                        block_idx: 0,
                        block_type: block_type.into(),
                        label: label.into(),
                    }),
                },
            ],
            ..FileTree::default()
        }
    }

    #[test]
    fn block_row_shows_type_tag_and_alias() {
        let mut t = term(30, 8);
        let tree = block_row("http", "login");
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("http"), "type tag: {text}");
        assert!(text.contains("login"), "alias: {text}");
    }

    #[test]
    fn db_block_row_uses_db_tag() {
        let mut t = term(30, 8);
        let tree = block_row("db-postgres", "audit");
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("db"), "db tag: {text}");
        assert!(text.contains("audit"));
    }

    #[test]
    fn dirty_file_gets_dot() {
        let mut t = term(30, 8);
        let tree = fixture();
        let mut dirty = HashSet::new();
        dirty.insert("notes/alpha.md".to_string());
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &dirty))
            .unwrap();
        assert!(buffer_text(&t).contains("●"), "dirty dot missing");
    }

    #[test]
    fn clean_files_have_no_dot() {
        let mut t = term(30, 8);
        let tree = fixture();
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        assert!(!buffer_text(&t).contains("●"), "unexpected dirty dot");
    }
}
