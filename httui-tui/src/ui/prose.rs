//! Prose rendering with regex-based markdown highlight.
//!
//! Tree-sitter-md is the future home for this — it nails nested
//! constructs, link parsing, table cells. Until we need that, regex
//! line-by-line covers the 90% of markdown that lands in technical
//! notes (headings, **bold**, *italic*, `code`, lists, blockquotes,
//! dividers, links).

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use ropey::Rope;

/// Render prose visible inside `area`, starting at line `top_line` of
/// the rope (relative to the rope, not the document). Lines past the
/// rope are simply not drawn.
pub fn render_prose(frame: &mut Frame, area: Rect, rope: &Rope, top_line: usize) {
    let mut lines = Vec::with_capacity(area.height as usize);
    let total = rope.len_lines();
    for off in 0..area.height as usize {
        let idx = top_line + off;
        if idx >= total {
            break;
        }
        let raw = rope.line(idx).to_string();
        let trimmed_nl = raw.trim_end_matches('\n');
        lines.push(highlight_line(trimmed_nl));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

/// Convert one line of markdown text to styled spans.
pub fn highlight_line(line: &str) -> Line<'static> {
    if let Some(spans) = conflict_marker(line) {
        return Line::from(spans);
    }
    if let Some(spans) = heading(line) {
        return Line::from(spans);
    }
    if let Some(spans) = divider(line) {
        return Line::from(spans);
    }
    if let Some(spans) = blockquote(line) {
        return Line::from(spans);
    }
    if let Some(spans) = list_item(line) {
        return Line::from(spans);
    }
    Line::from(inline_spans(line))
}

/// Style `<<<<<<<` / `=======` / `>>>>>>>` merge-conflict markers.
/// `None` for any other line.
fn conflict_marker(line: &str) -> Option<Vec<Span<'static>>> {
    if line.starts_with("<<<<<<<") {
        Some(vec![Span::styled(
            line.to_string(),
            Style::default()
                .fg(crate::ui::palette::foreground())
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        )])
    } else if line.starts_with("=======") && line.trim_end_matches('=').is_empty() {
        Some(vec![Span::styled(
            line.to_string(),
            Style::default()
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::DIM),
        )])
    } else if line.starts_with(">>>>>>>") {
        Some(vec![Span::styled(
            line.to_string(),
            Style::default()
                .fg(crate::ui::palette::foreground())
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD),
        )])
    } else {
        None
    }
}

fn heading(line: &str) -> Option<Vec<Span<'static>>> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|c| *c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = trimmed.get(level..).unwrap_or("");
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let text = rest.trim_start();
    let color = match level {
        1 => Color::LightCyan,
        2 => Color::LightBlue,
        3 => Color::LightMagenta,
        4 => crate::ui::palette::popup_border_accent(),
        _ => Color::Gray,
    };
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    Some(vec![
        Span::styled(
            format!("{} ", "#".repeat(level)),
            Style::default().fg(crate::ui::palette::muted()),
        ),
        Span::styled(text.to_string(), style),
    ])
}

fn divider(line: &str) -> Option<Vec<Span<'static>>> {
    let t = line.trim();
    if t == "---" || t == "***" || t == "___" {
        // Render the literal markers so cursor offsets and visual
        // columns stay in lock-step. The styling makes the line look
        // like a divider without changing its width.
        Some(vec![Span::styled(
            line.to_string(),
            Style::default()
                .fg(crate::ui::palette::muted())
                .add_modifier(Modifier::DIM),
        )])
    } else {
        None
    }
}

fn blockquote(line: &str) -> Option<Vec<Span<'static>>> {
    let trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("> ") {
        // Keep the literal `> ` so column counts match the buffer.
        // (Earlier we substituted a fancy `│ ` glyph; that was the
        // same width but a different char, which made the cursor
        // confusing on the marker.)
        let mut spans = vec![Span::styled(
            "> ".to_string(),
            Style::default().fg(crate::ui::palette::muted()),
        )];
        spans.extend(inline_spans(rest).into_iter().map(|s| {
            Span::styled(
                s.content.into_owned(),
                s.style.add_modifier(Modifier::ITALIC | Modifier::DIM),
            )
        }));
        Some(spans)
    } else {
        None
    }
}

