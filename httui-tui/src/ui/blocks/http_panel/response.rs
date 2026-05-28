//! Response-panel rendering: per-tab line builders (Body / Headers /
//! Cookies / Stats / Raw) plus the dispatch + viewport height helper.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::buffer::block::BlockNode;

use super::format_byte_size;
use super::highlight::{highlight_json_line, highlight_xml_line};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BodyLang {
    Json,
    Xml,
    Html,
    Plain,
}

/// Tabs map for HTTP: re-uses ResultPanelTab so the keymap that
/// cycles tabs (`gt`/`gT`) keeps working — labels just change.
pub(crate) fn render_http_response_panel(
    frame: &mut Frame,
    area: Rect,
    b: &BlockNode,
    tab: crate::app::ResultPanelTab,
) {
    use crate::app::ResultPanelTab;
    let lines = match tab {
        ResultPanelTab::Result => http_response_body_lines(b),
        ResultPanelTab::Messages => http_response_headers_lines(b),
        ResultPanelTab::Plan => http_response_cookies_lines(b),
        ResultPanelTab::Stats => http_response_stats_lines(b),
        ResultPanelTab::Raw => http_response_raw_lines(b),
    };
    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) fn http_response_body_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no body)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(result) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let body = result.get("body");
    let (text, body_is_json) = match body {
        Some(serde_json::Value::String(s)) => (s.clone(), false),
        Some(other) => (
            serde_json::to_string_pretty(other).unwrap_or_default(),
            true,
        ),
        None => return vec![placeholder],
    };
    if text.is_empty() {
        return vec![placeholder];
    }
    let lang = if body_is_json {
        BodyLang::Json
    } else {
        lang_from_content_type(content_type_of(result).as_deref())
    };
    match lang {
        BodyLang::Json => text
            .lines()
            .map(|l| Line::from(highlight_json_line(l)))
            .collect(),
        BodyLang::Xml | BodyLang::Html => text
            .lines()
            .map(|l| Line::from(highlight_xml_line(l)))
            .collect(),
        BodyLang::Plain => text.lines().map(|l| Line::from(l.to_string())).collect(),
    }
}

pub(super) fn content_type_of(result: &serde_json::Value) -> Option<String> {
    let arr = result.get("headers")?.as_array()?;
    arr.iter().find_map(|h| {
        let key = h.get("key")?.as_str()?;
        if key.eq_ignore_ascii_case("content-type") {
            h.get("value")?.as_str().map(String::from)
        } else {
            None
        }
    })
}

pub(super) fn lang_from_content_type(ct: Option<&str>) -> BodyLang {
    let Some(ct) = ct else { return BodyLang::Plain };
    let mime = ct
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if mime.contains("json") {
        BodyLang::Json
    } else if mime.contains("html") {
        BodyLang::Html
    } else if mime.contains("xml") || mime == "image/svg+xml" {
        BodyLang::Xml
    } else {
        BodyLang::Plain
    }
}

fn http_response_headers_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no headers)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(result) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let Some(headers) = result.get("headers").and_then(|v| v.as_array()) else {
        return vec![placeholder];
    };
    if headers.is_empty() {
        return vec![placeholder];
    }
    headers
        .iter()
        .map(|h| {
            let key = h.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
            Line::from(vec![
                Span::styled(format!(" {key}"), Style::default().fg(Color::Cyan)),
                Span::styled(": ", Style::default().fg(Color::DarkGray)),
                Span::raw(value.to_string()),
            ])
        })
        .collect()
}

fn http_response_cookies_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no cookies)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(result) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let Some(cookies) = result.get("cookies").and_then(|v| v.as_array()) else {
        return vec![placeholder];
    };
    if cookies.is_empty() {
        return vec![placeholder];
    }
    cookies
        .iter()
        .map(|c| {
            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let value = c.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let domain = c.get("domain").and_then(|v| v.as_str()).unwrap_or("");
            let mut spans = vec![
                Span::styled(format!(" {name}"), Style::default().fg(Color::Cyan)),
                Span::styled("=", Style::default().fg(Color::DarkGray)),
                Span::raw(value.to_string()),
            ];
            if !domain.is_empty() {
                spans.push(Span::styled(
                    format!("  ({domain})"),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            Line::from(spans)
        })
        .collect()
}

fn http_response_stats_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no stats)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(result) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let dim = Style::default().fg(Color::DarkGray);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let push_kv = |lines: &mut Vec<Line<'static>>, key: &str, value: String| {
        lines.push(Line::from(vec![
            Span::styled(format!(" {key}: "), dim),
            Span::raw(value),
        ]));
    };
    if let Some(status) = result.get("status").and_then(|v| v.as_u64()) {
        let txt = result
            .get("status_text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        push_kv(&mut lines, "status", format!("{status} {txt}"));
    }
    if let Some(timing) = result.get("timing") {
        if let Some(ms) = timing.get("total_ms").and_then(|v| v.as_u64()) {
            push_kv(&mut lines, "total", format!("{ms} ms"));
        }
        if let Some(ms) = timing.get("ttfb_ms").and_then(|v| v.as_u64()) {
            push_kv(&mut lines, "ttfb", format!("{ms} ms"));
        }
    }
    if let Some(bytes) = result.get("size_bytes").and_then(|v| v.as_u64()) {
        push_kv(&mut lines, "size", format_byte_size(bytes));
    }
    if lines.is_empty() {
        vec![placeholder]
    } else {
        lines
    }
}

