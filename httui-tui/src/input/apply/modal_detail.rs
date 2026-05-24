// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
//! Result-detail modal appliers: DB row-detail + HTTP response-detail
//! (open / close / copy + the title/body/size formatters they share).
//! Mechanically moved out of `vim/dispatch.rs` (tui-v2 vertical 1,
//! fase 1 p5e) with no logic change.

use crate::app::{App, StatusKind};
use crate::buffer::block::BlockNode;
use crate::buffer::{Cursor, Segment};
use crate::input::action::Action;
use crate::vim::mode::Mode;

// ───────────── result-detail modals (DB row + HTTP response) ─────────────

/// `<CR>` dispatcher: routes to the right modal based on the block
/// type the cursor is parked on. DB blocks open the row-detail modal
/// (column → value pairs of the focused row); HTTP blocks open the
/// response-detail modal (status line + headers + full body). For any
/// other position the action is a no-op.
pub(crate) fn apply_open_result_detail(app: &mut App) {
    let Some(doc) = app.document() else { return };
    let Cursor::InBlockResult { segment_idx, .. } = doc.cursor() else {
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        return;
    };
    let Segment::Block(block) = seg else { return };
    if block.is_http() {
        apply_open_http_response_detail(app);
    } else if block.is_db() {
        apply_open_db_row_detail(app);
    }
}

/// `<CR>` in normal mode → open the row-detail modal. Validates the
/// cursor is parked on a real result row of a `select`, snapshots
/// the row's columns into a freshly-built `Document` (body text as
/// a single prose run), and flips the mode. The pending vim state
/// is reset so a stale count from the editor doesn't leak into the
/// modal's first keystroke.
pub(crate) fn apply_open_db_row_detail(app: &mut App) {
    let Some(doc) = app.document() else { return };
    let Cursor::InBlockResult { segment_idx, row } = doc.cursor() else {
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        return;
    };
    let Segment::Block(block) = seg else { return };
    if !block.is_db() {
        return;
    }
    let title = build_db_row_modal_title(block, row);
    let body_text = match build_db_row_body_text(block, row) {
        Some(t) => t,
        None => return,
    };
    // Build a Document from the body text. `from_markdown` of plain
    // text yields a single Prose segment, which is exactly what we
    // want — the motion engine treats it as one editable run. We
    // sanitize triple-backticks first so a row carrying ``` doesn't
    // accidentally open a fence and split the body in two.
    let safe_body = body_text.replace("```", "ʼʼʼ");
    let modal_doc = match crate::buffer::Document::from_markdown(&safe_body) {
        Ok(d) => d,
        Err(_) => return,
    };
    app.modal = Some(crate::modal::Modal::DbRowDetail(crate::app::DbRowDetailState {
        segment_idx,
        row,
        title,
        doc: modal_doc,
        // Updated by the renderer on the first paint; 1 is just a
        // safe lower bound so the first half-page motion (rare, but
        // possible if the user types `Ctrl-d` immediately) doesn't
        // divide by zero anywhere.
        viewport_height: 1,
        viewport_top: 0,
    }));
    app.vim.mode = Mode::DbRowDetail;
    app.vim.reset_pending();
}

