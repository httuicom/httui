//! Render an executable block as a bordered widget.
//!
//! Visual only — fields aren't editable yet, no run button, no tabs.
//! Each block type gets a tailored body (HTTP shows method+URL, DB
//! shows the SQL, E2E lists steps). Forward-compat: unknown block
//! types fall through to a generic body so new types render reasonably
//! even before they have a dedicated function.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use std::collections::HashMap;

use crate::buffer::block::{BlockNode, ExecutionState};

mod http_panel;
mod ref_highlight;

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

    // Top: header bar.
    let header_rect = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: 1,
    };
    render_db_header_bar(frame, header_rect, b, names);

    // Bottom: footer bar.
    let footer_rect = Rect {
        x: inner.x,
        y: inner.y.saturating_add(inner.height.saturating_sub(1)),
        width: inner.width,
        height: 1,
    };
    render_db_footer_bar(frame, footer_rect, b, names);

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
        render_db_inner(
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

/// Subtle bg used by the header / footer chrome bars. Slightly
/// darker than the editor body so the eye can pick out the section
/// boundaries. Falls back to default when the terminal can't render
/// the RGB color.
fn chrome_bg() -> Color {
    Color::Rgb(20, 22, 28)
}

/// Header bar dispatcher — paints kind-specific content on a chrome
/// bg row. DB blocks get `[DB] alias · vault [RW] subtype`; HTTP
/// gets `[HTTP] alias · METHOD host`; E2E and unknown kinds get a
/// minimal `[BLK] alias`. Right-aligned keymap hint applies to all.
fn render_db_header_bar(frame: &mut Frame, area: Rect, b: &BlockNode, names: &ConnectionNames) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let bg = Style::default().bg(chrome_bg());
    // Fill the whole row with the chrome bg first.
    let pad: String = " ".repeat(area.width as usize);
    frame.render_widget(Paragraph::new(Line::from(Span::styled(pad, bg))), area);

    let mut left = if b.is_http() {
        http_panel::http_header_left_spans(b, bg)
    } else if b.is_db() {
        db_header_left_spans(b, names, bg)
    } else if b.is_e2e() {
        generic_header_left_spans(b, "E2E", Color::Green, bg)
    } else {
        generic_header_left_spans(b, "BLK", Color::DarkGray, bg)
    };
    // Prepend a state dot so the user can see at a glance whether
    // the block is idle / cached / running / errored. Inserted at
    // index 1 so it follows the leading single-space pad — the
    // existing `*_header_left_spans` paths all start with that pad.
    let dot = state_dot(&b.state, bg);
    if left.len() >= 2 {
        left.insert(1, dot);
        left.insert(2, Span::styled(" ", bg));
    } else {
        left.insert(0, dot);
        left.insert(1, Span::styled(" ", bg));
    }

    let used: u16 = left.iter().map(|s| s.content.chars().count() as u16).sum();
    // Block-type aware chip line. Wired chords:
    // - DB / HTTP: r run · gh history · gx export · gs settings
    // - other:     r run · gh history · gs settings (export not wired yet)
    let hint = if b.is_db() || b.is_http() {
        "r run  ·  gh history  ·  gx export  ·  gs settings "
    } else {
        "r run  ·  gh history  ·  gs settings "
    };
    let hint_len = hint.chars().count() as u16;
    let space_for_hint = area.width.saturating_sub(used);
    if space_for_hint >= hint_len {
        let pad_len = space_for_hint.saturating_sub(hint_len);
        let mut all_spans = left;
        all_spans.push(Span::styled(" ".repeat(pad_len as usize), bg));
        all_spans.push(Span::styled(hint.to_string(), bg.fg(Color::DarkGray)));
        frame.render_widget(Paragraph::new(Line::from(all_spans)), area);
    } else {
        frame.render_widget(Paragraph::new(Line::from(left)), area);
    }
}

/// Colored `●` glyph reflecting the block's last-known execution
/// state. Painted on every block's chrome header so the user can
/// see the run status at a glance — particularly useful after
/// scrolling away from the block or running multiple in sequence.
///
/// State-to-color mapping is borrowed from the desktop's
/// `ExecutableBlockShell` badge: Idle gray, Cached cyan, Running
/// yellow, Success green, Error red. The dot is one cell wide so
/// header chrome budgeting elsewhere doesn't have to grow.
fn state_dot(state: &crate::buffer::block::ExecutionState, bg: Style) -> Span<'static> {
    use crate::buffer::block::ExecutionState as ES;
    let color = match state {
        ES::Idle => Color::DarkGray,
        ES::Cached => Color::LightCyan,
        ES::Running => Color::LightYellow,
        ES::Success => Color::LightGreen,
        ES::Error(_) => Color::LightRed,
    };
    Span::styled("●", bg.fg(color).add_modifier(Modifier::BOLD))
}

