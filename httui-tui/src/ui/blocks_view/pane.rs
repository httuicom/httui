use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{region_label, BlockMeta, BlocksWorkspace, EditField, FileBlocks, RegionEdit};
use crate::pane::Pane;

pub(super) fn render_leaf(
    frame: &mut Frame,
    area: Rect,
    leaf: &Pane,
    focused: bool,
    workspace: Option<&BlocksWorkspace>,
    vault: &std::path::Path,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let border_color = if focused {
        crate::ui::palette::accent()
    } else {
        crate::ui::palette::muted()
    };
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let Some(target) = leaf.block_selected else {
        paint_empty_state(frame, inner);
        return;
    };
    let Some(ws) = workspace else {
        paint_empty_state(frame, inner);
        return;
    };
    let Some(file) = ws.index.files.get(target.file_idx) else {
        paint_missing(frame, inner);
        return;
    };
    let Some(block) = file.blocks.get(target.block_idx) else {
        paint_missing(frame, inner);
        return;
    };

    render_block(frame, inner, file, block, leaf, focused, vault, visual_overlay);
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
    vault_path: &Path,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    let dirty = pane.block_draft.is_some();
    render_header(frame, chunks[0], file, block, dirty);

    let parsed = load_view(vault_path, file, block, pane);
    let region = pane.block_region;
    if block.block_type == "http" {
        render_http_regions(
            frame,
            chunks[1],
            region,
            &parsed,
            &block.block_type,
            pane,
            pane_focused,
            visual_overlay,
        );
    } else if block.block_type.starts_with("db") {
        render_db_regions(
            frame,
            chunks[1],
            region,
            &parsed,
            &block.block_type,
            pane,
            pane_focused,
            visual_overlay,
        );
    } else {
        render_fallback(frame, chunks[1], &parsed.raw);
    }
}

fn render_header(
    frame: &mut Frame,
    area: Rect,
    file: &FileBlocks,
    block: &BlockMeta,
    dirty: bool,
) {
    if area.width == 0 {
        return;
    }
    let badge = badge_text(&block.block_type);
    let badge_bg = if block.block_type == "http" {
        crate::ui::palette::accent()
    } else {
        crate::ui::palette::popup_border_accent()
    };
    let muted = Style::default().fg(crate::ui::palette::muted());
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(
            badge,
            Style::default()
                .bg(badge_bg)
                .fg(crate::ui::palette::popup_bg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            block.label(),
            Style::default()
                .fg(crate::ui::palette::accent())
                .add_modifier(Modifier::BOLD),
        ),
    ];
    if dirty {
        spans.push(Span::styled(
            " *",
            Style::default()
                .fg(crate::ui::palette::amber())
                .add_modifier(Modifier::BOLD),
        ));
    }
    spans.push(Span::styled("  ·  ", muted));
    spans.push(Span::styled(file.display.clone(), muted));
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[allow(clippy::too_many_arguments)]
fn render_http_regions(
    frame: &mut Frame,
    area: Rect,
    region: usize,
    parsed: &ParsedView,
    block_type: &str,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Min(3),
            Constraint::Min(3),
        ])
        .split(area);
    let editing_url = pane_focused
        && pane
            .block_edit
            .as_ref()
            .map(|e| matches!(e.field, EditField::HttpUrl))
            .unwrap_or(false);
    let method = parsed.method.as_deref().unwrap_or("GET").to_string();
    let url_value = pane
        .block_edit
        .as_ref()
        .filter(|e| matches!(e.field, EditField::HttpUrl))
        .map(|e| e.current_text())
        .unwrap_or_else(|| parsed.url.clone().unwrap_or_default());
    render_request_region(
        frame,
        chunks[0],
        block_type,
        region == 0,
        editing_url,
        &method,
        &url_value,
        pane.block_edit
            .as_deref()
            .filter(|_| editing_url),
        visual_overlay,
    );
    render_headers_region(
        frame,
        chunks[1],
        block_type,
        region == 1,
        parsed,
        pane,
        pane_focused,
        visual_overlay,
    );
    render_multiline_region(
        frame,
        chunks[2],
        block_type,
        2,
        region == 2,
        &parsed.body,
        "(no body)",
        pane,
        pane_focused,
        |f| matches!(f, EditField::HttpBody),
        visual_overlay,
    );
    let response_lines: Vec<String> = if parsed.cached.is_empty() {
        vec!["(no response — run with Alt+R)".to_string()]
    } else {
        parsed.cached.lines().map(str::to_string).collect()
    };
    render_region(
        frame,
        chunks[3],
        3,
        block_type,
        region == 3,
        &response_lines,
    );
}

