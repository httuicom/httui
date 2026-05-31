use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Wrap},
    Frame,
};

use crate::app::{BlockMeta, EditField, FileBlocks};
use crate::pane::Pane;

use super::BlocksRenderCtx;

mod view;
use view::{
    block_node_from_pane, block_node_id, edit_cursor_row_col, load_block_node, load_view,
    ParsedView,
};

mod header;
mod http;
mod db;
mod footer;

pub(super) fn render_leaf(
    frame: &mut Frame,
    area: Rect,
    leaf: &Pane,
    focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
    running: Option<&str>,
    ctx: &mut BlocksRenderCtx<'_>,
) {
    // No outer pane frame — the region cards (REQUEST / RESPONSE /
    // QUERY / RESULT) carry their own borders, and focus reads off
    // their accent colouring, so a wrapping border would just nest.
    if area.width == 0 || area.height == 0 {
        return;
    }

    let Some(target) = leaf.block_selected else {
        paint_empty_state(frame, area);
        return;
    };
    let Some(ws) = ctx.workspace else {
        paint_empty_state(frame, area);
        return;
    };
    let Some(file) = ws.index.files.get(target.file_idx) else {
        paint_missing(frame, area);
        return;
    };
    let Some(block) = file.blocks.get(target.block_idx) else {
        paint_missing(frame, area);
        return;
    };

    render_block(
        frame,
        area,
        file,
        block,
        leaf,
        focused,
        visual_overlay,
        running,
        ctx,
    );
}

fn paint_empty_state(frame: &mut Frame, area: Rect) {
    if area.height < 2 {
        return;
    }
    let muted = Style::default().fg(crate::ui::palette::muted());
    let mid = area.y + area.height / 2;
    let lines = [
        ("Select a block from the sidebar", true),
        ("Enter on a block opens it here · Tab cycles regions", false),
    ];
    for (offset, (text, bold)) in lines.iter().enumerate() {
        let y = mid.saturating_add(offset as u16);
        if y >= area.y + area.height {
            break;
        }
        let style = if *bold {
            muted.add_modifier(Modifier::BOLD)
        } else {
            muted
        };
        let line = Line::from(Span::styled(text.to_string(), style));
        let row = Rect {
            x: area.x,
            y,
            width: area.width,
            height: 1,
        };
        frame.render_widget(Paragraph::new(line), row);
    }
}

fn paint_missing(frame: &mut Frame, area: Rect) {
    if area.height < 1 {
        return;
    }
    let muted = Style::default().fg(crate::ui::palette::muted());
    let line = Line::from(Span::styled(
        "(block missing — vault changed?)".to_string(),
        muted,
    ));
    frame.render_widget(Paragraph::new(line), area);
}

#[allow(clippy::too_many_arguments)]
fn render_block(
    frame: &mut Frame,
    area: Rect,
    file: &FileBlocks,
    block: &BlockMeta,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
    running: Option<&str>,
    ctx: &mut BlocksRenderCtx<'_>,
) {
    let dirty = pane.block_draft.is_some();
    let parsed = load_view(ctx.vault, file, block, pane);
    let footer_lines = footer::compute_footer_lines(ctx.vault, file, block, &parsed);
    let footer_h = footer_lines.len() as u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(footer_h),
        ])
        .split(area);
    let region = pane.block_region;
    header::render_header(
        frame,
        chunks[0],
        block,
        &parsed,
        dirty,
        running,
        ctx,
        pane,
        pane_focused,
        region,
        visual_overlay,
    );

    if block.block_type == "http" {
        http::render_http_regions(
            frame,
            chunks[1],
            region,
            &parsed,
            &block.block_type,
            pane,
            pane_focused,
            visual_overlay,
            running,
            file,
            block,
            ctx,
        );
    } else if block.block_type.starts_with("db") {
        db::render_db_regions(
            frame,
            chunks[1],
            region,
            &parsed,
            &block.block_type,
            pane,
            pane_focused,
            visual_overlay,
            file,
            block,
            ctx,
        );
    } else {
        render_fallback(frame, chunks[1], &parsed.raw);
    }

    if footer_h > 0 {
        footer::render_block_usage_footer(frame, chunks[2], &footer_lines);
    }
}

