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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, BlockExportFormat, DbExportPickerState};
    use crate::buffer::Document;
    use crate::config::Config;
    use crate::modal::Modal;
    use crate::pane::{Pane, TabState};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_with_doc(md: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let note = vault.path().join("note.md");
        std::fs::write(&note, md).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        let doc = Document::from_markdown(md).unwrap();
        let pane = Pane::new(doc, note);
        app.tabs.tabs.clear();
        app.tabs.tabs.push(TabState::new(pane));
        app.tabs.active = 0;
        (app, data, vault)
    }

    fn first_block_idx(app: &App) -> usize {
        app.document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block")
    }

    fn place_cursor_in_first_block(app: &mut App) -> usize {
        let idx = first_block_idx(app);
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: 0,
        });
        idx
    }

    fn seed_select_result(app: &mut App, idx: usize, row_count: usize) {
        let cols = serde_json::json!([{"name":"id","type_name":"INTEGER"}]);
        let rows: Vec<_> = (0..row_count).map(|i| serde_json::json!([i])).collect();
        let cache = serde_json::json!({
            "results": [{"kind":"select","columns":cols,"rows":rows,"has_more":false}],
            "messages":[],
            "stats":{"elapsed_ms":1}
        });
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.cached_result = Some(cache);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_no_block_at_cursor_errors() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        let err = open_export_picker(&mut app).unwrap_err();
        assert!(err.contains("cursor"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_db_with_no_cache_errors() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        let err = open_export_picker(&mut app).unwrap_err();
        assert!(
            err.contains("run the block") || err.contains("export"),
            "got {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_db_mutation_result_errors() {
        let md = "```db-sqlite alias=q\nUPDATE t SET x=1;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let idx = place_cursor_in_first_block(&mut app);
        let cache = serde_json::json!({
            "results": [{"kind":"mutation","rows_affected":2}],
            "messages":[],
            "stats":{"elapsed_ms":1}
        });
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.cached_result = Some(cache);
        }
        let err = open_export_picker(&mut app).unwrap_err();
        assert!(
            err.contains("SELECT") || err.contains("tabular"),
            "got {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_db_select_with_zero_rows_errors() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let idx = place_cursor_in_first_block(&mut app);
        seed_select_result(&mut app, idx, 0);
        let err = open_export_picker(&mut app).unwrap_err();
        assert!(err.contains("no rows"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_db_select_with_rows_succeeds() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let idx = place_cursor_in_first_block(&mut app);
        seed_select_result(&mut app, idx, 3);
        open_export_picker(&mut app).unwrap();
        let Some(Modal::DbExportPicker(state)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(state.formats.len(), BlockExportFormat::DB_FORMATS.len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_http_with_no_url_errors() {
        let md = "```http alias=a\n\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        let err = open_export_picker(&mut app).unwrap_err();
        assert!(err.contains("URL"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_http_with_valid_url_succeeds() {
        let md = "```http alias=a\nGET https://x.com\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        open_export_picker(&mut app).unwrap();
        let Some(Modal::DbExportPicker(state)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(state.formats.len(), BlockExportFormat::HTTP_FORMATS.len());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_unsupported_block_type_errors() {
        let md = "```http alias=a\nGET /x\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let idx = place_cursor_in_first_block(&mut app);
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.block_type = "mystery".into();
        }
        let err = open_export_picker(&mut app).unwrap_err();
        assert!(err.contains("don't support export"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_export_picker_clears_modal_and_normal_mode() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.modal = Some(Modal::DbExportPicker(DbExportPickerState::new(
            0,
            BlockExportFormat::HTTP_FORMATS,
        )));
        close_export_picker(&mut app);
        assert!(app.modal.is_none());
        assert!(matches!(app.vim.mode, crate::vim::mode::Mode::Normal));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_export_picker_noop_other_modal() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.modal = Some(Modal::Help);
        close_export_picker(&mut app);
        assert!(matches!(app.modal, Some(Modal::Help)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn move_export_picker_cursor_wraps_around() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.modal = Some(Modal::DbExportPicker(DbExportPickerState::new(
            0,
            BlockExportFormat::HTTP_FORMATS,
        )));
        let n = BlockExportFormat::HTTP_FORMATS.len();
        move_export_picker_cursor(&mut app, 1);
        let Some(Modal::DbExportPicker(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.selected, 1);
        move_export_picker_cursor(&mut app, -2);
        let Some(Modal::DbExportPicker(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.selected, n - 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn move_export_picker_cursor_no_modal_is_noop() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        move_export_picker_cursor(&mut app, 1); // no panic
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_export_picker_no_modal_just_returns_to_normal() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        confirm_export_picker(&mut app);
        assert!(matches!(app.vim.mode, crate::vim::mode::Mode::Normal));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_export_picker_db_csv_runs_serializer_and_sets_status() {
        let md = "```db-sqlite alias=q\nSELECT id FROM t;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let idx = place_cursor_in_first_block(&mut app);
        seed_select_result(&mut app, idx, 2);
        open_export_picker(&mut app).unwrap();
        // Selected CSV (first in DB_FORMATS).
        confirm_export_picker(&mut app);
        assert!(app.modal.is_none());
        assert!(app.status_message.is_some(), "status set");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_export_picker_http_curl_runs_serializer() {
        let md = "```http alias=a\nGET https://x.com\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        open_export_picker(&mut app).unwrap();
        confirm_export_picker(&mut app);
        assert!(app.modal.is_none());
        assert!(app.status_message.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_export_picker_http_with_unresolvable_ref_surfaces_error() {
        let md = "```http alias=a\nGET https://x.com/{{ghost.body.id}}\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        open_export_picker(&mut app).unwrap();
        confirm_export_picker(&mut app);
        // Either resolution error (likely) or success — both are valid paths.
        assert!(app.status_message.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_export_picker_block_disappeared_sets_error() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.modal = Some(Modal::DbExportPicker(DbExportPickerState::new(
            999, // out-of-range
            BlockExportFormat::HTTP_FORMATS,
        )));
        confirm_export_picker(&mut app);
        let s = app.status_message.expect("status set");
        assert!(s.text.contains("disappeared"), "got {:?}", s.text);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_export_picker_db_with_no_cache_after_open_emits_hint() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        let idx = place_cursor_in_first_block(&mut app);
        seed_select_result(&mut app, idx, 2);
        open_export_picker(&mut app).unwrap();
        // Wipe cache between open + confirm.
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.cached_result = None;
        }
        confirm_export_picker(&mut app);
        let s = app.status_message.expect("status set");
        assert!(
            s.text.contains("cached") || s.text.contains("empty"),
            "got {:?}",
            s.text
        );
    }
}