/// Convert a `RegionEdit` sub-Document cursor into a visual `(row,
/// col)` pair counted from the start of the buffer text. Works for
/// single-line and multi-line fields since the doc is prose-only.
fn edit_cursor_row_col(edit: &RegionEdit) -> (usize, usize) {
    let offset = match edit.doc.cursor() {
        crate::buffer::Cursor::InProse { offset, .. } => offset,
        crate::buffer::Cursor::InBlock { offset, .. } => offset,
        crate::buffer::Cursor::InBlockResult { .. } => 0,
    };
    let text = edit.current_text();
    let mut row = 0usize;
    let mut col = 0usize;
    for ch in text.chars().take(offset) {
        if ch == '\n' {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (row, col)
}

#[allow(clippy::too_many_arguments)]
fn render_request_region(
    frame: &mut Frame,
    area: Rect,
    block_type: &str,
    focused: bool,
    editing: bool,
    method: &str,
    url: &str,
    edit: Option<&RegionEdit>,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let block_widget = region_block(block_type, 0, focused, editing);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let prefix = format!(" {method}  ");
    let prefix_w = prefix.chars().count() as u16;
    let value_style = if focused {
        Style::default()
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::UNDERLINED)
    } else {
        Style::default()
    };
    let line = Line::from(vec![
        Span::raw(prefix),
        Span::styled(url.to_string(), value_style),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
    if let Some(edit) = edit {
        let (_row, col) = edit_cursor_row_col(edit);
        let cx = inner.x.saturating_add(prefix_w).saturating_add(col as u16);
        if cx < inner.x + inner.width && inner.height > 0 {
            frame.set_cursor_position((cx, inner.y));
        }
        // Paint the visual selection background on top of the URL
        // text. The text area starts at `inner.x + prefix_w`, so we
        // offset the overlay rect to match.
        if let Some(overlay) = visual_overlay {
            let text_area = ratatui::layout::Rect {
                x: inner.x.saturating_add(prefix_w),
                y: inner.y,
                width: inner.width.saturating_sub(prefix_w),
                height: inner.height,
            };
            crate::ui::overlay_visual_selection(
                frame,
                text_area,
                &edit.doc,
                0,
                overlay,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_headers_region(
    frame: &mut Frame,
    area: Rect,
    block_type: &str,
    focused: bool,
    parsed: &ParsedView,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let editing = pane_focused
        && focused
        && pane
            .block_edit
            .as_ref()
            .map(|e| {
                matches!(
                    e.field,
                    EditField::HttpHeaderKey(_) | EditField::HttpHeaderValue(_)
                )
            })
            .unwrap_or(false);
    let block_widget = region_block(block_type, 1, focused, editing);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    if parsed.headers.is_empty() && pane.block_edit.is_none() {
        let muted = Style::default().fg(crate::ui::palette::muted());
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("(no headers)", muted))),
            inner,
        );
        return;
    }
    let key_w: u16 = parsed
        .headers
        .iter()
        .map(|(k, _)| k.chars().count() as u16)
        .max()
        .unwrap_or(8)
        .max(8);
    let cursor_row = if focused { pane.block_row } else { usize::MAX };
    let cursor_col = if focused { pane.block_col } else { usize::MAX };
    let edit_row_col = pane.block_edit.as_ref().and_then(|e| match e.field {
        EditField::HttpHeaderKey(r) => Some((r, 0usize)),
        EditField::HttpHeaderValue(r) => Some((r, 1usize)),
        _ => None,
    });
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(parsed.headers.len());
    for (i, (k, v)) in parsed.headers.iter().enumerate() {
        let mut key_text = k.clone();
        let mut value_text = v.clone();
        if let Some((row, col)) = edit_row_col {
            if row == i {
                let buf = pane
                    .block_edit
                    .as_ref()
                    .map(|e| e.current_text())
                    .unwrap_or_default();
                if col == 0 {
                    key_text = buf;
                } else {
                    value_text = buf;
                }
            }
        }
        let key_focused = cursor_row == i && cursor_col == 0;
        let value_focused = cursor_row == i && cursor_col == 1;
        let key_style = field_style(key_focused);
        let value_style = field_style(value_focused);
        let padded_key = format!("{key_text:<width$}", width = key_w as usize);
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(padded_key, key_style),
            Span::raw("  "),
            Span::styled(value_text, value_style),
        ]));
    }
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
    // Place the terminal caret at the edited field's cursor column,
    // accounting for the leading padding and the key column width.
    if let Some(edit) = pane.block_edit.as_ref() {
        let (row, col) = match edit.field {
            EditField::HttpHeaderKey(r) => (r, 0usize),
            EditField::HttpHeaderValue(r) => (r, 1usize),
            _ => return,
        };
        if row >= parsed.headers.len() {
            return;
        }
        let row_y = inner.y.saturating_add(row as u16);
        if row_y >= inner.y + inner.height {
            return;
        }
        let leading = 2u16;
        let (_doc_row, doc_col) = edit_cursor_row_col(edit);
        let cell_col = doc_col as u16;
        let base_x = if col == 0 {
            inner.x + leading
        } else {
            inner.x + leading + key_w + 2
        };
        let cx = base_x.saturating_add(cell_col);
        if cx < inner.x + inner.width {
            frame.set_cursor_position((cx, row_y));
        }
        // Visual selection overlay over the edited field's cell.
        if let Some(overlay) = visual_overlay {
            let cell_w = if col == 0 {
                key_w
            } else {
                inner.width.saturating_sub(leading + key_w + 2)
            };
            let cell_area = ratatui::layout::Rect {
                x: base_x,
                y: row_y,
                width: cell_w,
                height: 1,
            };
            crate::ui::overlay_visual_selection(
                frame,
                cell_area,
                &edit.doc,
                0,
                overlay,
            );
        }
    }
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

#[allow(clippy::too_many_arguments)]
fn render_db_regions(
    frame: &mut Frame,
    area: Rect,
    region: usize,
    parsed: &ParsedView,
    block_type: &str,
    pane: &Pane,
    pane_focused: bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Min(3),
        ])
        .split(area);
    let conn = parsed
        .connection
        .clone()
        .unwrap_or_else(|| "(no connection)".to_string());
    render_region(frame, chunks[0], 0, block_type, region == 0, &[conn]);
    render_multiline_region(
        frame,
        chunks[1],
        block_type,
        1,
        region == 1,
        &parsed.body,
        "(empty query)",
        pane,
        pane_focused,
        |f| matches!(f, EditField::DbQuery),
        visual_overlay,
    );
    let result_lines: Vec<String> = if parsed.cached.is_empty() {
        vec!["(no result — run with Alt+R)".to_string()]
    } else {
        parsed.cached.lines().map(str::to_string).collect()
    };
    render_region(frame, chunks[2], 2, block_type, region == 2, &result_lines);
}

