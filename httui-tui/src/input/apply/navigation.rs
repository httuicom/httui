//! Navigation / tree-prompt / search / DB-prefetch / block-jump /
//! scroll / write-all appliers. Mechanically moved out of
//! `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p5f) with no logic
//! change.

use crate::app::{App, StatusKind};
use crate::buffer::{Cursor, Segment};
use crate::input::action::Action;
use crate::tree::{TreePrompt, TreePromptKind};
use crate::vim::ex;
use crate::vim::search;

/// Execute the pending tree prompt against `app`. Refreshes the tree on
/// success so the sidebar shows the new state without a manual `R`.
pub(crate) fn run_tree_prompt(app: &mut App, prompt: TreePrompt) {
    let buffer = prompt.input.buffer;
    let outcome = match prompt.kind {
        TreePromptKind::Create { dir } => {
            let raw = buffer.trim();
            if raw.is_empty() {
                Err("create: name required".to_string())
            } else {
                // Trailing slash → folder; otherwise file.
                let is_folder = raw.ends_with('/') || raw.ends_with(std::path::MAIN_SEPARATOR);
                let name = raw.trim_end_matches(['/', std::path::MAIN_SEPARATOR]);
                if name.is_empty() {
                    Err("create: name required".into())
                } else {
                    let path = if dir.is_empty() {
                        std::path::PathBuf::from(name)
                    } else {
                        std::path::Path::new(&dir).join(name)
                    };
                    if is_folder {
                        app.create_folder(path)
                    } else {
                        app.create_document(path, false)
                    }
                }
            }
        }
        TreePromptKind::Rename { from } => {
            let dst = buffer.trim();
            if dst.is_empty() || dst == from {
                Err("rename: destination unchanged".to_string())
            } else {
                app.rename_path(
                    Some(std::path::PathBuf::from(&from)),
                    std::path::PathBuf::from(dst),
                )
            }
        }
        TreePromptKind::Delete { target } => {
            let answer = buffer.trim().to_lowercase();
            if answer == "y" || answer == "yes" {
                app.delete_path(Some(std::path::PathBuf::from(&target)), true)
            } else {
                Err("delete: cancelled".to_string())
            }
        }
    };
    match outcome {
        Ok(msg) => {
            let vault = app.vault_path.clone();
            app.tree.refresh(&vault);
            app.set_status(StatusKind::Info, msg);
        }
        Err(msg) => app.set_status(StatusKind::Error, msg),
    }
}

/// Recursively list every `.md` file in the vault, returning paths
/// relative to the vault root. Hidden directories and the usual
/// build-artifact dirs are filtered by `httui_core::fs::list_workspace`,
/// so we just walk what it gives us.
pub(crate) fn list_vault_md_files(vault: &str) -> Vec<String> {
    let Ok(entries) = httui_core::fs::list_workspace(vault) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_md(&entries, &mut out);
    // Stable order: alphabetic by full path. The fuzzy filter sort takes
    // over once the user types something.
    out.sort();
    out
}

pub(crate) fn collect_md(entries: &[httui_core::fs::FileEntry], out: &mut Vec<String>) {
    for e in entries {
        if e.is_dir {
            if let Some(children) = e.children.as_deref() {
                collect_md(children, out);
            }
        } else if e.name.ends_with(".md") {
            out.push(e.path.clone());
        }
    }
}

pub(crate) fn execute_search(app: &mut App, pattern: &str, forward: bool, save: bool) {
    if pattern.is_empty() {
        return;
    }
    // Any new search re-arms the highlight that `:noh` may have hidden.
    app.vim.search_highlight = true;
    let result = app
        .document()
        .and_then(|doc| search::search(doc, pattern, forward));
    match result {
        Some(cursor) => {
            if let Some(doc) = app.document_mut() {
                doc.set_cursor(cursor);
            }
            if save {
                app.vim.last_search = Some(pattern.to_string());
                app.vim.last_search_forward = forward;
            }
            app.refresh_viewport_for_cursor();
        }
        None => {
            // Still save the pattern — `n` after a missed search should
            // try the same query again rather than re-prompting.
            if save {
                app.vim.last_search = Some(pattern.to_string());
                app.vim.last_search_forward = forward;
            }
            app.set_status(
                StatusKind::Error,
                format!("E486: Pattern not found: {pattern}"),
            );
        }
    }
}

/// Distance from the bottom of the loaded result that triggers an
/// eager fetch of the next page. Half the on-screen viewport feels
/// natural — by the time the user is looking at the last visible
/// row, the next batch is usually already there.
pub(crate) const DB_PREFETCH_THRESHOLD: usize = 5;

