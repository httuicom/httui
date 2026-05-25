//! Direct cURL copy (Ctrl+Shift+C). Same flow as the `gx` export
//! picker's HTTP path but without the picker — the "express" route.

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};
use crate::commands::db::load_active_env_vars;

use super::refs::resolve_in_http_params;

/// `<C-S-c>` on an HTTP block — resolve `{{refs}}` and copy a cURL
/// command to the clipboard. Surfaces failures (no HTTP block /
/// empty URL / clipboard down / ref resolution failed) as status
/// messages.
pub fn copy_as_curl(app: &mut App) {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => {
            app.set_status(StatusKind::Info, "place the cursor on an HTTP block first");
            return;
        }
    };
    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => {
            app.set_status(StatusKind::Info, "no block at cursor");
            return;
        }
    };
    if !block.is_http() {
        app.set_status(
            StatusKind::Info,
            format!("`{}` blocks don't have a cURL form", block.block_type),
        );
        return;
    }
    let url_ok = block
        .params
        .get("url")
        .and_then(|v| v.as_str())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);
    if !url_ok {
        app.set_status(StatusKind::Info, "set a URL on the block before copying");
        return;
    }

    // Resolve refs the same way the run path does. Failure stays
    // soft — surface it via the status line, don't crash.
    let env_vars = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(load_active_env_vars(&app.environments_store))
    })
    .unwrap_or_default();
    let segments_snapshot: Vec<Segment> = app
        .document()
        .map(|d| d.segments().to_vec())
        .unwrap_or_default();
    let mut resolved = block.params.clone();
    if let Err(msg) =
        resolve_in_http_params(&mut resolved, &segments_snapshot, segment_idx, &env_vars)
    {
        app.set_status(StatusKind::Error, format!("ref resolution failed: {msg}"));
        return;
    }

    let payload = httui_core::blocks::http_codegen::to_curl(&resolved);
    match crate::clipboard::set_text(&payload) {
        Ok(()) => {
            app.set_status(
                StatusKind::Info,
                format!("copied as cURL ({} bytes) to clipboard", payload.len()),
            );
        }
        Err(e) => {
            app.set_status(StatusKind::Error, e);
        }
    }
}
