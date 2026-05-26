//! Paint `{{ref}}` placeholders inside a block's editable text.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use std::collections::HashSet;

pub fn normal_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

pub fn error_style() -> Style {
    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
}

pub fn highlight_refs(text: &str, error_refs: &HashSet<String>) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut buf = String::new();
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(close) = find_close(&bytes[i + 2..]) {
                let close = i + 2 + close;
                if !buf.is_empty() {
                    spans.push(Span::raw(std::mem::take(&mut buf)));
                }
                let chunk = &text[i..close + 2];
                let inner = &text[i + 2..close];
                let alias = inner.split('.').next().unwrap_or("").trim();
                let style = if !alias.is_empty() && error_refs.contains(alias) {
                    error_style()
                } else {
                    normal_style()
                };
                spans.push(Span::styled(chunk.to_string(), style));
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

fn find_close(b: &[u8]) -> Option<usize> {
    (0..b.len().saturating_sub(1)).find(|&i| b[i] == b'}' && b[i + 1] == b'}')
}

pub fn parse_error_refs(msg: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    for needle in ["block `", "`"] {
        if let Some(start) = msg.find(needle) {
            let after = &msg[start + needle.len()..];
            if let Some(end) = after.find('`') {
                let candidate = after[..end].trim();
                if !candidate.is_empty() && !candidate.contains(' ') {
                    out.insert(candidate.to_string());
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refs(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn plain_text_yields_a_single_raw_span() {
        let spans = highlight_refs("just text", &HashSet::new());
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "just text");
    }

    #[test]
    fn ref_in_middle_is_split_with_normal_style() {
        let spans = highlight_refs("before {{a.b}} after", &HashSet::new());
        // [raw "before "] [styled "{{a.b}}"] [raw " after"]
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "before ");
        assert_eq!(spans[1].content, "{{a.b}}");
        assert_eq!(spans[1].style, normal_style());
        assert_eq!(spans[2].content, " after");
    }

    #[test]
    fn ref_whose_alias_is_in_error_set_uses_error_style() {
        let spans = highlight_refs("/users/{{ghost.id}}", &refs(&["ghost"]));
        let styled: Vec<_> = spans.iter().filter(|s| s.content.contains("{{")).collect();
        assert_eq!(styled.len(), 1);
        assert_eq!(styled[0].style, error_style());
    }

    #[test]
    fn unmatched_open_brace_falls_through_as_raw() {
        let spans = highlight_refs("oops {{ nope", &HashSet::new());
        // The `{{` never closes, so the whole string is raw text.
        let combined: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(combined, "oops {{ nope");
        assert!(spans.iter().all(|s| s.style == Style::default()));
    }

    #[test]
    fn parse_error_refs_extracts_block_not_found_alias() {
        let got = parse_error_refs("block `ghost` not found above this one");
        assert!(got.contains("ghost"));
    }

    #[test]
    fn parse_error_refs_returns_empty_for_unrelated_error() {
        let got = parse_error_refs("connection refused");
        assert!(got.is_empty(), "got: {got:?}");
    }
}