/// Render a region whose value is a multi-line string (HTTP body / DB
/// query). When the pane is in EDIT for the matching field, paints the
/// `MultilineBuffer` contents in place of the disk value and places the
/// terminal caret at the buffer's (row, col).
#[allow(clippy::too_many_arguments)]
fn render_multiline_region(
    frame: &mut Frame,
    area: Rect,
    block_type: &str,
    index: usize,
    focused: bool,
    fallback: &str,
    placeholder: &str,
    pane: &Pane,
    pane_focused: bool,
    field_matches: impl Fn(&EditField) -> bool,
    visual_overlay: Option<crate::ui::VisualOverlay>,
) {
    let editing = pane_focused
        && focused
        && pane
            .block_edit
            .as_ref()
            .map(|e| field_matches(&e.field))
            .unwrap_or(false);
    let block_widget = region_block(block_type, index, focused, editing);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let active_edit = pane
        .block_edit
        .as_ref()
        .filter(|e| field_matches(&e.field));
    let (lines, caret): (Vec<String>, Option<(u16, u16)>) = if let Some(edit) = active_edit {
        let text = edit.current_text();
        let mut ls: Vec<String> = text.split('\n').map(str::to_string).collect();
        if ls.is_empty() {
            ls.push(String::new());
        }
        let (row, col) = edit_cursor_row_col(edit);
        let cy = inner.y.saturating_add(row as u16);
        let cx = inner.x.saturating_add(col as u16);
        (ls, Some((cx, cy)))
    } else if fallback.is_empty() {
        (vec![placeholder.to_string()], None)
    } else {
        (fallback.lines().map(str::to_string).collect(), None)
    };
    let rendered: Vec<Line<'static>> = lines
        .iter()
        .map(|l| Line::from(Span::raw(l.clone())))
        .collect();
    frame.render_widget(Paragraph::new(rendered).wrap(Wrap { trim: false }), inner);
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

struct ParsedView {
    method: Option<String>,
    url: Option<String>,
    headers: Vec<(String, String)>,
    body: String,
    connection: Option<String>,
    cached: String,
    raw: String,
}

fn load_view(
    vault_path: &Path,
    file: &FileBlocks,
    block: &BlockMeta,
    pane: &Pane,
) -> ParsedView {
    // Per-pane draft wins over disk so committed edits are reflected
    // before save (otherwise the renderer would still show stale
    // values after Esc). Draft and disk parse share the same
    // ParsedBlock shape, so the view mapping is identical.
    if let Some(draft) = pane.block_draft.as_ref() {
        if draft.block_line_start == block.line_start && draft.block.block_type == block.block_type
        {
            let raw = httui_core::blocks::serialize_block(&draft.block);
            return parsed_to_view(&draft.block, raw);
        }
    }
    let Ok(raw) = httui_core::fs::read_note(
        &vault_path.to_string_lossy(),
        &file.path.to_string_lossy(),
    ) else {
        return ParsedView::empty();
    };
    let parsed = httui_core::blocks::parse_blocks(&raw);
    let Some(p) = parsed.iter().find(|p| {
        p.line_start == block.line_start && p.block_type == block.block_type
    }) else {
        return ParsedView::empty();
    };
    let lines: Vec<&str> = raw.lines().collect();
    let end = p.line_end.min(lines.len().saturating_sub(1));
    let start = p.line_start.min(end);
    let raw_block = lines[start..=end].join("\n");
    parsed_to_view(p, raw_block)
}

fn parsed_to_view(p: &httui_core::blocks::parser::ParsedBlock, raw: String) -> ParsedView {
    let method = p
        .params
        .get("method")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let url = p
        .params
        .get("url")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let headers = p
        .params
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|h| {
                    let k = h
                        .get("key")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let v = h
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    (k, v)
                })
                .collect()
        })
        .unwrap_or_default();
    let body = p
        .params
        .get("body")
        .and_then(|v| v.as_str())
        .or_else(|| p.params.get("query").and_then(|v| v.as_str()))
        .unwrap_or("")
        .trim_end_matches('\n')
        .to_string();
    let connection = p
        .params
        .get("connection")
        .or_else(|| p.params.get("connection_id"))
        .and_then(|v| v.as_str())
        .map(str::to_string);
    ParsedView {
        method,
        url,
        headers,
        body,
        connection,
        cached: String::new(),
        raw,
    }
}

impl ParsedView {
    fn empty() -> Self {
        Self {
            method: None,
            url: None,
            headers: Vec::new(),
            body: String::new(),
            connection: None,
            cached: String::new(),
            raw: String::new(),
        }
    }
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

fn badge_text(block_type: &str) -> String {
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

    #[test]
    fn badge_text_classifies_kinds() {
        assert_eq!(badge_text("http"), " HTTP ");
        assert_eq!(badge_text("db-postgres"), " DB ");
        assert_eq!(badge_text("custom"), " custom ");
    }
}
