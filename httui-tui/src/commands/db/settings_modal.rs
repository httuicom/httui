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

    app.modal = Some(crate::modal::Modal::DbSettings(crate::app::DbSettingsState {
        segment_idx,
        fields,
        focus: 0,
    }));
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
