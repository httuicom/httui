//! Editor render pipeline: layout → clip to viewport → dispatch
//! per-segment renderer → cursor → status bar.

mod block_history;
mod block_template_picker;
mod blocks;
mod completion_popup;
mod connection_picker;
mod content_search;
mod cursor;
mod db_confirm_run;
mod db_export_picker;
pub mod db_row_detail;
mod db_settings_modal;
mod environment_picker;
mod fence_edit;
mod help;
pub mod http_response_detail;
mod prose;
mod quickopen;
mod sql_highlight;
mod status;
mod tab_picker;
mod tabs;
mod tree;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use ropey::Rope;

use crate::app::App;
use crate::buffer::{
    layout::{layout_document, SegmentLayout},
    Cursor, Document, Segment,
};
use crate::pane::{PaneNode, SplitDir};
use crate::vim::mode::Mode;
use crate::vim::search;

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Wipe every cell from the previous frame. Without this, ratatui's
    // double-buffering keeps stale glyphs in cells that the new frame's
    // widgets don't explicitly write to — visible as ghost characters
    // along the right edge when prose lines shrink between frames, and
    // as bleed-through under modal popovers.
    frame.render_widget(Clear, area);

    // Vertical layout: optional tab bar (1 row) → body → status (1 row).
    let show_tabs = app.tabs.len() > 1;
    let constraints: &[Constraint] = if show_tabs {
        &[
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ]
    } else {
        &[Constraint::Min(1), Constraint::Length(1)]
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints.to_vec())
        .split(area);
    let (tab_area, body_area, status_area) = if show_tabs {
        (Some(chunks[0]), chunks[1], chunks[2])
    } else {
        (None, chunks[0], chunks[1])
    };

    if let Some(ta) = tab_area {
        tabs::render(frame, ta, &app.tabs);
    }

    // Split the body horizontally when the tree is visible.
    let (sidebar_area, editor_area) = if app.tree.visible {
        let sidebar_w = tree::width().min(body_area.width.saturating_sub(20));
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(sidebar_w), Constraint::Min(1)])
            .split(body_area);
        (Some(split[0]), split[1])
    } else {
        (None, body_area)
    };

    // Highlight matches of the last executed search across visible prose.
    // Live editing of the search buffer (while in `Mode::Search`) also
    // highlights, so users see what their query is hitting before pressing
    // Enter — incremental search affordance.
    let live_search = app.vim.mode == Mode::Search && !app.vim.search_buf.is_empty();
    let search_pattern: Option<String> = if live_search {
        Some(app.vim.search_buf.as_str().to_string())
    } else if app.vim.search_highlight {
        app.vim.last_search.clone()
    } else {
        // `:noh` was issued and no new search has happened since.
        None
    };

    // The cursor is hidden whenever the user's keystrokes are flowing
    // somewhere other than the buffer (status-bar prompt, modal, tree
    // sidebar). The renderer still paints every pane; only the
    // focused leaf's cursor differs.
    // The DB row-detail modal owns input independently of `mode` —
    // it can be visible while `mode == Visual` (visual selection
    // inside the modal). Suppress the editor's cursor whenever the
    // modal is up, regardless of which transient mode the user is
    // navigating with.
    let suppress_cursor = matches!(
        app.vim.mode,
        Mode::CommandLine
            | Mode::Search
            | Mode::QuickOpen
            | Mode::Tree
            | Mode::TreePrompt
            | Mode::DbRowDetail
            | Mode::HttpResponseDetail
            | Mode::ConnectionPicker
            | Mode::ContentSearch
            | Mode::EnvironmentPicker
            | Mode::Help
            | Mode::BlockTemplatePicker
            | Mode::TabPicker
    ) || app.db_row_detail.is_some()
        || app.http_response_detail.is_some()
        || app.connection_picker.is_some()
        || app.content_search.is_some()
        || app.environment_picker.is_some()
        || app.help_visible
        || app.block_template_picker.is_some()
        || app.tab_picker.is_some();

    // Snapshot the current result-panel tab so the render tree can
    // pass it down without re-borrowing `app` at every level.
    let result_tab_global = app.db_result_tab;

    // Capture the visual-selection overlay (only painted on the focused
    // leaf — the moving end of the selection is the cursor, the anchor
    // lives on `VimState`).
    let visual_overlay = match (app.vim.mode, app.vim.visual_anchor) {
        (Mode::Visual, Some(anchor)) => Some(VisualOverlay {
            anchor,
            linewise: false,
        }),
        (Mode::VisualLine, Some(anchor)) => Some(VisualOverlay {
            anchor,
            linewise: true,
        }),
        _ => None,
    };

    // Walk the active tab's pane tree, painting each leaf in its slice
    // and setting its `viewport_height` for the next dispatch tick.
    let vault = app.vault_path.clone();
    // Snapshot the connection names map so the renderer can flip
    // UUIDs to human labels without holding a borrow on `app`.
    let connection_names = app.connection_names.clone();
    // Split-borrow: pull the active tab and the result-viewport-top
    // map mutably from disjoint `App` fields. Both need to be `&mut`
    // for the render path (panes update their viewport_height; the
    // result table writes back its scroll offset).
    let active_idx = app.tabs.active;
    let result_viewport_top = &mut app.result_viewport_top;
    if let Some(tab) = app.tabs.tabs.get_mut(active_idx) {
        let focused = tab.focused.clone();
        if matches!(tab.root, PaneNode::Leaf(ref p) if p.document.is_none())
            && tab.leaf_count() == 1
        {
            // Single empty leaf — keep the friendly "vault has no
            // files" placeholder.
            render_empty_state_inline(frame, editor_area, &vault);
            if let PaneNode::Leaf(ref mut p) = tab.root {
                p.viewport_height = editor_area.height;
            }
        } else {
            render_pane_tree(
                frame,
                editor_area,
                &mut tab.root,
                Some(focused.as_slice()),
                suppress_cursor,
                search_pattern.as_deref(),
                visual_overlay,
                &connection_names,
                result_viewport_top,
                result_tab_global,
            );
        }
    } else {
        render_empty_state_inline(frame, editor_area, &vault);
    }

    let tree_focused = matches!(app.vim.mode, Mode::Tree | Mode::TreePrompt);
    if let Some(sa) = sidebar_area {
        tree::render(frame, sa, &app.tree, tree_focused);
    }
    status::render_status_bar(frame, status_area, app);

    // Mode-specific terminal-cursor placement (the editor cursor was
    // already drawn by `render_pane_tree` for non-suppressing modes).
    match app.vim.mode {
        Mode::CommandLine | Mode::Search => {
            let cursor_chars = if app.vim.mode == Mode::CommandLine {
                app.vim.cmdline.cursor_col()
            } else {
                app.vim.search_buf.cursor_col()
            };
            let col = (cursor_chars as u16).saturating_add(1);
            let x = status_area.x + col.min(status_area.width.saturating_sub(1));
            frame.set_cursor_position((x, status_area.y));
        }
        Mode::QuickOpen => {
            let (cx, cy) = quickopen::render(frame, editor_area, &app.vim.quickopen);
            frame.set_cursor_position((cx, cy));
        }
        Mode::ContentSearch => {
            if let Some(state) = app.content_search.as_ref() {
                let (cx, cy) = content_search::render(frame, editor_area, state);
                frame.set_cursor_position((cx, cy));
            }
        }
        Mode::TreePrompt => {
            if let Some(prompt) = app.tree.prompt.as_ref() {
                let label_len = match &prompt.kind {
                    crate::tree::TreePromptKind::Create { dir } => {
                        if dir.is_empty() {
                            "new file: ".chars().count()
                        } else {
                            format!("new file in {dir}/: ").chars().count()
                        }
                    }
                    crate::tree::TreePromptKind::Rename { from } => {
                        format!("rename {from} → ").chars().count()
                    }
                    crate::tree::TreePromptKind::Delete { target } => {
                        format!("delete {target}? (y/N) ").chars().count()
                    }
                };
                let col = (label_len + prompt.cursor_col()) as u16;
                let x = status_area
                    .x
                    .saturating_add(col.min(status_area.width.saturating_sub(1)));
                frame.set_cursor_position((x, status_area.y));
            }
        }
        _ => {}
    }
    // Modal is independent of mode — it stays painted while the
    // user is in `Mode::DbRowDetail` *and* while a transient mode
    // (Visual, VisualLine) is active over the modal's body.
    if let Some(state) = app.db_row_detail.as_mut() {
        let visual = match (app.vim.mode, app.vim.visual_anchor) {
            (Mode::Visual, Some(anchor)) => Some(VisualOverlay {
                anchor,
                linewise: false,
            }),
            (Mode::VisualLine, Some(anchor)) => Some(VisualOverlay {
                anchor,
                linewise: true,
            }),
            _ => None,
        };
        db_row_detail::render(frame, editor_area, state, visual);
    }
    // HTTP response-detail modal — same paint-while-state-is-Some
    // rule as the DB row-detail modal so visual mode keeps the modal
    // up.
    if let Some(state) = app.http_response_detail.as_mut() {
        let visual = match (app.vim.mode, app.vim.visual_anchor) {
            (Mode::Visual, Some(anchor)) => Some(VisualOverlay {
                anchor,
                linewise: false,
            }),
            (Mode::VisualLine, Some(anchor)) => Some(VisualOverlay {
                anchor,
                linewise: true,
            }),
            _ => None,
        };
        http_response_detail::render(frame, editor_area, state, visual);
    }

    // Connection picker popup — same independence-from-mode rule as
    // the row-detail modal: paints whenever its state is `Some`.
    // Compute the focused block's screen rect so the popup can
    // anchor right above it (or below if there's no headroom).
    if let Some(state) = app.connection_picker.as_ref() {
        let anchor = compute_block_anchor(app, editor_area, state.segment_idx);
        connection_picker::render(frame, editor_area, state, anchor);
    }

    // Inline fence-metadata edit popup (alias today; limit / timeout
    // soon). Anchored above the block being edited so the user keeps
    // visual context — the previous status-bar prompt felt detached
    // from the action.
    if let Some(state) = app.fence_edit.as_ref() {
        let anchor = compute_block_anchor(app, editor_area, state.segment_idx);
        fence_edit::render(frame, editor_area, state, anchor);
    }

    // SQL completion popup — paints whenever its state is `Some`,
    // independent of mode (typing in Insert keeps the popup open and
    // re-filtered). Anchored below the focused DB block; falls back
    // above or centered if no room. Painted last so it floats above
    // the editor cursor and any earlier overlays.
    if let Some(state) = app.completion_popup.as_ref() {
        let anchor = compute_block_anchor(app, editor_area, state.segment_idx);
        completion_popup::render(frame, editor_area, state, anchor);
    }

    // Run-confirm modal — painted last so it floats over everything
    // else (including a stuck completion popup, though both being up
    // simultaneously shouldn't happen in practice).
    if let Some(state) = app.db_confirm_run.as_ref() {
        db_confirm_run::render(frame, editor_area, state);
    }

    // Export-format picker — opened by `gx` over a DB block with
    // rows. Same chrome as the connection picker; anchored above
    // the block when there's headroom.
    if let Some(state) = app.db_export_picker.as_ref() {
        let anchor = compute_block_anchor(app, editor_area, state.segment_idx);
        db_export_picker::render(frame, editor_area, state, anchor);
    }

    // Settings modal — opened by `gs` over a DB block. Two-input
    // form (limit + timeout) with Tab focus cycle. Anchored above
    // the block; falls back below or centered when no headroom.
    if let Some(state) = app.db_settings.as_ref() {
        let anchor = compute_block_anchor(app, editor_area, state.segment_idx);
        db_settings_modal::render(frame, editor_area, state, anchor);
    }

    // Block history modal — opened by `gh` over an HTTP block.
    // Read-only listing of the last N runs; same chrome as the
    // connection picker but wider (the timestamp column needs room).
    if let Some(state) = app.block_history.as_ref() {
        let anchor = compute_block_anchor(app, editor_area, state.segment_idx);
        block_history::render(frame, editor_area, state, anchor);
    }

    // Environment picker — opened by `gE`. Centered popup (no
    // anchor: envs are global state), magenta border to match the
    // status-bar chip.
    if let Some(state) = app.environment_picker.as_ref() {
        environment_picker::render(frame, editor_area, state);
    }

    // Help modal — opened by `g?`. Stateless overlay listing the
    // chord vocabulary grouped by section. Painted last so it
    // floats above any other modal that might still be on screen.
    if app.help_visible {
        help::render(frame, editor_area);
    }

    // Block-template picker — opened by `gN`. Centered popup with
    // a fixed list of fence templates; confirm splices the picked
    // template into the prose at the cursor and re-parses.
    if let Some(state) = app.block_template_picker.as_ref() {
        block_template_picker::render(frame, editor_area, state);
    }

    // Tab picker — opened by `gb`. Lists every open tab by its
    // focused-leaf path; current tab marked with `●`, dirty tabs
    // get a trailing `*`. Painted last so it floats above any
    // other modal (none of the others share the gb chord).
    if let Some(state) = app.tab_picker.as_ref() {
        tab_picker::render(frame, editor_area, state, app.tabs.active);
    }
}

