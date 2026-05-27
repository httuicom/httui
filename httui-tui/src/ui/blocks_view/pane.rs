use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{region_label, BlockMeta, BlocksWorkspace, FileBlocks};
use crate::pane::Pane;

pub(super) fn render_leaf(
    frame: &mut Frame,
    area: Rect,
    leaf: &Pane,
    focused: bool,
    workspace: Option<&BlocksWorkspace>,
    vault: &std::path::Path,
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

    render_block(frame, inner, file, block, leaf.block_region, vault);
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

fn render_block(
    frame: &mut Frame,
    area: Rect,
    file: &FileBlocks,
    block: &BlockMeta,
    region: usize,
    vault_path: &Path,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    render_header(frame, chunks[0], file, block);

    let parsed = load_parsed(vault_path, file, block);
    if block.block_type == "http" {
        render_http_regions(frame, chunks[1], region, &parsed, &block.block_type);
    } else if block.block_type.starts_with("db") {
        render_db_regions(frame, chunks[1], region, &parsed, &block.block_type);
    } else {
        render_fallback(frame, chunks[1], &parsed.raw);
    }
}

fn render_header(frame: &mut Frame, area: Rect, file: &FileBlocks, block: &BlockMeta) {
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
    let line = Line::from(vec![
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
        Span::styled("  ·  ", muted),
        Span::styled(file.display.clone(), muted),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_http_regions(
    frame: &mut Frame,
    area: Rect,
    region: usize,
    parsed: &ParsedView,
    block_type: &str,
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
    render_region(
        frame,
        chunks[0],
        0,
        block_type,
        region == 0,
        &[format!(
            " {}  {}",
            parsed.method.as_deref().unwrap_or("GET"),
            parsed.url.as_deref().unwrap_or("")
        )],
    );
    let header_lines: Vec<String> = if parsed.headers.is_empty() {
        vec!["(no headers)".to_string()]
    } else {
        parsed
            .headers
            .iter()
            .map(|(k, v)| format!("  {k}: {v}"))
            .collect()
    };
    render_region(frame, chunks[1], 1, block_type, region == 1, &header_lines);
    let body_lines: Vec<String> = if parsed.body.is_empty() {
        vec!["(no body)".to_string()]
    } else {
        parsed.body.lines().map(str::to_string).collect()
    };
    render_region(frame, chunks[2], 2, block_type, region == 2, &body_lines);
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

fn render_db_regions(
    frame: &mut Frame,
    area: Rect,
    region: usize,
    parsed: &ParsedView,
    block_type: &str,
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
    let query_lines: Vec<String> = if parsed.body.is_empty() {
        vec!["(empty query)".to_string()]
    } else {
        parsed.body.lines().map(str::to_string).collect()
    };
    render_region(frame, chunks[1], 1, block_type, region == 1, &query_lines);
    let result_lines: Vec<String> = if parsed.cached.is_empty() {
        vec!["(no result — run with Alt+R)".to_string()]
    } else {
        parsed.cached.lines().map(str::to_string).collect()
    };
    render_region(frame, chunks[2], 2, block_type, region == 2, &result_lines);
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

fn load_parsed(vault_path: &Path, file: &FileBlocks, block: &BlockMeta) -> ParsedView {
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
        raw: raw_block,
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
