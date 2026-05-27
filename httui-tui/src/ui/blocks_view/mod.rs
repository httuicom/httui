use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::BlocksViewState;
use crate::buffer::{block::BlockNode, Document, Segment};

mod http_block;

pub fn render(frame: &mut Frame, area: Rect, state: &BlocksViewState, document: Option<&Document>) {
    frame.render_widget(Clear, area);
    let canvas = Block::default().style(Style::default().bg(crate::ui::palette::background()));
    frame.render_widget(canvas, area);

    let Some(doc) = document else {
        paint_placeholder(frame, area, "No file open");
        return;
    };
    let Some(block) = resolve_block(doc, state.segment_idx) else {
        paint_placeholder(frame, area, "Block missing (document mutated)");
        return;
    };

    match state.kind {
        crate::app::BlocksViewKind::Http => http_block::render(frame, area, state, block),
    }
}

pub(super) fn resolve_block(doc: &Document, segment_idx: usize) -> Option<&BlockNode> {
    match doc.segments().get(segment_idx)? {
        Segment::Block(b) => Some(b),
        Segment::Prose(_) => None,
    }
}

fn paint_placeholder(frame: &mut Frame, area: Rect, text: &str) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let y = area.y + area.height / 2;
    let line = Line::from(vec![Span::styled(
        text.to_string(),
        Style::default().fg(crate::ui::palette::muted()),
    )]);
    let row = Rect {
        x: area.x,
        y,
        width: area.width,
        height: 1,
    };
    frame.render_widget(Paragraph::new(line), row);
}

pub(super) fn region_border(focused: bool) -> (BorderType, Style) {
    if focused {
        (
            BorderType::Double,
            Style::default().fg(crate::ui::palette::accent()),
        )
    } else {
        (
            BorderType::Plain,
            Style::default().fg(crate::ui::palette::muted()),
        )
    }
}

pub(super) fn region_title(index: usize, label: &str, focused: bool) -> Block<'static> {
    let (border_type, style) = region_border(focused);
    let title_style = if focused {
        Style::default()
            .fg(crate::ui::palette::accent())
            .add_modifier(ratatui::style::Modifier::BOLD)
    } else {
        Style::default().fg(crate::ui::palette::muted())
    };
    Block::default()
        .borders(Borders::ALL)
        .border_type(border_type)
        .border_style(style)
        .title(Span::styled(
            format!(" [{}] {label} ", index + 1),
            title_style,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::BlocksViewKind;
    use crate::buffer::block::{BlockId, BlockNode, ExecutionState};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use ropey::Rope;
    use serde_json::json;
    use std::path::PathBuf;

    fn http_block() -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: Rope::from_str("```http alias=q\nGET https://x.com\n```"),
            block_type: "http".into(),
            alias: Some("q".into()),
            display_mode: None,
            params: json!({"method":"GET","url":"https://x.com","headers":[],"body":""}),
            state: ExecutionState::Idle,
            cached_result: None,
        }
    }

    #[test]
    fn region_border_focused_uses_accent_double() {
        let (bt, _) = region_border(true);
        assert!(matches!(bt, BorderType::Double));
        let (bt, _) = region_border(false);
        assert!(matches!(bt, BorderType::Plain));
    }

    #[test]
    fn region_title_includes_one_indexed_number() {
        let block = region_title(0, "Request", false);
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| f.render_widget(block, f.area()))
            .unwrap();
        let buf = terminal.backend().buffer();
        let cell_text: String = (0..40).map(|x| buf[(x, 0)].symbol().to_string()).collect();
        assert!(cell_text.contains("[1] Request"), "got `{cell_text}`");
    }

    #[test]
    fn render_no_document_paints_placeholder_without_panic() {
        let state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        let backend = TestBackend::new(60, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &state, None))
            .unwrap();
        let buf = terminal.backend().buffer();
        let mid = 10 / 2;
        let row_text: String = (0..60).map(|x| buf[(x, mid)].symbol().to_string()).collect();
        assert!(row_text.contains("No file open"), "got `{row_text}`");
    }

    #[test]
    fn resolve_block_returns_none_for_prose_index() {
        let doc = Document::from_markdown("just text\n").unwrap();
        assert!(resolve_block(&doc, 0).is_none());
    }

    #[test]
    fn resolve_block_returns_block_for_block_index() {
        let md = "# t\n\n```http alias=q\nGET https://x.com\n```\n";
        let doc = Document::from_markdown(md).unwrap();
        let block_idx = doc
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        assert!(resolve_block(&doc, block_idx).is_some());
    }

    #[test]
    fn render_http_smoke_test_paints_request_region() {
        let state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        let block = http_block();
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                http_block::render(f, area, &state, &block);
            })
            .unwrap();
        let buf = terminal.backend().buffer();
        let joined: String = (0..20)
            .map(|y| {
                (0..80)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("[1] Request"),
            "expected [1] Request somewhere; got `{joined}`",
        );
    }
}