/// Locate `segment_idx` in the active pane's layout and translate
/// to screen coordinates (subtract `pane.viewport_top`). Returns
/// `None` when the pane has no document, the segment isn't in the
/// layout, or it's entirely scrolled off-screen — caller falls
/// back to a centered popup.
fn compute_block_anchor(app: &App, editor_area: Rect, segment_idx: usize) -> Option<BlockAnchor> {
    let pane = app.active_pane()?;
    let doc = pane.document.as_ref()?;
    let layouts = layout_document(doc, editor_area.width);
    let layout = layouts.iter().find(|l| l.segment_idx == segment_idx)?;
    let viewport_top = pane.viewport_top;
    let block_bottom = layout.y_start.saturating_add(layout.height);
    if block_bottom <= viewport_top {
        return None;
    }
    let screen_top = editor_area
        .y
        .saturating_add(layout.y_start.saturating_sub(viewport_top));
    let visible_height = layout
        .height
        .saturating_sub(viewport_top.saturating_sub(layout.y_start))
        .min(
            editor_area
                .height
                .saturating_sub(screen_top.saturating_sub(editor_area.y)),
        );
    if visible_height == 0 {
        return None;
    }
    Some(BlockAnchor {
        screen_top,
        height: visible_height,
    })
}

/// Screen-coordinate rect of a focused block — used by the
/// connection picker popup to anchor itself.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockAnchor {
    pub screen_top: u16,
    pub height: u16,
}