/// IDE-style region frame. The focused region gets an accent left-rail
/// (`▎`) plus a lifted panel background; unfocused regions stay flat on
/// the canvas (the dim label + dim content carry the de-emphasis). No
/// box border. Returns the content rect, left-padded past the rail so
/// content aligns whether or not the region is focused.
fn region_frame(frame: &mut Frame, area: Rect, focused: bool) -> Rect {
    if area.width == 0 || area.height == 0 {
        return area;
    }
    if focused {
        let bg = crate::ui::palette::block_body_bg();
        frame.render_widget(Block::default().style(Style::default().bg(bg)), area);
        let bar_style = Style::default().fg(crate::ui::palette::accent()).bg(bg);
        let buf = frame.buffer_mut();
        for y in area.y..area.y.saturating_add(area.height) {
            if let Some(cell) = buf.cell_mut((area.x, y)) {
                cell.set_symbol("▎");
                cell.set_style(bar_style);
            }
        }
    }
    Rect {
        x: area.x.saturating_add(2),
        y: area.y,
        width: area.width.saturating_sub(2),
        height: area.height,
    }
}

/// One header row for a region: the title, then its sub-tab cells. The
/// active cell gets a subtle lifted bg (T4); inactive cells are muted
/// text — no bright fill. The title is accent-bold when the region is
/// focused, muted otherwise. Pass an empty `labels` for regions with no
/// sub-tabs (just the title shows).
fn render_region_tabs(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    labels: &[String],
    active: usize,
    focused: bool,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let title_style = if focused {
        Style::default()
            .fg(crate::ui::palette::accent())
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(crate::ui::palette::muted())
    };
    let active_style = Style::default()
        .bg(crate::ui::palette::block_active_bg())
        .fg(crate::ui::palette::foreground());
    let idle_style = Style::default().fg(crate::ui::palette::muted());
    // Drop the title when the first tab already carries the same word
    // (e.g. the DB "Result" region whose first tab is also "Result") —
    // the tab leads instead of repeating it.
    let skip_title = labels
        .first()
        .is_some_and(|l| l.eq_ignore_ascii_case(title));
    let mut spans: Vec<Span<'static>> = if skip_title {
        Vec::new()
    } else {
        vec![Span::styled(format!("{title}  "), title_style)]
    };
    for (i, label) in labels.iter().enumerate() {
        let style = if i == active { active_style } else { idle_style };
        spans.push(Span::styled(format!(" {label} "), style));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn field_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::UNDERLINED)
    } else {
        Style::default()
    }
}

/// `{{ref}}` placeholders rendered as chips via the same highlighter the
/// DOC view uses, so refs paint identically across views. Underlines
/// every span when the field carries the NAV cursor.
fn refs_spans(text: &str, focused: bool) -> Vec<Span<'static>> {
    let mut spans = crate::ui::blocks::ref_highlight::highlight_refs(
        text,
        &std::collections::HashSet::new(),
    );
    if focused {
        for s in &mut spans {
            s.style = s.style.add_modifier(Modifier::UNDERLINED);
        }
    }
    spans
}

