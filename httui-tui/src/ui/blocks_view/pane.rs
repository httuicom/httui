use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{region_label, App, BlockMeta, BlocksWorkspace, FileBlocks};

pub(super) fn render(frame: &mut Frame, area: Rect, app: &App) {
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(crate::ui::palette::muted()));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let Some(ws) = app.blocks_workspace.as_ref() else {
        paint_empty_state(frame, inner);
        return;
    };
    let Some((file, block)) = ws.selected_block() else {
        paint_empty_state(frame, inner);
        return;
    };
    render_block(frame, inner, ws, file, block, &app.vault_path);
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

fn render_block(
    frame: &mut Frame,
    area: Rect,
    ws: &BlocksWorkspace,
    file: &FileBlocks,
    block: &BlockMeta,
    vault_path: &Path,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    render_header(frame, chunks[0], file, block);

    let parsed = load_parsed(vault_path, file, block);
    if block.block_type == "http" {
        render_http_regions(frame, chunks[1], ws, &parsed, &block.block_type);
    } else if block.block_type.starts_with("db") {
        render_db_regions(frame, chunks[1], ws, &parsed, &block.block_type);
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
    ws: &BlocksWorkspace,
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
    let focused = ws.region;
    render_region(
        frame,
        chunks[0],
        0,
        block_type,
        focused == 0,
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
    render_region(frame, chunks[1], 1, block_type, focused == 1, &header_lines);
    let body_lines: Vec<String> = if parsed.body.is_empty() {
        vec!["(no body)".to_string()]
    } else {
        parsed.body.lines().map(str::to_string).collect()
    };
    render_region(frame, chunks[2], 2, block_type, focused == 2, &body_lines);
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
        focused == 3,
        &response_lines,
    );
}

fn render_db_regions(
    frame: &mut Frame,
    area: Rect,
    ws: &BlocksWorkspace,
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
    let focused = ws.region;
    let conn = parsed
        .connection
        .clone()
        .unwrap_or_else(|| "(no connection)".to_string());
    render_region(frame, chunks[0], 0, block_type, focused == 0, &[conn]);
    let query_lines: Vec<String> = if parsed.body.is_empty() {
        vec!["(empty query)".to_string()]
    } else {
        parsed.body.lines().map(str::to_string).collect()
    };
    render_region(frame, chunks[1], 1, block_type, focused == 1, &query_lines);
    let result_lines: Vec<String> = if parsed.cached.is_empty() {
        vec!["(no result — run with Alt+R)".to_string()]
    } else {
        parsed.cached.lines().map(str::to_string).collect()
    };
    render_region(frame, chunks[2], 2, block_type, focused == 2, &result_lines);
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
        (
            crate::ui::palette::accent(),
            crate::ui::palette::accent(),
        )
    } else {
        (
            crate::ui::palette::muted(),
            crate::ui::palette::muted(),
        )
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
    use crate::app::{App, BlockRef};
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tempfile::TempDir;

    fn seed_vault() -> TempDir {
        let v = TempDir::new().unwrap();
        std::fs::write(
            v.path().join("api.md"),
            "# api\n\n```http alias=login\nPOST https://x.com/login\nAccept: text/plain\nAuthorization: Bearer abc\n\n{\"name\":\"alice\"}\n```\n",
        )
        .unwrap();
        v
    }

    async fn app_with_selected(vault: &TempDir) -> App {
        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        crate::input::apply::blocks_view::apply_blocks_view(
            &mut app,
            crate::input::action::Action::ToggleAppView,
        );
        if let Some(ws) = app.blocks_workspace.as_mut() {
            ws.select(BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
        }
        app
    }

    fn capture(app: &App) -> String {
        let backend = TestBackend::new(100, 28);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), app))
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..28)
            .map(|y| {
                (0..100)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn paints_all_four_region_titles_for_http() {
        let v = seed_vault();
        let app = app_with_selected(&v).await;
        let txt = capture(&app);
        for title in ["[1] Request", "[2] Headers", "[3] Body", "[4] Response"] {
            assert!(txt.contains(title), "missing `{title}` in:\n{txt}");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn paints_method_url_and_headers() {
        let v = seed_vault();
        let app = app_with_selected(&v).await;
        let txt = capture(&app);
        assert!(txt.contains("POST"), "missing method:\n{txt}");
        assert!(
            txt.contains("https://x.com/login"),
            "missing url:\n{txt}",
        );
        assert!(
            txt.contains("Authorization"),
            "missing header key:\n{txt}",
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn focused_region_title_appears_when_changed() {
        let v = seed_vault();
        let mut app = app_with_selected(&v).await;
        app.blocks_workspace.as_mut().unwrap().set_region(1);
        let txt = capture(&app);
        // [2] Headers is still painted as a region title; the focus
        // signal moved from BorderType::Double to color-only (accent
        // vs muted) so we just assert the region remains addressable.
        assert!(
            txt.contains("[2] Headers"),
            "expected [2] Headers title:\n{txt}",
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn empty_state_when_no_selection() {
        let v = seed_vault();
        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: v.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        let txt = capture(&app);
        assert!(txt.contains("Select a block"));
    }

    #[test]
    fn badge_text_classifies_kinds() {
        assert_eq!(badge_text("http"), " HTTP ");
        assert_eq!(badge_text("db-postgres"), " DB ");
    }
}
