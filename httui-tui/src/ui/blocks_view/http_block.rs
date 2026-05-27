use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};

use crate::app::BlocksViewState;
use crate::buffer::block::BlockNode;

use super::region_title;

const REGION_REQUEST: usize = 0;
const REGION_HEADERS: usize = 1;
const REGION_BODY: usize = 2;
const REGION_RESPONSE: usize = 3;

const HEADER_BAR_HEIGHT: u16 = 1;

pub(super) fn render(frame: &mut Frame, area: Rect, state: &BlocksViewState, block: &BlockNode) {
    if area.height < 4 || area.width < 10 {
        return;
    }

    let header_rect = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: HEADER_BAR_HEIGHT,
    };
    render_header_bar(frame, header_rect, block);

    let body_rect = Rect {
        x: area.x,
        y: area.y.saturating_add(HEADER_BAR_HEIGHT),
        width: area.width,
        height: area.height.saturating_sub(HEADER_BAR_HEIGHT),
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(4),
            Constraint::Min(4),
            Constraint::Min(4),
        ])
        .split(body_rect);

    render_request_region(frame, chunks[0], block, state.region == REGION_REQUEST);
    render_headers_region(frame, chunks[1], block, state.region == REGION_HEADERS);
    render_body_region(frame, chunks[2], block, state.region == REGION_BODY);
    render_response_region(frame, chunks[3], block, state.region == REGION_RESPONSE);
}

