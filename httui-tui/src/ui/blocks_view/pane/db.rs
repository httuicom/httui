use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn render_db_regions(
    frame: &mut Frame,
    area: Rect,
    region: usize,
    parsed: &ParsedView,
    block_type: &str,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
    file: &FileBlocks,
    block: &BlockMeta,
    ctx: &mut BlocksRenderCtx<'_>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Min(3)])
        .split(area);

    // Query region (region 1). Connection (region 0) lives in the header.
    let query_focused = pane_focused && region == 1;
    let inner = region_frame(frame, chunks[0], query_focused);
    if inner.width > 0 && inner.height > 0 {
        let parts = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);
        render_region_tabs(frame, parts[0], "Query", &[], 0, query_focused);
        if parts[1].height > 0 {
            let query_caret = render_multiline_region(
                frame,
                parts[1],
                block_type,
                region == 1,
                &parsed.body,
                "(empty query)",
                pane,
                pane_focused,
                |f| matches!(f, EditField::DbQuery),
                visual_overlay,
            );
            if let Some(cell) = query_caret {
                *ctx.popup_cursor_cell = Some(cell);
            }
        }
    }

    render_db_result_region(
        frame,
        chunks[1],
        block_type,
        pane_focused && region == 2,
        file,
        block,
        pane,
        ctx,
    );
}

/// `Result` region — delegates to `ui::blocks::result_tabs` +
/// `ui::blocks::db_table::build_result_table`. Carries the result
/// panel's full tab bar (Result / Messages / Plan / Stats), the
/// real result table widget (header bold + zebra + numeric align +
/// scroll viewport), and the error banner branch.
#[allow(clippy::too_many_arguments)]
fn render_db_result_region(
    frame: &mut Frame,
    area: Rect,
    block_type: &str,
    focused: bool,
    file: &FileBlocks,
    block: &BlockMeta,
    pane: &Pane,
    ctx: &mut BlocksRenderCtx<'_>,
) {
    let inner = region_frame(frame, area, focused);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let variants = crate::app::ResultPanelTab::variants_for(block_type);
    let labels: Vec<String> = variants
        .iter()
        .map(|t| t.label_for(block_type).to_string())
        .collect();
    // Prefer the pane's loaded document — that's where `cached_result`
    // lives after `run_focused_block`. Falls back to disk for a fresh
    // pane that hasn't loaded the file yet (no cached_result).
    let block_node = match block_node_from_pane(pane, file, block)
        .or_else(|| load_block_node(ctx.vault, file, block))
    {
        Some(b) => b,
        None => {
            let parts = ratatui::layout::Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    ratatui::layout::Constraint::Length(1),
                    ratatui::layout::Constraint::Min(0),
                ])
                .split(inner);
            render_region_tabs(frame, parts[0], "Result", &labels, 0, focused);
            if parts[1].height > 0 {
                frame.render_widget(Paragraph::new("(no result — press r to run)"), parts[1]);
            }
            return;
        }
    };
    let key = block_node_id(file, block);
    let viewport_key: usize = key.0 as usize;
    let tab = ctx
        .result_tabs
        .get(&key)
        .copied()
        .unwrap_or(crate::app::ResultPanelTab::Result);
    let active = variants.iter().position(|t| *t == tab).unwrap_or(0);
    let chunks = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Vertical)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(0),
        ])
        .split(inner);
    render_region_tabs(frame, chunks[0], "Result", &labels, active, focused);
    if chunks[1].height == 0 {
        return;
    }
    use crate::app::ResultPanelTab;
    match tab {
        ResultPanelTab::Result => {
            ctx.result_viewport_top.entry(viewport_key).or_insert(0);
            let selected_row = if focused { Some(pane.block_row) } else { None };
            // Fill the region: every row the rect can hold minus the
            // table's own header row.
            let max_rows = chunks[1].height.saturating_sub(1).max(1) as usize;
            if let Some((table, viewport_selected)) =
                crate::ui::blocks::db_table::build_result_table(
                    &block_node,
                    selected_row,
                    ctx.result_viewport_top.get_mut(&viewport_key),
                    max_rows,
                )
            {
                let mut state = ratatui::widgets::TableState::default();
                state.select(viewport_selected);
                let table = table.row_highlight_style(
                    Style::default()
                        .bg(crate::ui::palette::selection_bg())
                        .add_modifier(Modifier::BOLD),
                );
                frame.render_stateful_widget(table, chunks[1], &mut state);
            } else if let Some(lines) =
                crate::ui::blocks::result_tabs::build_error_lines(&block_node)
            {
                frame.render_widget(Paragraph::new(lines), chunks[1]);
            } else {
                frame.render_widget(Paragraph::new("(no result — press r to run)"), chunks[1]);
            }
        }
        ResultPanelTab::Messages => {
            let lines = crate::ui::blocks::result_tabs::build_messages_lines(&block_node);
            frame.render_widget(Paragraph::new(lines), chunks[1]);
        }
        ResultPanelTab::Plan => {
            let lines = crate::ui::blocks::result_tabs::build_plan_lines(&block_node);
            frame.render_widget(Paragraph::new(lines), chunks[1]);
        }
        ResultPanelTab::Stats => {
            let lines = crate::ui::blocks::result_tabs::build_stats_lines(&block_node);
            frame.render_widget(Paragraph::new(lines), chunks[1]);
        }
        ResultPanelTab::Raw => {
            // DB blocks don't expose Raw — fall back to Stats.
            let lines = crate::ui::blocks::result_tabs::build_stats_lines(&block_node);
            frame.render_widget(Paragraph::new(lines), chunks[1]);
        }
    }
}
