//! HTTP/DB block history modal (`gh` chord). Read-only browser over
//! `httui-core::block_history` rows keyed by `(file_path, alias)`.

use crate::app::{App, BlockHistoryState};
use crate::buffer::{Cursor, Segment};
use crate::modal::Modal;

/// `gh` on an HTTP block — open the read-only history modal. Reads
/// `httui-core::block_history::list_history(file, alias)` synchronously
/// (the read is cheap; opening can briefly block). Validates:
///   1. cursor sits on an HTTP block
///   2. block has an alias (history rows are keyed by alias)
///   3. active doc has a file path on disk
///   4. there's at least one row to show
///
/// Each failure surfaces a status hint instead of opening an empty
/// modal — empty modals waste a keystroke.
pub fn open_block_history(app: &mut App) -> Result<(), String> {
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
    if !block.is_http() && !block.is_db() {
        return Err(format!(
            "`{}` blocks don't record runs yet",
            block.block_type
        ));
    }
    let alias = match block.alias.as_deref().filter(|s| !s.is_empty()) {
        Some(a) => a.to_string(),
        None => return Err("anonymous block has no history (give it an `alias=`)".into()),
    };
    let file_path = super::active_file_path_string(app)
        .ok_or_else(|| "save the file first — history is keyed by file path".to_string())?;

    let pool = app.pool_manager.app_pool().clone();
    let entries: Vec<httui_core::block_history::HistoryEntry> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move {
            httui_core::block_history::list_history(&pool, &file_path, &alias).await
        })
    })
    .map_err(|e| format!("history read failed: {e}"))?;

    if entries.is_empty() {
        return Err("no history yet — run the block at least once".into());
    }

    // Title format differs by block type so users see context
    // matching their mental model:
    // - HTTP: `<METHOD> <alias>` (`GET myreq`)
    // - DB:   `DB <alias>`       (`DB userlist`)
    // The driver kind is already encoded in each row's `method`
    // column so we don't repeat it in the title.
    let title = if block.is_http() {
        format!(
            "{} {}",
            block
                .params
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("GET"),
            block.alias.as_deref().unwrap_or(""),
        )
    } else {
        format!("DB {}", block.alias.as_deref().unwrap_or(""))
    };

    app.modal = Some(Modal::BlockHistory(BlockHistoryState {
        segment_idx,
        title,
        entries,
        selected: 0,
    }));
    app.vim.mode = crate::vim::mode::Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub fn close_block_history(app: &mut App) {
    if matches!(app.modal, Some(Modal::BlockHistory(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub fn move_block_history_cursor(app: &mut App, delta: i32) {
    let Some(Modal::BlockHistory(state)) = app.modal.as_mut() else {
        return;
    };
    if state.entries.is_empty() {
        return;
    }
    let last = state.entries.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}