fn render_header_bar(frame: &mut Frame, area: Rect, block: &BlockNode) {
    if area.width == 0 {
        return;
    }
    let alias = block.alias.clone().unwrap_or_else(|| "—".into());
    let method = method_of(block);
    let bg = Style::default().bg(crate::ui::palette::block_body_bg());
    let line = Line::from(vec![
        Span::raw(" "),
        Span::styled(
            " HTTP ",
            Style::default()
                .bg(crate::ui::palette::popup_border_accent())
                .fg(crate::ui::palette::popup_bg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", bg),
        Span::styled(
            alias,
            bg.fg(crate::ui::palette::foreground())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  ", bg.fg(crate::ui::palette::muted())),
        Span::styled(
            format!(" {method} "),
            Style::default()
                .bg(crate::ui::palette::accent())
                .fg(crate::ui::palette::popup_bg())
                .add_modifier(Modifier::BOLD),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_request_region(frame: &mut Frame, area: Rect, block: &BlockNode, focused: bool) {
    let block_widget = region_title(REGION_REQUEST, "Request", focused);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let method = method_of(block);
    let url = url_of(block);
    // UNDERLINED degrades on terminals without underline support; the
    // double-line border is the redundant focus cue.
    let url_style = if focused {
        Style::default()
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::UNDERLINED)
    } else {
        Style::default().fg(crate::ui::palette::foreground())
    };
    let line = Line::from(vec![
        Span::styled(
            format!(" {method} "),
            Style::default()
                .bg(crate::ui::palette::accent())
                .fg(crate::ui::palette::popup_bg())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(url, url_style),
    ]);
    frame.render_widget(Paragraph::new(line), inner);
}

fn render_headers_region(frame: &mut Frame, area: Rect, block: &BlockNode, focused: bool) {
    let block_widget = region_title(REGION_HEADERS, "Headers", focused);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let headers = headers_of(block);
    if headers.is_empty() {
        let muted = Style::default().fg(crate::ui::palette::muted());
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("(no headers)", muted))),
            inner,
        );
        return;
    }
    let lines: Vec<Line<'static>> = headers
        .into_iter()
        .map(|(key, value)| {
            Line::from(vec![
                Span::styled(
                    format!("  {key}"),
                    Style::default().fg(crate::ui::palette::accent()),
                ),
                Span::styled(": ", Style::default().fg(crate::ui::palette::muted())),
                Span::raw(value),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_body_region(frame: &mut Frame, area: Rect, block: &BlockNode, focused: bool) {
    let block_widget = region_title(REGION_BODY, "Body", focused);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let body = body_of(block);
    if body.is_empty() {
        let muted = Style::default().fg(crate::ui::palette::muted());
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled("(no body)", muted))),
            inner,
        );
        return;
    }
    let lines: Vec<Line<'static>> = body
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn render_response_region(frame: &mut Frame, area: Rect, block: &BlockNode, focused: bool) {
    let block_widget = region_title(REGION_RESPONSE, "Response", focused);
    let inner = block_widget.inner(area);
    frame.render_widget(block_widget, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let muted = Style::default().fg(crate::ui::palette::muted());
    let placeholder = match &block.cached_result {
        Some(value) => value
            .get("body")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| serde_json::to_string_pretty(value).unwrap_or_default()),
        None => "(no response — run with Alt+R)".to_string(),
    };
    let lines: Vec<Line<'static>> = placeholder
        .lines()
        .map(|l| Line::from(Span::styled(l.to_string(), muted)))
        .collect();
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

fn method_of(block: &BlockNode) -> String {
    block
        .params
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_string()
}

fn url_of(block: &BlockNode) -> String {
    block
        .params
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

fn headers_of(block: &BlockNode) -> Vec<(String, String)> {
    let Some(arr) = block.params.get("headers").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
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
}

fn body_of(block: &BlockNode) -> String {
    block
        .params
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_end_matches('\n')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::BlocksViewKind;
    use crate::buffer::block::{BlockId, ExecutionState};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use ropey::Rope;
    use serde_json::json;
    use std::path::PathBuf;

    fn block_with(
        method: &str,
        url: &str,
        headers: serde_json::Value,
        body: &str,
    ) -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: Rope::from_str("```http alias=q\n```"),
            block_type: "http".into(),
            alias: Some("q".into()),
            display_mode: None,
            params: json!({
                "method": method,
                "url": url,
                "headers": headers,
                "body": body,
            }),
            state: ExecutionState::Idle,
            cached_result: None,
        }
    }

    fn render_to_lines(state: &BlocksViewState, block: &BlockNode) -> Vec<String> {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), state, block))
            .unwrap();
        let buf = terminal.backend().buffer();
        (0..24)
            .map(|y| {
                (0..80)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn method_url_headers_body_helpers_read_params() {
        let b = block_with(
            "POST",
            "https://api.x.com/u",
            json!([{"key":"Authorization","value":"Bearer xyz"}]),
            "{\"name\":\"alice\"}",
        );
        assert_eq!(method_of(&b), "POST");
        assert_eq!(url_of(&b), "https://api.x.com/u");
        assert_eq!(
            headers_of(&b),
            vec![("Authorization".into(), "Bearer xyz".into())]
        );
        assert_eq!(body_of(&b), "{\"name\":\"alice\"}");
    }

    #[test]
    fn missing_method_defaults_to_get() {
        let mut b = block_with("GET", "https://x.com", json!([]), "");
        b.params = json!({"url": "https://x.com"});
        assert_eq!(method_of(&b), "GET");
    }

    #[test]
    fn render_paints_all_four_region_titles() {
        let state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        let block = block_with("GET", "https://x.com", json!([]), "");
        let rows = render_to_lines(&state, &block);
        let joined = rows.join("\n");
        for title in ["[1] Request", "[2] Headers", "[3] Body", "[4] Response"] {
            assert!(joined.contains(title), "missing `{title}` in:\n{joined}");
        }
    }

    #[test]
    fn render_paints_method_and_url() {
        let state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        let block = block_with("PUT", "https://api.x.com/users/42", json!([]), "");
        let rows = render_to_lines(&state, &block);
        let joined = rows.join("\n");
        assert!(joined.contains("PUT"), "method missing in:\n{joined}");
        assert!(
            joined.contains("https://api.x.com/users/42"),
            "url missing in:\n{joined}",
        );
    }

    #[test]
    fn render_empty_headers_shows_placeholder() {
        let state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        let block = block_with("GET", "https://x.com", json!([]), "");
        let rows = render_to_lines(&state, &block);
        let joined = rows.join("\n");
        assert!(joined.contains("(no headers)"));
    }

    #[test]
    fn render_empty_body_shows_placeholder() {
        let state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        let block = block_with("GET", "https://x.com", json!([]), "");
        let rows = render_to_lines(&state, &block);
        let joined = rows.join("\n");
        assert!(joined.contains("(no body)"));
    }

    #[test]
    fn render_no_cached_response_shows_run_hint() {
        let state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        let block = block_with("GET", "https://x.com", json!([]), "");
        let rows = render_to_lines(&state, &block);
        let joined = rows.join("\n");
        assert!(joined.contains("(no response"), "missing run hint:\n{joined}");
    }

    #[test]
    fn render_cached_response_body_is_painted() {
        let state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        let mut block = block_with("GET", "https://x.com", json!([]), "");
        block.cached_result = Some(json!({"body": "ok-cached-marker"}));
        let rows = render_to_lines(&state, &block);
        let joined = rows.join("\n");
        assert!(
            joined.contains("ok-cached-marker"),
            "missing cached body in:\n{joined}",
        );
    }

    #[test]
    fn render_focused_region_paints_double_border() {
        let mut state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        state.set_region(REGION_HEADERS);
        let block = block_with(
            "GET",
            "https://x.com",
            json!([{"key":"X","value":"1"}]),
            "",
        );
        let rows = render_to_lines(&state, &block);
        let joined = rows.join("\n");
        assert!(
            joined.contains('═'),
            "expected double-line border when [2] focused:\n{joined}",
        );
    }

    #[test]
    fn render_with_tiny_area_does_not_panic() {
        let state = BlocksViewState::new(PathBuf::from("api.md"), 0, BlocksViewKind::Http);
        let block = block_with("GET", "https://x.com", json!([]), "");
        let backend = TestBackend::new(20, 3);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), &state, &block))
            .unwrap();
    }
}
