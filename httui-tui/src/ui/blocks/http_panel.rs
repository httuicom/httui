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

/// Syntax-highlight the raw HTTP-message body the cursor is
/// editing. Walks line-by-line tracking which section we're in:
///
///   line 0 (after blanks): request line — METHOD badge + URL with
///                          `{{ref}}` placeholders cyan
///   1..first blank:        headers — key cyan, `: `, value plain
///   after blank:           body — JSON highlight when the text
///                          parses as JSON; plain otherwise
///   leading `#` lines:     dim grey (comments, ignored by parser)
fn highlight_http_message(text: &str) -> Vec<Line<'static>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Line<'static>> = Vec::with_capacity(lines.len());
    let mut state = HttpHighlightState::PreRequest;
    // Collect the body block as one string at the end so we can
    // pretty-detect JSON; still emit individual lines.
    for raw in &lines {
        let line = *raw;
        match state {
            HttpHighlightState::PreRequest => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    out.push(Line::from(""));
                    continue;
                }
                if trimmed.starts_with('#') {
                    out.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::DarkGray),
                    )));
                    continue;
                }
                out.push(Line::from(highlight_http_request_line(line)));
                state = HttpHighlightState::Headers;
            }
            HttpHighlightState::Headers => {
                if line.trim().is_empty() {
                    out.push(Line::from(""));
                    state = HttpHighlightState::Body;
                    continue;
                }
                if line.trim_start().starts_with('#') {
                    out.push(Line::from(Span::styled(
                        line.to_string(),
                        Style::default().fg(Color::DarkGray),
                    )));
                    continue;
                }
                if line.trim_start().starts_with('?') || line.trim_start().starts_with('&') {
                    out.push(Line::from(highlight_http_query_continuation(line)));
                    continue;
                }
                out.push(Line::from(highlight_http_header_line(line)));
            }
            HttpHighlightState::Body => {
                // Try JSON-aware highlighting on each body line. The
                // per-line lexer already handles comma / brace / etc.
                // gracefully, so even non-JSON text won't blow up.
                let spans = highlight_json_line(line);
                out.push(Line::from(spans));
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy)]
enum HttpHighlightState {
    PreRequest,
    Headers,
    Body,
}

/// Render `METHOD URL` so total span width equals `line.chars().count()`.
/// METHOD is a colored badge; URL renders with `{{...}}` refs in cyan.
///
/// Width invariant: cursors in the rope are positioned by byte offset,
/// so any visual padding here (e.g. ` GET ` around the badge or an
/// inserted separator space) would skew the cursor against what the
/// user sees on screen. Tests in this module assert length-preservation;
/// don't reintroduce padding.
fn highlight_http_request_line(line: &str) -> Vec<Span<'static>> {
    let split = line.find(char::is_whitespace).unwrap_or(line.len());
    let (method, rest) = line.split_at(split);
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::styled(
        method.to_string(),
        Style::default()
            .bg(method_color(method))
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    ));
    if !rest.is_empty() {
        let url_start = rest
            .find(|c: char| !c.is_whitespace())
            .unwrap_or(rest.len());
        let (ws, url) = rest.split_at(url_start);
        if !ws.is_empty() {
            spans.push(Span::raw(ws.to_string()));
        }
        if !url.is_empty() {
            spans.extend(highlight_refs_in_text(url));
        }
    }
    spans
}

/// Highlight a header row `Key: value` — key cyan, separator dim,
/// value plain (with refs cyan when present).
fn highlight_http_header_line(line: &str) -> Vec<Span<'static>> {
    if let Some(colon) = line.find(':') {
        let key = &line[..colon];
        let rest = &line[colon + 1..];
        let mut spans = vec![
            Span::styled(key.to_string(), Style::default().fg(Color::Cyan)),
            Span::styled(":".to_string(), Style::default().fg(Color::DarkGray)),
        ];
        spans.extend(highlight_refs_in_text(rest));
        spans
    } else {
        vec![Span::raw(line.to_string())]
    }
}

