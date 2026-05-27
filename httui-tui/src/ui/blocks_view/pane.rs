use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, BlockMeta, FileBlocks};

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
    render_block(frame, inner, file, block, &app.vault_path);
}

fn paint_empty_state(frame: &mut Frame, area: Rect) {
    if area.height < 2 {
        return;
    }
    let muted = Style::default().fg(crate::ui::palette::muted());
    let mid = area.y + area.height / 2;
    let lines = [
        ("Select a block from the sidebar", true),
        ("j/k navigate · Enter expand or open · Alt+M back to DOC", false),
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
    file: &FileBlocks,
    block: &BlockMeta,
    vault_path: &Path,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(area);
    render_header(frame, chunks[0], file, block);
    render_body(frame, chunks[1], file, block, vault_path);
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

fn render_body(
    frame: &mut Frame,
    area: Rect,
    file: &FileBlocks,
    block: &BlockMeta,
    vault_path: &Path,
) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let text = read_block_text(vault_path, file, block);
    let lines: Vec<Line<'static>> = text
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
}

fn read_block_text(vault_path: &Path, file: &FileBlocks, block: &BlockMeta) -> String {
    let Ok(raw) = httui_core::fs::read_note(
        &vault_path.to_string_lossy(),
        &file.path.to_string_lossy(),
    ) else {
        return "(failed to read file)".to_string();
    };
    let parsed = httui_core::blocks::parse_blocks(&raw);
    let Some(p) = parsed.iter().find(|p| {
        p.line_start == block.line_start && p.block_type == block.block_type
    }) else {
        return "(block not found — file may have changed)".to_string();
    };
    let lines: Vec<&str> = raw.lines().collect();
    let end = p.line_end.min(lines.len().saturating_sub(1));
    let start = p.line_start.min(end);
    lines[start..=end].join("\n")
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
    use crate::app::App;
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
            "# api\n\n```http alias=login\nGET https://x.com\nAccept: text/plain\n```\n",
        )
        .unwrap();
        v
    }

    async fn app_for(vault: &TempDir) -> App {
        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        App::new(Config::default(), resolved, pool)
    }

    fn capture(app: &App) -> String {
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), app))
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..20)
            .map(|y| {
                (0..80)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn empty_state_when_nothing_selected() {
        let v = seed_vault();
        let app = app_for(&v).await;
        let txt = capture(&app);
        assert!(txt.contains("Select a block"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn renders_block_header_and_body_when_selected() {
        let v = seed_vault();
        let mut app = app_for(&v).await;
        crate::input::apply::blocks_view::apply_blocks_view(
            &mut app,
            crate::input::action::Action::ToggleAppView,
        );
        if let Some(ws) = app.blocks_workspace.as_mut() {
            ws.selected = Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
        }
        let txt = capture(&app);
        assert!(txt.contains("HTTP"), "missing HTTP badge in:\n{txt}");
        assert!(txt.contains("login"), "missing alias label in:\n{txt}");
        assert!(
            txt.contains("GET https://x.com"),
            "missing request line in:\n{txt}",
        );
    }

    #[test]
    fn badge_text_classifies_known_types() {
        assert_eq!(badge_text("http"), " HTTP ");
        assert_eq!(badge_text("db-postgres"), " DB ");
        assert_eq!(badge_text("custom"), " custom ");
    }
}