/// Recursively paint a pane tree into `area`. Each leaf's
/// `viewport_height` is updated to match the rect it was given so
/// motions like `Ctrl+D` know how far to jump on the next tick.
///
/// `focused_path` is the path from the current node to the focused
/// leaf, or `None` if focus lives in another subtree. When the path is
/// `Some([])` the current node *is* the focused leaf.
/// Visual-mode selection state passed down through the pane tree.
/// Only painted on the *focused* leaf (the cursor is the moving end).
/// Re-used by the row-detail modal to paint its own selection
/// overlay when the modal is up + visual mode is active.
#[derive(Debug, Clone, Copy)]
pub(crate) struct VisualOverlay {
    pub anchor: Cursor,
    pub linewise: bool,
}

#[allow(clippy::too_many_arguments)] // bundle into RenderContext if it grows further.
fn render_pane_tree(
    frame: &mut Frame,
    area: Rect,
    node: &mut PaneNode,
    focused_path: Option<&[u8]>,
    suppress_cursor: bool,
    search_pattern: Option<&str>,
    visual_overlay: Option<VisualOverlay>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    match node {
        PaneNode::Leaf(pane) => {
            pane.viewport_height = area.height;
            let is_focused = matches!(focused_path, Some(p) if p.is_empty());
            match pane.document.as_ref() {
                Some(doc) => {
                    if is_focused && !suppress_cursor {
                        render_document(
                            frame,
                            area,
                            doc,
                            pane.viewport_top,
                            search_pattern,
                            connection_names,
                            result_viewport_top,
                            result_tab,
                        );
                    } else {
                        render_document_no_cursor(
                            frame,
                            area,
                            doc,
                            pane.viewport_top,
                            search_pattern,
                            connection_names,
                            result_viewport_top,
                            result_tab,
                        );
                    }
                    // Selection highlight only on the focused leaf —
                    // visual mode is single-pane.
                    if is_focused {
                        if let Some(overlay) = visual_overlay {
                            overlay_visual_selection(frame, area, doc, pane.viewport_top, overlay);
                        }
                    }
                }
                None => {
                    // Empty pane — leave the area blank. A future iteration
                    // could surface a `(no buffer)` hint here.
                }
            }
        }
        PaneNode::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (rect_a, rect_b, sep_rect) = split_rect(area, *direction, *ratio);
            draw_separator(frame, sep_rect, *direction);
            let (path_first, path_second) = match focused_path {
                Some(p) if !p.is_empty() => {
                    let head = p[0];
                    let rest = &p[1..];
                    if head == 0 {
                        (Some(rest), None)
                    } else {
                        (None, Some(rest))
                    }
                }
                _ => (None, None),
            };
            render_pane_tree(
                frame,
                rect_a,
                first,
                path_first,
                suppress_cursor,
                search_pattern,
                visual_overlay,
                connection_names,
                result_viewport_top,
                result_tab,
            );
            render_pane_tree(
                frame,
                rect_b,
                second,
                path_second,
                suppress_cursor,
                search_pattern,
                visual_overlay,
                connection_names,
                result_viewport_top,
                result_tab,
            );
        }
    }
}

