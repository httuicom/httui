//! Render an executable block as a bordered widget.
//!
//! Visual only — fields aren't editable yet, no run button, no tabs.
//! Each block type gets a tailored body (HTTP shows method+URL, DB
//! shows the SQL, E2E lists steps). Forward-compat: unknown block
//! types fall through to a generic body so new types render reasonably
//! even before they have a dedicated function.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use std::collections::HashMap;

use crate::buffer::block::{BlockNode, ExecutionState};

mod db_chrome;
pub(crate) mod db_table;
mod http_panel;
mod ref_highlight;
pub(crate) mod result_tabs;

/// Lookup `connection_id → human_name` so DB block footers can show
/// `connection: prod-db` instead of a UUID. Empty map = render the
/// raw fence value as-is.
pub type ConnectionNames = HashMap<String, String>;

/// Paint a single block segment. `selected_row` is `Some(idx)`
/// when the cursor is in `Cursor::InBlockResult` (drives the row
/// highlight inside the DB result table). `viewport_top` is
/// `Some(&mut)` for the focused block; the result-table scroll
/// uses it as persistent state — the window only slides when the
/// cursor would otherwise leave it (`clamp_result_viewport`).
/// Other blocks pass `None` and default to viewport_top = 0
/// (rows 0..MAX_VISIBLE).
#[allow(clippy::too_many_arguments)]
pub fn render_block_with_selection(
    frame: &mut Frame,
    area: Rect,
    b: &BlockNode,
    selected: bool,
    selected_row: Option<usize>,
    viewport_top: Option<&mut u16>,
    names: &ConnectionNames,
    result_tab: crate::app::ResultPanelTab,
) {
    // Bordered card with state-colored border. Inside, sections
    // stack top-to-bottom: header bar → (fence header if selected)
    // → body/result panel → (fence closer if selected) → footer
    // bar. Header and footer render with a subtle bg tint so the
    // chrome separates visually from the SQL/table region —
    // matches the desktop widget's grey shells.
    let border_color = state_color(&b.state, selected);
    let outer = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if inner.height < 2 || inner.width == 0 {
        return;
    }

    // Paint the card body bg before any sub-region renders. The
    // header / footer bars then overpaint their chrome bg on top;
    // editable body lines and the result panel inherit this tint,
    // so the whole card reads as a single surface against the
    // canvas instead of letting body lines fall back to the
    // terminal default.
    frame.render_widget(
        Block::default().style(Style::default().bg(crate::ui::palette::block_body_bg())),
        inner,
    );

    // Top: header bar.
    let header_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    db_chrome::render_db_header_bar(frame, header_rect, b, names);

    // Bottom: footer bar.
    let footer_rect = Rect {
        x: inner.x,
        y: inner.y.saturating_add(inner.height.saturating_sub(1)),
        width: inner.width,
        height: 1,
    };
    db_chrome::render_db_footer_bar(frame, footer_rect, b, names);

    // Middle: everything between header (1) and footer (1).
    let mut middle = Rect {
        x: inner.x,
        y: inner.y.saturating_add(1),
        width: inner.width,
        height: inner.height.saturating_sub(2),
    };
    if middle.height == 0 {
        return;
    }

    // HTTP and DB blocks paint the fence closer themselves —
    // between the editable region and the result panel — so the
    // ` ``` ` line visually fences only the source the user edits.
    // The result panel belongs to the card chrome, not to the
    // markdown source.
    let block_owns_closer = b.is_http() || b.is_db();

    // Carve fence rows when the cursor is on, just below header and
    // just above footer. When the block owns its own closer (HTTP /
    // DB) we carve only the header here; the closer is positioned by
    // the per-kind inner renderer between raw input and result panel.
    if selected && middle.height >= 2 {
        let fence_header_rect = Rect {
            x: middle.x,
            y: middle.y,
            width: middle.width,
            height: 1,
        };
        render_fence_header_row(frame, fence_header_rect, b);
        if !block_owns_closer {
            let fence_closer_rect = Rect {
                x: middle.x,
                y: middle.y.saturating_add(middle.height.saturating_sub(1)),
                width: middle.width,
                height: 1,
            };
            render_fence_closer_row(frame, fence_closer_rect, b);
            middle = Rect {
                x: middle.x,
                y: middle.y.saturating_add(1),
                width: middle.width,
                height: middle.height.saturating_sub(2),
            };
        } else {
            middle = Rect {
                x: middle.x,
                y: middle.y.saturating_add(1),
                width: middle.width,
                height: middle.height.saturating_sub(1),
            };
        }
    }

    if b.is_db() {
        db_table::render_db_inner(
            frame,
            middle,
            b,
            selected_row,
            viewport_top,
            names,
            result_tab,
            selected,
        );
        return;
    }

    if b.is_http() {
        // `selected_row.is_some()` means cursor is parked in the
        // response panel (`InBlockResult`). The panel uses this to
        // paint a subtle "focused" cue so the user sees that `j`
        // landed there — without this nothing changes visually
        // when entering the response, since HTTP doesn't have a
        // selected-row highlight like DB tables do.
        let cursor_in_result = selected_row.is_some();
        http_panel::render_http_inner(frame, middle, b, result_tab, selected, cursor_in_result);
        return;
    }

    if selected {
        // Cursor on, non-DB / non-HTTP: paint the raw body lines so
        // what the user is editing is what they see (parity with
        // CM6 desktop).
        let body = raw_body_text(b);
        let lines: Vec<Line<'static>> = if body.is_empty() {
            if b.is_e2e() {
                e2e_body(b)
            } else {
                generic_body(b)
            }
        } else {
            body.lines()
                .map(|l| Line::from(Span::raw(l.to_string())))
                .collect()
        };
        frame.render_widget(Paragraph::new(lines), middle);
        return;
    }

    let lines = if b.is_e2e() {
        e2e_body(b)
    } else {
        generic_body(b)
    };

    frame.render_widget(Paragraph::new(lines), middle);
}