/// `Esc`/`q`/`Ctrl-c` inside the modal → drop the state and return
/// to normal mode. The editor cursor stays on the result row that
/// was being inspected, which feels right when the modal closes.
pub(crate) fn apply_close_db_row_detail(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::DbRowDetail(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

/// Build the modal's title line. Uses the block's alias when set so
/// `Row 7 · 4 fields · q1` reads naturally; falls back to
/// `Row N · M fields` when no alias is present.
pub(crate) fn build_db_row_modal_title(block: &BlockNode, row: usize) -> String {
    let columns = block
        .cached_result
        .as_ref()
        .and_then(|v| v.get("results"))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("columns"))
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let suffix = if columns == 1 { "field" } else { "fields" };
    match block.alias.as_deref() {
        Some(alias) => format!(" Row {} · {} {} · {} ", row + 1, columns, suffix, alias),
        None => format!(" Row {} · {} {} ", row + 1, columns, suffix),
    }
}

/// Render one row as the body text the modal will navigate. Mirrors
/// `ui::db_row_detail::build_body_lines` (column header line + 2-
/// space-indented value lines + blank separator) but emits a `String`
/// so it can be parsed into a `Document` for the motion engine.
pub(crate) fn build_db_row_body_text(block: &BlockNode, row: usize) -> Option<String> {
    let cached = block.cached_result.as_ref()?;
    let first = cached
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return None;
    }
    let columns: Vec<(String, String)> = first
        .get("columns")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
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
                .collect()
        })
        .unwrap_or_default();
    if columns.is_empty() {
        return None;
    }
    let row_obj = first.get("rows").and_then(|v| v.as_array())?.get(row)?;
    let mut out = String::new();
    for (i, (name, ty)) in columns.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if ty.is_empty() {
            out.push_str(name);
        } else {
            out.push_str(&format!("{name}  ({ty})"));
        }
        out.push('\n');
        let value = row_obj
            .get(name)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        for line in render_value_text(&value) {
            out.push_str("  ");
            out.push_str(&line);
            out.push('\n');
        }
    }
    Some(out)
}

/// Plain-text rendering of a JSON value for the body. Strings that
/// look like JSON (stringified objects/arrays — common with
/// Postgres `jsonb` over wire) are unwrapped + pretty-printed so
/// `metadata` columns aren't a single illegible blob.
pub(crate) fn render_value_text(v: &serde_json::Value) -> Vec<String> {
    match v {
        serde_json::Value::Null => vec!["NULL".into()],
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => vec![v.to_string()],
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            let looks_jsonish = (trimmed.starts_with('{') && trimmed.ends_with('}'))
                || (trimmed.starts_with('[') && trimmed.ends_with(']'));
            if looks_jsonish {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    return serde_json::to_string_pretty(&parsed)
                        .unwrap_or_default()
                        .lines()
                        .map(String::from)
                        .collect();
                }
            }
            if s.is_empty() {
                vec!["(empty)".into()]
            } else {
                s.lines().map(String::from).collect()
            }
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string_pretty(v)
                .unwrap_or_default()
                .lines()
                .map(String::from)
                .collect()
        }
    }
}

/// `y` inside the modal → copy the inspected row to the system
/// clipboard as pretty-printed JSON. Status hints differentiate the
/// success path ("row copied as JSON") from environments where no
/// clipboard backend is reachable (SSH without a forwarder, headless
/// container, sandbox).
pub(crate) fn apply_copy_db_row_detail_json(app: &mut App) {
    let Some(state) = app.db_row_detail() else {
        return;
    };
    let (segment_idx, row) = (state.segment_idx, state.row);
    let Some(payload) = db_row_payload(app, segment_idx, row) else {
        app.set_status(StatusKind::Error, "row no longer available");
        return;
    };
    let text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string());
    match crate::clipboard::set_text(&text) {
        Ok(()) => app.set_status(StatusKind::Info, "row copied as JSON"),
        Err(msg) => app.set_status(StatusKind::Error, msg),
    }
}

/// Snapshot of a single result row as a `{column: value}` JSON
/// object. Source for the modal's `y` clipboard copy. Returns
/// `None` if the block / row vanished between the keystroke and
/// the dispatch (e.g. user re-ran the block in another tab).
///
/// Must read from the active pane's note doc, not `app.document()`:
/// while the modal is open, `document()` redirects to the modal's
/// body doc (single Prose segment), so indexing by `segment_idx`
/// would never find a Block. The owning note lives on the pane.
pub(crate) fn db_row_payload(
    app: &App,
    segment_idx: usize,
    row: usize,
) -> Option<serde_json::Value> {
    let doc = app.tabs.active_document()?;
    let Segment::Block(block) = doc.segments().get(segment_idx)? else {
        return None;
    };
    let cached = block.cached_result.as_ref()?;
    let first = cached
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return None;
    }
    let columns: Vec<&str> = first
        .get("columns")
        .and_then(|v| v.as_array())?
        .iter()
        .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
        .collect();
    let row_obj = first.get("rows").and_then(|v| v.as_array())?.get(row)?;
    let mut out = serde_json::Map::new();
    for name in columns {
        out.insert(
            name.to_string(),
            row_obj
                .get(name)
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
    }
    Some(serde_json::Value::Object(out))
}