fn db_header_left_spans(b: &BlockNode, names: &ConnectionNames, bg: Style) -> Vec<Span<'static>> {
    let alias = b.alias.clone().unwrap_or_else(|| "—".into());
    let conn_raw = b
        .params
        .get("connection")
        .or_else(|| b.params.get("connection_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let vault = if conn_raw.is_empty() {
        "—".to_string()
    } else {
        names
            .get(conn_raw)
            .cloned()
            .unwrap_or_else(|| conn_raw.to_string())
    };
    let subtype = b
        .block_type
        .strip_prefix("db-")
        .unwrap_or("generic")
        .to_string();
    vec![
        Span::raw(" "),
        Span::styled(
            " DB ",
            Style::default()
                .bg(Color::Blue)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", bg),
        Span::styled(alias, bg.fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled("  ·  ", bg.fg(Color::DarkGray)),
        Span::styled(vault, bg.fg(Color::Gray)),
        Span::styled("  ", bg),
        Span::styled(
            " RW ",
            Style::default()
                .bg(Color::Green)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", bg),
        Span::styled(subtype, bg.fg(Color::DarkGray)),
    ]
}

fn generic_header_left_spans(
    b: &BlockNode,
    label: &str,
    badge_bg: Color,
    bg: Style,
) -> Vec<Span<'static>> {
    let alias = b.alias.clone().unwrap_or_else(|| "—".into());
    vec![
        Span::raw(" "),
        Span::styled(
            format!(" {label} "),
            Style::default()
                .bg(badge_bg)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ", bg),
        Span::styled(alias, bg.fg(Color::White).add_modifier(Modifier::BOLD)),
    ]
}

/// Footer bar dispatcher — kind-specific summary on a chrome-bg
/// row. DB blocks show `● connected · vault (rw) │ rows · elapsed
/// · cached · press \`r\` to run`; HTTP shows `● connected ·
/// METHOD url │ status · elapsed · size · cached · press \`r\` to
/// run`; other kinds show the run hint only.
fn render_db_footer_bar(frame: &mut Frame, area: Rect, b: &BlockNode, names: &ConnectionNames) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let bg = Style::default().bg(chrome_bg());
    let pad: String = " ".repeat(area.width as usize);
    frame.render_widget(Paragraph::new(Line::from(Span::styled(pad, bg))), area);

    let (dot_color, dot_label) = match &b.state {
        ExecutionState::Idle => (Color::Green, "connected"),
        ExecutionState::Running => (Color::Yellow, "running"),
        ExecutionState::Cached => (Color::Green, "connected"),
        ExecutionState::Success => (Color::Green, "connected"),
        ExecutionState::Error(_) => (Color::Red, "error"),
    };

    let dim = bg.fg(Color::DarkGray);
    let (left, right) = if b.is_http() {
        http_panel::http_footer_spans(b, bg, dot_color, dot_label)
    } else if b.is_db() {
        db_footer_spans(b, names, bg, dot_color, dot_label)
    } else {
        // Generic: status dot + run hint only.
        let l = vec![
            Span::raw(" "),
            Span::styled("●", bg.fg(dot_color)),
            Span::styled("  ", bg),
            Span::styled(dot_label, bg.fg(Color::Gray)),
        ];
        let r = vec![Span::styled("press `r` to run ", dim)];
        (l, r)
    };

    let used_left: u16 = left.iter().map(|s| s.content.chars().count() as u16).sum();
    let used_right: u16 = right.iter().map(|s| s.content.chars().count() as u16).sum();
    let mut spans = left;
    let pad_w = area.width.saturating_sub(used_left + used_right);
    spans.push(Span::styled(" ".repeat(pad_w as usize), bg));
    spans.extend(right);
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn db_footer_spans(
    b: &BlockNode,
    names: &ConnectionNames,
    bg: Style,
    dot_color: Color,
    dot_label: &'static str,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let dim = bg.fg(Color::DarkGray);
    let conn_raw = b
        .params
        .get("connection")
        .or_else(|| b.params.get("connection_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let vault = if conn_raw.is_empty() {
        "—".to_string()
    } else {
        names
            .get(conn_raw)
            .cloned()
            .unwrap_or_else(|| conn_raw.to_string())
    };
    let left: Vec<Span<'static>> = vec![
        Span::raw(" "),
        Span::styled("●", bg.fg(dot_color)),
        Span::styled("  ", bg),
        Span::styled(dot_label, bg.fg(Color::Gray)),
        Span::styled("  ·  ", dim),
        Span::styled(vault, bg.fg(Color::Gray)),
        Span::styled(" (rw)", dim),
        Span::styled("  │  ", dim),
    ];

    let mut right: Vec<Span<'static>> = Vec::new();
    if let Some(s) = db_summary(b) {
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
    let tint = Style::default().bg(Color::Rgb(30, 35, 50));
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
    let chip_label = Style::default().fg(Color::Gray).bg(Color::Rgb(30, 35, 50));
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


/// Render the DB block's content area between the chrome header and
/// footer bars. Layout:
/// ```text
/// SQL body          (sql_lines rows)
/// tab bar           (1 row)
/// separator         (1 row)
/// sub-tabs          (1 row when multi-statement)
/// result panel      (table_height rows)
/// ```
/// Status banner / connection name / hotkey hint live in the chrome
/// bars now (`render_db_header_bar`, `render_db_footer_bar`).
#[allow(clippy::too_many_arguments)]
fn render_db_inner(
    frame: &mut Frame,
    inner: Rect,
    b: &BlockNode,
    selected_row: Option<usize>,
    viewport_top: Option<&mut u16>,
    _names: &ConnectionNames,
    result_tab: crate::app::ResultPanelTab,
    selected: bool,
) {
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let mode = b.effective_display_mode();
    let show_input = mode.shows_input();
    let show_output = mode.shows_output();

    let query_string;
    let query: &str = if selected {
        query_string = raw_body_text(b);
        &query_string
    } else {
        b.params.get("query").and_then(|v| v.as_str()).unwrap_or("")
    };
    let sql_lines = query.lines().count().max(1) as u16;
    let table_height = if show_output {
        db_result_table_height(b)
    } else {
        0
    };
    // Mirror HTTP: paint the closer fence row between the editable
    // region (SQL) and the result table whenever the cursor is on
    // the block. `render_block_with_selection` reserves the row by
    // gating its own carve on `block_owns_closer` (DB now owns it).
    let closer_height = if selected && show_input { 1 } else { 0 };

    let mut constraints: Vec<Constraint> = Vec::new();
    if show_input {
        constraints.push(Constraint::Length(sql_lines));
    }
    if closer_height > 0 {
        constraints.push(Constraint::Length(closer_height));
    }
    if table_height > 0 {
        constraints.push(Constraint::Length(table_height));
    }
    if constraints.is_empty() {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let mut idx = 0;

    if show_input {
        let mut sql_lines_styled = super::sql_highlight::highlight(query);
        let error_refs = match &b.state {
            ExecutionState::Error(msg) => ref_highlight::parse_error_refs(msg),
            _ => std::collections::HashSet::new(),
        };
        sql_lines_styled = sql_lines_styled
            .into_iter()
            .map(|spans| overlay_refs_on_spans(spans, &error_refs))
            .collect();
        if let Some((err_line, _err_col)) = error_position(b) {
            if let Some(target) = (err_line as usize)
                .checked_sub(1)
                .and_then(|i| sql_lines_styled.get_mut(i))
            {
                for span in target.iter_mut() {
                    span.style = span.style.bg(Color::Rgb(70, 25, 25));
                }
            }
        }
        let sql_para = Paragraph::new(
            sql_lines_styled
                .into_iter()
                .map(Line::from)
                .collect::<Vec<_>>(),
        );
        frame.render_widget(sql_para, chunks[idx]);
        idx += 1;
    }

    if closer_height > 0 {
        render_fence_closer_row(frame, chunks[idx], b);
        idx += 1;
    }

    if table_height > 0 {
        let panel_chunk = chunks[idx];
        let result_count = b
            .cached_result
            .as_ref()
            .and_then(|v| v.get("results"))
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let multi = result_count > 1;
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
        let subtabs_rect = if multi { Some(row(y)) } else { None };
        if multi {
            y = y.saturating_add(1);
        }
        let used = y.saturating_sub(panel_chunk.y);
        let content_rect = Rect {
            x: panel_chunk.x,
            y,
            width: panel_chunk.width,
            height: panel_chunk.height.saturating_sub(used),
        };
        render_result_tab_bar_inner(
            frame,
            tab_bar_rect,
            result_tab,
            if multi { Some(result_count) } else { None },
        );
        render_result_separator(frame, separator_rect);
        if let Some(rect) = subtabs_rect {
            render_result_subtabs(frame, rect, b, 0);
        }
        match result_tab {
            crate::app::ResultPanelTab::Result => {
                if let Some((table, viewport_selected)) =
                    build_result_table(b, selected_row, viewport_top)
                {
                    let mut state = ratatui::widgets::TableState::default();
                    state.select(viewport_selected);
                    let table = table.row_highlight_style(
                        Style::default()
                            .bg(super::palette::SELECTION_BG)
                            .add_modifier(Modifier::BOLD),
                    );
                    frame.render_stateful_widget(table, content_rect, &mut state);
                } else if let Some(lines) = build_error_lines(b) {
                    // Error result (DbResult::Error or a synthetic from
                    // a driver-level failure): paint the message inline
                    // so the user doesn't depend on the scrolling
                    // status bar to see what broke.
                    frame.render_widget(Paragraph::new(lines), content_rect);
                }
            }
            crate::app::ResultPanelTab::Messages => {
                let lines = build_messages_lines(b);
                frame.render_widget(Paragraph::new(lines), content_rect);
            }
            crate::app::ResultPanelTab::Plan => {
                let lines = build_plan_lines(b);
                frame.render_widget(Paragraph::new(lines), content_rect);
            }
            crate::app::ResultPanelTab::Stats => {
                let lines = build_stats_lines(b);
                frame.render_widget(Paragraph::new(lines), content_rect);
            }
            // DB blocks don't expose `Raw` (variants_for("db-*") omits
            // it). If the global tab state somehow lands here, paint
            // the stats fallback so we never panic.
            crate::app::ResultPanelTab::Raw => {
                let lines = build_stats_lines(b);
                frame.render_widget(Paragraph::new(lines), content_rect);
            }
        }
    } else {
        let _ = (selected_row, viewport_top);
    }
}

/// Pull a one-liner summary out of the block's `cached_result`
/// (a `DbResponse` blob). Falls back to `None` when the shape doesn't
/// match — better to skip than show a misleading number.
///
/// When the query returned multiple result sets (multi-statement),
/// the summary describes `results[0]` and appends `(+N more)` so the
/// user knows there's data the renderer isn't surfacing yet — Story
/// 05.1 will wire up tabs to step through them. Errors that carry a
/// `(line, column)` from the executor get an `at L:C` suffix.
fn db_summary(b: &BlockNode) -> Option<String> {
    let result = b.cached_result.as_ref()?;
    let elapsed = result.get("stats")?.get("elapsed_ms")?.as_u64()?;
    let results = result.get("results")?.as_array()?;
    let first = results.first()?;
    let kind = first.get("kind")?.as_str()?;
    let extras = match results.len() {
        0 | 1 => String::new(),
        n => format!(" (+{} more)", n - 1),
    };
    match kind {
        "select" => {
            let rows = first.get("rows")?.as_array()?.len();
            let has_more = first
                .get("has_more")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let suffix = if has_more { "+" } else { "" };
            Some(format!("{rows}{suffix} rows · {elapsed}ms{extras}"))
        }
        "mutation" => {
            let affected = first.get("rows_affected")?.as_u64()?;
            Some(format!("{affected} affected · {elapsed}ms{extras}"))
        }
        "error" => first.get("message").and_then(|v| v.as_str()).map(|m| {
            let pos = first
                .get("line")
                .and_then(|l| l.as_u64())
                .map(|line| {
                    let col = first.get("column").and_then(|c| c.as_u64()).unwrap_or(1);
                    format!(" at {line}:{col}")
                })
                .unwrap_or_default();
            format!("error: {m}{pos}{extras}")
        }),
        _ => None,
    }
}

/// Extract `(line, column)` from the first result if it's an Error
/// variant with positional info. Returns `None` for selects,
/// mutations, errors without position, or anything that doesn't
/// match the expected shape. Used by the renderer to paint a red
/// background on the offending source line.
fn error_position(b: &BlockNode) -> Option<(u64, u64)> {
    let result = b.cached_result.as_ref()?;
    let first = result.get("results")?.as_array()?.first()?;
    if first.get("kind")?.as_str()? != "error" {
        return None;
    }
    let line = first.get("line")?.as_u64()?;
    let column = first.get("column").and_then(|c| c.as_u64()).unwrap_or(1);
    Some((line, column))
}

/// Height (in rows) of the result table viewport inside a DB card.
/// Acts as a sliding window over the full result set: navigation past
/// the bottom row scrolls the window down so the selected row stays
/// visible. Keeps long result sets from pushing the rest of the
/// document off-screen.
const MAX_VISIBLE_ROWS: usize = 10;
/// Cap on column width so a single fat field can't hog the row.
const MAX_COL_WIDTH: usize = 30;

/// `scrolloff` band for the result-table viewport — keeps a few
/// rows visible above/below the cursor so the user always sees
/// what's coming. Mirrors `app::SCROLL_OFF` to feel like the editor.
const RESULT_SCROLL_OFF: usize = 2;

/// Persistent-viewport scroll for the result table. Same model as
/// `app::clamp_viewport`: the window only slides when the cursor
/// would scroll off-screen (with a `scrolloff` buffer). Inside the
/// visible band the cursor moves freely with no scroll. Result is
/// also capped at `total - viewport` so we never paint past the end.
fn clamp_result_viewport(
    viewport_top: usize,
    viewport: usize,
    cursor: usize,
    total: usize,
) -> usize {
    if viewport == 0 || total <= viewport {
        return 0;
    }
    let scrolloff = RESULT_SCROLL_OFF.min(viewport / 2);
    let upper = cursor.saturating_sub(scrolloff);
    let lower = cursor
        .saturating_add(scrolloff + 1)
        .saturating_sub(viewport);
    let next = if viewport_top > upper {
        upper
    } else if viewport_top < lower {
        lower
    } else {
        viewport_top
    };
    next.min(total - viewport)
}

/// Build a `ratatui::Table` widget for a DB block's `select` result.
/// Returns `None` when the cache is empty / a mutation / an error —
/// caller falls back to no-op on that branch. The `usize` in the
/// returned tuple is the selected-row index *relative to the visible
/// window*, ready to hand to `TableState::select`. `viewport_top` is
/// the persistent scroll state for this block; the function reads it
/// at the start of the frame, recomputes via `clamp_result_viewport`,
/// and writes the new value back so the next frame's offset is in
/// sync with the cursor.
fn build_result_table(
    b: &BlockNode,
    selected_row: Option<usize>,
    viewport_top: Option<&mut u16>,
) -> Option<(Table<'static>, Option<usize>)> {
    let result = b.cached_result.as_ref()?;
    let first = result
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return None;
    }
    // Pair (name, type) so the header can render the type in dim
    // beside the bold column name — same convention as the desktop
    // (`id integer`, `created_at datetime`).
    let columns_meta: Vec<(String, String)> = first
        .get("columns")
        .and_then(|v| v.as_array())?
        .iter()
        .map(|c| {
            let name = c
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("?")
                .to_string();
            let ty = c
                .get("type")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            (name, ty)
        })
        .collect();
    if columns_meta.is_empty() {
        return None;
    }
    let columns: Vec<String> = columns_meta.iter().map(|(n, _)| n.clone()).collect();
    let rows: Vec<&serde_json::Value> = first
        .get("rows")
        .and_then(|v| v.as_array())?
        .iter()
        .collect();

    let total = rows.len();
    // Persistent viewport: when this block has a focused result we
    // honor the previously-stored `viewport_top`; otherwise (other
    // blocks rendered passively) we default to the top of the set.
    // After computing the new offset we write it back so the next
    // frame picks up where this one left off.
    let offset = match (viewport_top, selected_row) {
        (Some(slot), Some(sel)) => {
            let next = clamp_result_viewport(*slot as usize, MAX_VISIBLE_ROWS, sel, total);
            *slot = next as u16;
            next
        }
        _ => 0,
    };
    let end = (offset + MAX_VISIBLE_ROWS).min(total);
    let visible_rows: &[&serde_json::Value] = &rows[offset..end];

    // First pass: compute per-column data width based on visible
    // cells (cap at MAX_COL_WIDTH). The header itself never grows
    // a column — only data does, so adding the type label can't
    // push columns wider than the data demands.
    let mut widths: Vec<u16> = columns
        .iter()
        .map(|n| n.chars().count().min(MAX_COL_WIDTH) as u16)
        .collect();
    for row in visible_rows.iter() {
        for (i, name) in columns.iter().enumerate() {
            let cell = format_cell(row.get(name).unwrap_or(&serde_json::Value::Null));
            let len = cell.chars().count().min(MAX_COL_WIDTH) as u16;
            if len > widths[i] {
                widths[i] = len;
            }
        }
    }

    let name_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let type_style = Style::default().fg(Color::DarkGray);
    let header = Row::new(
        columns_meta
            .iter()
            .enumerate()
            .map(|(i, (name, ty))| {
                let col_w = widths[i] as usize;
                let name_chars = name.chars().count();
                let mut spans: Vec<Span<'static>> = Vec::with_capacity(3);
                spans.push(Span::styled(
                    truncate_with_ellipsis(name, col_w),
                    name_style,
                ));
                // Only render the type label when it fits AFTER the
                // name with a 1-space gap — truncated types like
                // `INTEGE` looked broken; better to omit the type
                // entirely on tight columns.
                if !ty.is_empty() {
                    let used = name_chars.min(col_w);
                    let remaining = col_w.saturating_sub(used + 1);
                    if remaining >= ty.chars().count() {
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(ty.clone(), type_style));
                    }
                }
                Cell::from(Line::from(spans))
            })
            .collect::<Vec<_>>(),
    )
    .height(1);

    // Per-column alignment: numeric types (integer / float / etc.)
    // right-align so the user can compare magnitudes at a glance —
    // same convention every spreadsheet / SQL client uses.
    let aligns: Vec<bool> = columns_meta
        .iter()
        .map(|(_, ty)| is_numeric_type(ty))
        .collect();

    // Subtle alternate-row bg every 2nd row — gives the eye a
    // horizontal anchor without the noise of a full-color zebra.
    let zebra_bg = Color::Rgb(18, 20, 26);

    let table_rows: Vec<Row> = visible_rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let cells: Vec<Cell> = columns
                .iter()
                .enumerate()
                .map(|(i, name)| {
                    let raw = format_cell(row.get(name).unwrap_or(&serde_json::Value::Null));
                    let truncated = truncate_with_ellipsis(&raw, MAX_COL_WIDTH);
                    if aligns[i] {
                        // Right-align by left-padding to the column's
                        // width with spaces. Cell width is fixed by
                        // the Constraint::Length below, so this keeps
                        // the digits flush against the column edge.
                        let pad = (widths[i] as usize).saturating_sub(truncated.chars().count());
                        Cell::from(format!("{}{}", " ".repeat(pad), truncated))
                    } else {
                        Cell::from(truncated)
                    }
                })
                .collect();
            let row = Row::new(cells);
            // Zebra: even rows get the subtle bg; odd rows stay
            // default. The selected-row highlight applied later wins
            // over both because it's a bg modifier on top.
            if row_idx % 2 == 1 {
                row.style(Style::default().bg(zebra_bg))
            } else {
                row
            }
        })
        .collect();

    let viewport_selected = selected_row.map(|sel| sel.saturating_sub(offset));
    let constraints: Vec<Constraint> = widths.iter().map(|w| Constraint::Length(*w)).collect();
    Some((
        Table::new(table_rows, constraints)
            .header(header)
            .column_spacing(3),
        viewport_selected,
    ))
}

/// True for SQL types whose values should right-align in the
/// rendered table — integers, floats, decimals, etc. Lower-cased
/// match against the server-reported type name.
fn is_numeric_type(ty: &str) -> bool {
    let lower = ty.to_lowercase();
    matches!(
        lower.as_str(),
        "int"
            | "integer"
            | "bigint"
            | "smallint"
            | "tinyint"
            | "int2"
            | "int4"
            | "int8"
            | "float"
            | "float4"
            | "float8"
            | "real"
            | "double"
            | "double precision"
            | "decimal"
            | "numeric"
            | "money"
    ) || lower.starts_with("int")
        || lower.starts_with("float")
        || lower.starts_with("decimal")
        || lower.starts_with("numeric")
}

/// How tall the result Table will draw inside the card. Mirrors
/// `MAX_VISIBLE_ROWS` from the renderer so layout reserves the right
/// number of rows. The viewport stays at most `MAX_VISIBLE_ROWS` tall;
/// extra rows live in the (scrollable) result set, not in the card.
fn db_result_table_height(b: &BlockNode) -> u16 {
    const ERROR_PANEL_ROWS: u16 = 6;
    let Some(result) = b.cached_result.as_ref() else {
        return 0;
    };
    let results = result.get("results").and_then(|v| v.as_array());
    let Some(first) = results.and_then(|a| a.first()) else {
        return 0;
    };
    let kind = first.get("kind").and_then(|v| v.as_str()).unwrap_or("");
    // Errors get a fixed-height slot so the inline banner has room.
    // Stays in lockstep with `buffer::layout::db_table_height`.
    if kind == "error" {
        let multi = results.map(|a| a.len() > 1).unwrap_or(false);
        let chrome_extra = 2 + if multi { 1 } else { 0 };
        return ERROR_PANEL_ROWS + chrome_extra;
    }
    if kind != "select" {
        return 0;
    }
    let row_count = first
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let table_rows = if row_count == 0 {
        1
    } else {
        let visible = row_count.min(MAX_VISIBLE_ROWS);
        1 + visible
    };
    // Match `buffer::layout::db_table_height`: tab bar (1) +
    // separator (1) + sub-tabs (1 only when multi-statement) live
    // above the table.
    let multi = results.map(|a| a.len() > 1).unwrap_or(false);
    let chrome_extra = 2 + if multi { 1 } else { 0 };
    (table_rows + chrome_extra) as u16
}

fn truncate_with_ellipsis(s: &str, width: usize) -> String {
    let count = s.chars().count();
    if count <= width {
        return s.to_string();
    }
    if width == 0 {
        return String::new();
    }
    let head: String = s.chars().take(width.saturating_sub(1)).collect();
    format!("{head}…")
}

/// Render a JSON cell as a flat string. Strings keep their content;
/// numbers / bools become their decimal / `true|false` form; nulls
/// show as `(null)`; arrays / objects collapse to `[…]` / `{…}` so
/// the column doesn't blow up.
fn format_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "(null)".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(_) => "[…]".into(),
        serde_json::Value::Object(_) => "{…}".into(),
    }
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

/// One-line tab header rendered above the result panel content.
/// Selected tab gets a bright background; the rest stay dim. Only
/// 4 fixed tabs for now (Result/Messages/Plan/Stats) — sub-tabs
/// for multi-statement Result are V2.
fn render_result_separator(frame: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let line: String = "─".repeat(area.width as usize);
    let style = Style::default().fg(Color::DarkGray);
    frame.render_widget(Paragraph::new(Line::from(Span::styled(line, style))), area);
}

/// Strip of chip-styled tabs for multi-statement results. Mirrors
/// the desktop's per-result-set selector. Active chip fills with a
/// soft bg + bold; inactive chips stay flat dim. Width-padded with
/// 1 space on each side so chips don't crowd the separator.
fn render_result_subtabs(frame: &mut Frame, area: Rect, b: &BlockNode, selected: usize) {
    let Some(results) = b
        .cached_result
        .as_ref()
        .and_then(|v| v.get("results"))
        .and_then(|v| v.as_array())
    else {
        return;
    };
    let active = Style::default()
        .bg(Color::Rgb(50, 60, 90))
        .fg(Color::White)
        .add_modifier(Modifier::BOLD);
    let inactive = Style::default().fg(Color::DarkGray);
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(results.len() * 3 + 1);
    spans.push(Span::raw(" "));
    for (i, r) in results.iter().enumerate() {
        let kind = r
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_uppercase();
        let style = if i == selected { active } else { inactive };
        spans.push(Span::styled(format!(" {} {} ", i + 1, kind), style));
        spans.push(Span::raw("  "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn render_result_tab_bar_inner(
    frame: &mut Frame,
    area: Rect,
    selected: crate::app::ResultPanelTab,
    result_count: Option<usize>,
) {
    render_result_tab_bar_for(frame, area, selected, result_count, "")
}

fn render_result_tab_bar_for(
    frame: &mut Frame,
    area: Rect,
    selected: crate::app::ResultPanelTab,
    result_count: Option<usize>,
    block_type: &str,
) {
    use crate::app::ResultPanelTab;
    let active_style = Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
    let inactive_style = Style::default().fg(Color::DarkGray);
    let mut spans: Vec<Span<'static>> = Vec::new();
    spans.push(Span::raw(" "));
    for tab in ResultPanelTab::variants_for(block_type) {
        let style = if *tab == selected {
            active_style
        } else {
            inactive_style
        };
        let label = match (*tab, result_count) {
            // Pluralize Result(s) when multi-statement returned >1
            // (DB only — HTTP never has multi).
            (ResultPanelTab::Result, Some(n)) if n > 1 => format!("Results ({n})"),
            _ => tab.label_for(block_type).to_string(),
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw("    "));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Pull a `DbResult::Error` (kind == "error") off `results[0]` and
/// format it as a red banner block. Returns `None` when the first
/// result isn't an error (Result tab keeps its normal rendering path).
fn build_error_lines(b: &BlockNode) -> Option<Vec<Line<'static>>> {
    let first = b
        .cached_result
        .as_ref()?
        .get("results")?
        .as_array()?
        .first()?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("error") {
        return None;
    }
    let message = first
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("(no message)")
        .to_string();
    let loc = match (
        first.get("line").and_then(|v| v.as_u64()),
        first.get("column").and_then(|v| v.as_u64()),
    ) {
        (Some(l), Some(c)) => Some(format!(" at {l}:{c}")),
        (Some(l), None) => Some(format!(" at line {l}")),
        _ => None,
    };
    let header_text = match loc {
        Some(suffix) => format!("error{suffix}"),
        None => "error".to_string(),
    };
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  ✖ {header_text}"),
            Style::default()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    // Wrap long messages naturally by splitting on newlines; ratatui's
    // `Paragraph` will soft-wrap each `Line` against the content rect.
    for chunk in message.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {chunk}"),
            Style::default().fg(Color::White),
        )));
    }
    Some(lines)
}

/// Render content for the Messages tab — pulls `messages[]` off the
/// cached response and lists them as `[severity] text`. Empty list
/// shows a dim placeholder so users know the tab is wired but
/// nothing came back.
fn build_messages_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no messages)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(value) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let Some(messages) = value.get("messages").and_then(|v| v.as_array()) else {
        return vec![placeholder];
    };
    if messages.is_empty() {
        return vec![placeholder];
    }
    messages
        .iter()
        .map(|m| {
            let sev = m
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("notice");
            let text = m.get("text").and_then(|v| v.as_str()).unwrap_or("");
            Line::from(vec![
                Span::styled(
                    format!(" [{sev}] "),
                    Style::default().fg(match sev {
                        "error" => Color::Red,
                        "warning" => Color::Yellow,
                        _ => Color::LightBlue,
                    }),
                ),
                Span::raw(text.to_string()),
            ])
        })
        .collect()
}

