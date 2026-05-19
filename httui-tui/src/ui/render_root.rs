//! Top-level frame composition: vertical layout (tabs / body /
//! status), pane-tree dispatch, mode-specific cursor placement, and
//! the modal/popup overlay stack painted last.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::Clear,
    Frame,
};

use crate::app::App;
use crate::pane::PaneNode;
use crate::vim::mode::Mode;

use super::{
    anchor, block_history, block_template_picker, completion_popup, connection_picker,
    content_search, db_confirm_run, db_export_picker, db_row_detail, db_settings_modal,
    environment_picker, fence_edit, help, http_response_detail, quickopen,
    render_empty_state_inline, render_pane_tree, status, tab_picker, tabs, tree, VisualOverlay,
};

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
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
        connection_picker::render(frame, editor_area, state, anchor);
    }

    // Inline fence-metadata edit popup (alias today; limit / timeout
    // soon). Anchored above the block being edited so the user keeps
    // visual context — the previous status-bar prompt felt detached
    // from the action.
    if let Some(state) = app.fence_edit.as_ref() {
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
        fence_edit::render(frame, editor_area, state, anchor);
    }

    // SQL completion popup — paints whenever its state is `Some`,
    // independent of mode (typing in Insert keeps the popup open and
    // re-filtered). Anchored below the focused DB block; falls back
    // above or centered if no room. Painted last so it floats above
    // the editor cursor and any earlier overlays.
    if let Some(state) = app.completion_popup.as_ref() {
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
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
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
        db_export_picker::render(frame, editor_area, state, anchor);
    }

    // Settings modal — opened by `gs` over a DB block. Two-input
    // form (limit + timeout) with Tab focus cycle. Anchored above
    // the block; falls back below or centered when no headroom.
    if let Some(state) = app.db_settings.as_ref() {
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
        db_settings_modal::render(frame, editor_area, state, anchor);
    }

    // Block history modal — opened by `gh` over an HTTP block.
    // Read-only listing of the last N runs; same chrome as the
    // connection picker but wider (the timestamp column needs room).
    if let Some(state) = app.block_history.as_ref() {
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
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
