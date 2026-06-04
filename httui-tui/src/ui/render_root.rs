//! Top-level frame composition: vertical layout (tabs / body /
//! status), pane-tree dispatch, mode-specific cursor placement, and
//! the modal/popup overlay stack painted last.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::Style,
    widgets::{Block, Clear},
    Frame,
};

use crate::app::App;
use crate::pane::PaneNode;
use crate::vim::mode::Mode;

use super::{
    content_search, git_panel, quickopen, render_empty_state_inline, render_pane_tree, status,
    tabs, tree, VisualOverlay,
};

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();

    // Wipe every cell from the previous frame. Without this, ratatui's
    // double-buffering keeps stale glyphs in cells that the new frame's
    // widgets don't explicitly write to — visible as ghost characters
    // along the right edge when prose lines shrink between frames, and
    // as bleed-through under modal popovers.
    frame.render_widget(Clear, area);

    // Paint the theme's frame-wide background + foreground onto every
    // cell. Sub-widgets that don't set their own bg/fg inherit these
    // via ratatui's Style patching, so a light preset paints a light
    // canvas even on a dark terminal. Dark / terminal-native presets
    // leave both at `Color::Reset` so the terminal's own chrome shows.
    let base = Style::default()
        .bg(super::palette::background())
        .fg(super::palette::foreground());
    frame.render_widget(Block::default().style(base), area);

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

    // `[tree? | editor | git_panel?]` — each sidebar collapses out
    // when hidden so the editor reclaims its width.
    let tree_w = if app.tree.visible {
        Some(tree::width().min(body_area.width.saturating_sub(20)))
    } else {
        None
    };
    let git_w = if app.git_panel.visible {
        let budget = body_area.width.saturating_sub(20 + tree_w.unwrap_or(0));
        Some(git_panel::width().min(budget))
    } else {
        None
    };
    let mut constraints: Vec<Constraint> = Vec::new();
    if let Some(w) = tree_w {
        constraints.push(Constraint::Length(w));
    }
    constraints.push(Constraint::Min(1));
    if let Some(w) = git_w {
        constraints.push(Constraint::Length(w));
    }
    let slots = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(body_area);
    let mut idx = 0;
    let sidebar_area = if tree_w.is_some() {
        let r = slots[idx];
        idx += 1;
        Some(r)
    } else {
        None
    };
    let editor_area = slots[idx];
    idx += 1;
    let git_area = if git_w.is_some() {
        Some(slots[idx])
    } else {
        None
    };

    // Highlight matches of the last executed search across visible prose.
    // Live editing of the search buffer (while in `Mode::Search`) also
    // highlights, so users see what their query is hitting before pressing
    // Enter — incremental search affordance.
    let live_search_buf = if app.vim.mode == Mode::Search {
        match app.modal.as_ref().and_then(|m| m.as_prompt()) {
            Some((crate::modal::PromptKind::Search { .. }, le)) if !le.is_empty() => {
                Some(le.as_str().to_string())
            }
            _ => None,
        }
    } else {
        None
    };
    let search_pattern: Option<String> = if let Some(buf) = live_search_buf {
        Some(buf)
    } else if app.vim.search_highlight {
        app.vim.last_search.clone()
    } else {
        // `:noh` was issued and no new search has happened since.
        None
    };

    // CompletionPopup is a passive overlay — keystrokes still flow
    // into the underlying block, so the editor cursor must stay.
    let modal_owns_input = app
        .modal
        .as_ref()
        .is_some_and(|m| !matches!(m, crate::modal::Modal::CompletionPopup(_)));
    let suppress_cursor = matches!(
        app.vim.mode,
        Mode::CommandLine
            | Mode::Search
            | Mode::QuickOpen
            | Mode::Tree
            | Mode::TreePrompt
            | Mode::DbRowDetail
            | Mode::HttpResponseDetail
            | Mode::ContentSearch
            | Mode::Git
            | Mode::Modal
    ) || modal_owns_input;

    // Snapshot the per-block result-tab map so the render tree can
    // read it without re-borrowing `app` at every level. Clone is
    // cheap (`HashMap<BlockId, ResultPanelTab>` is small — one entry
    // per executed block per session).
    let result_tabs_snapshot = app.result_tabs.clone();

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
        // Standard (non-modal) profile has no `Mode::Visual`; the
        // Shift+arrow selection lives on `app.standard.anchor`. The
        // pure helper returns `None` for the Vim profile, so this arm
        // can never leak into the vim render path — Cenário 2 stays
        // byte-identical (no vim arm touched). Disjoint `&` reads of
        // `app.config` / `app.standard`, before any `&mut app.tabs`.
        _ => crate::input::apply::standard_sel::standard_overlay_anchor(
            app.config.editor.mode,
            app.standard.anchor,
        )
        .map(|anchor| VisualOverlay {
            anchor,
            linewise: false,
        }),
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
    let workspace_snap = app.blocks_workspace.clone();
    let running_snap = if matches!(app.view, crate::app::AppView::Blocks) {
        crate::ui::running_chip_label(app)
    } else {
        None
    };
    let mut popup_cursor_cell: Option<(u16, u16)> = None;
    let result_viewport_top = &mut app.result_viewport_top;
    if matches!(app.view, crate::app::AppView::Blocks) {
        if let Some(tab) = app.tabs.tabs.get_mut(active_idx) {
            let focused = tab.focused.clone();
            let mut ctx = crate::ui::blocks_view::BlocksRenderCtx {
                vault: &vault,
                workspace: workspace_snap.as_ref(),
                connection_names: &connection_names,
                result_tabs: &result_tabs_snapshot,
                result_viewport_top,
                visual_overlay,
                running: running_snap,
                popup_cursor_cell: &mut popup_cursor_cell,
            };
            crate::ui::blocks_view::render(frame, editor_area, &mut tab.root, &focused, &mut ctx);
        }
    } else if let Some(tab) = app.tabs.tabs.get_mut(active_idx) {
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
                &result_tabs_snapshot,
            );
        }
    } else {
        render_empty_state_inline(frame, editor_area, &vault);
    }

    let tree_focused = matches!(app.vim.mode, Mode::Tree | Mode::TreePrompt);
    if let Some(sa) = sidebar_area {
        // Files with a committed-but-unsaved block draft in any pane get
        // a dirty dot. `block_draft` is only ever set in BLOCKS view, so
        // this set is naturally empty in DOC mode.
        let dirty_files: std::collections::HashSet<String> = app
            .tabs
            .tabs
            .iter()
            .flat_map(|t| t.root.leaf_panes())
            .filter_map(|p| p.block_draft.as_ref())
            .map(|d| d.file_path.to_string_lossy().into_owned())
            .collect();
        tree::render(frame, sa, &app.tree, tree_focused, &dirty_files);
    }
    let git_cursor = if let Some(ga) = git_area {
        let focused = app.vim.mode == Mode::Git;
        git_panel::render(frame, ga, &app.git_panel, focused, &app.git_commit_template)
    } else {
        None
    };
    status::render_status_bar(frame, status_area, app);

    // Mode-specific terminal-cursor placement (the editor cursor was
    // already drawn by `render_pane_tree` for non-suppressing modes).
    match app.vim.mode {
        Mode::CommandLine | Mode::Search => {
            let cursor_chars = app
                .modal
                .as_ref()
                .and_then(|m| m.as_prompt())
                .map(|(_, le)| le.cursor_col())
                .unwrap_or(0);
            let col = (cursor_chars as u16).saturating_add(1);
            let x = status_area.x + col.min(status_area.width.saturating_sub(1));
            frame.set_cursor_position((x, status_area.y));
        }
        Mode::QuickOpen => {
            if let Some(qo) = app.quickopen() {
                let (cx, cy) = quickopen::render(frame, editor_area, qo);
                frame.set_cursor_position((cx, cy));
            }
        }
        Mode::ContentSearch => {
            if let Some(state) = app.content_search() {
                let (cx, cy) = content_search::render(frame, editor_area, state);
                frame.set_cursor_position((cx, cy));
            }
        }
        Mode::Git => {
            if let Some((cx, cy)) = git_cursor {
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
                    crate::tree::TreePromptKind::DeleteBlock { label, .. } => {
                        format!("delete block {label}? (y/N) ").chars().count()
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
    crate::ui::render_modals::render_modals(frame, app, editor_area, popup_cursor_cell);
}

#[cfg(test)]
#[path = "render_root_tests.rs"]
mod tests;