/// Pure decision function for the infinite-scroll prefetch. Returns
/// `true` when the cursor is close enough to the bottom of the
/// currently loaded rows that we should fetch the next page.
///
/// `cursor_row` is 0-indexed, `total` is the number of rows currently
/// in the cache, and `has_more` is the backend's signal that more
/// pages are still available.
pub(crate) fn should_prefetch(
    cursor_row: usize,
    total: usize,
    has_more: bool,
    threshold: usize,
) -> bool {
    has_more && cursor_row + threshold >= total
}

/// Hook called from the motion dispatcher: when the cursor is parked
/// inside a DB result whose backend reports `has_more`, fetch the
/// next page once we're within `DB_PREFETCH_THRESHOLD` rows of the
/// loaded bottom. Mirrors the desktop's near-bottom load-more pattern
/// (`DbFencedPanel.tsx` → `ResultTable.handleScroll`).
pub(crate) fn maybe_prefetch_db_more_rows(app: &mut App) {
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
    let Some(cached) = block.cached_result.as_ref() else {
        return;
    };
    let Some(first) = cached
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
    else {
        return;
    };
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return;
    }
    let has_more = first
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let total = first
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if !should_prefetch(row, total, has_more, DB_PREFETCH_THRESHOLD) {
        return;
    }
    // While a query is already in flight, the prefetch silently
    // backs off — the user is moving the cursor around naturally
    // and we don't want to spam the status bar with "another query
    // is already running" on every motion.
    if app.running_query.is_some() {
        return;
    }
    if let Err(msg) = crate::commands::db::load_more_db_block(app, segment_idx) {
        app.set_status(StatusKind::Error, format!("load more: {msg}"));
    }
}

// ───────────── block-jump motions (g] / g[) ─────────────

#[derive(Debug, Clone, Copy)]
pub(crate) enum JumpDir {
    Next,
    Prev,
}

/// Move the cursor to the first body offset of the next or previous
/// block segment relative to the current position. No-wrap: when
/// the cursor is already past the last block (or before the first),
/// the call is a silent no-op — matches vim's `]m` / `[m` feel.
///
/// "Current position" resolves through the cursor's segment_idx.
/// Sitting *inside* a block, `g]` jumps to the next block, not the
/// current one; `g[` jumps to the previous one. Sitting in prose,
/// the same rule applies relative to the surrounding segment index.
pub(crate) fn apply_jump_block(app: &mut App, dir: JumpDir) {
    let Some(doc) = app.document() else { return };
    let cur_idx = match doc.cursor() {
        Cursor::InProse { segment_idx, .. }
        | Cursor::InBlock { segment_idx, .. }
        | Cursor::InBlockResult { segment_idx, .. } => segment_idx,
    };
    let target_idx = match dir {
        JumpDir::Next => doc
            .segments()
            .iter()
            .enumerate()
            .skip(cur_idx + 1)
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i)),
        JumpDir::Prev => doc
            .segments()
            .iter()
            .enumerate()
            .take(cur_idx)
            .rev()
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i)),
    };
    let Some(target) = target_idx else { return };
    if let Some(doc) = app.document_mut() {
        // Park the cursor on offset 0 of the block's raw rope. The
        // first character of the fence header lives there, so the
        // user lands on the block "from above" (predictable spot
        // they can scroll down or directly type into).
        doc.set_cursor(Cursor::InBlock {
            segment_idx: target,
            offset: 0,
        });
    }
    app.refresh_viewport_for_cursor();
}

// ───────────── rerun last block (gr) ─────────────