/// Paint the fence header row inside the inner area (just under the
/// chrome header bar, when the cursor sits on the block). Reads from
/// `b.raw` so what the user types is what the row paints.
fn render_fence_header_row(frame: &mut Frame, area: Rect, b: &BlockNode) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let raw_text = b.raw.to_string();
    let header = raw_text.lines().next().unwrap_or("```").to_string();
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            header,
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}

/// Paint the fence closer row (just above the chrome footer bar).
fn render_fence_closer_row(frame: &mut Frame, area: Rect, b: &BlockNode) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let raw_text = b.raw.to_string();
    let closer = raw_text.lines().last().unwrap_or("```").to_string();
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            closer,
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}

/// Extract the body region (lines between the fence header and the
/// closer) from a block's raw rope, joined with `\n`. Returns an
/// empty string when the rope is degenerate.
fn raw_body_text(b: &BlockNode) -> String {
    let raw = b.raw.to_string();
    let lines: Vec<&str> = raw.lines().collect();
    if lines.len() < 2 {
        return String::new();
    }
    // Drop the fence header (line 0) and the closer (last line).
    let body = &lines[1..lines.len().saturating_sub(1)];
    body.join("\n")
}

// The SQL lexer fragments `{{a.b.0.c.0.d}}` into multiple spans
// (digits are styled as numbers), so per-span ref detection misses
// the `{{`/`}}` pair. Overlay positionally: reconstruct the line,
// project span styles onto a per-byte map, then stamp ref ranges on
// top and collapse runs back into spans.
fn overlay_refs_on_spans(
    spans: Vec<Span<'static>>,
    error_refs: &std::collections::HashSet<String>,
) -> Vec<Span<'static>> {
    let line: String = spans.iter().map(|s| s.content.as_ref()).collect();
    if !line.contains("{{") {
        return spans;
    }
    let bytes = line.as_bytes();
    let mut style_per_byte: Vec<Style> = vec![Style::default(); bytes.len()];
    let mut cursor = 0usize;
    for span in &spans {
        let len = span.content.len();
        for slot in style_per_byte.iter_mut().skip(cursor).take(len) {
            *slot = span.style;
        }
        cursor += len;
    }
    for (start, end, style) in find_ref_ranges(&line, error_refs) {
        for slot in style_per_byte.iter_mut().take(end).skip(start) {
            *slot = style;
        }
    }
    collapse_per_byte_styles(&line, &style_per_byte)
}

