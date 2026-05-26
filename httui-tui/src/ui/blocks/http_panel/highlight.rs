//! HTTP-message syntax highlighting for the request body view +
//! per-line JSON/XML/HTML colorers reused by the response panel.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use super::method_color;

#[derive(Debug, Clone, Copy)]
enum HttpHighlightState {
    PreRequest,
    Headers,
    Body,
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
pub(super) fn highlight_http_message(
    text: &str,
    error_refs: &std::collections::HashSet<String>,
) -> Vec<Line<'static>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Line<'static>> = Vec::with_capacity(lines.len());
    let mut state = HttpHighlightState::PreRequest;
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
                out.push(Line::from(highlight_http_request_line(line, error_refs)));
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
                    out.push(Line::from(highlight_http_query_continuation(
                        line, error_refs,
                    )));
                    continue;
                }
                out.push(Line::from(highlight_http_header_line(line, error_refs)));
            }
            HttpHighlightState::Body => {
                // Per-line JSON-aware highlight; falls through gracefully on
                // non-JSON text.
                let spans = highlight_json_line(line);
                out.push(Line::from(spans));
            }
        }
    }
    out
}

/// Render `METHOD URL` so total span width equals `line.chars().count()`.
/// METHOD is a colored badge; URL renders with `{{...}}` refs in cyan.
///
/// Width invariant: cursors in the rope are positioned by byte offset,
/// so any visual padding here (e.g. ` GET ` around the badge or an
/// inserted separator space) would skew the cursor against what the
/// user sees on screen. Tests assert length-preservation.
pub(super) fn highlight_http_request_line(
    line: &str,
    error_refs: &std::collections::HashSet<String>,
) -> Vec<Span<'static>> {
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
            spans.extend(highlight_refs_in_text(url, error_refs));
        }
    }
    spans
}

/// Highlight a header row `Key: value` — key cyan, separator dim,
/// value plain (with refs cyan when present).
fn highlight_http_header_line(
    line: &str,
    error_refs: &std::collections::HashSet<String>,
) -> Vec<Span<'static>> {
    if let Some(colon) = line.find(':') {
        let key = &line[..colon];
        let rest = &line[colon + 1..];
        let mut spans = vec![
            Span::styled(key.to_string(), Style::default().fg(Color::Cyan)),
            Span::styled(":".to_string(), Style::default().fg(Color::DarkGray)),
        ];
        spans.extend(highlight_refs_in_text(rest, error_refs));
        spans
    } else {
        vec![Span::raw(line.to_string())]
    }
}

/// Highlight `?key=value` / `&key=value` continuation rows used by
/// the parser to extend the URL's query string.
fn highlight_http_query_continuation(
    line: &str,
    error_refs: &std::collections::HashSet<String>,
) -> Vec<Span<'static>> {
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
        spans.extend(highlight_refs_in_text(&body[eq + 1..], error_refs));
    } else {
        spans.push(Span::raw(body.to_string()));
    }
    spans
}

fn highlight_refs_in_text(
    text: &str,
    error_refs: &std::collections::HashSet<String>,
) -> Vec<Span<'static>> {
    crate::ui::blocks::ref_highlight::highlight_refs(text, error_refs)
}

/// Cheap per-line XML/HTML highlighter: angle brackets + tag names in
/// cyan, attribute names in yellow, attribute values in green, text
/// content default. Doesn't try to track nesting or quote escapes —
/// enough for an eyeball read.
pub(super) fn highlight_xml_line(line: &str) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0usize;
    let punct = Style::default().fg(Color::DarkGray);
    let tag = Style::default().fg(Color::Cyan);
    let attr = Style::default().fg(Color::Yellow);
    let val = Style::default().fg(Color::Green);
    while i < bytes.len() {
        if bytes[i] == b'<' {
            spans.push(Span::styled("<".to_string(), punct));
            i += 1;
            if i < bytes.len() && bytes[i] == b'/' {
                spans.push(Span::styled("/".to_string(), punct));
                i += 1;
            }
            let name_start = i;
            while i < bytes.len() && !matches!(bytes[i], b' ' | b'\t' | b'>' | b'/' | b'?' | b'!') {
                i += 1;
            }
            if i > name_start {
                spans.push(Span::styled(line[name_start..i].to_string(), tag));
            }
            while i < bytes.len() && bytes[i] != b'>' {
                if bytes[i] == b' ' || bytes[i] == b'\t' {
                    spans.push(Span::raw((bytes[i] as char).to_string()));
                    i += 1;
                } else if bytes[i] == b'=' {
                    spans.push(Span::styled("=".to_string(), punct));
                    i += 1;
                } else if bytes[i] == b'"' || bytes[i] == b'\'' {
                    let quote = bytes[i];
                    let start = i;
                    i += 1;
                    while i < bytes.len() && bytes[i] != quote {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1;
                    }
                    spans.push(Span::styled(line[start..i].to_string(), val));
                } else {
                    let start = i;
                    while i < bytes.len()
                        && !matches!(bytes[i], b'=' | b' ' | b'\t' | b'>' | b'"' | b'\'')
                    {
                        i += 1;
                    }
                    if i > start {
                        spans.push(Span::styled(line[start..i].to_string(), attr));
                    }
                }
            }
            if i < bytes.len() {
                spans.push(Span::styled(">".to_string(), punct));
                i += 1;
            }
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            if i > start {
                spans.push(Span::raw(line[start..i].to_string()));
            }
        }
    }
    spans
}