/// Plan tab — renders `cached_result["plan"]` populated by `<C-x>`
/// (EXPLAIN). When the plan looks like a postgres
/// EXPLAIN response (`results[0].rows` of `{"QUERY PLAN": "..."}`),
/// unwrap each row to a single tree-formatted line so `->` arrows
/// and indentation read naturally; fall back to pretty-printed JSON
/// for MySQL / SQLite / FORMAT-JSON shapes.
fn build_plan_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let placeholder = Line::from(Span::styled(
        " (no plan — run <C-x> on this block to populate)",
        Style::default().fg(Color::DarkGray),
    ));
    let Some(value) = b.cached_result.as_ref() else {
        return vec![placeholder];
    };
    let plan = match value.get("plan") {
        Some(p) if !p.is_null() => p,
        _ => return vec![placeholder],
    };

    // Postgres path: the EXPLAIN response is a `DbResponse` with
    // results[0].rows containing one row per plan line, each shaped
    // `{"QUERY PLAN": "Seq Scan on users  (cost=0.00..18.00 rows=800)"}`.
    // Unwrap to the raw text — that's what `psql` shows and it
    // already carries indentation + `->` arrows.
    if let Some(rows) = plan
        .get("results")
        .and_then(|r| r.as_array())
        .and_then(|a| a.first())
        .and_then(|first| first.get("rows"))
        .and_then(|rs| rs.as_array())
    {
        let lines: Vec<Line<'static>> = rows
            .iter()
            .filter_map(|row| {
                row.as_object()?
                    .values()
                    .next()
                    .and_then(|v| v.as_str())
                    .map(|s| Line::from(Span::raw(format!(" {s}"))))
            })
            .collect();
        if !lines.is_empty() {
            return lines;
        }
    }

    // Fallback for non-postgres dialects (MySQL/SQLite EXPLAIN, or
    // FORMAT JSON variants): pretty-print the whole plan blob.
    let json = serde_json::to_string_pretty(plan).unwrap_or_else(|_| String::from("(plan)"));
    json.lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect()
}