// ───────────── HTTP response-detail modal ─────────────

/// Open the HTTP response-detail modal. Validates the cursor is on
/// an HTTP block with a cached response, snapshots the response
/// (status line + headers + body) into a fresh `Document`, flips
/// the mode. Pending vim state is reset to keep stale counts /
/// operators from leaking into the modal.
pub(crate) fn apply_open_http_response_detail(app: &mut App) {
    let Some(doc) = app.document() else { return };
    let Cursor::InBlockResult { segment_idx, .. } = doc.cursor() else {
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        return;
    };
    let Segment::Block(block) = seg else { return };
    if !block.is_http() {
        return;
    }
    let title = build_http_response_modal_title(block);
    let body_text = match build_http_response_body_text(block) {
        Some(t) => t,
        None => {
            app.set_status(StatusKind::Info, "no cached response on this block");
            return;
        }
    };
    // Sanitize triple-backticks so a body that carries ``` doesn't
    // open a fence and split the modal doc — we want a single Prose
    // segment for the motion engine to operate on.
    let safe_body = body_text.replace("```", "ʼʼʼ");
    let modal_doc = match crate::buffer::Document::from_markdown(&safe_body) {
        Ok(d) => d,
        Err(_) => return,
    };
    app.modal = Some(crate::modal::Modal::HttpResponseDetail(
        crate::app::HttpResponseDetailState {
            segment_idx,
            title,
            doc: modal_doc,
            viewport_height: 1,
            viewport_top: 0,
        },
    ));
    app.vim.mode = Mode::HttpResponseDetail;
    app.vim.reset_pending();
}

