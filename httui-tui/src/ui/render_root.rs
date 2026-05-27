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
    anchor, block_history, block_template_picker, blocks_unsaved_prompt, completion_popup,
    connection_delete_confirm, connection_form, connection_picker, connections_page,
    content_search, db_confirm_run, db_export_picker, db_row_detail, db_settings_modal,
    environment_picker, envs_page, fence_edit, git_branch_picker, git_conflict_resolver,
    git_log_page, git_panel, git_set_upstream_confirm, help, http_response_detail, quickopen,
    render_empty_state_inline, render_pane_tree, status, tab_picker, tabs, tree, vault_clone_form,
    vault_create_form, vault_missing_secrets, vault_open_picker, vault_picker, VisualOverlay,
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
    let result_viewport_top = &mut app.result_viewport_top;
    if matches!(app.view, crate::app::AppView::Blocks) {
        let workspace = app.blocks_workspace.clone();
        let running = crate::ui::running_chip_label(app);
        if let Some(tab) = app.tabs.tabs.get_mut(active_idx) {
            let focused = tab.focused.clone();
            crate::ui::blocks_view::render(
                frame,
                editor_area,
                &mut tab.root,
                &focused,
                workspace.as_ref(),
                &vault,
                visual_overlay,
                running,
            );
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
        tree::render(frame, sa, &app.tree, tree_focused);
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
    // Modal is independent of mode — it stays painted while the
    // user is in `Mode::DbRowDetail` *and* while a transient mode
    // (Visual, VisualLine) is active over the modal's body.
    let vim_mode = app.vim.mode;
    let visual_anchor = app.vim.visual_anchor;
    if let Some(state) = app.db_row_detail_mut() {
        let visual = match (vim_mode, visual_anchor) {
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
    if let Some(state) = app.http_response_detail_mut() {
        let visual = match (vim_mode, visual_anchor) {
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
    if let Some(crate::modal::Modal::ConnectionPicker(state)) = app.modal.as_ref() {
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
        connection_picker::render(frame, editor_area, state, anchor);
    }

    // Inline fence-metadata edit popup (alias today; limit / timeout
    // soon). Anchored above the block being edited so the user keeps
    // visual context — the previous status-bar prompt felt detached
    // from the action.
    if let Some((crate::modal::PromptKind::FenceEditAlias { segment_idx }, le)) =
        app.modal.as_ref().and_then(|m| m.as_prompt())
    {
        let anchor = anchor::compute_block_anchor(app, editor_area, segment_idx);
        fence_edit::render(frame, editor_area, "alias", le, anchor);
    }

    // SQL completion popup — paints whenever its state is `Some`,
    // independent of mode (typing in Insert keeps the popup open and
    // re-filtered). Anchored below the focused DB block; falls back
    // above or centered if no room. Painted last so it floats above
    // the editor cursor and any earlier overlays.
    if let Some(state) = app.completion_popup() {
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
        completion_popup::render(frame, editor_area, state, anchor);
    }

    // Run-confirm modal — painted last so it floats over everything
    // else (including a stuck completion popup, though both being up
    // simultaneously shouldn't happen in practice).
    if let Some(crate::modal::Modal::DbConfirmRun(state)) = app.modal.as_ref() {
        db_confirm_run::render(frame, editor_area, state);
    }

    // Export-format picker — opened by `gx` over a DB block with
    // rows. Same chrome as the connection picker; anchored above
    // the block when there's headroom.
    if let Some(crate::modal::Modal::DbExportPicker(state)) = app.modal.as_ref() {
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
        db_export_picker::render(frame, editor_area, state, anchor);
    }

    // Settings modal — opened by `gs` over a DB block. Two-input
    // form (limit + timeout) with Tab focus cycle. Anchored above
    // the block; falls back below or centered when no headroom.
    if let Some(state) = app.db_settings() {
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
        db_settings_modal::render(frame, editor_area, state, anchor);
    }

    // Block history modal — opened by `gh` over an HTTP block.
    // Read-only listing of the last N runs; same chrome as the
    // connection picker but wider (the timestamp column needs room).
    if let Some(crate::modal::Modal::BlockHistory(state)) = app.modal.as_ref() {
        let anchor = anchor::compute_block_anchor(app, editor_area, state.segment_idx);
        block_history::render(frame, editor_area, state, anchor);
    }

    // Environment picker — opened by `gE`. Centered popup (no
    // anchor: envs are global state), magenta border to match the
    // status-bar chip.
    if let Some(crate::modal::Modal::EnvironmentPicker(state)) = app.modal.as_ref() {
        environment_picker::render(frame, editor_area, state);
    }

    // vault picker — same wider variant of the env picker.
    if let Some(crate::modal::Modal::VaultPicker(state)) = app.modal.as_ref() {
        vault_picker::render(frame, editor_area, state);
    }

    // vault create form (opened by `n` inside the picker).
    if let Some(crate::modal::Modal::VaultCreateForm(state)) = app.modal.as_ref() {
        if let Some((cx, cy)) = vault_create_form::render(frame, editor_area, state) {
            frame.set_cursor_position((cx, cy));
        }
    }

    // vault clone form (opened by `c` inside the picker).
    if let Some(crate::modal::Modal::VaultCloneForm(state)) = app.modal.as_ref() {
        if let Some((cx, cy)) = vault_clone_form::render(frame, editor_area, state) {
            frame.set_cursor_position((cx, cy));
        }
    }

    // directory navigator (opened by `o` inside the picker).
    if let Some(crate::modal::Modal::VaultOpenPicker(state)) = app.modal.as_ref() {
        vault_open_picker::render(frame, editor_area, state);
    }

    // first-run secrets modal (opened automatically
    // after switch_vault / startup when there are missing refs).
    if let Some(crate::modal::Modal::VaultMissingSecrets(state)) = app.modal.as_ref() {
        if let Some((cx, cy)) = vault_missing_secrets::render(frame, editor_area, state) {
            frame.set_cursor_position((cx, cy));
        }
    }

    // Help modal — opened by `g?`. Stateless overlay listing the
    // chord vocabulary grouped by section. Painted last so it
    // floats above any other modal that might still be on screen.
    if matches!(app.modal, Some(crate::modal::Modal::Help)) {
        help::render(frame, editor_area);
    }

    // Block-template picker — opened by `gN`. Centered popup with
    // a fixed list of fence templates; confirm splices the picked
    // template into the prose at the cursor and re-parses.
    if let Some(crate::modal::Modal::BlockTemplatePicker(state)) = app.modal.as_ref() {
        block_template_picker::render(frame, editor_area, state);
    }

    // Tab picker — opened by `gb`. Lists every open tab by its
    // focused-leaf path; current tab marked with `●`, dirty tabs
    // get a trailing `*`. Painted last so it floats above any
    // other modal (none of the others share the gb chord).
    if let Some(crate::modal::Modal::TabPicker(state)) = app.modal.as_ref() {
        tab_picker::render(frame, editor_area, state, app.tabs.active);
    }

    // Connections page — opened by `gC` / `Alt+P` (V3). Fullscreen
    // master-detail; takes the whole editor area. Painted last so it
    // sits above every other surface — leaving via `Esc` restores
    // the editor underneath.
    if let Some(crate::modal::Modal::Connections(state)) = app.modal.as_ref() {
        connections_page::render(
            frame,
            editor_area,
            state,
            &app.schema_cache,
            &app.session_overrides,
        );
    }

    // V3 P3: create-connection form modal. Painted on top of the
    // Connections page (which replaces itself when the form opens —
    // single-modal stack for now). Centered popup ~62x22. The
    // renderer returns the cursor position for the focused field
    // so the terminal can place its native (blinking) cursor.
    if let Some(crate::modal::Modal::ConnectionForm(state)) = app.modal.as_ref() {
        if let Some((cx, cy)) = connection_form::render(frame, editor_area, state) {
            frame.set_cursor_position((cx, cy));
        }
    }

    // V3 P4: delete-confirm. Always painted last — sits above the
    // Connections page (the prior modal); `n`/`Esc` reopens the page.
    if let Some(crate::modal::Modal::ConnectionDeleteConfirm(state)) = app.modal.as_ref() {
        connection_delete_confirm::render(frame, editor_area, state);
    }

    if let Some(crate::modal::Modal::GitSetUpstreamConfirm(state)) = app.modal.as_ref() {
        git_set_upstream_confirm::render(frame, editor_area, state);
    }

    if let Some(crate::modal::Modal::GitBranchPicker(state)) = app.modal.as_ref() {
        git_branch_picker::render(frame, editor_area, state);
    }

    if matches!(app.modal, Some(crate::modal::Modal::GitLogPage(_))) {
        crate::input::apply::git_log_page::ensure_diff_loaded(app);
        if let Some(crate::modal::Modal::GitLogPage(state)) = app.modal.as_ref() {
            git_log_page::render(frame, editor_area, state);
        }
    }

    if matches!(app.modal, Some(crate::modal::Modal::GitConflictResolver(_))) {
        crate::input::apply::git_conflict_resolver::ensure_versions_loaded(app);
        if let Some(crate::modal::Modal::GitConflictResolver(state)) = app.modal.as_ref() {
            git_conflict_resolver::render(frame, editor_area, state);
        }
    }

    // V4 P2-P4: envs/vars surfaces.
    if let Some(crate::modal::Modal::EnvsPage(state)) = app.modal.as_ref() {
        envs_page::render(frame, editor_area, state);
    }
    if let Some(crate::modal::Modal::EnvForm(state)) = app.modal.as_ref() {
        if let Some((cx, cy)) = envs_page::render_env_form(frame, editor_area, state) {
            frame.set_cursor_position((cx, cy));
        }
    }
    if let Some(crate::modal::Modal::VarForm(state)) = app.modal.as_ref() {
        if let Some((cx, cy)) = envs_page::render_var_form(frame, editor_area, state) {
            frame.set_cursor_position((cx, cy));
        }
    }
    if let Some(crate::modal::Modal::EnvDeleteConfirm(state)) = app.modal.as_ref() {
        envs_page::render_env_delete_confirm(frame, editor_area, state);
    }
    if let Some(crate::modal::Modal::VarDeleteConfirm(state)) = app.modal.as_ref() {
        envs_page::render_var_delete_confirm(frame, editor_area, state);
    }
    if let Some(crate::modal::Modal::EnvCloneForm(state)) = app.modal.as_ref() {
        if let Some((cx, cy)) =
            crate::ui::envs_clone::render_env_clone_form(frame, editor_area, state)
        {
            frame.set_cursor_position((cx, cy));
        }
    }

    // Settings page is painted last so it floats above every other
    // surface; `Esc` then restores the editor underneath.
    if let Some(crate::modal::Modal::Settings(state)) = app.modal.as_ref() {
        let keymaps = crate::input::apply::settings_page::keymap_view(app);
        let themes = crate::input::apply::settings_page::theme_view(app);
        let editor = crate::input::apply::settings_page::editor_view(app);
        crate::ui::settings_page::render(frame, editor_area, state, &keymaps, &themes, &editor);
    }

    // BLOCKS-view unsaved guard — painted on top of the BLOCKS pane so
    // the file list + chip row sit over the workspace.
    if let Some(crate::modal::Modal::BlocksUnsavedPrompt(state)) = app.modal.as_ref() {
        blocks_unsaved_prompt::render(frame, editor_area, state);
    }
}

#[cfg(test)]
#[path = "render_root_tests.rs"]
mod tests;
