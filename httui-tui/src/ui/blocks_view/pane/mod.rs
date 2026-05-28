use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{region_label, BlockMeta, EditField, FileBlocks};
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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    let dirty = pane.block_draft.is_some();
    let parsed = load_view(ctx.vault, file, block, pane);
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
}

/// Rounded card border with a left title; accent when focused, muted
/// otherwise. Shared by the request/response/query/result bands.
fn card_block(title: &str, focused: bool) -> Block<'static> {
    let color = if focused {
        crate::ui::palette::accent()
    } else {
        crate::ui::palette::muted()
    };
    let title_style = if focused {
        Style::default().fg(color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(color)
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color))
        .title(Span::styled(format!(" {title} "), title_style))
}

/// Paint a strip of tab cells. Only the active cell gets a background
/// (accent when focused, popup-accent otherwise); inactive cells sit on
/// the canvas background so the active tab is the sole highlight.
fn render_subtab_cells(frame: &mut Frame, area: Rect, labels: &[String], active: usize, focused: bool) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let active_bg = if focused {
        crate::ui::palette::accent()
    } else {
        crate::ui::palette::popup_border_accent()
    };
    let active_style = Style::default()
        .bg(active_bg)
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let idle_style = Style::default().fg(crate::ui::palette::muted());
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    for (i, label) in labels.iter().enumerate() {
        let style = if i == active { active_style } else { idle_style };
        spans.push(Span::styled(format!(" {label} "), style));
        spans.push(Span::raw(" "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Thin separator below a tab strip, drawn in the border colour so it
/// blends with the canvas and just divides tabs from content.
fn render_tab_separator(frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line: String = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            line,
            Style::default().fg(crate::ui::palette::border()),
        ))),
        area,
    );
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

/// Title-bearing border for a region. When `editing` is true, the title
/// gets a trailing `EDIT` chip so it's obvious from any pane width that
/// the keystrokes are flowing into the buffer, not the doc.
fn region_block(block_type: &str, index: usize, focused: bool, editing: bool) -> Block<'static> {
    let (border_color, title_color) = if focused {
        (crate::ui::palette::accent(), crate::ui::palette::accent())
    } else {
        (crate::ui::palette::muted(), crate::ui::palette::muted())
    };
    let title_style = if focused {
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(title_color)
    };
    let label = format!(" [{}] {} ", index + 1, region_label(block_type, index));
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(label, title_style));
    if editing {
        let edit_chip = Span::styled(
            " EDIT ",
            Style::default()
                .bg(crate::ui::palette::amber())
                .fg(crate::ui::palette::popup_bg())
                .add_modifier(Modifier::BOLD),
        );
        block = block.title_top(Line::from(edit_chip).right_aligned());
    }
    block
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

fn render_region(
    frame: &mut Frame,
    area: Rect,
    index: usize,
    block_type: &str,
    focused: bool,
    lines: &[String],
) {
    let (border_color, title_color) = if focused {
        (crate::ui::palette::accent(), crate::ui::palette::accent())
    } else {
        (crate::ui::palette::muted(), crate::ui::palette::muted())
    };
    let title_style = if focused {
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(title_color)
    };
    let block_widget = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(
            format!(" [{}] {} ", index + 1, region_label(block_type, index)),
            title_style,
        ));
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let rendered: Vec<Line<'static>> = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let style = if focused && i == 0 {
                Style::default()
                    .fg(crate::ui::palette::foreground())
                    .add_modifier(Modifier::UNDERLINED)
            } else {
                Style::default()
            };
            Line::from(Span::styled(l.clone(), style))
        })
        .collect();
    frame.render_widget(Paragraph::new(rendered).wrap(Wrap { trim: false }), inner);
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
    if area.width < 5 || area.height < 3 {
        return;
    }
    let letter = if (1..=26).contains(&n) {
        (b'a' + (n - 1) as u8) as char
    } else {
        '?'
    };
    let label = format!("[ {letter} ]");
    let cy = area.y + area.height / 2;
    let label_w = label.chars().count() as u16;
    let cx = area.x + area.width.saturating_sub(label_w) / 2;
    let style = Style::default()
        .bg(crate::ui::palette::popup_border_accent())
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let row = Rect {
        x: cx,
        y: cy,
        width: label_w.min(area.width),
        height: 1,
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(label, style))),
        row,
    );
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

