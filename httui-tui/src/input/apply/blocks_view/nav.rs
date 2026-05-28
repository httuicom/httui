use super::*;

/// Vertical band neighbour for `dir` (-1 up / +1 down). HTTP bands:
/// URL(0) → Request(1/2) → Response(3). DB: Connection(0) → Query(1) →
/// Result(2). `None` at the top/bottom edge.
pub(crate) fn band_neighbor(block_type: &str, region: usize, dir: isize) -> Option<usize> {
    if block_type == "http" {
        match (region, dir.signum()) {
            (0, 1) => Some(1),
            (1 | 2, -1) => Some(0),
            (1 | 2, 1) => Some(3),
            (3, -1) => Some(1),
            _ => None,
        }
    } else {
        match (region, dir.signum()) {
            (0, 1) => Some(1),
            (1, -1) => Some(0),
            (1, 1) => Some(2),
            (2, -1) => Some(1),
            _ => None,
        }
    }
}

/// Move to the band above/below, landing on its bottom row going up and
/// its top row going down (vim split-edge feel).
pub(crate) fn move_band(app: &mut App, dir: isize) {
    let Some(bt) = active_block_type(app) else {
        return;
    };
    let region = app.active_pane().map(|p| p.block_region).unwrap_or(0);
    let Some(target) = band_neighbor(&bt, region, dir) else {
        return;
    };
    if let Some(pane) = app.active_pane_mut() {
        pane.block_region = target;
        pane.block_col = 1;
    }
    let count = active_region_row_count(app);
    if let Some(pane) = app.active_pane_mut() {
        pane.block_row = if dir < 0 { count.saturating_sub(1) } else { 0 };
    }
}

