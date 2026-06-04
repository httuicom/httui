//! Renderer for the `{{ref}}` hover-preview popup (Modal::RefPreview).
//! Borrowed chrome from `completion_popup`: same `popup_bg` hard-fill,
//! same border + title format, same fixed width so both popups read
//! as one family.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::ref_preview::{RefPreviewState, RefSource};

/// Match `completion_popup::POPUP_WIDTH` — same chrome family.
const POPUP_WIDTH: u16 = 36;
/// Soft cap on visible body rows so a multi-KB value can't paint the
/// entire viewport. Long values get wrapped + truncated; closing the
/// popup is one keystroke.
const MAX_VISIBLE_ROWS: usize = 8;

pub(super) fn render(
    frame: &mut Frame,
    area: Rect,
    state: &RefPreviewState,
    caret_cell: Option<(u16, u16)>,
) {
    if area.width < 20 || area.height < 4 {
        return;
    }
    let width = POPUP_WIDTH.min(area.width.saturating_sub(2)).max(20);
    let body_lines = wrap_value_lines(&state.value, width.saturating_sub(2));
    let value_rows = body_lines.len().clamp(1, MAX_VISIBLE_ROWS) as u16;
    // 2 = top + bottom border. 1 = source-chip line above the value.
    let popup_height = value_rows.saturating_add(3);

    let popup_area = crate::ui::anchor::place_near_caret(
        area,
        width,
        popup_height,
        caret_cell,
        crate::ui::anchor::CaretPlacement::FlipAbove,
    );

    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    // Hard-fill so editor content doesn't bleed through — same trick
    // `completion_popup::render` uses.
    {
        let buf = frame.buffer_mut();
        for y in popup_area.y..popup_area.y.saturating_add(popup_area.height) {
            for x in popup_area.x..popup_area.x.saturating_add(popup_area.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let title = build_title(state);
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(Color::LightCyan)
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup_area);
    frame.render_widget(outer, popup_area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let value_style = bg_style;
    let muted_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::muted());

    // First body line: source chip. The autocomplete popup uses the
    // same kind-suffix vocabulary as a secondary span on each item;
    // here it's the leading line because there's only one entry.
    let (source_text, source_style) = source_label(state);
    let mut lines: Vec<Line<'static>> = Vec::with_capacity(body_lines.len().saturating_add(1));
    lines.push(Line::from(Span::styled(source_text, source_style)));
    if body_lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(not resolved)".to_string(),
            muted_style,
        )));
    } else {
        for l in body_lines.into_iter().take(MAX_VISIBLE_ROWS) {
            lines.push(Line::from(Span::styled(l, value_style)));
        }
    }
    let value_para = Paragraph::new(lines).style(bg_style).wrap(Wrap { trim: false });
    frame.render_widget(value_para, inner);
}

// Positioning lives in `ui::anchor::place_near_caret` — the same
// helper feeds the completion popup, which keeps both popups in
// agreement on horizontal clamp + caret-anchor semantics.

/// Title is just `{{name}}` so it fits the autocomplete-width chrome
/// (36 cells). The source chip — `from env: X`, `from block: alias`
/// — moves into the body as its first line, mirroring the
/// autocomplete popup's "label · detail" pattern.
fn build_title(state: &RefPreviewState) -> Line<'static> {
    Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("{{{{{}}}}}", state.name),
            Style::default()
                .fg(crate::ui::palette::accent())
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ])
}

/// First body line ("from env: staging" / "from block: alias · no
/// result" / "unresolved"). Same kind-suffix vocabulary the
/// completion popup uses, so the user sees a familiar shape across
/// both popups.
fn source_label(state: &RefPreviewState) -> (String, Style) {
    let popup_bg = crate::ui::palette::popup_bg();
    match &state.source {
        RefSource::Env(env) => {
            let label = match env {
                Some(name) => format!("from env: {name}"),
                None => "from env".to_string(),
            };
            (label, Style::default().fg(crate::ui::palette::success()).bg(popup_bg))
        }
        RefSource::Block { alias, cached } => {
            let label = if *cached {
                format!("from block: {alias}")
            } else {
                format!("from block: {alias} · no result")
            };
            (label, Style::default().fg(crate::ui::palette::accent()).bg(popup_bg))
        }
        RefSource::Unknown => (
            "unresolved".to_string(),
            Style::default().fg(crate::ui::palette::amber()).bg(popup_bg),
        ),
    }
}

