//! Block settings modal (limit/timeout per DB/HTTP block).

use crate::app::{App, StatusKind};
use crate::buffer::Cursor;

/// `gs` on a DB or HTTP block — open the settings modal prefilled
/// with the current values. Block-type aware:
///   - DB: limit + timeout fields, focus starts on Limit
///   - HTTP: timeout only (no row-cap concept), focus on Timeout
///
/// Tab cycles fields (no-op on single-field HTTP modal), Enter saves
/// all, Esc cancels. Non-DB / non-HTTP blocks surface a status hint
/// and bail.
pub fn open_db_settings_modal(app: &mut App) -> Result<(), String> {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => return Err("place the cursor on a block first".into()),
    };
    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(crate::buffer::Segment::Block(b)) => b,
        _ => return Err("place the cursor on a block first".into()),
    };
    if !block.is_db() && !block.is_http() {
        return Err(format!(
            "`{}` blocks have no settings yet",
            block.block_type
        ));
    }

    // Build the field list per block type. Order pinned here —
    // Tab/BackTab cycle in this order; users build muscle memory.
    // Future fields slot in by adding a new entry here, no other
    // code change. All values are stringified u64 (positive
    // integer) — empty input means "clear the field".
    let mut fields: Vec<crate::app::SettingsField> = Vec::new();
    if block.is_db() {
        let limit_str = block
            .params
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string())
            .unwrap_or_default();
        fields.push(crate::app::SettingsField {
            label: "Limit (rows, blank = no cap)",
            key: "limit",
            input: crate::vim::lineedit::LineEdit::from_str(limit_str),
        });
    }
    let timeout_str = block
        .params
        .get("timeout_ms")
        .and_then(|v| v.as_u64())
        .map(|n| n.to_string())
        .unwrap_or_default();
    fields.push(crate::app::SettingsField {
        label: "Timeout (ms, blank = default)",
        key: "timeout_ms",
        input: crate::vim::lineedit::LineEdit::from_str(timeout_str),
    });

    app.modal = Some(crate::modal::Modal::DbSettings(
        crate::app::DbSettingsState {
            segment_idx,
            fields,
            focus: 0,
        },
    ));
    app.vim.mode = crate::vim::mode::Mode::DbSettings;
    app.vim.reset_pending();
    Ok(())
}