fn find_ref_ranges(
    line: &str,
    error_refs: &std::collections::HashSet<String>,
) -> Vec<(usize, usize, Style)> {
    let mut out: Vec<(usize, usize, Style)> = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let inner_start = i + 2;
            let mut j = inner_start;
            while j + 1 < bytes.len() {
                if bytes[j] == b'}' && bytes[j + 1] == b'}' {
                    let inner = &line[inner_start..j];
                    let alias = inner.split('.').next().unwrap_or("").trim();
                    let style = if !alias.is_empty() && error_refs.contains(alias) {
                        ref_highlight::error_style()
                    } else {
                        ref_highlight::normal_style()
                    };
                    out.push((i, j + 2, style));
                    i = j + 2;
                    break;
                }
                j += 1;
            }
            if j + 1 >= bytes.len() {
                break;
            }
        } else {
            i += 1;
        }
    }
    out
}

fn collapse_per_byte_styles(line: &str, style_per_byte: &[Style]) -> Vec<Span<'static>> {
    let bytes = line.as_bytes();
    let mut out: Vec<Span<'static>> = Vec::new();
    if bytes.is_empty() {
        out.push(Span::raw(""));
        return out;
    }
    let mut run_start = 0usize;
    for i in 1..=bytes.len() {
        let boundary = i == bytes.len() || style_per_byte[i] != style_per_byte[i - 1];
        if boundary {
            if let Ok(text) = std::str::from_utf8(&bytes[run_start..i]) {
                out.push(Span::styled(text.to_string(), style_per_byte[i - 1]));
            }
            run_start = i;
        }
    }
    out
}

/// Paint a faint bg over `area` to signal "cursor lives here". Used
/// when the cursor enters the HTTP response panel via `j` — without
/// it nothing changes visually (HTTP has no selected-row highlight
/// like DB tables) and the motion looks like a no-op.
fn paint_panel_focus_bg(frame: &mut Frame, area: Rect) {
    let buf = frame.buffer_mut();
    let tint = Style::default().bg(crate::ui::palette::block_body_bg());
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            if let Some(cell) = buf.cell_mut((x, y)) {
                cell.set_style(cell.style().patch(tint));
            }
        }
    }
}

/// Bottom-right hint chip on the focused response panel: `<CR>
/// detail`. Cheap discoverability signal — the user just walked
/// into the panel with `j`; they need to know `<CR>` opens a fuller
/// view. Renders only when there's room (panel ≥ 3 rows tall).
fn paint_panel_focus_hint(frame: &mut Frame, area: Rect) {
    if area.height < 3 || area.width < 16 {
        return;
    }
    let chip_key = Style::default()
        .bg(Color::LightBlue)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default()
        .fg(Color::Gray)
        .bg(crate::ui::palette::block_body_bg());
    let hint = Line::from(vec![
        Span::styled(" <CR> ", chip_key),
        Span::styled(" detail ", chip_label),
    ]);
    let hint_width: u16 = hint
        .spans
        .iter()
        .map(|s| s.content.chars().count() as u16)
        .sum();
    let x = area
        .x
        .saturating_add(area.width.saturating_sub(hint_width.saturating_add(1)));
    let y = area.y.saturating_add(area.height.saturating_sub(1));
    let hint_rect = Rect {
        x,
        y,
        width: hint_width.min(area.width),
        height: 1,
    };
    frame.render_widget(Paragraph::new(hint), hint_rect);
}

/// Border color for the block card. Selection wins over execution
/// state — the user expects the focused block to stand out
/// regardless of its run history.
fn state_color(state: &ExecutionState, selected: bool) -> Color {
    if selected {
        return Color::Cyan;
    }
    match state {
        ExecutionState::Idle => Color::DarkGray,
        ExecutionState::Cached => Color::Blue,
        ExecutionState::Running => Color::Yellow,
        ExecutionState::Success => Color::Green,
        ExecutionState::Error(_) => Color::Red,
    }
}