/// Render the response as an HTTP-message: status line + headers +
/// blank + body. Mirrors how `curl -i` displays a response so users
/// can copy/eyeball the wire format. Body uses the same lang-by-CT
/// pick as the Body tab (JSON pretty, XML/HTML basic highlight,
/// otherwise raw).
pub(super) fn http_response_raw_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no response)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(result) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    // Status line.
    let status = result.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    let status_text = result
        .get("status_text")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    lines.push(Line::from(Span::styled(
        format!(" HTTP/1.1 {status} {status_text}"),
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    // Headers.
    if let Some(headers) = result.get("headers").and_then(|v| v.as_array()) {
        for h in headers {
            let key = h.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
            lines.push(Line::from(vec![
                Span::styled(format!(" {key}"), Style::default().fg(Color::Cyan)),
                Span::styled(": ", Style::default().fg(Color::DarkGray)),
                Span::raw(value.to_string()),
            ]));
        }
    }
    // Blank separator + body lines.
    lines.push(Line::from(""));
    let body_lines = http_response_body_lines(b);
    lines.extend(body_lines);
    lines
}

pub(super) fn http_response_panel_height(_b: &BlockNode) -> u16 {
    // Tab bar (1) + separator (1) + content viewport (8). Body
    // content scrolls beyond the viewport; we don't grow the card
    // unboundedly to fit a 50 KB JSON response.
    const VIEWPORT: u16 = 8;
    1 + 1 + VIEWPORT
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::{BlockId, ExecutionState};
    use serde_json::json;

    fn block_with_result(result: serde_json::Value) -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "http".into(),
            alias: None,
            display_mode: None,
            params: json!({}),
            state: ExecutionState::Idle,
            cached_result: Some(result),
        }
    }

    fn lines_text(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn body_lines_placeholder_when_no_cached_result() {
        let b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "http".into(),
            alias: None,
            display_mode: None,
            params: json!({}),
            state: ExecutionState::Idle,
            cached_result: None,
        };
        let lines = http_response_body_lines(&b);
        assert!(lines_text(&lines).contains("no body"));
    }

    #[test]
    fn body_lines_pretty_prints_json_value() {
        let b = block_with_result(json!({"body": {"k": 1}, "headers": []}));
        let text = lines_text(&http_response_body_lines(&b));
        assert!(text.contains("\"k\""));
    }

    #[test]
    fn body_lines_renders_string_body_with_lang_from_content_type() {
        let b = block_with_result(json!({
            "body": "<b>hi</b>",
            "headers": [{"key": "Content-Type", "value": "text/html"}],
        }));
        let text = lines_text(&http_response_body_lines(&b));
        assert!(text.contains("hi"));
    }

    #[test]
    fn body_lines_renders_xml_string_body() {
        let b = block_with_result(json!({
            "body": "<a/>",
            "headers": [{"key": "Content-Type", "value": "application/xml"}],
        }));
        let text = lines_text(&http_response_body_lines(&b));
        assert!(text.contains("a"));
    }

    #[test]
    fn body_lines_renders_plain_string_body() {
        let b = block_with_result(json!({
            "body": "hello world",
            "headers": [{"key": "Content-Type", "value": "text/plain"}],
        }));
        let text = lines_text(&http_response_body_lines(&b));
        assert!(text.contains("hello world"));
    }

    #[test]
    fn body_lines_placeholder_when_body_is_empty_string() {
        let b = block_with_result(json!({"body": "", "headers": []}));
        let text = lines_text(&http_response_body_lines(&b));
        assert!(text.contains("no body"));
    }

    #[test]
    fn body_lines_placeholder_when_body_field_missing() {
        let b = block_with_result(json!({"headers": []}));
        let text = lines_text(&http_response_body_lines(&b));
        assert!(text.contains("no body"));
    }

    #[test]
    fn headers_lines_placeholder_for_missing_or_empty() {
        let b = block_with_result(json!({}));
        assert!(lines_text(&http_response_headers_lines(&b)).contains("no headers"));
        let b = block_with_result(json!({"headers": []}));
        assert!(lines_text(&http_response_headers_lines(&b)).contains("no headers"));
    }

    #[test]
    fn headers_lines_renders_each_header() {
        let b = block_with_result(json!({
            "headers": [
                {"key": "X-One", "value": "a"},
                {"key": "X-Two", "value": "b"},
            ]
        }));
        let text = lines_text(&http_response_headers_lines(&b));
        assert!(text.contains("X-One"));
        assert!(text.contains("X-Two"));
    }

    #[test]
    fn cookies_lines_placeholder_for_missing_or_empty() {
        let b = block_with_result(json!({}));
        assert!(lines_text(&http_response_cookies_lines(&b)).contains("no cookies"));
        let b = block_with_result(json!({"cookies": []}));
        assert!(lines_text(&http_response_cookies_lines(&b)).contains("no cookies"));
    }

    #[test]
    fn cookies_lines_with_and_without_domain() {
        let b = block_with_result(json!({
            "cookies": [
                {"name": "sess", "value": "abc"},
                {"name": "track", "value": "1", "domain": "x.com"},
            ]
        }));
        let text = lines_text(&http_response_cookies_lines(&b));
        assert!(text.contains("sess"));
        assert!(text.contains("track"));
        assert!(text.contains("x.com"));
    }

    #[test]
    fn stats_lines_emits_status_total_ttfb_and_size() {
        let b = block_with_result(json!({
            "status": 200,
            "status_text": "OK",
            "timing": {"total_ms": 12, "ttfb_ms": 7},
            "size_bytes": 2048,
        }));
        let text = lines_text(&http_response_stats_lines(&b));
        assert!(text.contains("status"));
        assert!(text.contains("200"));
        assert!(text.contains("total"));
        assert!(text.contains("ttfb"));
        assert!(text.contains("size"));
    }

    #[test]
    fn stats_lines_placeholder_when_nothing_present() {
        let b = block_with_result(json!({}));
        assert!(lines_text(&http_response_stats_lines(&b)).contains("no stats"));
    }

    #[test]
    fn raw_lines_assembles_status_headers_blank_and_body() {
        let b = block_with_result(json!({
            "status": 201,
            "status_text": "Created",
            "headers": [{"key": "X", "value": "y"}],
            "body": "ok",
        }));
        let text = lines_text(&http_response_raw_lines(&b));
        assert!(text.contains("HTTP/1.1 201 Created"));
        assert!(text.contains("X"));
        assert!(text.contains("ok"));
    }

    #[test]
    fn raw_lines_placeholder_for_no_cached_result() {
        let b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "http".into(),
            alias: None,
            display_mode: None,
            params: json!({}),
            state: ExecutionState::Idle,
            cached_result: None,
        };
        assert!(lines_text(&http_response_raw_lines(&b)).contains("no response"));
    }

    #[test]
    fn content_type_returns_none_when_no_headers_array() {
        let v = json!({});
        assert_eq!(content_type_of(&v), None);
    }

    #[test]
    fn content_type_skips_other_keys() {
        let v = json!({"headers": [{"key": "X", "value": "y"}]});
        assert_eq!(content_type_of(&v), None);
    }

    #[test]
    fn lang_routes_all_supported_content_types() {
        assert_eq!(
            lang_from_content_type(Some("application/json")),
            BodyLang::Json
        );
        assert_eq!(lang_from_content_type(Some("text/html")), BodyLang::Html);
        assert_eq!(lang_from_content_type(Some("text/xml")), BodyLang::Xml);
        assert_eq!(lang_from_content_type(Some("image/svg+xml")), BodyLang::Xml);
        assert_eq!(lang_from_content_type(Some("text/plain")), BodyLang::Plain);
        assert_eq!(lang_from_content_type(None), BodyLang::Plain);
    }

    #[test]
    fn panel_height_is_constant() {
        let b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "http".into(),
            alias: None,
            display_mode: None,
            params: json!({}),
            state: ExecutionState::Idle,
            cached_result: None,
        };
        assert_eq!(http_response_panel_height(&b), 10);
    }
}