/// Highlight `?key=value` / `&key=value` continuation rows used by
/// the parser to extend the URL's query string.
fn highlight_http_query_continuation(line: &str) -> Vec<Span<'static>> {
    let prefix_len = line.len() - line.trim_start().len();
    let prefix = &line[..prefix_len];
    let rest = line[prefix_len..]
        .chars()
        .next()
        .map(|c| c.to_string())
        .unwrap_or_default();
    let body = &line[prefix_len + rest.len()..];
    let mut spans: Vec<Span<'static>> = Vec::new();
    if !prefix.is_empty() {
        spans.push(Span::raw(prefix.to_string()));
    }
    spans.push(Span::styled(rest, Style::default().fg(Color::DarkGray)));
    if let Some(eq) = body.find('=') {
        spans.push(Span::styled(
            body[..eq].to_string(),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::styled(
            "=".to_string(),
            Style::default().fg(Color::DarkGray),
        ));
        spans.extend(highlight_refs_in_text(&body[eq + 1..]));
    } else {
        spans.push(Span::raw(body.to_string()));
    }
    spans
}

/// Walk `text` emitting plain spans for normal characters and
/// cyan-styled spans for `{{ref}}` placeholders. Used by URL /
/// header values / body so refs visibly stand out from regular
/// text. Unmatched `{{` (mid-edit) renders as plain text.
fn highlight_refs_in_text(text: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut buf = String::new();
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            // Find matching `}}`.
            if let Some(close) = (i + 2..bytes.len().saturating_sub(1))
                .find(|&j| bytes[j] == b'}' && bytes[j + 1] == b'}')
            {
                if !buf.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
                let chunk = &text[i..close + 2];
                spans.push(Span::styled(
                    chunk.to_string(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ));
                i = close + 2;
                continue;
            }
        }
        buf.push(bytes[i] as char);
        i += 1;
    }
    if !buf.is_empty() {
        spans.push(Span::raw(buf));
    }
    spans
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

fn format_byte_size(bytes: u64) -> String {
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
            highlight_http_message(&body)
        }
    } else {
        http_body(b)
    };
    let request_height = request_lines.len() as u16;

    let has_response = b.cached_result.is_some();
    let response_height = if has_response {
        http_response_panel_height(b)
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
        render_http_response_panel(frame, content_rect, b, result_tab);
        // Hint paragraph at the bottom of the panel: tells the user
        // they can `<CR>` for the full body. Painted last so it
        // overwrites whatever the body lines wrote on the bottom row.
        if cursor_in_result {
            paint_panel_focus_hint(frame, panel_chunk);
        }
    }
}

/// Tabs map for HTTP: re-uses ResultPanelTab so the keymap that
/// cycles tabs (`gt`/`gT`) keeps working — labels just change.
/// Result → Body, Messages → Headers, Plan → Cookies, Stats →
/// Stats. Raw is folded into Body for V1.
fn render_http_response_panel(
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
    };
    frame.render_widget(Paragraph::new(lines), area);
}

fn http_response_body_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no body)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(result) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let body = result.get("body");
    // Body is either a JSON value (object/array → pretty-print and
    // syntax-highlight as JSON) or a string (rendered as-is). We
    // highlight the JSON path because that covers the common case
    // of `application/json` responses.
    let (text, is_json) = match body {
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
    if is_json {
        text.lines()
            .map(|l| Line::from(highlight_json_line(l)))
            .collect()
    } else {
        text.lines().map(|l| Line::from(l.to_string())).collect()
    }
}