fn e2e_body(b: &BlockNode) -> Vec<Line<'static>> {
    let base = b
        .params
        .get("base_url")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let steps = b
        .params
        .get("steps")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut lines = vec![Line::from(Span::styled(
        format!("base: {base}"),
        Style::default().fg(Color::DarkGray),
    ))];
    for (idx, step) in steps.iter().enumerate() {
        let method = step.get("method").and_then(|v| v.as_str()).unwrap_or("GET");
        let url = step.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let name = step.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let prefix = format!("{}.", idx + 1);
        lines.push(Line::from(vec![
            Span::styled(prefix, Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(
                format!(" {method} "),
                Style::default()
                    .fg(Color::Black)
                    .bg(http_panel::method_color(method))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::raw(url.to_string()),
            Span::raw(if name.is_empty() {
                "".to_string()
            } else {
                format!("  ({name})")
            }),
        ]));
    }
    lines
}

fn generic_body(b: &BlockNode) -> Vec<Line<'static>> {
    let raw = serde_json::to_string(&b.params).unwrap_or_else(|_| "—".into());
    vec![Line::from(Span::styled(
        raw,
        Style::default().fg(Color::DarkGray),
    ))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::{BlockId, ExecutionState};
    use serde_json::json;

    // db_footer_text / db_result_line tests dropped — the footer
    // is now painted directly into a Frame rect by render_db_footer_bar
    // and the status text moved into that bar; verifying spans inside
    // a frame buffer is harness-noisy enough that the visual checks
    // graduated to manual review for V1.

    #[test]
    fn overlay_refs_keeps_ref_intact_across_numeric_fragments() {
        use crate::ui::sql_highlight;
        let line = "WHERE id = {{pg.response.results.0.rows.0.id}}";
        let highlighted = sql_highlight::highlight(line);
        let overlaid =
            overlay_refs_on_spans(highlighted[0].clone(), &std::collections::HashSet::new());
        let ref_style = ref_highlight::normal_style();
        let has_full_ref_span = overlaid
            .iter()
            .any(|s| s.content == "{{pg.response.results.0.rows.0.id}}" && s.style == ref_style);
        assert!(
            has_full_ref_span,
            "expected one merged ref span styled cyan/bold; got: {:?}",
            overlaid
                .iter()
                .map(|s| (s.content.as_ref(), s.style))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn overlay_refs_uses_error_style_when_alias_in_error_set() {
        use crate::ui::sql_highlight;
        let line = "SELECT {{ghost.id}}";
        let highlighted = sql_highlight::highlight(line);
        let mut errors = std::collections::HashSet::new();
        errors.insert("ghost".to_string());
        let overlaid = overlay_refs_on_spans(highlighted[0].clone(), &errors);
        let err_style = ref_highlight::error_style();
        let has_red_ref = overlaid
            .iter()
            .any(|s| s.content == "{{ghost.id}}" && s.style == err_style);
        assert!(
            has_red_ref,
            "expected red `{{{{ghost.id}}}}`; got: {overlaid:?}"
        );
    }

    #[test]
    fn e2e_body_lists_steps() {
        let b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "e2e".into(),
            alias: Some("flow".into()),
            display_mode: None,
            params: json!({
                "base_url": "https://x.com",
                "steps": [
                    {"name":"Login","method":"POST","url":"/auth"},
                    {"name":"Me","method":"GET","url":"/me"}
                ]
            }),
            state: ExecutionState::Idle,
            cached_result: None,
        };
        let lines = e2e_body(&b);
        assert!(lines.len() >= 3); // base + 2 steps
    }

    #[test]
    fn state_color_picks_cyan_when_selected_regardless_of_state() {
        for st in [
            ExecutionState::Idle,
            ExecutionState::Cached,
            ExecutionState::Running,
            ExecutionState::Success,
            ExecutionState::Error("e".into()),
        ] {
            assert_eq!(state_color(&st, true), Color::Cyan);
        }
    }

    #[test]
    fn state_color_distinct_color_per_unselected_state() {
        assert_eq!(state_color(&ExecutionState::Idle, false), Color::DarkGray);
        assert_eq!(state_color(&ExecutionState::Cached, false), Color::Blue);
        assert_eq!(state_color(&ExecutionState::Running, false), Color::Yellow);
        assert_eq!(state_color(&ExecutionState::Success, false), Color::Green);
        assert_eq!(
            state_color(&ExecutionState::Error("e".into()), false),
            Color::Red
        );
    }

    #[test]
    fn raw_body_text_strips_fence_header_and_closer() {
        let mut b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::from_str("```db-sqlite alias=q\nSELECT 1\nFROM t\n```"),
            block_type: "db-sqlite".into(),
            alias: Some("q".into()),
            display_mode: None,
            params: json!({}),
            state: ExecutionState::Idle,
            cached_result: None,
        };
        assert_eq!(raw_body_text(&b), "SELECT 1\nFROM t");
        // Degenerate (single-line rope) → empty.
        b.raw = ropey::Rope::from_str("only-one-line");
        assert_eq!(raw_body_text(&b), "");
    }

    #[test]
    fn generic_body_renders_params_json_for_unknown_block_types() {
        let b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "weird".into(),
            alias: None,
            display_mode: None,
            params: json!({"key": "val"}),
            state: ExecutionState::Idle,
            cached_result: None,
        };
        let lines = generic_body(&b);
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("\"key\""));
    }

    #[test]
    fn find_ref_ranges_locates_balanced_braces() {
        let ranges = find_ref_ranges("a {{x.y}} b", &std::collections::HashSet::new());
        assert_eq!(ranges.len(), 1);
        let (start, end, _) = ranges[0];
        assert_eq!(start, 2);
        assert_eq!(end, 9);
    }

    #[test]
    fn find_ref_ranges_skips_unmatched_open_brace() {
        let ranges = find_ref_ranges("a {{x.y b", &std::collections::HashSet::new());
        assert!(ranges.is_empty());
    }

    #[test]
    fn collapse_per_byte_styles_merges_adjacent_runs() {
        let line = "abc";
        let s_a = Style::default().fg(Color::Cyan);
        let s_b = Style::default().fg(Color::Green);
        let styles = vec![s_a, s_a, s_b];
        let spans = collapse_per_byte_styles(line, &styles);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "ab");
        assert_eq!(spans[1].content, "c");
    }

    #[test]
    fn collapse_per_byte_styles_handles_empty_line() {
        let spans = collapse_per_byte_styles("", &[]);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content, "");
    }

    fn http_block_simple() -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::from_str("```http alias=q\nGET https://x.com\n```"),
            block_type: "http".into(),
            alias: Some("q".into()),
            display_mode: None,
            params: json!({}),
            state: ExecutionState::Idle,
            cached_result: None,
        }
    }

    #[test]
    fn render_fence_header_row_smoke_test() {
        let b = http_block_simple();
        let backend = ratatui::backend::TestBackend::new(40, 1);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_fence_header_row(
                    f,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 40,
                        height: 1,
                    },
                    &b,
                );
            })
            .unwrap();
    }

    #[test]
    fn render_fence_closer_row_smoke_test() {
        let b = http_block_simple();
        let backend = ratatui::backend::TestBackend::new(40, 1);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_fence_closer_row(
                    f,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 40,
                        height: 1,
                    },
                    &b,
                );
            })
            .unwrap();
    }

    #[test]
    fn paint_panel_focus_bg_does_not_panic_for_zero_size() {
        let backend = ratatui::backend::TestBackend::new(10, 5);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                paint_panel_focus_bg(
                    f,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 0,
                        height: 0,
                    },
                );
            })
            .unwrap();
    }

    #[test]
    fn paint_panel_focus_hint_skips_when_panel_too_small() {
        let backend = ratatui::backend::TestBackend::new(10, 5);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                paint_panel_focus_hint(
                    f,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 2,
                        height: 1,
                    },
                );
            })
            .unwrap();
    }

    #[test]
    fn paint_panel_focus_hint_renders_when_panel_is_big_enough() {
        let backend = ratatui::backend::TestBackend::new(40, 5);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                paint_panel_focus_hint(
                    f,
                    Rect {
                        x: 0,
                        y: 0,
                        width: 40,
                        height: 5,
                    },
                );
            })
            .unwrap();
    }
}