/// Paint a bg highlight under the active visual selection. Charwise:
/// inclusive char range `[min, max]`. Linewise: every cell of every
/// line from `min(line)` to `max(line)`. Cross-segment selections are
/// skipped (they're refused by the operator engine too).
fn overlay_visual_selection(
    frame: &mut Frame,
    area: Rect,
    doc: &Document,
    viewport_top: u16,
    overlay: VisualOverlay,
) {
    let (a_seg, a_off) = match overlay.anchor {
        Cursor::InProse {
            segment_idx,
            offset,
        } => (segment_idx, offset),
        Cursor::InBlock {
            segment_idx,
            offset,
        } => (segment_idx, offset),
        Cursor::InBlockResult { .. } => return,
    };
    let (c_seg, c_off) = match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => (segment_idx, offset),
        Cursor::InBlock {
            segment_idx,
            offset,
        } => (segment_idx, offset),
        Cursor::InBlockResult { .. } => return,
    };
    // Establish lo / hi by (segment, offset) so the highlight
    // sweeps in document order regardless of which end is the
    // anchor and which is the moving cursor.
    let (lo_seg, lo_off, hi_seg, hi_off) = if (a_seg, a_off) <= (c_seg, c_off) {
        (a_seg, a_off, c_seg, c_off)
    } else {
        (c_seg, c_off, a_seg, a_off)
    };

    let layouts = layout_document(doc, area.width);
    let style = Style::default().bg(Color::Rgb(60, 70, 110));

    for seg_idx in lo_seg..=hi_seg {
        let Some(seg) = doc.segments().get(seg_idx) else {
            break;
        };
        let layout = match layouts.iter().find(|l| l.segment_idx == seg_idx) {
            Some(l) => *l,
            None => continue,
        };
        // Synthesize an owned rope for blocks (their raw rope is
        // their on-screen content); prose segments hand us their
        // rope directly.
        let rope_owned: ropey::Rope;
        let rope: &ropey::Rope = match seg {
            Segment::Prose(r) => r,
            Segment::Block(b) => {
                rope_owned = b.raw.clone();
                &rope_owned
            }
        };
        let total = rope.len_chars();
        // What slice of this segment is selected?
        let seg_lo_off = if seg_idx == lo_seg {
            lo_off.min(total)
        } else {
            0
        };
        let seg_hi_off = if seg_idx == hi_seg {
            hi_off.min(total)
        } else {
            total
        };
        if seg_hi_off < seg_lo_off {
            continue;
        }

        let (start_line, start_col, end_line, end_col_inclusive) = if overlay.linewise {
            let lo_line = rope.char_to_line(seg_lo_off);
            let hi_line = if total == 0 {
                0
            } else {
                rope.char_to_line(seg_hi_off.saturating_sub(0).min(total))
            };
            (lo_line, 0usize, hi_line, usize::MAX)
        } else {
            let lo_line = rope.char_to_line(seg_lo_off);
            let lo_col = seg_lo_off - rope.line_to_char(lo_line);
            let hi_line = if total == 0 {
                0
            } else {
                rope.char_to_line(seg_hi_off.min(total))
            };
            let hi_col = seg_hi_off.saturating_sub(rope.line_to_char(hi_line));
            (lo_line, lo_col, hi_line, hi_col)
        };

        // Map raw line index → screen Y. For prose segments lines
        // are contiguous (rope_line N → y_start + N). Block segments
        // have chrome (top border + header bar) above the fence
        // header and a result panel between the body and the
        // closer, so the mapping is non-linear.
        let line_to_y: Box<dyn Fn(usize) -> u16> = match seg {
            Segment::Prose(_) => {
                let y_start = layout.y_start;
                Box::new(move |line: usize| y_start.saturating_add(line as u16))
            }
            Segment::Block(_) => {
                let y_start = layout.y_start;
                let height = layout.height;
                let last_raw = rope.len_lines().saturating_sub(1);
                Box::new(move |line: usize| {
                    if line == 0 {
                        // Fence header sits just inside the top
                        // border + chrome header bar.
                        y_start.saturating_add(2)
                    } else if line >= last_raw {
                        // Closer sits one row above the chrome
                        // footer bar and bottom border.
                        y_start.saturating_add(height.saturating_sub(3))
                    } else {
                        // Body line N (raw line N): right after
                        // the fence header.
                        y_start.saturating_add(2).saturating_add(line as u16)
                    }
                })
            }
        };

        paint_segment_highlight(
            frame,
            area,
            viewport_top,
            line_to_y.as_ref(),
            rope,
            start_line,
            start_col,
            end_line,
            end_col_inclusive,
            overlay.linewise,
            style,
            // Charwise selection is "inclusive at both ends" only
            // when this segment owns the hi endpoint; mid-segment
            // highlight paints the whole line.
            seg_idx == hi_seg,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_segment_highlight(
    frame: &mut Frame,
    area: Rect,
    viewport_top: u16,
    line_to_y: &dyn Fn(usize) -> u16,
    rope: &ropey::Rope,
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col_inclusive: usize,
    linewise: bool,
    style: Style,
    inclusive_hi: bool,
) {
    let buf = frame.buffer_mut();
    let total_lines = rope.len_lines();
    for line in start_line..=end_line {
        if line >= total_lines {
            break;
        }
        let absolute_y = line_to_y(line);
        if absolute_y < viewport_top {
            continue;
        }
        let y = absolute_y - viewport_top;
        if y >= area.height {
            break;
        }
        let line_text = rope.line(line).to_string();
        let line_chars = line_text.trim_end_matches('\n').chars().count();
        let from = if line == start_line { start_col } else { 0 };
        let to = if linewise {
            area.width as usize
        } else if line == end_line && inclusive_hi {
            (end_col_inclusive + 1).min(line_chars.max(1))
        } else {
            line_chars.max(1)
        };
        if to <= from {
            continue;
        }
        let max_x = area.x.saturating_add(area.width);
        for col in from..to {
            let x = area.x.saturating_add(col as u16);
            if x >= max_x {
                break;
            }
            let cell = &mut buf[(x, area.y + y)];
            cell.set_style(style);
        }
    }
}

/// Carve `area` into two child rects plus a 1-cell separator strip.
/// The `ratio` is clamped so neither child gets less than one row /
/// column; the separator is dropped when `area` is too small to fit
/// it.
fn split_rect(area: Rect, dir: SplitDir, ratio: f32) -> (Rect, Rect, Rect) {
    let ratio = ratio.clamp(0.1, 0.9);
    match dir {
        SplitDir::Vertical => {
            let total = area.width;
            let sep_w = if total >= 3 { 1 } else { 0 };
            let usable = total.saturating_sub(sep_w);
            let mut first_w = (usable as f32 * ratio).round() as u16;
            first_w = first_w.clamp(1, usable.saturating_sub(1).max(1));
            let second_w = usable.saturating_sub(first_w);
            let a = Rect {
                x: area.x,
                y: area.y,
                width: first_w,
                height: area.height,
            };
            let sep = Rect {
                x: area.x.saturating_add(first_w),
                y: area.y,
                width: sep_w,
                height: area.height,
            };
            let b = Rect {
                x: area.x.saturating_add(first_w + sep_w),
                y: area.y,
                width: second_w,
                height: area.height,
            };
            (a, b, sep)
        }
        SplitDir::Horizontal => {
            let total = area.height;
            let sep_h = if total >= 3 { 1 } else { 0 };
            let usable = total.saturating_sub(sep_h);
            let mut first_h = (usable as f32 * ratio).round() as u16;
            first_h = first_h.clamp(1, usable.saturating_sub(1).max(1));
            let second_h = usable.saturating_sub(first_h);
            let a = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: first_h,
            };
            let sep = Rect {
                x: area.x,
                y: area.y.saturating_add(first_h),
                width: area.width,
                height: sep_h,
            };
            let b = Rect {
                x: area.x,
                y: area.y.saturating_add(first_h + sep_h),
                width: area.width,
                height: second_h,
            };
            (a, b, sep)
        }
    }
}

fn draw_separator(frame: &mut Frame, area: Rect, dir: SplitDir) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let style = Style::default().fg(Color::DarkGray);
    let glyph = match dir {
        SplitDir::Vertical => "│",
        SplitDir::Horizontal => "─",
    };
    let buf = frame.buffer_mut();
    for y in 0..area.height {
        for x in 0..area.width {
            let cell = &mut buf[(area.x + x, area.y + y)];
            cell.set_symbol(glyph);
            cell.set_style(style);
        }
    }
}