/// Render a multi-line value (HTTP body / DB query) borderless into
/// `inner`. In EDIT it paints the buffer and places the caret.
#[allow(clippy::too_many_arguments)]
fn render_multiline_region(
    frame: &mut Frame,
    inner: Rect,
    block_type: &str,
    focused: bool,
    fallback: &str,
    placeholder: &str,
    pane: &Pane,
    pane_focused: bool,
    field_matches: impl Fn(&EditField) -> bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) -> Option<(u16, u16)> {
    let _ = (focused, pane_focused);
    if inner.width == 0 || inner.height == 0 {
        return None;
    }
    let active_edit = pane
        .block_edit
        .as_ref()
        .filter(|e| field_matches(&e.field));
    let (text, caret): (String, Option<(u16, u16)>) = if let Some(edit) = active_edit {
        let text = edit.current_text();
        let (row, col) = edit_cursor_row_col(edit);
        let cy = inner.y.saturating_add(row as u16);
        let cx = inner.x.saturating_add(col as u16);
        (text, Some((cx, cy)))
    } else if fallback.is_empty() {
        (placeholder.to_string(), None)
    } else {
        (fallback.to_string(), None)
    };
    // SQL blocks pick up the same syntax highlighter the DOC view
    // uses (`ui::sql_highlight::highlight`). HTTP body paints `{{ref}}`
    // as chips in NAV; while editing it stays verbatim so the caret
    // column and the visual-selection overlay map 1:1 to bytes.
    let rendered: Vec<Line<'static>> = if block_type.starts_with("db") {
        crate::ui::sql_highlight::highlight(&text)
            .into_iter()
            .map(Line::from)
            .collect()
    } else if active_edit.is_some() {
        text.split('\n')
            .map(|l| Line::from(Span::raw(l.to_string())))
            .collect()
    } else {
        text.split('\n')
            .map(|l| Line::from(refs_spans(l, false)))
            .collect()
    };
    frame.render_widget(Paragraph::new(rendered), inner);
    if let Some((cx, cy)) = caret {
        if cx < inner.x + inner.width && cy < inner.y + inner.height {
            frame.set_cursor_position((cx, cy));
        }
    }
    // Visual-mode selection overlay over the multi-line text. Only
    // paints while EDIT is active and the engine is in Visual /
    // VisualLine on the sub-doc.
    if let (Some(edit), Some(overlay)) = (active_edit, visual_overlay) {
        crate::ui::overlay_visual_selection(frame, inner, &edit.doc, 0, overlay);
    }
    caret
}

fn render_fallback(frame: &mut Frame, area: Rect, raw: &str) {
    let lines: Vec<Line<'static>> = raw
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        area,
    );
}

pub(super) fn paint_picker_overlay(frame: &mut Frame, area: Rect, n: usize) {
    if area.width < 1 || area.height < 1 {
        return;
    }
    let letter = if (1..=26).contains(&n) {
        (b'A' + (n - 1) as u8) as char
    } else {
        '?'
    };
    // Single full-width band centered vertically — minimal chrome,
    // unmistakable: solid accent bg across the whole pane width with
    // the bold letter at the horizontal centre.
    let band_y = area.y + area.height / 2;
    let fill_style = Style::default()
        .bg(crate::ui::palette::accent())
        .fg(crate::ui::palette::popup_bg());
    let buf = frame.buffer_mut();
    for x in area.x..area.x + area.width {
        if let Some(cell) = buf.cell_mut((x, band_y)) {
            cell.set_symbol(" ");
            cell.set_style(fill_style);
        }
    }
    let letter_x = area.x + area.width / 2;
    if let Some(cell) = buf.cell_mut((letter_x, band_y)) {
        cell.set_symbol(&letter.to_string());
        cell.set_style(fill_style.add_modifier(Modifier::BOLD));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refs_spans_chips_a_reference() {
        let spans = refs_spans("/users/{{id}}", false);
        let chip = spans
            .iter()
            .find(|s| s.content == "{{id}}")
            .expect("ref chip span");
        assert_eq!(
            chip.style,
            crate::ui::blocks::ref_highlight::normal_style()
        );
    }

    #[test]
    fn refs_spans_plain_text_is_a_single_span() {
        let spans = refs_spans("no refs here", false);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "no refs here");
    }

    #[test]
    fn refs_spans_underlines_every_span_when_focused() {
        let spans = refs_spans("a {{b}} c", true);
        assert!(!spans.is_empty());
        assert!(spans
            .iter()
            .all(|s| s.style.add_modifier.contains(Modifier::UNDERLINED)));
    }
}

