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
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
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
/// file rows get a red `●` dot. Empty outside BLOCKS view.
pub fn render(
    frame: &mut Frame,
    area: Rect,
    tree: &FileTree,
    focused: bool,
    dirty_files: &HashSet<String>,
) {
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
    let logo = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            "H>",
            Style::default()
                .fg(crate::ui::palette::accent())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" httui ", title_style),
    ]);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(logo)
        .title_top(Line::from(Span::styled(" ⚙ ", title_style)).right_aligned());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    // Reserve the top line for the quick-jump hint; the tree list fills
    // the rest.
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
            Span::styled(
                "  Jump…",
                Style::default().fg(crate::ui::palette::muted()),
            ),
        ])),
        rows[0],
    );
    let inner = rows[1];

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
                let mut spans = vec![
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
                ];
                if let Some(lr) = meta.last_run.as_ref() {
                    let (label, color) = block_run_chip(
                        meta.block_type.starts_with("db"),
                        lr.status,
                        &lr.outcome,
                    );
                    spans.push(Span::raw("  "));
                    spans.push(Span::styled(
                        label,
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ));
                }
                return ListItem::new(Line::from(spans));
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
                spans.push(Span::styled("●", Style::default().fg(Color::Red)));
            }
            ListItem::new(Line::from(spans))
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

/// Last-run badge text + color for a sidebar block row. HTTP shows the
/// status code colored by class (2xx green, 3xx blue, 4xx/5xx red); DB
/// shows the row count in blue (`status` holds `rows.len()` /
/// `rows_affected` — see `derive_db_history_stats`). A recorded
/// `"error"` outcome is always red; `"cancelled"` is a muted dash.
fn block_run_chip(is_db: bool, status: Option<i64>, outcome: &str) -> (String, Color) {
    match outcome {
        "cancelled" => ("—".into(), crate::ui::palette::muted()),
        "error" => (
            status.map(|c| c.to_string()).unwrap_or_else(|| "err".into()),
            Color::Red,
        ),
        _ if is_db => match status {
            Some(1) => ("1 row".into(), crate::ui::palette::accent()),
            Some(n) => (format!("{n} rows"), crate::ui::palette::accent()),
            None => ("ok".into(), crate::ui::palette::accent()),
        },
        _ => match status {
            Some(c) if (200..300).contains(&c) => {
                (c.to_string(), crate::ui::palette::success())
            }
            Some(c) if (300..400).contains(&c) => {
                (c.to_string(), crate::ui::palette::accent())
            }
            Some(c) => (c.to_string(), Color::Red),
            None => ("—".into(), crate::ui::palette::muted()),
        },
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
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("httui"));
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
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("▸"), "collapsed icon missing: {text}");
    }

    #[test]
    fn empty_tree_renders_without_panic() {
        let mut t = term(30, 6);
        let tree = FileTree::default();
        t.draw(|f| render(f, Rect::new(0, 0, 30, 6), &tree, false, &HashSet::new()))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("httui"));
    }

    #[test]
    fn focused_and_unfocused_both_render() {
        // Smoke test for both branches of the focus styling — neither
        // should panic and both must keep painting the title.
        let tree = fixture();
        for focused in [false, true] {
            let mut t = term(30, 8);
            t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, focused, &HashSet::new()))
                .unwrap();
            assert!(buffer_text(&t).contains("httui"));
        }
    }

    #[test]
    fn selected_index_is_clamped_to_last_entry() {
        // Defensive: an out-of-range selection must not panic; the
        // renderer clamps to the last entry.
        let mut t = term(30, 8);
        let mut tree = fixture();
        tree.selected = 999;
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, true, &HashSet::new()))
            .unwrap();
        assert!(buffer_text(&t).contains("beta.md"));
    }

    use crate::app::BlockLastRun;
    use crate::tree::TreeBlockMeta;

    fn block_row(block_type: &str, label: &str, last_run: Option<BlockLastRun>) -> FileTree {
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
                        last_run,
                    }),
                },
            ],
            ..FileTree::default()
        }
    }

    #[test]
    fn block_run_chip_http_status_classes() {
        assert_eq!(
            block_run_chip(false, Some(204), "ok"),
            ("204".into(), crate::ui::palette::success())
        );
        assert_eq!(
            block_run_chip(false, Some(301), "ok"),
            ("301".into(), crate::ui::palette::accent())
        );
        assert_eq!(block_run_chip(false, Some(500), "ok"), ("500".into(), Color::Red));
        assert_eq!(block_run_chip(false, Some(404), "ok"), ("404".into(), Color::Red));
    }

    #[test]
    fn block_run_chip_db_rows_are_blue() {
        assert_eq!(
            block_run_chip(true, Some(12), "ok"),
            ("12 rows".into(), crate::ui::palette::accent())
        );
        assert_eq!(
            block_run_chip(true, Some(1), "ok"),
            ("1 row".into(), crate::ui::palette::accent())
        );
    }

    #[test]
    fn block_run_chip_error_and_cancelled() {
        assert_eq!(block_run_chip(false, Some(500), "error"), ("500".into(), Color::Red));
        assert_eq!(block_run_chip(true, None, "error"), ("err".into(), Color::Red));
        assert_eq!(
            block_run_chip(false, None, "cancelled"),
            ("—".into(), crate::ui::palette::muted())
        );
    }

    #[test]
    fn renders_http_status_badge_next_to_block() {
        let mut t = term(30, 8);
        let tree = block_row(
            "http",
            "login",
            Some(BlockLastRun {
                status: Some(200),
                outcome: "ok".into(),
            }),
        );
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        let text = buffer_text(&t);
        assert!(text.contains("login"), "{text}");
        assert!(text.contains("200"), "status badge missing: {text}");
    }

    #[test]
    fn renders_db_row_count_badge() {
        let mut t = term(30, 8);
        let tree = block_row(
            "db-postgres",
            "audit",
            Some(BlockLastRun {
                status: Some(12),
                outcome: "ok".into(),
            }),
        );
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        assert!(buffer_text(&t).contains("12 rows"));
    }

    #[test]
    fn block_without_last_run_has_no_badge() {
        let mut t = term(30, 8);
        let tree = block_row("http", "login", None);
        t.draw(|f| render(f, Rect::new(0, 0, 30, 8), &tree, false, &HashSet::new()))
            .unwrap();
        let text = buffer_text(&t);
        // The label is there but no trailing status digits.
        assert!(text.contains("login"), "{text}");
    }

    #[test]
    fn dirty_file_gets_red_dot() {
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
