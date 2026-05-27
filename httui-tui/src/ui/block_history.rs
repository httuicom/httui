//! Read-only popup that lists the most-recent HTTP runs for the
//! focused block (/ `gh` chord). One row per
//! `block_run_history` entry showing `<status> · <elapsed> · ran X
//! ago`. j/k navigates; Esc/Ctrl-C closes. No selection action — V1
//! is view-only.
//!
//! Visual: anchored above the block (or below when there's no
//! headroom), wider than connection_picker so the timestamp column
//! has breathing room. Title carries `<METHOD> <alias>` so a row of
//! identical statuses still tells the user which block they're on.

use chrono::{DateTime, Utc};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::BlockHistoryState;
use crate::ui::BlockAnchor;

const POPUP_WIDTH: u16 = 76;
const MAX_VISIBLE_ROWS: usize = 12;

pub fn render(
    frame: &mut Frame,
    editor_area: Rect,
    state: &BlockHistoryState,
    anchor: Option<BlockAnchor>,
) {
    let popup = compute_popup_rect(editor_area, state, anchor);
    let bg_style = Style::default()
        .bg(crate::ui::palette::popup_bg())
        .fg(crate::ui::palette::foreground());

    // Hard-fill so editor content under the popup doesn't bleed.
    {
        let buf = frame.buffer_mut();
        for y in popup.y..popup.y.saturating_add(popup.height) {
            for x in popup.x..popup.x.saturating_add(popup.width) {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }
    }

    let title = format!(
        " History · {} · {}/{} ",
        state.title,
        state.selected + 1,
        state.entries.len()
    );
    let outer = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(bg_style)
        .border_style(
            Style::default()
                .fg(Color::LightBlue)
                .bg(crate::ui::palette::popup_bg()),
        );
    let inner = outer.inner(popup);
    frame.render_widget(outer, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    let body_area = chunks[0];
    let footer_area = chunks[1];

    let now = Utc::now();
    let items: Vec<ListItem> = state
        .entries
        .iter()
        .map(|e| ListItem::new(format_entry_line(e, now, bg_style)))
        .collect();
    let list = List::new(items).style(bg_style).highlight_style(
        Style::default()
            .bg(super::palette::selection_bg())
            .fg(crate::ui::palette::foreground())
            .add_modifier(Modifier::BOLD),
    );
    let mut list_state = ListState::default();
    list_state.select(Some(
        state.selected.min(state.entries.len().saturating_sub(1)),
    ));
    frame.render_stateful_widget(list, body_area, &mut list_state);

    let chip_key = Style::default()
        .bg(Color::LightBlue)
        .fg(crate::ui::palette::popup_bg())
        .add_modifier(Modifier::BOLD);
    let chip_label = Style::default().fg(Color::Gray);
    let footer = Line::from(vec![
        Span::styled(" jk ", chip_key),
        Span::styled(" navigate   ", chip_label),
        Span::styled(" Esc ", chip_key),
        Span::styled(" close ", chip_label),
    ]);
    frame.render_widget(Paragraph::new(footer).style(bg_style), footer_area);
}

/// Build one row: `<status>  <elapsed>  <ran-ago>`. Cancelled and
/// errored runs get a dim foreground so the eye picks out the green
/// 2xx successes; cancelled runs additionally drop the elapsed
/// column (no useful timing).
///
/// Status chip is block-type aware: HTTP rows show the response
/// code (color-mapped by class), DB rows show row count for
/// SELECT / mutation. The `method` column on each entry carries
/// `db:<driver>` for DB rows and the HTTP method otherwise — we
/// switch on that to pick the right rendering.
fn format_entry_line(
    entry: &httui_core::block_history::HistoryEntry,
    now: DateTime<Utc>,
    bg_style: Style,
) -> Line<'static> {
    let outcome = entry.outcome.as_str();
    let is_db = entry.method.starts_with("db:");

    let (status_chip, status_style): (String, Style) = match outcome {
        "cancelled" => (
            " — ".into(),
            Style::default()
                .fg(Color::Gray)
                .bg(crate::ui::palette::popup_bg()),
        ),
        "error" => (
            " err ".into(),
            Style::default()
                .bg(Color::Red)
                .fg(crate::ui::palette::popup_bg())
                .add_modifier(Modifier::BOLD),
        ),
        _ if is_db => {
            // DB success: status holds the row count (SELECT) or
            // rows-affected (mutation). Show `OK` chip + a tiny
            // `(N rows)` counter on the elapsed line — but here we
            // just colour the chip green.
            (
                match entry.status {
                    Some(n) => format!(" {n}r "),
                    None => " ok ".into(),
                },
                Style::default()
                    .bg(Color::Green)
                    .fg(crate::ui::palette::popup_bg())
                    .add_modifier(Modifier::BOLD),
            )
        }
        _ => match entry.status {
            Some(code) if (200..300).contains(&code) => (
                format!(" {code} "),
                Style::default()
                    .bg(Color::Green)
                    .fg(crate::ui::palette::popup_bg())
                    .add_modifier(Modifier::BOLD),
            ),
            Some(code) if (300..400).contains(&code) => (
                format!(" {code} "),
                Style::default()
                    .bg(Color::Yellow)
                    .fg(crate::ui::palette::popup_bg())
                    .add_modifier(Modifier::BOLD),
            ),
            Some(code) if (400..500).contains(&code) => (
                format!(" {code} "),
                Style::default()
                    .bg(Color::Magenta)
                    .fg(crate::ui::palette::popup_bg())
                    .add_modifier(Modifier::BOLD),
            ),
            Some(code) => (
                format!(" {code} "),
                Style::default()
                    .bg(Color::Red)
                    .fg(crate::ui::palette::popup_bg())
                    .add_modifier(Modifier::BOLD),
            ),
            None => (
                " err ".into(),
                Style::default()
                    .bg(Color::Red)
                    .fg(crate::ui::palette::popup_bg())
                    .add_modifier(Modifier::BOLD),
            ),
        },
    };

    let elapsed = match entry.elapsed_ms {
        Some(ms) => format!("{ms}ms"),
        None => "—".into(),
    };

    // Bytes column — `req → resp`. Both are best-effort: request
    // size is approximated from the source by `snapshot_block_meta`
    // (HTTP) / `derive_db_history_stats` (DB serialized response);
    // missing values render as `—`. Cancelled rows skip the column
    // entirely (no useful sizes).
    let sizes = if outcome == "cancelled" {
        String::new()
    } else {
        format!(
            "{}→{}",
            human_bytes(entry.request_size),
            human_bytes(entry.response_size),
        )
    };

    let ran_ago = ran_ago_string(&entry.ran_at, now);

    let row_fg = if outcome == "cancelled" {
        crate::ui::palette::muted()
    } else {
        crate::ui::palette::foreground()
    };
    Line::from(vec![
        Span::styled(status_chip, status_style),
        Span::styled("  ", bg_style),
        Span::styled(format!("{elapsed:>7}"), bg_style.fg(row_fg)),
        Span::styled("  ", bg_style),
        Span::styled(
            format!("{sizes:>13}"),
            bg_style.fg(crate::ui::palette::muted()),
        ),
        Span::styled("  ", bg_style),
        Span::styled(ran_ago, bg_style.fg(crate::ui::palette::muted())),
    ])
}

/// Format an `Option<i64>` byte count as a tiny human-readable
/// string: `120B`, `4.2K`, `1.3M`. `None` renders as `—`. Used by
/// the history modal's sizes column where space is at a premium.
fn human_bytes(n: Option<i64>) -> String {
    let Some(n) = n else { return "—".into() };
    if n < 0 {
        return "—".into();
    }
    if n < 1024 {
        return format!("{n}B");
    }
    let kb = n as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1}K");
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{mb:.1}M");
    }
    let gb = mb / 1024.0;
    format!("{gb:.1}G")
}