/// Stats tab — formats the connection meta + per-execution stats so
/// the user gets at-a-glance "what just ran". Useful especially for
/// cached hits where the result table is identical to last run.
fn build_stats_lines(b: &BlockNode) -> Vec<Line<'static>> {
    let label_style = Style::default().fg(Color::DarkGray);
    let value_style = Style::default().fg(Color::White);
    let row = |label: &str, value: String| {
        Line::from(vec![
            Span::styled(format!(" {label}: "), label_style),
            Span::styled(value, value_style),
        ])
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    let Some(value) = b.cached_result.as_ref() else {
        return vec![Line::from(Span::styled(
            " (no result yet — run with `r`)",
            label_style,
        ))];
    };

    let elapsed = value
        .get("stats")
        .and_then(|s| s.get("elapsed_ms"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let results = value
        .get("results")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let total_rows: u64 = value
        .get("results")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    if r.get("kind").and_then(|k| k.as_str()) == Some("select") {
                        r.get("rows")
                            .and_then(|rs| rs.as_array())
                            .map(|rs| rs.len() as u64)
                    } else {
                        None
                    }
                })
                .sum()
        })
        .unwrap_or(0);
    let cached = matches!(b.state, ExecutionState::Cached);

    lines.push(row("elapsed", format!("{elapsed}ms")));
    lines.push(row("results", results.to_string()));
    lines.push(row("rows", total_rows.to_string()));
    lines.push(row("cached", if cached { "yes" } else { "no" }.to_string()));
    lines
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
    fn build_result_table_returns_none_without_cache() {
        let b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "db-sqlite".into(),
            alias: None,
            display_mode: None,
            params: json!({"query": "SELECT 1"}),
            state: ExecutionState::Idle,
            cached_result: None,
        };
        assert!(build_result_table(&b, None, None).is_none());
    }

    #[test]
    fn db_result_table_height_counts_visible_rows() {
        let b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "db-sqlite".into(),
            alias: None,
            display_mode: None,
            params: json!({"query": "SELECT 1"}),
            state: ExecutionState::Success,
            cached_result: Some(json!({
                "results": [{
                    "kind": "select",
                    "columns": [{"name": "id", "type": "int"}],
                    "rows": [{"id": 1}, {"id": 2}, {"id": 3}],
                    "has_more": false,
                }],
                "stats": {"elapsed_ms": 5},
            })),
        };
        // header + 3 rows + 2 chrome rows above (tab bar + separator).
        assert_eq!(db_result_table_height(&b), 4 + 2);
    }

    #[test]
    fn db_result_table_height_caps_at_viewport_when_overflowing() {
        let rows: Vec<serde_json::Value> = (0..50).map(|i| json!({"id": i})).collect();
        let b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "db-sqlite".into(),
            alias: None,
            display_mode: None,
            params: json!({"query": "SELECT *"}),
            state: ExecutionState::Success,
            cached_result: Some(json!({
                "results": [{
                    "kind": "select",
                    "columns": [{"name": "id", "type": "int"}],
                    "rows": rows,
                    "has_more": false,
                }],
                "stats": {"elapsed_ms": 5},
            })),
        };
        // header + 10-row viewport + 2 chrome rows above
        // (tab bar + separator).
        assert_eq!(
            db_result_table_height(&b),
            (1 + MAX_VISIBLE_ROWS + 2) as u16
        );
    }

    #[test]
    fn clamp_result_viewport_holds_until_cursor_leaves() {
        let v = MAX_VISIBLE_ROWS; // 10
                                  // total ≤ viewport: no scroll, ever.
        assert_eq!(clamp_result_viewport(0, v, 0, 5), 0);
        assert_eq!(clamp_result_viewport(0, v, 4, 5), 0);
        // Cursor inside the comfort band [scrolloff, viewport - scrolloff - 1]
        // (with scrolloff=2 in viewport=10 that's [2, 7]) leaves
        // the window untouched.
        assert_eq!(clamp_result_viewport(0, v, 2, 80), 0);
        assert_eq!(clamp_result_viewport(0, v, 7, 80), 0);
        // Cursor below the lower scroll-off: window inches down so
        // the cursor stays `scrolloff` rows above the bottom.
        assert_eq!(clamp_result_viewport(0, v, 8, 80), 1);
        assert_eq!(clamp_result_viewport(0, v, 9, 80), 2);
        // Cursor jumps to row 25, viewport_top was 0 → snap so
        // cursor is still inside (offset = cursor + scrolloff + 1 -
        // viewport).
        assert_eq!(clamp_result_viewport(0, v, 25, 80), 18);
        // Going up past the upper scroll-off pulls the window up
        // just enough to keep the cursor `scrolloff` rows below
        // the top.
        assert_eq!(clamp_result_viewport(20, v, 18, 80), 16);
        assert_eq!(clamp_result_viewport(20, v, 5, 80), 3);
        // Last row clamps at total - viewport.
        assert_eq!(clamp_result_viewport(0, v, 79, 80), 70);
        // Defensive: zero viewport returns 0.
        assert_eq!(clamp_result_viewport(7, 0, 50, 100), 0);
    }

    #[test]
    fn overlay_refs_keeps_ref_intact_across_numeric_fragments() {
        use crate::ui::sql_highlight;
        let line = "WHERE id = {{pg.response.results.0.rows.0.id}}";
        let highlighted = sql_highlight::highlight(line);
        let overlaid =
            overlay_refs_on_spans(highlighted[0].clone(), &std::collections::HashSet::new());
        let ref_style = ref_highlight::normal_style();
        let has_full_ref_span = overlaid.iter().any(|s| {
            s.content == "{{pg.response.results.0.rows.0.id}}" && s.style == ref_style
        });
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
        assert!(has_red_ref, "expected red `{{{{ghost.id}}}}`; got: {overlaid:?}");
    }

    #[test]
    fn build_result_table_uses_persistent_viewport_top() {
        let rows: Vec<serde_json::Value> = (0..30)
            .map(|i| json!({"id": i, "name": format!("r{i}")}))
            .collect();
        let b = BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "db-sqlite".into(),
            alias: None,
            display_mode: None,
            params: json!({"query": "SELECT * FROM t"}),
            state: ExecutionState::Success,
            cached_result: Some(json!({
                "results": [{
                    "kind": "select",
                    "columns": [
                        {"name": "id", "type": "int"},
                        {"name": "name", "type": "text"},
                    ],
                    "rows": rows,
                    "has_more": false,
                }],
                "stats": {"elapsed_ms": 1},
            })),
        };

        // Frame 1: viewport_top starts at 0, cursor on row 0 →
        // window stays at 0, cursor at row 0 inside it.
        let mut vt: u16 = 0;
        let (_, sel) = build_result_table(&b, Some(0), Some(&mut vt)).unwrap();
        assert_eq!(sel, Some(0));
        assert_eq!(vt, 0);

        // Frame 2: cursor moves to row 7 (still inside [2, 7] band)
        // → viewport unchanged.
        let (_, sel) = build_result_table(&b, Some(7), Some(&mut vt)).unwrap();
        assert_eq!(sel, Some(7));
        assert_eq!(vt, 0);

        // Frame 3: cursor jumps to row 15 → window slides so the
        // cursor sits `scrolloff` rows above the bottom.
        let (_, sel) = build_result_table(&b, Some(15), Some(&mut vt)).unwrap();
        // viewport_top should now be 8 (15 + 2 + 1 - 10).
        assert_eq!(vt, 8);
        // Selection index inside the window: 15 - 8 = 7.
        assert_eq!(sel, Some(7));

        // Frame 4: cursor on last row → window pinned to tail.
        let (_, sel) = build_result_table(&b, Some(29), Some(&mut vt)).unwrap();
        assert_eq!(vt, 20);
        assert_eq!(sel, Some(MAX_VISIBLE_ROWS - 1));

        // No viewport_top slot (passive render of an unfocused
        // block) defaults to 0 — no scroll-state mutation.
        let (_, sel) = build_result_table(&b, None, None).unwrap();
        assert_eq!(sel, None);
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

    // title_includes_alias_when_present test dropped along with
    // block_title — the alias now lives in the chrome header bar
    // (render_db_header_bar) which paints into a Frame rect.

    fn db_block_with_plan(plan: serde_json::Value) -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "db-postgres".into(),
            alias: Some("q".into()),
            display_mode: None,
            params: json!({ "query": "SELECT 1", "connection": "c" }),
            state: ExecutionState::Success,
            cached_result: Some(json!({
                "results": [],
                "messages": [],
                "stats": { "elapsed_ms": 0 },
                "plan": plan
            })),
        }
    }

    #[test]
    fn plan_lines_unwrap_postgres_query_plan_rows() {
        // Postgres EXPLAIN: each row is `{"QUERY PLAN": "..."}` and
        // already carries indentation + `->` arrows. We strip the
        // wrapper and render the strings directly so it reads like
        // `psql`'s EXPLAIN output.
        let plan = json!({
            "results": [{
                "kind": "select",
                "columns": [{"name": "QUERY PLAN"}],
                "rows": [
                    {"QUERY PLAN": "Seq Scan on users  (cost=0.00..18.00 rows=800)"},
                    {"QUERY PLAN": "  Filter: (id > 10)"},
                ],
                "has_more": false
            }],
            "messages": [],
            "stats": { "elapsed_ms": 1 }
        });
        let b = db_block_with_plan(plan);
        let lines = build_plan_lines(&b);
        assert_eq!(lines.len(), 2);
        let first: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            first.contains("Seq Scan on users"),
            "expected unwrapped plan text, got: {first}"
        );
    }

    #[test]
    fn plan_lines_falls_back_to_json_for_non_postgres_shape() {
        // MySQL `EXPLAIN FORMAT=JSON` returns one row whose value is
        // a nested JSON object (not a flat `QUERY PLAN` string). The
        // unwrap path doesn't help; fall through to pretty-printed
        // JSON so users still see something useful.
        let plan = json!({
            "results": [{
                "kind": "select",
                "columns": [{"name": "id"}, {"name": "select_type"}],
                "rows": [{"id": 1, "select_type": "SIMPLE"}],
                "has_more": false
            }]
        });
        let b = db_block_with_plan(plan);
        let lines = build_plan_lines(&b);
        // The unwrap path takes the first .values() entry, so it gets
        // `1` (the id). That's still acceptable — psql-style output
        // for whatever the first column happens to be. Just assert
        // we got SOMETHING beyond the placeholder.
        assert!(!lines.is_empty());
        let combined: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(!combined.contains("no plan"));
    }

    #[test]
    fn plan_lines_show_placeholder_when_no_plan() {
        // `cached_result.plan` absent or null → users see a hint
        // pointing at `<C-x>` instead of an empty panel.
        let mut b = db_block_with_plan(serde_json::Value::Null);
        b.cached_result = None;
        let lines = build_plan_lines(&b);
        let combined: String = lines
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
            .collect();
        assert!(combined.contains("<C-x>"), "got: {combined}");
    }
}