/// `gr` — re-execute the block recorded in `App.last_run_anchor`,
/// without requiring the cursor to be on it. Resolution rules:
///
/// 1. If `last_run_anchor` is `None` → status hint "no block has
///    been run yet this session".
/// 2. If the active document's path doesn't match the anchor's
///    file → status hint "last run was in <path>" so the user
///    knows where to switch.
/// 3. Look up the target block by alias (preferred — survives
///    edits above the block) and fall back to `segment_idx`. If
///    neither resolves to a block, status hint "anchor lost".
/// 4. Park the cursor on the resolved segment (offset 0) and
///    delegate to `apply_run_block`. The dispatch chain there
///    handles HTTP vs DB, schedules the async task, and updates
///    `last_run_anchor` again with the freshly-resolved index.
pub(crate) fn apply_rerun_last_block(app: &mut App) {
    let Some(anchor) = app.last_run_anchor.clone() else {
        app.set_status(StatusKind::Info, "no block has been run yet");
        return;
    };
    let Some(active_path) = app.active_pane().and_then(|p| p.document_path.clone()) else {
        app.set_status(StatusKind::Info, "no document open");
        return;
    };
    if active_path != anchor.file_path {
        app.set_status(
            StatusKind::Info,
            format!("last run was in {}", anchor.file_path.display()),
        );
        return;
    }
    let target_idx = {
        let Some(doc) = app.document() else { return };
        // Alias-first lookup so edits that shifted segment_idx don't
        // fire the wrong block.
        let by_alias = anchor.alias.as_deref().and_then(|a| {
            doc.segments()
                .iter()
                .enumerate()
                .find_map(|(i, s)| match s {
                    Segment::Block(b) if b.alias.as_deref() == Some(a) => Some(i),
                    _ => None,
                })
        });
        by_alias.or_else(|| {
            // Fall back to the recorded index, but only if it still
            // points at a block — otherwise the anchor is stale.
            match doc.segments().get(anchor.segment_idx) {
                Some(Segment::Block(_)) => Some(anchor.segment_idx),
                _ => None,
            }
        })
    };
    let Some(idx) = target_idx else {
        app.set_status(
            StatusKind::Info,
            "previous block no longer exists in this file",
        );
        return;
    };
    if let Some(doc) = app.document_mut() {
        doc.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: 0,
        });
    }
    app.refresh_viewport_for_cursor();
    crate::commands::db::apply_run_block(app);
}

// ───────────── scroll-positioning chords (zz / zt / zb) ─────────────

/// Re-anchor the active pane's viewport so the cursor's row lands
/// at `pos` within the visible window. Mirrors vim's `zz` / `zt` /
/// `zb`. Reuses the `cursor_y` + `layout_document` plumbing from
/// `App::refresh_viewport_for_cursor` (private there) by computing
/// the target offset directly.
pub(crate) fn apply_scroll_cursor_to(app: &mut App, pos: crate::vim::parser::ScrollPos) {
    use crate::buffer::layout::layout_document;
    use crate::vim::parser::ScrollPos;

    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let Some(doc) = pane.document.as_ref() else {
        return;
    };
    // `width = 80` matches `App::refresh_viewport_for_cursor`'s
    // sentinel — block-aware layout doesn't actually use width
    // for vertical positioning.
    let layouts = layout_document(doc, 80);
    let cursor_y = compute_cursor_y(doc, &layouts);
    let height = pane.viewport_height.max(1);
    let new_top = match pos {
        ScrollPos::Top => cursor_y,
        ScrollPos::Center => cursor_y.saturating_sub(height / 2),
        ScrollPos::Bottom => cursor_y.saturating_sub(height.saturating_sub(1)),
    };
    pane.viewport_top = new_top;
}

/// Local mirror of `app::cursor_y` (private there). Resolves the
/// document-absolute Y row of the cursor by walking the segment
/// layout. Block cursors land at the body row; result-row cursors
/// land at the result table's offset.
pub(crate) fn compute_cursor_y(
    doc: &crate::buffer::Document,
    layouts: &[crate::buffer::layout::SegmentLayout],
) -> u16 {
    use crate::buffer::block::raw_section_at;
    use crate::buffer::block::RawSection;
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let layout = layouts
                .iter()
                .find(|l| l.segment_idx == segment_idx)
                .copied();
            let line_offset = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(rope)) => {
                    rope.char_to_line(offset.min(rope.len_chars())) as u16
                }
                _ => 0,
            };
            layout
                .map(|l| l.y_start)
                .unwrap_or(0)
                .saturating_add(line_offset)
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let Some(layout) = layouts.iter().find(|l| l.segment_idx == segment_idx) else {
                return 0;
            };
            let raw = match doc.segments().get(segment_idx) {
                Some(Segment::Block(b)) => &b.raw,
                _ => return layout.y_start,
            };
            // y_start is the top border row; +2 lands on the fence
            // header, +3 onward is body. Mirror `render_segment`'s
            // mapping in `ui::mod`.
            match raw_section_at(raw, offset) {
                RawSection::Header => layout.y_start.saturating_add(2),
                RawSection::Closer => layout
                    .y_start
                    .saturating_add(layout.height.saturating_sub(3)),
                RawSection::Body { line, .. } => {
                    layout.y_start.saturating_add(3).saturating_add(line as u16)
                }
            }
        }
        Cursor::InBlockResult { segment_idx, .. } => layouts
            .iter()
            .find(|l| l.segment_idx == segment_idx)
            .map(|l| l.y_start)
            .unwrap_or(0),
    }
}

