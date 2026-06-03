//! BLOCKS-view tab strip renderer. Drawn on its own row above the
//! block header when the focused pane has more than one tab. Each tab
//! reads `{method} {alias}{ •dirty}`; the active tab is a "chip" — a
//! lifted background fill with internal padding — so the user can spot
//! the focused tab at a glance without reading text styling. Inactive
//! tabs are muted and unpadded; a `│` separator divides them so the
//! row never reads as one long sentence.

use super::*;

/// Paint the tab strip onto `area` (single-row). No-op when the pane
/// has only one tab — the caller is expected to gate on
/// `pane.tab_count() > 1`. Active tab is read off the pane mirror;
/// inactive tabs read off `Pane::inactive_tab`.
pub(super) fn render_tab_bar(
    frame: &mut Frame,
    area: Rect,
    pane: &Pane,
    pane_focused: bool,
    ctx: &BlocksRenderCtx<'_>,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let count = pane.tab_count();
    if count < 2 {
        return;
    }

    // Underline the whole row in muted to give the strip a baseline —
    // it reads as a tab bar even on a fast scan because the line has
    // visual mass under it. Then individual cells overprint without
    // touching that baseline.
    let baseline_bg = crate::ui::palette::block_chrome_bg();
    frame.render_widget(
        Block::default().style(Style::default().bg(baseline_bg)),
        area,
    );

    let mut spans: Vec<Span<'static>> = Vec::with_capacity(count * 6);
    for idx in 0..count {
        if idx > 0 {
            // Separator: dim vertical bar with one space of padding
            // each side. Reads as "next tab starts here" without
            // adding visual weight to the row.
            spans.push(Span::styled(
                " │ ".to_string(),
                Style::default()
                    .fg(crate::ui::palette::muted())
                    .bg(baseline_bg),
            ));
        }
        spans.extend(build_tab_cell(idx, pane, pane_focused, ctx, baseline_bg));
    }

    let line = Line::from(spans);
    frame.render_widget(Paragraph::new(line), area);
}