pub fn close_db_settings_modal(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::DbSettings(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub fn db_settings_focus_step(app: &mut App, delta: i32) {
    let Some(state) = app.db_settings_mut() else {
        return;
    };
    if delta >= 0 {
        state.focus_next();
    } else {
        state.focus_prev();
    }
}

/// `<CR>` in the modal — validate every field's input and write
/// back to `block.params`. Empty input clears the matching key;
/// non-numeric / out-of-range input keeps the modal open and
/// surfaces a per-field status error so the user can fix without
/// losing the other inputs. The fields are walked in vector order
/// — Tab order — and validation short-circuits on the first
/// failure (label included in the error so the user knows which
/// field needs attention).
pub fn confirm_db_settings_modal(app: &mut App) {
    let Some(state) = app.db_settings() else {
        app.vim.enter_normal();
        return;
    };
    let segment_idx = state.segment_idx;

    // Validate every field. Each field is `(key, parsed_value)` —
    // the writeback step inserts when `Some`, removes when `None`.
    let mut writes: Vec<(&'static str, Option<u64>)> = Vec::with_capacity(state.fields.len());
    for field in &state.fields {
        let raw = field.input.as_str().trim().to_string();
        match parse_optional_u64(&raw) {
            Ok(v) => writes.push((field.key, v)),
            Err(e) => {
                // Use the field label (already user-friendly) as the
                // prefix so errors are unambiguous in multi-field
                // modals.
                let label_short = field
                    .label
                    .split_whitespace()
                    .next()
                    .unwrap_or(field.key)
                    .to_lowercase();
                app.set_status(StatusKind::Error, format!("{label_short}: {e}"));
                return;
            }
        }
    }

    // All inputs validated — close the modal and persist.
    if matches!(app.modal, Some(crate::modal::Modal::DbSettings(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();

    if let Some(doc) = app.tabs.active_document_mut() {
        doc.snapshot();
        if let Some(block) = doc.block_at_mut(segment_idx) {
            if let Some(obj) = block.params.as_object_mut() {
                for (key, value) in &writes {
                    match value {
                        Some(n) => {
                            obj.insert((*key).to_string(), serde_json::Value::Number((*n).into()));
                        }
                        None => {
                            obj.remove(*key);
                        }
                    }
                }
            }
        }
    }

    // Status summary — one chunk per field, comma-joined.
    let chunks: Vec<String> = writes
        .iter()
        .map(|(key, value)| match value {
            Some(n) => format!("{key} {n}"),
            None => format!("{key} cleared"),
        })
        .collect();
    let summary = if chunks.is_empty() {
        "settings unchanged".to_string()
    } else {
        format!("settings saved · {}", chunks.join(" · "))
    };
    app.set_status(StatusKind::Info, summary);
}

/// Empty input ⇒ `Ok(None)` so the caller can clear the field;
/// otherwise a valid `u64` (no negatives, no decimals).
pub(crate) fn parse_optional_u64(s: &str) -> Result<Option<u64>, String> {
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<u64>()
        .map(Some)
        .map_err(|_| format!("`{s}` is not a non-negative integer"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, DbSettingsState, SettingsField};
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

    fn place_cursor_in_first_block(app: &mut App) {
        let idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .expect("block");
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: 0,
        });
    }

    #[test]
    fn parse_optional_u64_handles_empty_valid_and_invalid() {
        assert_eq!(parse_optional_u64("").unwrap(), None);
        assert_eq!(parse_optional_u64("42").unwrap(), Some(42));
        assert!(parse_optional_u64("-1").is_err());
        assert!(parse_optional_u64("abc").is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_no_block_returns_err() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        let err = open_db_settings_modal(&mut app).unwrap_err();
        assert!(err.contains("cursor"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_unsupported_block_returns_err() {
        let md = "```http alias=a\nGET /x\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        // Mutate block_type to a non-db, non-http kind to hit the err path.
        let idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .unwrap();
        if let Some(b) = app.document_mut().unwrap().block_at_mut(idx) {
            b.block_type = "mystery".into();
        }
        let err = open_db_settings_modal(&mut app).unwrap_err();
        assert!(err.contains("no settings"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_db_block_seeds_limit_and_timeout_fields() {
        let md = "```db-sqlite alias=q limit=50 timeout=5000\nSELECT 1;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        open_db_settings_modal(&mut app).unwrap();
        let Some(Modal::DbSettings(state)) = app.modal.as_ref() else {
            panic!("expected DbSettings modal");
        };
        assert_eq!(state.fields.len(), 2);
        assert_eq!(state.fields[0].key, "limit");
        assert_eq!(state.fields[0].input.as_str(), "50");
        assert_eq!(state.fields[1].key, "timeout_ms");
        assert_eq!(state.fields[1].input.as_str(), "5000");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_http_block_seeds_only_timeout_field() {
        let md = "```http alias=a timeout=3000\nGET /x\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        open_db_settings_modal(&mut app).unwrap();
        let Some(Modal::DbSettings(state)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(state.fields.len(), 1);
        assert_eq!(state.fields[0].key, "timeout_ms");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_db_settings_clears_modal_and_returns_to_normal() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.modal = Some(Modal::DbSettings(DbSettingsState {
            segment_idx: 0,
            fields: Vec::new(),
            focus: 0,
        }));
        app.vim.mode = crate::vim::mode::Mode::DbSettings;
        close_db_settings_modal(&mut app);
        assert!(app.modal.is_none());
        assert!(matches!(app.vim.mode, crate::vim::mode::Mode::Normal));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_db_settings_noop_when_other_modal() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.modal = Some(Modal::Help);
        close_db_settings_modal(&mut app);
        assert!(matches!(app.modal, Some(Modal::Help)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn db_settings_focus_step_no_modal_is_noop() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        db_settings_focus_step(&mut app, 1); // no panic
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn db_settings_focus_step_cycles_forward_and_backward() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        app.modal = Some(Modal::DbSettings(DbSettingsState {
            segment_idx: 0,
            fields: vec![
                SettingsField {
                    label: "Limit",
                    key: "limit",
                    input: crate::vim::lineedit::LineEdit::new(),
                },
                SettingsField {
                    label: "Timeout",
                    key: "timeout_ms",
                    input: crate::vim::lineedit::LineEdit::new(),
                },
            ],
            focus: 0,
        }));
        db_settings_focus_step(&mut app, 1);
        let Some(Modal::DbSettings(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.focus, 1);
        db_settings_focus_step(&mut app, -1);
        let Some(Modal::DbSettings(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.focus, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_db_settings_no_modal_just_returns_to_normal() {
        let (mut app, _d, _v) = app_with_doc("prose\n").await;
        confirm_db_settings_modal(&mut app);
        assert!(matches!(app.vim.mode, crate::vim::mode::Mode::Normal));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_db_settings_invalid_input_surfaces_error_keeps_modal() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        open_db_settings_modal(&mut app).unwrap();
        if let Some(Modal::DbSettings(state)) = app.modal.as_mut() {
            state.fields[0].input =
                crate::vim::lineedit::LineEdit::from_str("not-a-number".to_string());
        }
        confirm_db_settings_modal(&mut app);
        assert!(
            app.status_message
                .as_ref()
                .map(|s| s.text.contains("limit"))
                .unwrap_or(false),
            "expected error status: {:?}",
            app.status_message.as_ref().map(|s| &s.text)
        );
        assert!(app.modal.is_some(), "modal must stay open on error");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_db_settings_valid_writes_back_and_closes() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        open_db_settings_modal(&mut app).unwrap();
        if let Some(Modal::DbSettings(state)) = app.modal.as_mut() {
            state.fields[0].input = crate::vim::lineedit::LineEdit::from_str("100");
            state.fields[1].input = crate::vim::lineedit::LineEdit::from_str("3000");
        }
        confirm_db_settings_modal(&mut app);
        assert!(app.modal.is_none(), "modal closed after success");
        let block = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .find_map(|s| match s {
                crate::buffer::Segment::Block(b) => Some(b.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            block.params.get("limit").and_then(|v| v.as_u64()),
            Some(100)
        );
        assert_eq!(
            block.params.get("timeout_ms").and_then(|v| v.as_u64()),
            Some(3000)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn confirm_db_settings_empty_clears_fields() {
        let md = "```db-sqlite alias=q limit=42\nSELECT 1;\n```\n";
        let (mut app, _d, _v) = app_with_doc(md).await;
        place_cursor_in_first_block(&mut app);
        open_db_settings_modal(&mut app).unwrap();
        if let Some(Modal::DbSettings(state)) = app.modal.as_mut() {
            state.fields[0].input = crate::vim::lineedit::LineEdit::new(); // clear
        }
        confirm_db_settings_modal(&mut app);
        let block = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .find_map(|s| match s {
                crate::buffer::Segment::Block(b) => Some(b.clone()),
                _ => None,
            })
            .unwrap();
        assert!(
            block.params.get("limit").is_none(),
            "limit should be cleared"
        );
    }
}