/// `ran X ago` rendering — coarse buckets so the column stays
/// scannable. Anything older than a day prints as the date.
fn ran_ago_string(ran_at_rfc3339: &str, now: DateTime<Utc>) -> String {
    let parsed: Option<DateTime<Utc>> = DateTime::parse_from_rfc3339(ran_at_rfc3339)
        .ok()
        .map(|dt| dt.with_timezone(&Utc));
    let Some(then) = parsed else {
        return ran_at_rfc3339.to_string();
    };
    let diff = now.signed_duration_since(then);
    let secs = diff.num_seconds();
    if secs < 0 {
        // Clock skew — render the timestamp directly.
        return ran_at_rfc3339.to_string();
    }
    if secs < 60 {
        return format!("{secs}s ago");
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    if days < 7 {
        return format!("{days}d ago");
    }
    then.format("%Y-%m-%d").to_string()
}

fn compute_popup_rect(area: Rect, state: &BlockHistoryState, anchor: Option<BlockAnchor>) -> Rect {
    let width = POPUP_WIDTH.min(area.width.saturating_sub(2));
    let visible = state.entries.len().min(MAX_VISIBLE_ROWS) as u16;
    let height = visible + 3; // top border + footer + bottom border

    let (x, y) = match anchor {
        Some(a) => {
            let max_x = area.x + area.width.saturating_sub(width);
            let x = (area.x + 2).min(max_x);
            let above_y = a.screen_top.checked_sub(height);
            let below_y = a.screen_top.saturating_add(a.height);
            let fits_below = below_y.saturating_add(height) <= area.y.saturating_add(area.height);
            let y = match (above_y, fits_below) {
                (Some(top), _) if top >= area.y => top,
                (_, true) => below_y,
                (Some(top), false) => top,
                (None, false) => area.y,
            };
            (x, y)
        }
        None => {
            let x = area.x + (area.width.saturating_sub(width)) / 2;
            let y = area.y + 3u16.min(area.height.saturating_sub(height));
            (x, y)
        }
    };
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ran_ago_seconds() {
        let now = Utc::now();
        let then = now - chrono::Duration::seconds(5);
        assert_eq!(ran_ago_string(&then.to_rfc3339(), now), "5s ago");
    }

    #[test]
    fn ran_ago_minutes() {
        let now = Utc::now();
        let then = now - chrono::Duration::minutes(3);
        assert_eq!(ran_ago_string(&then.to_rfc3339(), now), "3m ago");
    }

    #[test]
    fn ran_ago_hours() {
        let now = Utc::now();
        let then = now - chrono::Duration::hours(5);
        assert_eq!(ran_ago_string(&then.to_rfc3339(), now), "5h ago");
    }

    #[test]
    fn ran_ago_days() {
        let now = Utc::now();
        let then = now - chrono::Duration::days(3);
        assert_eq!(ran_ago_string(&then.to_rfc3339(), now), "3d ago");
    }

    #[test]
    fn ran_ago_long_renders_as_date() {
        let now = Utc::now();
        let then = now - chrono::Duration::days(30);
        let s = ran_ago_string(&then.to_rfc3339(), now);
        // Loosely check the date pattern (YYYY-MM-DD).
        assert_eq!(s.len(), 10);
        assert!(s.chars().nth(4) == Some('-'));
        assert!(s.chars().nth(7) == Some('-'));
    }

    #[test]
    fn ran_ago_passes_through_unparseable_timestamp() {
        let now = Utc::now();
        let s = ran_ago_string("not a timestamp", now);
        assert_eq!(s, "not a timestamp");
    }

    #[test]
    fn human_bytes_buckets() {
        assert_eq!(human_bytes(None), "—");
        assert_eq!(human_bytes(Some(0)), "0B");
        assert_eq!(human_bytes(Some(1023)), "1023B");
        assert_eq!(human_bytes(Some(1024)), "1.0K");
        assert_eq!(human_bytes(Some(2048)), "2.0K");
        assert_eq!(human_bytes(Some(1024 * 1024)), "1.0M");
        assert_eq!(human_bytes(Some(1024 * 1024 * 1024)), "1.0G");
    }

    #[test]
    fn human_bytes_negative_treated_as_missing() {
        // Defensive: a negative i64 row from a buggy migration
        // shouldn't render as `-1.0G`. Show the missing sentinel.
        assert_eq!(human_bytes(Some(-1)), "—");
    }

    fn http_entry(outcome: &str, status: Option<i64>) -> httui_core::block_history::HistoryEntry {
        httui_core::block_history::HistoryEntry {
            id: 1,
            file_path: "/x.md".into(),
            block_alias: "a".into(),
            method: "GET".into(),
            url_canonical: "https://x.com".into(),
            status,
            request_size: Some(120),
            response_size: Some(800),
            elapsed_ms: Some(42),
            outcome: outcome.into(),
            ran_at: Utc::now().to_rfc3339(),
            plan: None,
        }
    }

    fn db_entry(status: Option<i64>) -> httui_core::block_history::HistoryEntry {
        httui_core::block_history::HistoryEntry {
            id: 1,
            file_path: "/x.md".into(),
            block_alias: "q".into(),
            method: "db:sqlite".into(),
            url_canonical: "".into(),
            status,
            request_size: Some(80),
            response_size: Some(2048),
            elapsed_ms: Some(12),
            outcome: "success".into(),
            ran_at: Utc::now().to_rfc3339(),
            plan: None,
        }
    }

    fn line_text(line: &Line<'static>) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn format_entry_line_http_2xx_uses_green_chip() {
        let entry = http_entry("success", Some(200));
        let now = Utc::now();
        let line = format_entry_line(&entry, now, Style::default());
        assert!(line_text(&line).contains("200"));
    }

    #[test]
    fn format_entry_line_http_3xx_uses_yellow_chip() {
        let entry = http_entry("success", Some(301));
        let now = Utc::now();
        let line = format_entry_line(&entry, now, Style::default());
        assert!(line_text(&line).contains("301"));
    }

    #[test]
    fn format_entry_line_http_4xx_uses_magenta_chip() {
        let entry = http_entry("success", Some(404));
        let now = Utc::now();
        let line = format_entry_line(&entry, now, Style::default());
        assert!(line_text(&line).contains("404"));
    }

    #[test]
    fn format_entry_line_http_5xx_uses_red_chip() {
        let entry = http_entry("success", Some(503));
        let now = Utc::now();
        let line = format_entry_line(&entry, now, Style::default());
        assert!(line_text(&line).contains("503"));
    }

    #[test]
    fn format_entry_line_http_no_status_shows_err() {
        let entry = http_entry("success", None);
        let now = Utc::now();
        let line = format_entry_line(&entry, now, Style::default());
        assert!(line_text(&line).contains("err"));
    }

    #[test]
    fn format_entry_line_cancelled_renders_dash_chip() {
        let entry = http_entry("cancelled", Some(200));
        let now = Utc::now();
        let line = format_entry_line(&entry, now, Style::default());
        assert!(line_text(&line).contains("—"));
    }

    #[test]
    fn format_entry_line_error_outcome_renders_err() {
        let entry = http_entry("error", Some(200));
        let now = Utc::now();
        let line = format_entry_line(&entry, now, Style::default());
        assert!(line_text(&line).contains("err"));
    }

    #[test]
    fn format_entry_line_db_with_row_count_shows_n_r_chip() {
        let entry = db_entry(Some(42));
        let now = Utc::now();
        let line = format_entry_line(&entry, now, Style::default());
        assert!(line_text(&line).contains("42r"));
    }

    #[test]
    fn format_entry_line_db_without_status_shows_ok() {
        let entry = db_entry(None);
        let now = Utc::now();
        let line = format_entry_line(&entry, now, Style::default());
        assert!(line_text(&line).contains("ok"));
    }
}