/// `Tab` cycles the sub-tabs of the focused band only: HTTP Request
/// toggles Headers↔Body, Response cycles its result sub-tabs; the URL
/// band has none. DB cycles only the Result sub-tabs.
pub(crate) fn cycle_band_subtab(app: &mut App, delta: isize) {
    let Some(bt) = active_block_type(app) else {
        return;
    };
    let region = app.active_pane().map(|p| p.block_region).unwrap_or(0);
    if bt == "http" {
        match region {
            1 | 2 => {
                let next = if region == 1 { 2 } else { 1 };
                if let Some(pane) = app.active_pane_mut() {
                    pane.block_region = next;
                    pane.block_row = 0;
                    pane.block_col = 1;
                }
            }
            3 => shift_response_subtab(app, delta),
            _ => {}
        }
    } else if region == 2 {
        shift_response_subtab(app, delta);
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum EnterMode {
    /// Profile picks: standard lands in INSERT, vim in NORMAL. Used by
    /// the `Enter` chord from NAV.
    Auto,
    /// Force INSERT (used by vim `i`/`a`/`o`).
    Insert,
}

pub(crate) fn shift_row(app: &mut App, delta: isize) {
    let count = active_region_row_count(app);
    let row = app.active_pane().map(|p| p.block_row).unwrap_or(0);
    // At a region's top/bottom edge, vertical motion crosses into the
    // neighbouring band instead of clamping in place.
    if delta < 0 && row == 0 {
        move_band(app, -1);
        return;
    }
    if delta > 0 && (count <= 1 || row + 1 >= count) {
        move_band(app, 1);
        return;
    }
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let last = (count - 1) as isize;
    pane.block_row = (pane.block_row as isize + delta).clamp(0, last) as usize;
}

pub(crate) fn shift_col(app: &mut App, delta: isize) {
    let cols = active_region_col_count(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    if cols == 0 {
        pane.block_col = 0;
        return;
    }
    let last = (cols - 1) as isize;
    pane.block_col = (pane.block_col as isize + delta).clamp(0, last) as usize;
}

/// Row count of the focused region in the focused pane. Single-line
/// regions return `1` so vertical motion is a no-op clamp rather than a
/// division-by-zero / panic.
pub(crate) fn active_region_row_count(app: &App) -> usize {
    let Some(pane) = app.active_pane() else {
        return 0;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return 0;
    };
    let Some(target) = pane.block_selected else {
        return 0;
    };
    let Some(file) = ws.index.files.get(target.file_idx) else {
        return 0;
    };
    let Some(block) = file.blocks.get(target.block_idx) else {
        return 0;
    };
    if block.block_type.starts_with("db") && pane.block_region == 2 {
        return db_result_row_count(pane).max(1);
    }
    if block.block_type != "http" {
        return 1;
    }
    match pane.block_region {
        1 => {
            // Headers row count comes from the draft if any, otherwise
            // the on-disk parse — the renderer reads the same source so
            // the cursor never points past a non-existent row.
            if let Some(draft) = pane.block_draft.as_ref() {
                draft.header_count().max(1)
            } else {
                read_header_count(&app.vault_path, &file.path, block.line_start).max(1)
            }
        }
        _ => 1,
    }
}

/// `Some((segment_idx, row))` when the focused pane is on `[3]
/// Result` of a DB block whose run is cached. Used to park the
/// cursor before delegating to the DOC view's row-detail handler.
pub(crate) fn focused_db_result_row(app: &App) -> Option<(usize, usize)> {
    let pane = app.active_pane()?;
    if pane.block_region != 2 {
        return None;
    }
    let ws = app.blocks_workspace.as_ref()?;
    let sel = pane.block_selected?;
    let file = ws.index.files.get(sel.file_idx)?;
    let block = file.blocks.get(sel.block_idx)?;
    if !block.block_type.starts_with("db") {
        return None;
    }
    let doc = pane.document.as_ref()?;
    for (idx, seg) in doc.segments().iter().enumerate() {
        if let crate::buffer::Segment::Block(b) = seg {
            if b.block_type == block.block_type && b.alias == block.alias {
                let total = b
                    .cached_result
                    .as_ref()
                    .and_then(|v| v.get("results"))
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|r| r.get("rows"))
                    .and_then(|r| r.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                if total == 0 {
                    return None;
                }
                let row = pane.block_row.min(total.saturating_sub(1));
                return Some((idx, row));
            }
        }
    }
    None
}

/// `Some(segment_idx)` when the focused pane is on `[3] Response` of an
/// HTTP block whose run is cached in the pane document. Mirrors
/// `focused_db_result_row` for the Enter→detail-modal path.
pub(crate) fn focused_http_response_segment(app: &App) -> Option<usize> {
    let pane = app.active_pane()?;
    if pane.block_region != 3 {
        return None;
    }
    let ws = app.blocks_workspace.as_ref()?;
    let sel = pane.block_selected?;
    let file = ws.index.files.get(sel.file_idx)?;
    let block = file.blocks.get(sel.block_idx)?;
    if block.block_type != "http" {
        return None;
    }
    let doc = pane.document.as_ref()?;
    for (idx, seg) in doc.segments().iter().enumerate() {
        if let crate::buffer::Segment::Block(b) = seg {
            if b.block_type == block.block_type && b.alias == block.alias {
                b.cached_result.as_ref()?;
                return Some(idx);
            }
        }
    }
    None
}

/// Number of result rows accessible from the focused pane's loaded
/// document. `0` when the block hasn't been run yet so up/down stays
/// pinned.
pub(crate) fn db_result_row_count(pane: &crate::pane::Pane) -> usize {
    let doc = match pane.document.as_ref() {
        Some(d) => d,
        None => return 0,
    };
    for seg in doc.segments() {
        if let crate::buffer::Segment::Block(b) = seg {
            if b.block_type.starts_with("db") {
                return b
                    .cached_result
                    .as_ref()
                    .and_then(|v| v.get("results"))
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|r| r.get("rows"))
                    .and_then(|r| r.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
            }
        }
    }
    0
}

/// Column count of the focused region. Headers have `2` (key + value).
/// Every other region is single-column.
pub(crate) fn active_region_col_count(app: &App) -> usize {
    let Some(pane) = app.active_pane() else {
        return 0;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return 0;
    };
    let Some(target) = pane.block_selected else {
        return 0;
    };
    let Some(file) = ws.index.files.get(target.file_idx) else {
        return 0;
    };
    let Some(block) = file.blocks.get(target.block_idx) else {
        return 0;
    };
    if block.block_type == "http" && pane.block_region == 1 {
        2
    } else {
        1
    }
}

pub(crate) fn read_header_count(
    vault: &std::path::Path,
    file: &std::path::Path,
    line_start: usize,
) -> usize {
    let Ok(text) = httui_core::fs::read_note(&vault.to_string_lossy(), &file.to_string_lossy())
    else {
        return 0;
    };
    let parsed = httui_core::blocks::parse_blocks(&text);
    let Some(p) = parsed.iter().find(|p| p.line_start == line_start) else {
        return 0;
    };
    p.params
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

/// `]b`/`[b` motion — flatten every block in the workspace into a
/// single list and step `delta` positions, wrapping at both ends.
pub(crate) fn shift_block(app: &mut App, delta: isize) {
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return;
    };
    let flat: Vec<crate::app::BlockRef> = ws
        .index
        .files
        .iter()
        .enumerate()
        .flat_map(|(fi, f)| {
            (0..f.blocks.len()).map(move |bi| crate::app::BlockRef {
                file_idx: fi,
                block_idx: bi,
            })
        })
        .collect();
    if flat.is_empty() {
        return;
    }
    let current = app
        .active_pane()
        .and_then(|p| p.block_selected)
        .or(ws.selected);
    let pos = current
        .and_then(|sel| flat.iter().position(|r| *r == sel))
        .unwrap_or(0) as isize;
    let len = flat.len() as isize;
    let next = ((pos + delta) % len + len) % len;
    let target = flat[next as usize];
    if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.select(target);
        if !ws.expanded.contains(&target.file_idx) {
            ws.expanded.insert(target.file_idx);
        }
        if let Some(row) = ws
            .rows()
            .iter()
            .position(|r| r.file_idx == target.file_idx && r.block_idx == Some(target.block_idx))
        {
            ws.cursor = row;
        }
    }
    if let Some(pane) = app.active_pane_mut() {
        pane.block_selected = Some(target);
        pane.block_region = 0;
        pane.block_row = 0;
        pane.block_col = 1;
    }
}

pub(crate) fn set_region(app: &mut App, index: usize) {
    let count = active_block_region_count(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    if count == 0 {
        pane.block_region = 0;
        return;
    }
    pane.block_region = index.min(count - 1);
    pane.block_row = 0;
    pane.block_col = 1;
}

pub(crate) fn active_block_region_count(app: &App) -> usize {
    let Some(pane) = app.active_pane() else {
        return 0;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return 0;
    };
    let Some(target) = pane.block_selected else {
        return 0;
    };
    ws.index
        .files
        .get(target.file_idx)
        .and_then(|f| f.blocks.get(target.block_idx))
        .map(|b| crate::app::region_count_for(&b.block_type))
        .unwrap_or(0)
}

pub(crate) fn active_block_type(app: &App) -> Option<String> {
    let pane = app.active_pane()?;
    let target = pane.block_selected?;
    let ws = app.blocks_workspace.as_ref()?;
    ws.index
        .files
        .get(target.file_idx)
        .and_then(|f| f.blocks.get(target.block_idx))
        .map(|b| b.block_type.clone())
}

/// Numeric jump targets the entry region of each band. HTTP has three
/// bands — `1`→URL, `2`→Request (Headers/Body sub-tabs), `3`→Response;
/// DB — `1`→Connection, `2`→Query, `3`→Result.
pub(crate) fn jump_target_region(block_type: &str, n: usize) -> usize {
    let bands: &[usize] = if block_type == "http" {
        &[0, 1, 3]
    } else {
        &[0, 1, 2]
    };
    let idx = n.saturating_sub(1).min(bands.len() - 1);
    bands[idx]
}