/// `Ctrl-c` inside the HTTP response-detail modal → drop the state
/// and return to normal mode.
pub(crate) fn apply_close_http_response_detail(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::HttpResponseDetail(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

/// `Y` inside the modal → copy the full response body (raw, not the
/// rendered modal text) to the clipboard. Falls back gracefully when
/// the clipboard isn't reachable or no body is cached.
pub(crate) fn apply_copy_http_response_body(app: &mut App) {
    let Some(state) = app.http_response_detail() else {
        return;
    };
    let segment_idx = state.segment_idx;
    let Some(text) = http_response_raw_body(app, segment_idx) else {
        app.set_status(StatusKind::Error, "no response body to copy");
        return;
    };
    match crate::clipboard::set_text(&text) {
        Ok(()) => app.set_status(StatusKind::Info, "response body copied"),
        Err(msg) => app.set_status(StatusKind::Error, msg),
    }
}

/// Title line for the modal. Reuses the alias when present so the
/// header reads naturally: ` Response · 200 · 1.4 kB · login `.
pub(crate) fn build_http_response_modal_title(block: &BlockNode) -> String {
    let cached = match block.cached_result.as_ref() {
        Some(c) => c,
        None => return " Response ".to_string(),
    };
    let status = cached
        .get("status")
        .and_then(|v| v.as_u64())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "?".into());
    let size = cached
        .get("size_bytes")
        .and_then(|v| v.as_u64())
        .map(format_size)
        .unwrap_or_default();
    let mut parts: Vec<String> = vec![format!("Response · {status}")];
    if !size.is_empty() {
        parts.push(size);
    }
    if let Some(alias) = block.alias.as_deref() {
        parts.push(alias.to_string());
    }
    format!(" {} ", parts.join(" · "))
}

/// Render the cached response into the body text the modal's motion
/// engine will navigate. Layout (fixed): a status line, blank,
/// `Headers` section (`name: value` each), blank, `Body` heading,
/// then the body — pretty-printed JSON when possible, raw text
/// otherwise.
pub(crate) fn build_http_response_body_text(block: &BlockNode) -> Option<String> {
    let cached = block.cached_result.as_ref()?;
    let mut out = String::new();
    let status = cached.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    let status_text = cached
        .get("status_text")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let elapsed_ms = cached
        .get("timing")
        .and_then(|t| t.get("total_ms"))
        .and_then(|v| v.as_u64());
    let size_bytes = cached.get("size_bytes").and_then(|v| v.as_u64());
    out.push_str(&format!("{status} {status_text}"));
    if let Some(ms) = elapsed_ms {
        out.push_str(&format!("  ·  {ms} ms"));
    }
    if let Some(sz) = size_bytes {
        out.push_str(&format!("  ·  {}", format_size(sz)));
    }
    out.push('\n');

    out.push_str("\nHeaders\n");
    if let Some(headers) = cached.get("headers").and_then(|v| v.as_array()) {
        if headers.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for h in headers {
                let key = h.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
                out.push_str(&format!("  {key}: {value}\n"));
            }
        }
    } else {
        out.push_str("  (none)\n");
    }

    out.push_str("\nBody\n");
    let body_text = render_http_body(cached);
    if body_text.is_empty() {
        out.push_str("  (empty)\n");
    } else {
        // Indent each line by two spaces so the body lines up with
        // header values — a small visual cue that they share the same
        // "section body" role.
        for line in body_text.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    Some(out)
}

/// Format a byte count as a short human-readable string (`1.4 kB`,
/// `12 B`, `2.0 MB`). Mirrors the footer/status formatting in
/// `ui::blocks` so the modal title matches the editor chrome.
pub(crate) fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} kB");
    }
    let mb = kb / 1024.0;
    format!("{mb:.1} MB")
}

/// Render the body as text. Tries pretty-JSON first (covers the
/// common API path), falls back to whatever string representation
/// the cached value carries.
pub(crate) fn render_http_body(cached: &serde_json::Value) -> String {
    let body = cached.get("body");
    match body {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(v) => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
        None => String::new(),
    }
}

/// Raw body for the `Y`-copy path. Same fallback chain as
/// [`render_http_body`], minus the leading section labels.
///
/// Same trap as `db_row_payload`: `app.document()` redirects to the
/// modal's body doc while the HTTP response detail is open, so the
/// caller must reach for the editor pane's note doc directly.
pub(crate) fn http_response_raw_body(app: &App, segment_idx: usize) -> Option<String> {
    let doc = app.tabs.active_document()?;
    let Segment::Block(block) = doc.segments().get(segment_idx)? else {
        return None;
    };
    let cached = block.cached_result.as_ref()?;
    let body = render_http_body(cached);
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

/// `apply_action` sub-match for the result-detail modal domain (DB
/// row-detail + HTTP response-detail open/close/copy). Mechanically
/// split out of the `apply_action` router in `vim/dispatch.rs` (tui-v2
/// vertical 1, fase 1 p6c) — arm bodies copied verbatim. The outer
/// router routes only this group's variants here, so the
/// `unreachable!` is a compile-time-backed invariant.
pub(crate) fn apply_modal_detail(app: &mut App, action: Action, _recording: bool) {
    match action {
        Action::OpenDbRowDetail => apply_open_result_detail(app),
        Action::CloseDbRowDetail => apply_close_db_row_detail(app),
        Action::CopyDbRowDetailJson => apply_copy_db_row_detail_json(app),
        Action::CloseHttpResponseDetail => apply_close_http_response_detail(app),
        Action::CopyHttpResponseBody => apply_copy_http_response_body(app),
        _ => unreachable!("apply_modal_detail: variante fora do grupo"),
    }
}