/// Tiny JSON-aware lexer: highlights string keys / values, numbers,
/// booleans, nulls, and structural punctuation. Per-line so it
/// composes with ratatui's line-by-line rendering. Strings split
/// across lines (multi-line escape sequences) are rare in
/// pretty-printed JSON; if they happen, the second half just
/// renders default — acceptable for V1.
fn highlight_json_line(line: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let key_style = Style::default().fg(Color::Cyan);
    let str_style = Style::default().fg(Color::Green);
    let num_style = Style::default().fg(Color::Rgb(255, 165, 0));
    let kw_style = Style::default()
        .fg(Color::Magenta)
        .add_modifier(Modifier::BOLD);
    let punct_style = Style::default().fg(Color::DarkGray);

    let bytes = line.as_bytes();
    let mut i = 0usize;
    let mut last_string: Option<String> = None; // tracks key-vs-value context

    while i < bytes.len() {
        let c = bytes[i] as char;
        if c == '"' {
            // Scan to closing quote (respecting \" escapes).
            let start = i;
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let chunk = &line[start..i];
            // Look ahead skipping spaces — `:` after a string means
            // it's a key.
            let mut j = i;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            let is_key = j < bytes.len() && bytes[j] == b':';
            spans.push(Span::styled(
                chunk.to_string(),
                if is_key { key_style } else { str_style },
            ));
            last_string = Some(chunk.to_string());
        } else if c.is_ascii_digit()
            || (c == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit())
        {
            let start = i;
            if c == '-' {
                i += 1;
            }
            while i < bytes.len()
                && ((bytes[i] as char).is_ascii_digit()
                    || bytes[i] == b'.'
                    || bytes[i] == b'e'
                    || bytes[i] == b'E'
                    || bytes[i] == b'+'
                    || bytes[i] == b'-')
            {
                i += 1;
            }
            spans.push(Span::styled(line[start..i].to_string(), num_style));
        } else if line[i..].starts_with("true") {
            spans.push(Span::styled("true".to_string(), kw_style));
            i += 4;
        } else if line[i..].starts_with("false") {
            spans.push(Span::styled("false".to_string(), kw_style));
            i += 5;
        } else if line[i..].starts_with("null") {
            spans.push(Span::styled("null".to_string(), kw_style));
            i += 4;
        } else if matches!(c, '{' | '}' | '[' | ']' | ',' | ':') {
            spans.push(Span::styled(c.to_string(), punct_style));
            i += 1;
        } else {
            // Whitespace / unknown — keep default style.
            let start = i;
            while i < bytes.len() {
                let ch = bytes[i] as char;
                if ch == '"'
                    || ch == '{'
                    || ch == '}'
                    || ch == '['
                    || ch == ']'
                    || ch == ','
                    || ch == ':'
                    || ch.is_ascii_digit()
                    || (ch == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit())
                    || line[i..].starts_with("true")
                    || line[i..].starts_with("false")
                    || line[i..].starts_with("null")
                {
                    break;
                }
                i += 1;
            }
            if i > start {
                spans.push(Span::raw(line[start..i].to_string()));
            }
        }
    }
    let _ = last_string;
    spans
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

fn http_response_panel_height(_b: &BlockNode) -> u16 {
    // Tab bar (1) + separator (1) + content viewport (8). Body
    // content scrolls beyond the viewport; we don't grow the card
    // unboundedly to fit a 50 KB JSON response.
    const VIEWPORT: u16 = 8;
    1 + 1 + VIEWPORT
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
        let spans = highlight_http_request_line(line);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, line.chars().count(), "spans={spans:?}");
    }

    #[test]
    fn http_request_line_preserves_extra_whitespace() {
        let line = "POST   https://api.example.com/users";
        let spans = highlight_http_request_line(line);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, line.chars().count(), "spans={spans:?}");
    }

    #[test]
    fn http_request_line_preserves_width_with_refs() {
        let line = "GET https://{{HOST}}/get";
        let spans = highlight_http_request_line(line);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, line.chars().count(), "spans={spans:?}");
    }

    #[test]
    fn http_request_line_method_only() {
        let line = "GET";
        let spans = highlight_http_request_line(line);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, line.chars().count());
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
}