/// One tab cell: `{method} {alias}{ •}` text with chip-like padding
/// + background on the active tab. The active chip uses the same
/// `block_active_bg` palette token as the focused-region rail so the
/// language is consistent across the editor.
fn build_tab_cell(
    idx: usize,
    pane: &Pane,
    pane_focused: bool,
    ctx: &BlocksRenderCtx<'_>,
    baseline_bg: ratatui::style::Color,
) -> Vec<Span<'static>> {
    let active = idx == pane.block_tab_active;
    let (selection, dirty) = if active {
        (pane.block_selected, pane.block_draft.is_some())
    } else {
        let snap = pane.inactive_tab(idx);
        match snap {
            Some(t) => (t.block_selected, t.block_draft.is_some()),
            None => (None, false),
        }
    };

    let (method, alias) = resolve_label(selection, ctx);
    let mut spans = Vec::with_capacity(6);

    // Chip background for the active tab; baseline for inactive.
    let chip_bg = if active {
        crate::ui::palette::block_active_bg()
    } else {
        baseline_bg
    };

    let alias_color = if active && pane_focused {
        crate::ui::palette::accent()
    } else if active {
        crate::ui::palette::foreground()
    } else {
        crate::ui::palette::muted()
    };
    let alias_style = if active {
        Style::default()
            .fg(alias_color)
            .bg(chip_bg)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(alias_color).bg(chip_bg)
    };

    // Leading pad — only on the active chip so inactive tabs don't
    // bleed into the separator.
    if active {
        spans.push(Span::styled(" ".to_string(), Style::default().bg(chip_bg)));
    }

    if !method.is_empty() {
        let method_color = if active {
            crate::ui::blocks::http_panel::method_color(&method)
        } else {
            crate::ui::palette::muted()
        };
        let method_style = if active {
            Style::default()
                .fg(method_color)
                .bg(chip_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(method_color).bg(chip_bg)
        };
        spans.push(Span::styled(method.clone(), method_style));
        spans.push(Span::styled(" ".to_string(), Style::default().bg(chip_bg)));
    }
    spans.push(Span::styled(alias, alias_style));

    if dirty {
        spans.push(Span::styled(
            " •".to_string(),
            Style::default()
                .fg(crate::ui::palette::amber())
                .bg(chip_bg)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Trailing pad — same logic as the leading pad.
    if active {
        spans.push(Span::styled(" ".to_string(), Style::default().bg(chip_bg)));
    }

    spans
}

/// Resolve a tab's `BlockRef` into `(method, alias)` via the workspace
/// index. Empty pair for empty tabs (greeter — no selection yet).
fn resolve_label(
    selection: Option<crate::app::BlockRef>,
    ctx: &BlocksRenderCtx<'_>,
) -> (String, String) {
    let Some(sel) = selection else {
        return (String::new(), "untitled".to_string());
    };
    let Some(ws) = ctx.workspace else {
        return (String::new(), "untitled".to_string());
    };
    let Some(file) = ws.index.files.get(sel.file_idx) else {
        return (String::new(), "untitled".to_string());
    };
    let Some(block) = file.blocks.get(sel.block_idx) else {
        return (String::new(), "untitled".to_string());
    };
    let method = if block.block_type == "http" {
        // Method isn't carried in BlockMeta — show kind label until the
        // index grows a method field. Tab bars still serve their main
        // purpose (alias + active highlight) without it.
        "HTTP".to_string()
    } else if block.block_type.starts_with("db") {
        "SQL".to_string()
    } else {
        String::new()
    };
    let alias = block
        .alias
        .as_ref()
        .filter(|a| !a.is_empty())
        .cloned()
        .unwrap_or_else(|| "untitled".to_string());
    (method, alias)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{BlockIndex, BlockMeta, BlocksWorkspace, FileBlocks};
    use crate::pane::Pane;
    use ratatui::{backend::TestBackend, buffer::Buffer, Terminal};
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn workspace_with_blocks(blocks: Vec<(&str, &str)>) -> BlocksWorkspace {
        let block_metas: Vec<BlockMeta> = blocks
            .iter()
            .map(|(ty, alias)| BlockMeta {
                alias: Some(alias.to_string()),
                block_type: ty.to_string(),
                line_start: 0,
            })
            .collect();
        let file = FileBlocks {
            path: PathBuf::from("note.md"),
            display: "note.md".to_string(),
            blocks: block_metas,
        };
        BlocksWorkspace::new(BlockIndex {
            files: vec![file],
        })
    }

    fn make_draft() -> Box<crate::app::BlockDraft> {
        Box::new(crate::app::BlockDraft {
            file_path: PathBuf::from("note.md"),
            block_line_start: 0,
            block: httui_core::blocks::parse_blocks("```http alias=a\nGET https://x\n```\n")
                .into_iter()
                .next()
                .expect("at least one parsed block"),
        })
    }

    fn paint(pane: &Pane, ws: &BlocksWorkspace) -> Buffer {
        let backend = TestBackend::new(60, 1);
        let mut term = Terminal::new(backend).unwrap();
        let names: HashMap<String, String> = HashMap::new();
        let result_tabs: HashMap<crate::buffer::block::BlockId, crate::app::ResultPanelTab> =
            HashMap::new();
        let mut result_viewport_top: HashMap<usize, u16> = HashMap::new();
        let vault = PathBuf::from("/tmp/v");
        let mut cell = None;
        term.draw(|f| {
            let area = Rect {
                x: 0,
                y: 0,
                width: 60,
                height: 1,
            };
            let ctx = BlocksRenderCtx {
                vault: &vault,
                workspace: Some(ws),
                connection_names: &names,
                result_tabs: &result_tabs,
                result_viewport_top: &mut result_viewport_top,
                visual_overlay: None,
                running: None,
                popup_cursor_cell: &mut cell,
            };
            render_tab_bar(f, area, pane, true, &ctx);
        })
        .unwrap();
        term.backend().buffer().clone()
    }

    fn buffer_to_lines(buf: &Buffer) -> Vec<String> {
        (0..buf.area().height)
            .map(|y| {
                (0..buf.area().width)
                    .filter_map(|x| buf.cell((x, y)))
                    .map(|c| c.symbol().to_string())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn render_tab_bar_noop_for_single_tab() {
        let ws = workspace_with_blocks(vec![("http", "ping")]);
        let mut pane = Pane::empty();
        pane.block_selected = Some(crate::app::BlockRef {
            file_idx: 0,
            block_idx: 0,
        });
        let buf = paint(&pane, &ws);
        let lines = buffer_to_lines(&buf);
        // Single tab → nothing painted, all spaces.
        assert!(
            lines[0].trim().is_empty(),
            "single-tab pane should not paint a tab bar, got: {:?}",
            lines[0]
        );
    }

    #[test]
    fn render_tab_bar_prints_alias_for_each_tab() {
        let ws =
            workspace_with_blocks(vec![("http", "ping"), ("db-postgres", "rows")]);
        let mut pane = Pane::empty();
        pane.block_selected = Some(crate::app::BlockRef {
            file_idx: 0,
            block_idx: 0,
        });
        let _ = pane.push_block_tab(crate::pane_tabs::BlockTab {
            block_selected: Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 1,
            }),
            ..crate::pane_tabs::BlockTab::empty()
        });
        // active tab now is index 1 ("rows" / SQL).
        let buf = paint(&pane, &ws);
        let lines = buffer_to_lines(&buf);
        assert!(
            lines[0].contains("ping"),
            "tab bar should include 'ping', got: {:?}",
            lines[0]
        );
        assert!(
            lines[0].contains("rows"),
            "tab bar should include 'rows', got: {:?}",
            lines[0]
        );
    }

    #[test]
    fn render_tab_bar_marks_dirty_with_dot() {
        let ws =
            workspace_with_blocks(vec![("http", "ping"), ("http", "save")]);
        let mut pane = Pane::empty();
        pane.block_selected = Some(crate::app::BlockRef {
            file_idx: 0,
            block_idx: 0,
        });
        pane.block_draft = Some(make_draft());
        let _ = pane.push_block_tab(crate::pane_tabs::BlockTab {
            block_selected: Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 1,
            }),
            ..crate::pane_tabs::BlockTab::empty()
        });
        let buf = paint(&pane, &ws);
        let lines = buffer_to_lines(&buf);
        assert!(
            lines[0].contains("•"),
            "dirty tab should show bullet, got: {:?}",
            lines[0]
        );
    }
}
