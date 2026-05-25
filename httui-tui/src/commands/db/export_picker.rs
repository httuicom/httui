//! Block export picker (DB: CSV/JSON/Markdown/INSERT; HTTP: cURL/Fetch/Python/HTTPie/.http).

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};

use super::load_active_env_vars;

/// `gx` on a DB or HTTP block — open the export-format picker.
/// Block-type aware:
///   - DB: validates `cached_result.results[0]` is a SELECT with ≥1
///     row, picker shows CSV/JSON/Markdown/INSERT.
///   - HTTP: any HTTP block (no result needed — code-gen exports
///     the *request*), picker shows cURL/Fetch/Python/HTTPie/.http.
///
/// Failures surface as `Err(_)` with a status hint; success flips
/// the mode and stashes [`DbExportPickerState`] on `app`. Cursor is
/// allowed to move while the picker is open — confirm re-resolves
/// the block via the saved `segment_idx`.
pub fn open_export_picker(app: &mut App) -> Result<(), String> {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => return Err("place the cursor on a block first".into()),
    };

    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => return Err("place the cursor on a block first".into()),
    };

    let formats: &'static [crate::app::BlockExportFormat] = if block.is_db() {
        // DB: needs a SELECT result with rows — code-gen wouldn't
        // make sense for an empty / mutation result.
        let cache = block
            .cached_result
            .as_ref()
            .ok_or_else(|| "run the block before exporting its result".to_string())?;
        let first = cache
            .get("results")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| "no result to export — run the block first".to_string())?;
        let kind = first.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        if kind != "select" {
            return Err(format!(
                "{kind} results have no tabular form — export is for SELECT only"
            ));
        }
        let row_count = first
            .get("rows")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        if row_count == 0 {
            return Err("result has no rows to export".into());
        }
        crate::app::BlockExportFormat::DB_FORMATS
    } else if block.is_http() {
        // HTTP: code-gen exports the *request* (method+url+headers+
        // body), so we only need a non-empty URL. Run state isn't
        // required.
        let url_ok = block
            .params
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if !url_ok {
            return Err("set a URL on the block before exporting".into());
        }
        crate::app::BlockExportFormat::HTTP_FORMATS
    } else {
        return Err(format!(
            "`{}` blocks don't support export yet",
            block.block_type
        ));
    };

    app.modal = Some(crate::modal::Modal::DbExportPicker(
        crate::app::DbExportPickerState::new(segment_idx, formats),
    ));
    app.vim.mode = crate::vim::mode::Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub fn close_export_picker(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::DbExportPicker(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub fn move_export_picker_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::DbExportPicker(state)) = app.modal.as_mut() else {
        return;
    };
    let n = state.formats.len() as i64;
    if n == 0 {
        return;
    }
    // Wrap so j/k cycle the list — feels right for a 4-5-item list
    // where the user is likely to overshoot.
    let next = ((state.selected as i64) + delta as i64).rem_euclid(n);
    state.selected = next as usize;
}