/// Tiny JSON-aware lexer: highlights string keys / values, numbers,
/// booleans, nulls, and structural punctuation. Per-line so it
/// composes with ratatui's line-by-line rendering. Strings split
/// across lines (multi-line escape sequences) are rare in
/// pretty-printed JSON; if they happen, the second half just
/// renders default.
pub(super) fn highlight_json_line(line: &str) -> Vec<Span<'static>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn span_text(spans: &[Span<'static>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn message_text(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .map(|l| span_text(&l.spans))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn message_handles_request_headers_body_sections() {
        let text = "GET https://x.com\nAuthorization: Bearer t\n\n{\"k\":1}";
        let lines = highlight_http_message(text, &HashSet::new());
        let joined = message_text(&lines);
        assert!(joined.contains("GET"));
        assert!(joined.contains("Authorization"));
        assert!(joined.contains("{"));
        assert!(joined.contains("\"k\""));
    }

    #[test]
    fn message_dims_comment_lines_in_pre_request() {
        let lines = highlight_http_message("# header comment\nGET /", &HashSet::new());
        // First line is the comment, dim styled.
        assert!(lines[0]
            .spans
            .iter()
            .all(|s| s.style.fg == Some(Color::DarkGray)));
    }

    #[test]
    fn message_skips_blank_lines_before_request_line() {
        let lines = highlight_http_message("\n\nGET /", &HashSet::new());
        assert_eq!(lines.len(), 3);
        assert!(message_text(&lines).contains("GET"));
    }

    #[test]
    fn message_recognizes_header_section_transitions() {
        let text = "GET /\nKey: value\n# comment\n?extra=1\n\nbody-line";
        let lines = highlight_http_message(text, &HashSet::new());
        // 6 input lines preserved.
        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn header_line_splits_key_value_at_colon() {
        let spans = highlight_http_header_line("X-Trace: abc", &HashSet::new());
        assert!(span_text(&spans).starts_with("X-Trace"));
        assert!(span_text(&spans).contains("abc"));
    }

    #[test]
    fn header_line_without_colon_emits_plain() {
        let spans = highlight_http_header_line("no-colon", &HashSet::new());
        assert_eq!(span_text(&spans), "no-colon");
    }

    #[test]
    fn query_continuation_emits_prefix_punct_and_equals() {
        let spans = highlight_http_query_continuation("  ?key=value", &HashSet::new());
        let text = span_text(&spans);
        assert!(text.contains("?"));
        assert!(text.contains("key"));
        assert!(text.contains("value"));
    }

    #[test]
    fn query_continuation_with_no_equals_renders_raw() {
        let spans = highlight_http_query_continuation("  ?flag", &HashSet::new());
        assert!(span_text(&spans).contains("flag"));
    }

    #[test]
    fn json_line_marks_keys_and_values_and_punct() {
        let spans = highlight_json_line("{\"a\": 1, \"b\": true, \"c\": null}");
        let text = span_text(&spans);
        assert!(text.contains("\"a\""));
        assert!(text.contains("\"b\""));
        assert!(text.contains("true"));
        assert!(text.contains("null"));
        assert!(text.contains("1"));
    }

    #[test]
    fn json_line_handles_negative_numbers_and_decimals() {
        let spans = highlight_json_line("[1.5, -3.2e2]");
        let text = span_text(&spans);
        assert!(text.contains("1.5"));
        assert!(text.contains("-3.2e2"));
    }

    #[test]
    fn json_line_handles_escaped_quotes_in_strings() {
        let spans = highlight_json_line(r#""he said \"hi\"""#);
        let text = span_text(&spans);
        assert!(text.contains("he said"));
    }

    #[test]
    fn json_line_falls_through_on_unknown_text() {
        let spans = highlight_json_line("plain text only");
        assert_eq!(span_text(&spans), "plain text only");
    }

    #[test]
    fn json_line_recognizes_false_keyword() {
        let spans = highlight_json_line("false");
        assert_eq!(span_text(&spans), "false");
        assert_eq!(
            spans[0].style.add_modifier,
            Style::default().add_modifier(Modifier::BOLD).add_modifier
        );
    }

    #[test]
    fn xml_line_handles_self_closing_tag() {
        let spans = highlight_xml_line(r#"<br/>"#);
        assert_eq!(span_text(&spans).chars().count(), "<br/>".chars().count());
    }

    #[test]
    fn xml_line_handles_close_tag() {
        let spans = highlight_xml_line(r#"</div>"#);
        let text = span_text(&spans);
        assert!(text.contains("/"));
        assert!(text.contains("div"));
    }

    #[test]
    fn xml_line_handles_attr_with_single_quotes() {
        let spans = highlight_xml_line(r#"<a href='x'>"#);
        let text = span_text(&spans);
        assert!(text.contains("href"));
        assert!(text.contains("'x'"));
    }

    #[test]
    fn xml_line_handles_text_content_outside_tags() {
        let spans = highlight_xml_line("hello <b>world</b>");
        let text = span_text(&spans);
        assert!(text.starts_with("hello "));
        assert!(text.contains("world"));
    }

    #[test]
    fn request_line_renders_url_with_refs_highlighted() {
        let mut errs = HashSet::new();
        errs.insert("BROKEN".into());
        let spans = highlight_http_request_line("GET https://{{BROKEN}}/x", &errs);
        let total: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        assert_eq!(total, "GET https://{{BROKEN}}/x".chars().count());
    }
}