/// Wrap the value into lines that fit inside `width`. Conservative —
/// breaks on whitespace only, falls back to hard chops for tokens that
/// are longer than the available width.
fn wrap_value_lines(value: &str, width: u16) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    let width = width.max(1) as usize;
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in value.split_whitespace() {
        if word.chars().count() > width {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            // Hard-chop oversize tokens into width-sized chunks.
            let mut head: String = String::new();
            for ch in word.chars() {
                head.push(ch);
                if head.chars().count() == width {
                    out.push(std::mem::take(&mut head));
                }
            }
            if !head.is_empty() {
                current = head;
            }
            continue;
        }
        let candidate_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if candidate_len > width {
            out.push(std::mem::take(&mut current));
            current.push_str(word);
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn paint(state: &RefPreviewState) -> ratatui::buffer::Buffer {
        paint_at(state, None)
    }

    fn paint_at(
        state: &RefPreviewState,
        caret: Option<(u16, u16)>,
    ) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(80, 12);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let area = Rect {
                x: 0,
                y: 0,
                width: 80,
                height: 12,
            };
            render(f, area, state, caret);
        })
        .unwrap();
        term.backend().buffer().clone()
    }

    fn flat(buf: &ratatui::buffer::Buffer) -> String {
        (0..buf.area().height)
            .flat_map(|y| {
                (0..buf.area().width)
                    .filter_map(move |x| buf.cell((x, y)))
                    .map(|c| c.symbol().to_string())
            })
            .collect::<String>()
    }

    #[test]
    fn render_env_var_shows_name_and_source() {
        let state = RefPreviewState {
            name: "BASE_URL".into(),
            source: RefSource::Env(Some("staging".into())),
            value: "https://api.example.com".into(),
        };
        let buf = paint(&state);
        let text = flat(&buf);
        assert!(text.contains("BASE_URL"), "name should appear: {text}");
        assert!(text.contains("from env: staging"));
        assert!(text.contains("api.example.com"));
    }

    #[test]
    fn render_block_ref_marks_no_result_when_uncached() {
        let state = RefPreviewState {
            name: "login.response.token".into(),
            source: RefSource::Block {
                alias: "login".into(),
                cached: false,
            },
            value: String::new(),
        };
        let buf = paint(&state);
        let text = flat(&buf);
        assert!(text.contains("from block: login"));
        assert!(text.contains("no result"));
        assert!(text.contains("(not resolved)"));
    }

    #[test]
    fn render_unknown_ref_shows_unresolved_chip() {
        let state = RefPreviewState {
            name: "MISSING".into(),
            source: RefSource::Unknown,
            value: String::new(),
        };
        let buf = paint(&state);
        let text = flat(&buf);
        assert!(text.contains("unresolved"));
    }

    #[test]
    fn wrap_value_lines_breaks_on_whitespace() {
        let value = "hello world this is a long value";
        let lines = wrap_value_lines(value, 12);
        assert!(!lines.is_empty());
        for l in &lines {
            assert!(l.chars().count() <= 12, "line too long: {l:?}");
        }
    }

    #[test]
    fn wrap_value_lines_chops_oversize_token() {
        let value = "aaaaaaaaaaaaaaaa";
        let lines = wrap_value_lines(value, 5);
        assert_eq!(lines, vec!["aaaaa", "aaaaa", "aaaaa", "a"]);
    }

    #[test]
    fn wrap_value_lines_empty_returns_empty() {
        assert!(wrap_value_lines("", 20).is_empty());
    }

    #[test]
    fn render_noop_when_area_too_small() {
        let backend = TestBackend::new(10, 2);
        let mut term = Terminal::new(backend).unwrap();
        let state = RefPreviewState {
            name: "X".into(),
            source: RefSource::Env(None),
            value: "v".into(),
        };
        term.draw(|f| {
            let area = Rect {
                x: 0,
                y: 0,
                width: 10,
                height: 2,
            };
            render(f, area, &state, None);
        })
        .unwrap();
        // No panic — that's the contract.
    }

    #[test]
    fn render_anchored_with_caret_does_not_panic() {
        let state = RefPreviewState {
            name: "X".into(),
            source: RefSource::Env(None),
            value: "v".into(),
        };
        let _ = paint_at(&state, Some((5, 5)));
    }
}