// ───────────── reselect visual (gv) ─────────────

/// `gv` — re-enter visual mode at the last-saved anchor. V1 lands
/// the cursor on the anchor itself (rather than the previous moving
/// end) so the selection collapses to a single position; the user
/// then re-extends with motions. Silent decline when there's no
/// saved selection (`last_visual` is `None`).
pub(crate) fn apply_reselect_visual(app: &mut App) {
    let Some(last) = app.vim.last_visual else {
        return;
    };
    if let Some(doc) = app.document_mut() {
        doc.set_cursor(last.anchor);
    }
    if last.linewise {
        app.vim.enter_visual_line(last.anchor);
    } else {
        app.vim.enter_visual(last.anchor);
    }
    app.refresh_viewport_for_cursor();
}

// ───────────── write-all (gW) ─────────────

/// `gW` — walk every tab, save the active leaf when it has unsaved
/// edits. Hits `:w` semantics for each one (path-required, error
/// on missing file name) but rolls them up into a single status
/// line: "N files written" / "N files written, M errored".
///
/// Strategy: capture the currently-active tab idx, then loop by
/// flipping `tabs.active` to each dirty tab and calling
/// `ex::execute(ExCmd::Write)`. The flip is cheap (just an index)
/// and lets us reuse the existing single-doc save path without
/// duplicating its keychain / dirty-bit / `mark_clean` logic.
/// Restores the original active tab at the end.
pub(crate) fn apply_write_all(app: &mut App) {
    let original_active = app.tabs.active;
    let total = app.tabs.len();
    let mut written = 0usize;
    let mut errored = 0usize;
    let mut last_err: Option<String> = None;

    for idx in 0..total {
        let dirty = app
            .tabs
            .tabs
            .get(idx)
            .and_then(|t| t.active_leaf().document.as_ref())
            .is_some_and(|d| d.is_dirty());
        if !dirty {
            continue;
        }
        // Path-less buffers (`(no name)`) can't be written. Skip
        // silently rather than counting them as errors — `gW` is a
        // bulk action; we want the user to see the successes.
        let has_path = app
            .tabs
            .tabs
            .get(idx)
            .and_then(|t| t.active_leaf().document_path.as_ref())
            .is_some();
        if !has_path {
            continue;
        }
        app.tabs.active = idx;
        match ex::execute(app, ex::ExCmd::Write) {
            ex::ExResult::Ok(_) => written += 1,
            ex::ExResult::Err(msg) => {
                errored += 1;
                last_err = Some(msg);
            }
            _ => {}
        }
    }
    app.tabs.active = original_active;

    let status_kind = if errored > 0 {
        StatusKind::Error
    } else {
        StatusKind::Info
    };
    let msg = match (written, errored) {
        (0, 0) => "no dirty buffers".to_string(),
        (n, 0) => format!("{n} files written"),
        (0, e) => format!(
            "{e} errored: {}",
            last_err.unwrap_or_else(|| "unknown".into())
        ),
        (n, e) => format!(
            "{n} files written, {e} errored: {}",
            last_err.unwrap_or_else(|| "unknown".into())
        ),
    };
    app.set_status(status_kind, msg);
}

/// `apply_action` sub-match for the block-jump / rerun / write-all /
/// reselect-visual / scroll-cursor navigation domain. Mechanically
/// split out of the `apply_action` router in `vim/dispatch.rs` (tui-v2
/// vertical 1, fase 1 p6f) — arm bodies copied verbatim. The
/// tree-prompt setup arms stay with the `misc` group (p6g) so this
/// module keeps its size budget. The outer router routes only this
/// group's variants here, so the `unreachable!` is a
/// compile-time-backed invariant.
pub(crate) fn apply_navigation(app: &mut App, action: Action, _recording: bool) {
    match action {
        Action::JumpNextBlock => apply_jump_block(app, JumpDir::Next),
        Action::JumpPrevBlock => apply_jump_block(app, JumpDir::Prev),
        Action::RerunLastBlock => apply_rerun_last_block(app),
        Action::WriteAll => apply_write_all(app),
        Action::ReselectVisual => apply_reselect_visual(app),
        Action::ScrollCursorTo(pos) => apply_scroll_cursor_to(app, pos),
        _ => unreachable!("apply_navigation: variante fora do grupo"),
    }
}