fn render_empty_state_inline(frame: &mut Frame, area: Rect, vault: &std::path::Path) {
    let vault = vault.to_string_lossy().into_owned();
    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "This vault has no markdown files yet.",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("vault: {vault}"),
            Style::default().add_modifier(Modifier::DIM),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Create a note from the file tree (Ctrl+E, then `a`) or open one.",
            Style::default().add_modifier(Modifier::DIM),
        )),
    ];
    let block = Block::default().borders(Borders::ALL).title("notes-tui");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

#[allow(clippy::too_many_arguments)]
fn render_document_no_cursor(
    frame: &mut Frame,
    area: Rect,
    doc: &Document,
    viewport_top: u16,
    search_pattern: Option<&str>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    // Same logic as `render_document`, but skip the cursor draw step.
    // Used while the prompt is open so the terminal caret isn't fighting
    // for position with the editor.
    let layouts = layout_document(doc, area.width);
    let viewport_bottom = viewport_top.saturating_add(area.height);
    for layout in &layouts {
        if layout.y_start.saturating_add(layout.height) <= viewport_top {
            continue;
        }
        if layout.y_start >= viewport_bottom {
            break;
        }
        render_segment_no_cursor(
            frame,
            area,
            doc,
            layout,
            viewport_top,
            search_pattern,
            connection_names,
            result_viewport_top,
            result_tab,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_segment_no_cursor(
    frame: &mut Frame,
    editor_area: Rect,
    doc: &Document,
    layout: &SegmentLayout,
    viewport_top: u16,
    search_pattern: Option<&str>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    let seg = match doc.segments().get(layout.segment_idx) {
        Some(s) => s,
        None => return,
    };
    let top_skip = viewport_top.saturating_sub(layout.y_start);
    let visible_height = layout.height.saturating_sub(top_skip).min(
        editor_area
            .height
            .saturating_sub(layout.y_start.saturating_sub(viewport_top)),
    );
    if visible_height == 0 {
        return;
    }
    let y_in_editor = layout.y_start.saturating_sub(viewport_top);
    let area = Rect {
        x: editor_area.x,
        y: editor_area.y + y_in_editor,
        width: editor_area.width,
        height: visible_height,
    };
    match seg {
        Segment::Prose(rope) => {
            prose::render_prose(frame, area, rope, top_skip as usize);
            if let Some(pattern) = search_pattern {
                overlay_search_highlights(frame, area, rope, top_skip as usize, pattern);
            }
        }
        Segment::Block(b) => {
            // Modal-cursor / prompt mode → no selected row, but the
            // result table can still own a stored viewport_top from
            // a previous focus. Pass it through so the renderer keeps
            // the scroll position stable.
            let viewport_slot = result_viewport_top.get_mut(&layout.segment_idx);
            blocks::render_block_with_selection(
                frame,
                area,
                b,
                false,
                None,
                viewport_slot,
                connection_names,
                result_tab,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn render_document(
    frame: &mut Frame,
    area: Rect,
    doc: &Document,
    viewport_top: u16,
    search_pattern: Option<&str>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    let layouts = layout_document(doc, area.width);
    let cursor = doc.cursor();
    let viewport_bottom = viewport_top.saturating_add(area.height);

    for layout in &layouts {
        if layout.y_start.saturating_add(layout.height) <= viewport_top {
            continue;
        }
        if layout.y_start >= viewport_bottom {
            break;
        }
        render_segment(
            frame,
            area,
            doc,
            layout,
            cursor,
            viewport_top,
            search_pattern,
            connection_names,
            result_viewport_top,
            result_tab,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn render_segment(
    frame: &mut Frame,
    editor_area: Rect,
    doc: &Document,
    layout: &SegmentLayout,
    cursor: Cursor,
    viewport_top: u16,
    search_pattern: Option<&str>,
    connection_names: &blocks::ConnectionNames,
    result_viewport_top: &mut std::collections::HashMap<usize, u16>,
    result_tab: crate::app::ResultPanelTab,
) {
    let seg = match doc.segments().get(layout.segment_idx) {
        Some(s) => s,
        None => return,
    };

    // Viewport clipping. `top_skip` is how many rows of this segment
    // are above the viewport top — they'll be drawn off-screen.
    let top_skip = viewport_top.saturating_sub(layout.y_start);
    let visible_height = layout.height.saturating_sub(top_skip).min(
        editor_area
            .height
            .saturating_sub(layout.y_start.saturating_sub(viewport_top)),
    );
    if visible_height == 0 {
        return;
    }
    let y_in_editor = layout.y_start.saturating_sub(viewport_top);
    let area = Rect {
        x: editor_area.x,
        y: editor_area.y + y_in_editor,
        width: editor_area.width,
        height: visible_height,
    };

    match seg {
        Segment::Prose(rope) => {
            prose::render_prose(frame, area, rope, top_skip as usize);
            if let Some(pattern) = search_pattern {
                overlay_search_highlights(frame, area, rope, top_skip as usize, pattern);
            }
            if let Cursor::InProse { segment_idx, .. } = cursor {
                if segment_idx == layout.segment_idx {
                    cursor::render_prose_cursor(frame, area, rope, cursor, top_skip as usize);
                }
            }
        }
        Segment::Block(b) => {
            // The block is "focused" whenever the cursor lives inside
            // it — drives the border highlight and tells the cursor
            // renderer where to park the terminal caret.
            let in_block = matches!(
                cursor,
                Cursor::InBlock { segment_idx, .. } if segment_idx == layout.segment_idx
            );
            let in_result = matches!(
                cursor,
                Cursor::InBlockResult { segment_idx, .. } if segment_idx == layout.segment_idx
            );
            let focused = in_block || in_result;
            let selected_row = match cursor {
                Cursor::InBlockResult { segment_idx, row } if segment_idx == layout.segment_idx => {
                    Some(row)
                }
                _ => None,
            };
            // Block widgets ignore `top_skip` — they always render
            // their full chrome; if partly off-screen the terminal
            // clips for us.
            //
            // For DB result blocks: hand the renderer a mut slot in
            // `result_viewport_top` so the table's scroll persists
            // across frames (cursor floats inside the visible window
            // — same feel as the editor pane scroll).
            let viewport_slot: Option<&mut u16> = if in_result {
                Some(result_viewport_top.entry(layout.segment_idx).or_insert(0))
            } else {
                result_viewport_top.get_mut(&layout.segment_idx)
            };
            blocks::render_block_with_selection(
                frame,
                area,
                b,
                focused,
                selected_row,
                viewport_slot,
                connection_names,
                result_tab,
            );
            if in_block {
                if let Cursor::InBlock { offset, .. } = cursor {
                    use crate::buffer::block::{raw_section_at, RawSection};
                    // New card layout: top border → header bar →
                    // fence header → body → fence closer → footer
                    // bar → bottom border. Fence header sits at
                    // `area.y + 2` (one past the chrome header bar);
                    // body at `area.y + 3..`; closer at
                    // `area.y + area.height - 3` (one above the
                    // chrome footer bar).
                    let raw = &b.raw;
                    let line_idx = raw.char_to_line(offset.min(raw.len_chars()));
                    let line_start = raw.line_to_char(line_idx);
                    let col = offset.saturating_sub(line_start);
                    let max_x = area.x.saturating_add(area.width.saturating_sub(2));
                    match raw_section_at(raw, offset) {
                        RawSection::Body { line, col } => {
                            cursor::render_inblock_cursor(frame, area, line, col);
                        }
                        RawSection::Header => {
                            let x = area
                                .x
                                .saturating_add(1)
                                .saturating_add(col as u16)
                                .min(max_x);
                            let y = area.y.saturating_add(2);
                            frame.set_cursor_position((x, y));
                        }
                        RawSection::Closer => {
                            let x = area
                                .x
                                .saturating_add(1)
                                .saturating_add(col as u16)
                                .min(max_x);
                            // HTTP blocks paint the closer between
                            // raw input and response panel — its row
                            // is `area.y + 3 + request_height`
                            // (border + header bar + fence header +
                            // request lines). All other block types
                            // paint the closer one row above the
                            // footer bar.
                            let y = if b.is_http() {
                                let request_height =
                                    crate::buffer::block::body_line_count(&b.raw).max(1) as u16;
                                area.y.saturating_add(3 + request_height)
                            } else {
                                area.y.saturating_add(area.height.saturating_sub(3))
                            };
                            frame.set_cursor_position((x, y));
                        }
                    }
                }
            }
        }
    }
}

/// Paint a yellow background under each match of `pattern` in the
/// visible portion of `rope`. Smartcase via [`search::is_case_sensitive`].
/// The overlay only sets the bg / fg fields, so existing markdown
/// styling (bold, italics, link colors) survives untouched in cells
/// that aren't matched.
fn overlay_search_highlights(
    frame: &mut Frame,
    area: Rect,
    rope: &Rope,
    top_line: usize,
    pattern: &str,
) {
    if pattern.is_empty() {
        return;
    }
    let case_sensitive = search::is_case_sensitive(pattern);
    let highlight = Style::default().bg(Color::Yellow).fg(Color::Black);
    let total = rope.len_lines();
    let buf = frame.buffer_mut();

    for row in 0..area.height as usize {
        let line_idx = top_line + row;
        if line_idx >= total {
            break;
        }
        let raw = rope.line(line_idx).to_string();
        let line_text = raw.trim_end_matches('\n');
        let matches = search::find_matches_in_line(line_text, pattern, case_sensitive);
        for (start, end) in matches {
            // `find_matches_in_line` returns char ranges; for ASCII
            // markdown the column is the same as the char count.
            let col_start = start as u16;
            let col_end = end as u16;
            let width = col_end.saturating_sub(col_start);
            if width == 0 {
                continue;
            }
            let x = area.x.saturating_add(col_start);
            let y = area.y.saturating_add(row as u16);
            let max_x = area.x.saturating_add(area.width);
            if x >= max_x || y >= area.y.saturating_add(area.height) {
                continue;
            }
            let visible_width = width.min(max_x - x);
            let rect = Rect {
                x,
                y,
                width: visible_width,
                height: 1,
            };
            buf.set_style(rect, highlight);
        }
    }
}
