//! Modal / popup overlay stack, painted last over the composed
//! frame. Extracted from `render_root::render` so the dispatcher
//! stays under the size gate; behaviour is unchanged.

use ratatui::{layout::Rect, Frame};

use crate::app::App;
use crate::vim::mode::Mode;

use super::*;

pub(super) fn render_modals(
    frame: &mut Frame,
    app: &mut App,
    editor_area: Rect,
    popup_cursor_cell: Option<(u16, u16)>,
) {
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
        // BLOCKS EDIT publishes its own cursor cell since
        // `compute_block_anchor` doesn't fit the BLOCKS-view layout.
        // Wrap the cell in a synthetic `BlockAnchor` whose math
        // resolves the cursor row to `cursor_y` inside
        // `compute_popup_rect` (it computes `cursor_y =
        // screen_top + 3 + anchor_line`, so we pre-subtract).
        let anchor = if let Some((cx, cy)) = popup_cursor_cell {
            let synth = crate::ui::BlockAnchor {
                screen_top: cy.saturating_sub(3),
                height: 1,
            };
            let _ = cx; // popup x derives from anchor_offset within editor_area
            Some(synth)
        } else {
            anchor::compute_block_anchor(app, editor_area, state.segment_idx)
        };
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