/// `Enter` in the picker — dispatch to the right serializer based
/// on the block type and copy the output to the clipboard. The
/// popup closes either way; clipboard failure shows in the status
/// line so the user notices it didn't paste.
pub fn confirm_export_picker(app: &mut App) {
    let state = match app.modal.take() {
        Some(crate::modal::Modal::DbExportPicker(s)) => s,
        other => {
            app.modal = other;
            app.vim.enter_normal();
            return;
        }
    };
    app.vim.enter_normal();

    let format = match state.formats.get(state.selected).copied() {
        Some(f) => f,
        None => return,
    };

    let block = match app
        .document()
        .and_then(|d| d.segments().get(state.segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => {
            app.set_status(StatusKind::Error, "block disappeared from the document");
            return;
        }
    };

    let payload_with_summary = match format {
        // ─── DB formats ───
        crate::app::BlockExportFormat::Csv
        | crate::app::BlockExportFormat::Json
        | crate::app::BlockExportFormat::Markdown
        | crate::app::BlockExportFormat::Insert => {
            let cache = match block.cached_result.as_ref() {
                Some(v) => v,
                None => {
                    app.set_status(StatusKind::Info, "block has no cached result");
                    return;
                }
            };
            let first = match cache
                .get("results")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
            {
                Some(v) => v,
                None => {
                    app.set_status(StatusKind::Info, "result is empty");
                    return;
                }
            };
            let columns: Vec<httui_core::db::connections::ColumnInfo> = first
                .get("columns")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let rows: Vec<serde_json::Value> = first
                .get("rows")
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default();
            if !httui_core::blocks::db_export::has_exportable_rows(&columns, &rows) {
                app.set_status(StatusKind::Info, "no rows to export");
                return;
            }
            let payload = match format {
                crate::app::BlockExportFormat::Csv => {
                    httui_core::blocks::db_export::to_csv(&columns, &rows)
                }
                crate::app::BlockExportFormat::Json => {
                    httui_core::blocks::db_export::to_json(&rows)
                }
                crate::app::BlockExportFormat::Markdown => {
                    httui_core::blocks::db_export::to_markdown(&columns, &rows)
                }
                crate::app::BlockExportFormat::Insert => {
                    let sql = block
                        .params
                        .get("query")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let table =
                        httui_core::blocks::db_export::infer_table_name(sql).unwrap_or_default();
                    httui_core::blocks::db_export::to_inserts(&columns, &rows, &table)
                }
                _ => unreachable!("filtered by outer match"),
            };
            let summary = format!(
                "copied {} ({} rows, {} bytes) to clipboard",
                format.label(),
                rows.len(),
                payload.len()
            );
            (payload, summary)
        }

        // ─── HTTP formats ───
        crate::app::BlockExportFormat::Curl
        | crate::app::BlockExportFormat::Fetch
        | crate::app::BlockExportFormat::Python
        | crate::app::BlockExportFormat::HTTPie
        | crate::app::BlockExportFormat::HttpFile => {
            // Resolve `{{refs}}` (env vars + block deps) BEFORE
            // serializing — the user expects a snippet they can
            // paste as-is. Carrying placeholders into the output
            // means the cURL command fails when run.
            let env_vars = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(load_active_env_vars(&app.environments_store))
            })
            .unwrap_or_default();
            let segments_snapshot: Vec<Segment> = app
                .document()
                .map(|d| d.segments().to_vec())
                .unwrap_or_default();
            let mut resolved = block.params.clone();
            if let Err(msg) = crate::commands::http::resolve_in_http_params(
                &mut resolved,
                &segments_snapshot,
                state.segment_idx,
                &env_vars,
            ) {
                app.set_status(StatusKind::Error, format!("ref resolution failed: {msg}"));
                return;
            }
            let payload = match format {
                crate::app::BlockExportFormat::Curl => {
                    httui_core::blocks::http_codegen::to_curl(&resolved)
                }
                crate::app::BlockExportFormat::Fetch => {
                    httui_core::blocks::http_codegen::to_fetch(&resolved)
                }
                crate::app::BlockExportFormat::Python => {
                    httui_core::blocks::http_codegen::to_python(&resolved)
                }
                crate::app::BlockExportFormat::HTTPie => {
                    httui_core::blocks::http_codegen::to_httpie(&resolved)
                }
                crate::app::BlockExportFormat::HttpFile => {
                    httui_core::blocks::http_codegen::to_http_file(&resolved)
                }
                _ => unreachable!("filtered by outer match"),
            };
            let summary = format!(
                "copied {} ({} bytes) to clipboard",
                format.label(),
                payload.len()
            );
            (payload, summary)
        }
    };

    let (payload, summary) = payload_with_summary;
    match crate::clipboard::set_text(&payload) {
        Ok(()) => {
            app.set_status(StatusKind::Info, summary);
        }
        Err(e) => {
            app.set_status(StatusKind::Error, e);
        }
    }
}
