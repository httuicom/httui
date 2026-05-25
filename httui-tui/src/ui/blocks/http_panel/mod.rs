//! HTTP block rendering — request panel, response panel, footer/header
//! chips, and the HTTP-message syntax highlighter. Mirrors the desktop
//! `HttpFencedPanel` shell: method badge + URL row, query/header rows,
//! body, then a result panel (Body / Headers / Cookies / Stats tabs).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::buffer::block::{BlockNode, ExecutionState};

use super::{
    paint_panel_focus_bg, paint_panel_focus_hint, raw_body_text, render_fence_closer_row,
    render_result_separator, render_result_tab_bar_for,
};

mod highlight;
mod response;

pub(super) fn http_header_left_spans(b: &BlockNode, bg: Style) -> Vec<Span<'static>> {
    let alias = b.alias.clone().unwrap_or_else(|| "—".into());
    let method = b
        .params
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_string();
    let url = b.params.get("url").and_then(|v| v.as_str()).unwrap_or("");
    let host = http_host_of(url);
    vec![
        Span::raw(" "),
        Span::styled(
            " HTTP ",
            Style::default()
                .bg(Color::Magenta)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", bg),
        Span::styled(alias, bg.fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("  ·  ", bg.fg(Color::DarkGray)),
        Span::styled(
            format!(" {method} "),
            Style::default()
                .bg(method_color(&method))
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", bg),
        Span::styled(host, bg.fg(Color::Gray)),
    ]
}

/// Pull the host (and optional port) out of an HTTP URL — `https://
/// api.x.com:443/v1/foo?q=1` → `api.x.com:443`. Returns the original
/// string when it doesn't parse as a URL (incomplete fences, refs).
pub(super) fn http_host_of(url: &str) -> String {
    let after_scheme = url.find("://").map(|i| &url[i + 3..]).unwrap_or(url);
    let host_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let host = &after_scheme[..host_end];
    if host.is_empty() {
        "—".into()
    } else {
        host.to_string()
    }
}

pub(super) fn http_footer_spans(
    b: &BlockNode,
    bg: Style,
    dot_color: Color,
    dot_label: &'static str,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let dim = bg.fg(Color::DarkGray);
    let method = b
        .params
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_string();
    let url = b
        .params
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    // Trim long URL to fit — `path` only when host fits in the badge.
    let path = http_path_of(&url);

    let mut left: Vec<Span<'static>> = Vec::new();
    left.push(Span::raw(" "));
    left.push(Span::styled("●", bg.fg(dot_color)));
    left.push(Span::styled("  ", bg));
    left.push(Span::styled(dot_label, bg.fg(Color::Gray)));
    left.push(Span::styled("  ·  ", dim));
    left.push(Span::styled(
        format!(" {method} "),
        Style::default()
            .bg(method_color(&method))
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    ));
    left.push(Span::styled("  ", bg));
    left.push(Span::styled(path, bg.fg(Color::Gray)));
    left.push(Span::styled("  │  ", dim));

    let mut right: Vec<Span<'static>> = Vec::new();
    if let Some(s) = http_summary(b) {
        right.push(Span::styled(s, bg.fg(Color::Gray)));
        right.push(Span::styled("  ·  ", dim));
    }
    if matches!(b.state, ExecutionState::Cached) {
        right.push(Span::styled("cached", bg.fg(Color::Cyan)));
        right.push(Span::styled("  ·  ", dim));
    }
    right.push(Span::styled("press `r` to run ", dim));
    (left, right)
}

/// Pull the path + query out of an HTTP URL. Returns `/` when the
/// URL has no path (just a host) or empty.
fn http_path_of(url: &str) -> String {
    let after_scheme = url.find("://").map(|i| &url[i + 3..]).unwrap_or(url);
    let path_start = after_scheme.find('/').unwrap_or(after_scheme.len());
    let path = &after_scheme[path_start..];
    if path.is_empty() {
        "/".into()
    } else {
        path.to_string()
    }
}

/// One-line summary of an HTTP block's `cached_result`. Returns
/// `None` when the cache is empty / shape doesn't match. Format:
/// `200 OK · 124ms · 4 KB`.
fn http_summary(b: &BlockNode) -> Option<String> {
    let result = b.cached_result.as_ref()?;
    let status = result.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    let status_text = result
        .get("status_text")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let elapsed = result
        .get("timing")
        .and_then(|t| t.get("total_ms"))
        .and_then(|v| v.as_u64())
        .or_else(|| result.get("elapsed_ms").and_then(|v| v.as_u64()));
    let size = result
        .get("size_bytes")
        .and_then(|v| v.as_u64())
        .or_else(|| {
            result
                .get("body")
                .and_then(|v| v.as_str())
                .map(|s| s.len() as u64)
        });
    let mut parts: Vec<String> = Vec::new();
    if status > 0 {
        if status_text.is_empty() {
            parts.push(format!("{status}"));
        } else {
            parts.push(format!("{status} {status_text}"));
        }
    }
    if let Some(ms) = elapsed {
        parts.push(format!("{ms}ms"));
    }
    if let Some(bytes) = size {
        parts.push(format_byte_size(bytes));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

pub(super) fn format_byte_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Render the request side of an HTTP block as a multi-line panel:
/// method+URL row, query-param continuations (`? key=value`), header
/// rows (`Authorization: Bearer …`), and a body block when present.
/// Syntax: method as colored badge, header keys cyan, separators
/// dim. Off-cursor — when the cursor enters, we paint the raw rope
/// instead so the user edits exactly what they see.
/// Mirror `render_db_inner` for HTTP blocks. Layout inside the
/// chrome-bordered card middle:
/// ```text
/// request body  (http_request_lines rows)
/// fence closer  (1 row, only when cursor is on the block)
/// tab bar       (1 row, only when cached_result exists)
/// separator     (1 row, only when cached_result exists)
/// response panel (rest)
/// ```
///
/// Note the `fence closer` slot: the ` ``` ` line lives between raw
/// input and response panel, not at the very bottom of the card.
/// This matches the user's mental model — ` ``` ` fences the
/// editable region, and the response panel is card chrome (not
/// markdown source).
pub(super) fn render_http_inner(
    frame: &mut Frame,
    inner: Rect,
    b: &BlockNode,
    result_tab: crate::app::ResultPanelTab,
    selected: bool,
    cursor_in_result: bool,
) {
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let request_lines = if selected {
        // Cursor on: paint raw rope so the user sees exactly what
        // they're editing (HTTP-message text). Highlight per-line:
        // method + URL on the first non-blank row, headers
        // afterward (until the first blank), then body (JSON
        // highlight when the text looks like JSON).
        let body = raw_body_text(b);
        if body.is_empty() {
            http_body(b)
        } else {
            let error_refs = match &b.state {
                crate::buffer::block::ExecutionState::Error(msg) => {
                    super::ref_highlight::parse_error_refs(msg)
                }
                _ => std::collections::HashSet::new(),
            };
            highlight::highlight_http_message(&body, &error_refs)
        }
    } else {
        http_body(b)
    };
    let request_height = request_lines.len() as u16;

    let has_response = b.cached_result.is_some();
    let response_height = if has_response {
        response::http_response_panel_height(b)
    } else {
        0
    };
    // Fence closer takes 1 row whenever the cursor is on the block.
    // When the cursor is off, the raw text is hidden anyway and the
    // closer would be visual noise — `render_block_with_selection`
    // also gates fence rendering on `selected`.
    let closer_height = if selected { 1 } else { 0 };

    let mut constraints: Vec<Constraint> = Vec::new();
    constraints.push(Constraint::Length(request_height));
    if closer_height > 0 {
        constraints.push(Constraint::Length(closer_height));
    }
    if response_height > 0 {
        constraints.push(Constraint::Length(response_height));
    }
    if constraints.is_empty() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let mut idx = 0;
    frame.render_widget(Paragraph::new(request_lines), chunks[idx]);
    idx += 1;
    if closer_height > 0 {
        render_fence_closer_row(frame, chunks[idx], b);
        idx += 1;
    }

    if response_height > 0 {
        let panel_chunk = chunks[idx];
        let mut y = panel_chunk.y;
        let row = |y: u16| Rect {
            x: panel_chunk.x,
            y,
            width: panel_chunk.width,
            height: 1,
        };
        let tab_bar_rect = row(y);
        y = y.saturating_add(1);
        let separator_rect = row(y);
        y = y.saturating_add(1);
        let used = y.saturating_sub(panel_chunk.y);
        let content_rect = Rect {
            x: panel_chunk.x,
            y,
            width: panel_chunk.width,
            height: panel_chunk.height.saturating_sub(used),
        };
        // Subtle background tint over the whole response panel when
        // the cursor is parked there — without this nothing changes
        // visually as `j` walks into the panel, so users think the
        // motion didn't do anything. The tint is dim enough to leave
        // text readable but distinct from the editor background.
        if cursor_in_result {
            paint_panel_focus_bg(frame, panel_chunk);
        }
        render_result_tab_bar_for(
            frame,
            tab_bar_rect,
            result_tab,
            None, /* http never multi-statement */
            "http",
        );
        render_result_separator(frame, separator_rect);
        response::render_http_response_panel(frame, content_rect, b, result_tab);
        // Hint paragraph at the bottom of the panel: tells the user
        // they can `<CR>` for the full body. Painted last so it
        // overwrites whatever the body lines wrote on the bottom row.
        if cursor_in_result {
            paint_panel_focus_hint(frame, panel_chunk);
        }
    }
}

fn http_body(b: &BlockNode) -> Vec<Line<'static>> {
    let method = b
        .params
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_string();
    let url = b
        .params
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Request line: colored method badge + URL.
    lines.push(Line::from(vec![
        Span::styled(
            format!(" {method} "),
            Style::default()
                .fg(Color::Black)
                .bg(method_color(&method))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::raw(url),
    ]));

    // Query params, one per row prefixed `? `.
    if let Some(params) = b.params.get("params").and_then(|v| v.as_array()) {
        for (i, p) in params.iter().enumerate() {
            let key = p.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let value = p.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let prefix = if i == 0 { "  ? " } else { "  & " };
            lines.push(Line::from(vec![
                Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                Span::styled(key.to_string(), Style::default().fg(Color::Cyan)),
                Span::styled("=", Style::default().fg(Color::DarkGray)),
                Span::raw(value.to_string()),
            ]));
        }
    }

    // Headers, one per row.
    if let Some(headers) = b.params.get("headers").and_then(|v| v.as_array()) {
        for h in headers {
            let key = h.get("key").and_then(|v| v.as_str()).unwrap_or("");
            let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
            lines.push(Line::from(vec![
                Span::styled(format!("  {key}"), Style::default().fg(Color::Cyan)),
                Span::styled(": ", Style::default().fg(Color::DarkGray)),
                Span::raw(value.to_string()),
            ]));
        }
    }

    // Body block (if any) — separator + text. Drop trailing
    // newlines so the panel doesn't leave a blank row at the end.
    if let Some(body) = b.params.get("body").and_then(|v| v.as_str()) {
        let trimmed = body.trim_end_matches('\n');
        if !trimmed.is_empty() {
            lines.push(Line::from(""));
            for body_line in trimmed.lines() {
                lines.push(Line::from(format!("  {body_line}")));
            }
        }
    }

    lines
}

pub(super) fn method_color(method: &str) -> Color {
    match method {
        "GET" => Color::Green,
        "POST" => Color::Blue,
        "PUT" => Color::Rgb(0xff, 0xa5, 0x00),
        "PATCH" => Color::Yellow,
        "DELETE" => Color::Red,
        "HEAD" => Color::Magenta,
        _ => Color::Gray,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::highlight::{highlight_http_request_line, highlight_xml_line};
    use super::response::{content_type_of, http_response_raw_lines, lang_from_content_type, BodyLang};
    use crate::buffer::block::{BlockId, ExecutionState};
    use serde_json::json;

    fn http_block() -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "http".into(),
            alias: Some("login".into()),
            display_mode: None,
            params: json!({
                "method": "POST",
                "url": "https://api.test.com/login",
                "params": [],
                "headers": [{"key": "Content-Type", "value": "application/json"}],
                "body": "{\"u\":\"a\"}"
            }),
            state: ExecutionState::Idle,
            cached_result: None,
        }
    }

    #[test]
    fn http_body_shows_method_and_url() {
        let b = http_block();
        let lines = http_body(&b);
        let first_text: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
        assert!(first_text.contains("POST"));
        assert!(first_text.contains("https://api.test.com/login"));
    }

    #[test]
    fn http_request_line_width_matches_source() {
        // Rendered span widths must equal source.chars().count() —
        // the cursor is positioned by byte offset, so any visual
        // padding drifts the caret off the rope.
        let line = "GET https://example.com/path";
        let spans = highlight_http_request_line(line, &std::collections::HashSet::new());
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, line.chars().count(), "spans={spans:?}");
    }

    #[test]
    fn http_request_line_preserves_extra_whitespace() {
        let line = "POST   https://api.example.com/users";
        let spans = highlight_http_request_line(line, &std::collections::HashSet::new());
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, line.chars().count(), "spans={spans:?}");
    }

    #[test]
    fn http_request_line_preserves_width_with_refs() {
        let line = "GET https://{{HOST}}/get";
        let spans = highlight_http_request_line(line, &std::collections::HashSet::new());
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, line.chars().count(), "spans={spans:?}");
    }

    #[test]
    fn http_request_line_method_only() {
        let line = "GET";
        let spans = highlight_http_request_line(line, &std::collections::HashSet::new());
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, line.chars().count());
    }

    #[test]
    fn lang_from_content_type_routes_json_xml_html_and_plain() {
        assert_eq!(
            lang_from_content_type(Some("application/json")),
            BodyLang::Json
        );
        assert_eq!(
            lang_from_content_type(Some("application/json; charset=utf-8")),
            BodyLang::Json
        );
        assert_eq!(
            lang_from_content_type(Some("application/xml")),
            BodyLang::Xml
        );
        assert_eq!(
            lang_from_content_type(Some("image/svg+xml")),
            BodyLang::Xml
        );
        assert_eq!(
            lang_from_content_type(Some("text/html; charset=utf-8")),
            BodyLang::Html
        );
        assert_eq!(
            lang_from_content_type(Some("text/plain")),
            BodyLang::Plain
        );
        assert_eq!(lang_from_content_type(None), BodyLang::Plain);
    }

    #[test]
    fn content_type_of_extracts_case_insensitively() {
        let result = json!({
            "headers": [
                {"key": "X-Trace", "value": "abc"},
                {"key": "content-type", "value": "application/xml"},
            ],
        });
        assert_eq!(
            content_type_of(&result),
            Some("application/xml".to_string())
        );
    }

    #[test]
    fn highlight_xml_line_marks_tag_attrs_and_value() {
        let spans = highlight_xml_line(r#"<a href="x" />"#);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, r#"<a href="x" />"#.chars().count());
    }

    #[test]
    fn http_response_raw_lines_paints_status_headers_and_body() {
        let mut b = http_block();
        b.cached_result = Some(json!({
            "status": 200,
            "status_text": "OK",
            "headers": [
                {"key": "Content-Type", "value": "text/plain"},
            ],
            "body": "hello"
        }));
        let lines = http_response_raw_lines(&b);
        let text: String = lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("HTTP/1.1 200 OK"));
        assert!(text.contains("Content-Type"));
        assert!(text.contains("hello"));
    }

    #[test]
    fn http_body_renders_request_lines() {
        // The HTTP body now reads as method+URL line + 1 row per
        // header + (separator + body lines). Old `meta` summary
        // ("headers: N · params: M · body: K chars") is gone — the
        // user wanted the actual request text, not stats.
        let b = http_block();
        let lines = http_body(&b);
        // Request line first; then 1 header (`Authorization: …`);
        // then a blank separator + 1 body line.
        assert!(lines.len() >= 4, "got {} lines", lines.len());
        let request: String = lines[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<Vec<_>>()
            .join("");
        assert!(request.contains("POST"));
        assert!(request.contains("api.test.com"));
    }

    fn host_block(method: &str, url: &str) -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "http".into(),
            alias: Some("a".into()),
            display_mode: None,
            params: json!({
                "method": method,
                "url": url,
                "headers": [],
                "params": [],
                "body": "",
            }),
            state: ExecutionState::Idle,
            cached_result: None,
        }
    }

    #[test]
    fn host_of_extracts_host_with_port_and_strips_path() {
        assert_eq!(http_host_of("https://api.x.com:8443/v1/foo"), "api.x.com:8443");
        assert_eq!(http_host_of("http://h/"), "h");
        assert_eq!(http_host_of("h.example.com?q=1"), "h.example.com");
        assert_eq!(http_host_of(""), "—");
        assert_eq!(http_host_of("h.example.com#frag"), "h.example.com");
    }

    #[test]
    fn path_of_returns_path_or_slash_for_root() {
        assert_eq!(http_path_of("https://x.com/abc?q=1"), "/abc?q=1");
        assert_eq!(http_path_of("https://x.com"), "/");
        assert_eq!(http_path_of(""), "/");
    }

    #[test]
    fn format_byte_size_chooses_b_kb_mb_gb_units() {
        assert_eq!(format_byte_size(100), "100 B");
        assert!(format_byte_size(2048).ends_with("KB"));
        assert!(format_byte_size(5 * 1024 * 1024).ends_with("MB"));
        assert!(format_byte_size(2u64 * 1024 * 1024 * 1024).ends_with("GB"));
    }

    #[test]
    fn method_color_picks_distinct_colors_per_verb() {
        assert_eq!(method_color("GET"), Color::Green);
        assert_eq!(method_color("POST"), Color::Blue);
        assert_eq!(method_color("PATCH"), Color::Yellow);
        assert_eq!(method_color("DELETE"), Color::Red);
        assert_eq!(method_color("HEAD"), Color::Magenta);
        assert_eq!(method_color("WHO"), Color::Gray);
    }

    #[test]
    fn http_summary_assembles_status_elapsed_and_size() {
        let mut b = http_block();
        b.cached_result = Some(json!({
            "status": 200,
            "status_text": "OK",
            "timing": {"total_ms": 12},
            "size_bytes": 256,
        }));
        let s = http_summary(&b).unwrap();
        assert!(s.contains("200"));
        assert!(s.contains("12ms"));
        assert!(s.contains("256 B"));
    }

    #[test]
    fn http_summary_falls_back_to_body_len_when_size_missing() {
        let mut b = http_block();
        b.cached_result = Some(json!({"body": "abcde"}));
        let s = http_summary(&b).unwrap();
        assert!(s.contains("5 B"));
    }

    #[test]
    fn http_summary_returns_none_when_no_cached_result() {
        let b = http_block();
        assert!(http_summary(&b).is_none());
    }

    #[test]
    fn http_summary_returns_none_when_all_parts_empty() {
        let mut b = http_block();
        b.cached_result = Some(json!({}));
        assert!(http_summary(&b).is_none());
    }

    #[test]
    fn header_left_spans_includes_alias_method_and_host() {
        let b = host_block("PUT", "https://api.x.com/v1/foo");
        let spans = http_header_left_spans(&b, Style::default());
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("PUT"));
        assert!(text.contains("api.x.com"));
        assert!(text.contains("a")); // alias
    }

    #[test]
    fn footer_spans_left_and_right_carry_status_and_hint() {
        let b = host_block("GET", "https://api.x.com/v1");
        let (left, right) = http_footer_spans(&b, Style::default(), Color::Green, "idle");
        let left_text: String = left.iter().map(|s| s.content.as_ref()).collect();
        let right_text: String = right.iter().map(|s| s.content.as_ref()).collect();
        assert!(left_text.contains("GET"));
        assert!(left_text.contains("idle"));
        assert!(right_text.contains("press"));
    }

    #[test]
    fn footer_spans_surface_cached_label_when_state_is_cached() {
        let mut b = host_block("GET", "https://api.x.com/v1");
        b.state = ExecutionState::Cached;
        let (_l, right) = http_footer_spans(&b, Style::default(), Color::Cyan, "ok");
        let right_text: String = right.iter().map(|s| s.content.as_ref()).collect();
        assert!(right_text.contains("cached"));
    }

    #[test]
    fn http_body_includes_query_params_and_body_lines() {
        let mut b = http_block();
        b.params = json!({
            "method": "POST",
            "url": "https://x.com",
            "params": [{"key": "k", "value": "v"}],
            "headers": [],
            "body": "line1\nline2",
        });
        let lines = http_body(&b);
        let text: String = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("k"));
        assert!(text.contains("line1"));
        assert!(text.contains("line2"));
    }
}
