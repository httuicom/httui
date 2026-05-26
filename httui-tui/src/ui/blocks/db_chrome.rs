//! Header / footer chrome bars for block cards. Each row is one cell
//! tall and renders a subtle dark bg so the chrome visually separates
//! from the block body.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::buffer::block::{BlockNode, ExecutionState};

use super::db_table::db_summary;
use super::http_panel;
use super::ConnectionNames;

/// Subtle bg used by the header / footer chrome bars.
pub(super) fn chrome_bg() -> Color {
    Color::Rgb(20, 22, 28)
}

/// Header bar dispatcher — paints kind-specific content on a chrome
/// bg row. DB blocks get `[DB] alias · vault [RW] subtype`; HTTP
/// gets `[HTTP] alias · METHOD host`; E2E and unknown kinds get a
/// minimal `[BLK] alias`. Right-aligned keymap hint applies to all.
pub(super) fn render_db_header_bar(
    frame: &mut Frame,
    area: Rect,
    b: &BlockNode,
    names: &ConnectionNames,
) {
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
/// see the run status at a glance.
fn state_dot(state: &ExecutionState, bg: Style) -> Span<'static> {
    use ExecutionState as ES;
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

/// Footer bar dispatcher — kind-specific summary on a chrome-bg row.
pub(super) fn render_db_footer_bar(
    frame: &mut Frame,
    area: Rect,
    b: &BlockNode,
    names: &ConnectionNames,
) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::block::BlockId;
    use serde_json::json;
    use std::collections::HashMap;

    fn db_block(alias: &str, conn: &str) -> BlockNode {
        BlockNode {
            id: BlockId(0),
            raw: ropey::Rope::new(),
            block_type: "db-postgres".into(),
            alias: Some(alias.into()),
            display_mode: None,
            params: json!({"connection": conn, "query": "SELECT 1"}),
            state: ExecutionState::Idle,
            cached_result: None,
        }
    }

    fn span_text(spans: &[Span<'static>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn chrome_bg_returns_rgb_color() {
        assert_eq!(chrome_bg(), Color::Rgb(20, 22, 28));
    }

    #[test]
    fn state_dot_picks_distinct_colors_per_state() {
        let bg = Style::default();
        let dots = [
            state_dot(&ExecutionState::Idle, bg),
            state_dot(&ExecutionState::Cached, bg),
            state_dot(&ExecutionState::Running, bg),
            state_dot(&ExecutionState::Success, bg),
            state_dot(&ExecutionState::Error("e".into()), bg),
        ];
        let colors: Vec<_> = dots.iter().map(|s| s.style.fg).collect();
        // All distinct.
        assert_eq!(
            colors
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            5
        );
    }

    #[test]
    fn db_header_spans_include_alias_and_vault_label() {
        let b = db_block("q", "prod");
        let mut names: ConnectionNames = HashMap::new();
        names.insert("prod".into(), "PROD-DB".into());
        let spans = db_header_left_spans(&b, &names, Style::default());
        let text = span_text(&spans);
        assert!(text.contains("DB"));
        assert!(text.contains("q"));
        assert!(text.contains("PROD-DB"));
        assert!(text.contains("postgres"));
    }

    #[test]
    fn db_header_spans_dash_when_no_connection() {
        let mut b = db_block("q", "");
        b.params = json!({"query": "SELECT 1"});
        let names: ConnectionNames = HashMap::new();
        let spans = db_header_left_spans(&b, &names, Style::default());
        let text = span_text(&spans);
        assert!(text.contains("—"));
    }

    #[test]
    fn generic_header_spans_emit_label_and_alias() {
        let b = db_block("note", "p");
        let spans = generic_header_left_spans(&b, "E2E", Color::Green, Style::default());
        let text = span_text(&spans);
        assert!(text.contains("E2E"));
        assert!(text.contains("note"));
    }

    #[test]
    fn db_footer_spans_carry_status_dot_and_run_hint() {
        let b = db_block("q", "prod");
        let names: ConnectionNames = HashMap::new();
        let (left, right) =
            db_footer_spans(&b, &names, Style::default(), Color::Green, "connected");
        assert!(span_text(&left).contains("connected"));
        assert!(span_text(&right).contains("press"));
    }

    #[test]
    fn db_footer_spans_surface_cached_when_state_is_cached() {
        let mut b = db_block("q", "prod");
        b.state = ExecutionState::Cached;
        b.cached_result = Some(json!({
            "stats": {"elapsed_ms": 5},
            "results": [{"kind": "select", "rows": [], "has_more": false}]
        }));
        let names: ConnectionNames = HashMap::new();
        let (_l, right) = db_footer_spans(&b, &names, Style::default(), Color::Green, "connected");
        assert!(span_text(&right).contains("cached"));
    }
}