fn list_item(line: &str) -> Option<Vec<Span<'static>>> {
    let leading_ws: String = line.chars().take_while(|c| c.is_whitespace()).collect();
    let body = &line[leading_ws.len()..];

    let bullet_len = if body.starts_with("- ") || body.starts_with("* ") || body.starts_with("+ ") {
        2
    } else if let Some(stripped) = body.strip_prefix(|c: char| c.is_ascii_digit()) {
        // Numbered list `1. ` / `12. `.
        let extra = stripped.chars().take_while(|c| c.is_ascii_digit()).count();
        let after_num = &body[extra + 1..];
        if after_num.starts_with(". ") {
            extra + 3
        } else {
            return None;
        }
    } else {
        return None;
    };

    let bullet = &body[..bullet_len];
    let rest = &body[bullet_len..];
    let mut spans = vec![Span::raw(leading_ws)];
    spans.push(Span::styled(
        bullet.to_string(),
        Style::default().fg(crate::ui::palette::muted()),
    ));
    spans.extend(inline_spans(rest));
    Some(spans)
}

/// Walk a line and extract bold / italic / inline-code / link runs.
/// Greedy left-to-right; nesting (e.g. bold inside italic) is not
/// modeled — first match wins.
///
/// Crucially, markdown markers (`**`, `*`, `` ` ``, `[`, `]`, `(`, `)`)
/// are kept in the rendered output (just dimmed) so each buffer char
/// corresponds 1:1 to a visual cell. That keeps cursor navigation,
/// search highlighting, and width calculations consistent with what
/// the user sees on screen.
pub fn inline_spans(line: &str) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut buffer = String::new();
    let marker = Style::default().fg(crate::ui::palette::muted());

    while i < bytes.len() {
        // **bold**
        if i + 1 < bytes.len() && &bytes[i..i + 2] == b"**" {
            if let Some(end) = find_close(line, i + 2, "**") {
                flush(&mut buffer, &mut out);
                out.push(Span::styled("**".to_string(), marker));
                let inner = &line[i + 2..end];
                out.push(Span::styled(
                    inner.to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                out.push(Span::styled("**".to_string(), marker));
                i = end + 2;
                continue;
            }
        }
        // *italic* — single asterisk, but not when it's actually `**`
        if bytes[i] == b'*' && (i + 1 >= bytes.len() || bytes[i + 1] != b'*') {
            if let Some(end) = find_close(line, i + 1, "*") {
                flush(&mut buffer, &mut out);
                out.push(Span::styled("*".to_string(), marker));
                let inner = &line[i + 1..end];
                out.push(Span::styled(
                    inner.to_string(),
                    Style::default().add_modifier(Modifier::ITALIC),
                ));
                out.push(Span::styled("*".to_string(), marker));
                i = end + 1;
                continue;
            }
        }
        // `code`
        if bytes[i] == b'`' {
            if let Some(end) = find_close(line, i + 1, "`") {
                flush(&mut buffer, &mut out);
                out.push(Span::styled("`".to_string(), marker));
                let inner = &line[i + 1..end];
                out.push(Span::styled(
                    inner.to_string(),
                    Style::default()
                        .fg(crate::ui::palette::popup_border_accent())
                        .add_modifier(Modifier::DIM),
                ));
                out.push(Span::styled("`".to_string(), marker));
                i = end + 1;
                continue;
            }
        }
        // [text](url)
        if bytes[i] == b'[' {
            if let Some(text_end) = find_close(line, i + 1, "]") {
                if line.get(text_end + 1..text_end + 2) == Some("(") {
                    if let Some(url_end) = find_close(line, text_end + 2, ")") {
                        flush(&mut buffer, &mut out);
                        out.push(Span::styled("[".to_string(), marker));
                        let text = &line[i + 1..text_end];
                        out.push(Span::styled(
                            text.to_string(),
                            Style::default()
                                .fg(crate::ui::palette::popup_key_label())
                                .add_modifier(Modifier::UNDERLINED),
                        ));
                        out.push(Span::styled("](".to_string(), marker));
                        let url = &line[text_end + 2..url_end];
                        out.push(Span::styled(
                            url.to_string(),
                            Style::default()
                                .fg(crate::ui::palette::muted())
                                .add_modifier(Modifier::DIM),
                        ));
                        out.push(Span::styled(")".to_string(), marker));
                        i = url_end + 1;
                        continue;
                    }
                }
            }
        }
        // Plain run: take one full UTF-8 char so multibyte sequences
        // aren't shredded byte-by-byte.
        let c = line[i..]
            .chars()
            .next()
            .expect("byte index inside &str must yield at least one char");
        let len = c.len_utf8();
        buffer.push(c);
        i += len;
    }
    flush(&mut buffer, &mut out);
    out
}

fn flush(buf: &mut String, out: &mut Vec<Span<'static>>) {
    if !buf.is_empty() {
        out.push(Span::raw(std::mem::take(buf)));
    }
}

fn find_close(line: &str, start: usize, needle: &str) -> Option<usize> {
    line[start..].find(needle).map(|p| start + p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn h1_styled_bold() {
        let l = highlight_line("# Hello");
        // 2 spans: hash + text
        assert_eq!(l.spans.len(), 2);
        assert!(l.spans[1].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn bold_inline() {
        let spans = inline_spans("this is **strong** text");
        assert!(spans.iter().any(|s| s.content == "strong"));
        let bold = spans.iter().find(|s| s.content == "strong").unwrap();
        assert!(bold.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn italic_inline() {
        let spans = inline_spans("an *emphatic* word");
        let italic = spans.iter().find(|s| s.content == "emphatic").unwrap();
        assert!(italic.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn code_inline() {
        let spans = inline_spans("call `fn()`!");
        let code = spans.iter().find(|s| s.content == "fn()").unwrap();
        assert_eq!(
            code.style.fg,
            Some(crate::ui::palette::popup_border_accent())
        );
    }

    #[test]
    fn link_text_only_kept() {
        let spans = inline_spans("see [here](https://x.com).");
        let link = spans.iter().find(|s| s.content == "here").unwrap();
        assert!(link.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn list_dash() {
        let l = highlight_line("- item one");
        assert!(l.spans.iter().any(|s| s.content == "- "));
        assert!(l.spans.iter().any(|s| s.content == "item one"));
    }

    #[test]
    fn list_numbered() {
        let l = highlight_line("12. twelfth");
        assert!(l.spans.iter().any(|s| s.content == "12. "));
    }

    #[test]
    fn divider_keeps_literal_chars() {
        // Width must match the buffer (3 chars), not be replaced by
        // 60 box-drawing glyphs — otherwise cursor offsets stop
        // matching what the user sees.
        let l = highlight_line("---");
        let concat: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(concat, "---");
    }

    #[test]
    fn blockquote_keeps_marker() {
        let l = highlight_line("> wisdom");
        assert_eq!(l.spans[0].content, "> ");
    }

    #[test]
    fn plain_text_passes_through() {
        let l = highlight_line("just words here");
        assert_eq!(l.spans.len(), 1);
        assert_eq!(l.spans[0].content, "just words here");
    }

    #[test]
    fn utf8_multibyte_chars_stay_intact() {
        // Em dash (U+2014, 3 bytes), accented letter (U+00E1, 2 bytes).
        let l = highlight_line("operação — feita");
        let concat: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(concat, "operação — feita");
    }

    #[test]
    fn conflict_marker_left_paints_red_background() {
        let line = highlight_line("<<<<<<< HEAD");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].content, "<<<<<<< HEAD");
        assert_eq!(line.spans[0].style.bg, Some(Color::Red));
    }

    #[test]
    fn conflict_marker_separator_paints_dim_gray() {
        let line = highlight_line("=======");
        assert_eq!(line.spans.len(), 1);
        assert!(line.spans[0].style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn conflict_marker_right_paints_green_background() {
        let line = highlight_line(">>>>>>> feature/x");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].style.bg, Some(Color::Green));
    }

    #[test]
    fn equals_inside_heading_is_not_a_conflict_separator() {
        // A line like `=== something` should fall through to the
        // normal pipeline because `conflict_marker` only matches when
        // the line is `=====...` (all equals).
        let line = highlight_line("=== something else");
        // No special bg color on the first span.
        assert_ne!(line.spans[0].style.bg, Some(Color::Red));
        assert_ne!(line.spans[0].style.bg, Some(Color::Green));
    }

    #[test]
    fn inline_markers_stay_visible_for_width_parity() {
        // The whole point: rendered output must reproduce the buffer
        // verbatim (just styled), so cursor / search positions match.
        let cases = [
            "**bold**",
            "*italic*",
            "`code`",
            "[link](url)",
            "ação **forte** —",
        ];
        for input in cases {
            let spans = inline_spans(input);
            let concat: String = spans.iter().map(|s| s.content.as_ref()).collect();
            assert_eq!(concat, input, "render must preserve raw markers");
        }
    }
}
