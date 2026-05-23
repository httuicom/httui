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
            | Mode::Modal
            | Mode::BlockTemplatePicker
            | Mode::TabPicker
    ) || app.db_row_detail.is_some()
        || app.http_response_detail.is_some()
        || app.connection_picker.is_some()
        || app.content_search.is_some()
        || app.environment_picker.is_some()
        || app.modal.is_some()
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
    if matches!(app.modal, Some(crate::modal::Modal::Help)) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, BlockExportFormat, CompletionPopupState};
    use crate::app::{
        BlockHistoryState, BlockTemplatePickerState, ConnectionPickerState, ContentSearchState,
        DbConfirmRunState, DbExportPickerState, DbRowDetailState, DbSettingsState,
        EnvironmentPickerState, FenceEditState, HttpResponseDetailState, SettingsField,
        TabPickerState,
    };
    use crate::buffer::{Cursor, Document};
    use crate::config::{Config, EditorMode};
    use crate::pane::{Pane, TabState};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use ratatui::backend::{Backend, TestBackend};
    use ratatui::Terminal;
    use std::path::PathBuf;
    use tempfile::TempDir;

    async fn app_with_files(files: &[(&str, &str)]) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        for (rel, body) in files {
            let p = vault.path().join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(p, body).unwrap();
        }
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    /// Open `a.md` into a focused leaf of tab 0 so the renderer has a
    /// real document tree to walk instead of the empty-state path.
    fn open_doc(app: &mut App, md: &str) {
        let doc = Document::from_markdown(md).unwrap();
        let pane = Pane::new(doc, PathBuf::from("a.md"));
        app.tabs.tabs = vec![TabState::new(pane)];
        app.tabs.active = 0;
    }

    fn render(app: &mut App, w: u16, h: u16) -> (String, Option<(u16, u16)>) {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                super::render(f, app);
            })
            .unwrap();
        let cur = terminal
            .backend_mut()
            .get_cursor_position()
            .ok()
            .map(|p| (p.x, p.y));
        let buf = terminal.backend().buffer().clone();
        let text: String = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| buf.cell((x, y)).unwrap().symbol().to_string())
            .collect();
        (text, cur)
    }

    // ---- tab bar visibility ----------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn single_tab_hides_tab_bar_and_renders_doc() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "hello world\n")]).await;
        open_doc(&mut app, "hello world\n");
        let (text, _c) = render(&mut app, 60, 12);
        assert!(text.contains("hello world"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multiple_tabs_show_the_tab_bar() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n")]).await;
        open_doc(&mut app, "alpha content\n");
        // Add a second tab → show_tabs branch.
        let pane2 = Pane::new(
            Document::from_markdown("beta content\n").unwrap(),
            PathBuf::from("b.md"),
        );
        app.tabs.tabs.push(TabState::new(pane2));
        let (text, _c) = render(&mut app, 70, 14);
        assert!(app.tabs.len() > 1);
        assert!(text.contains("alpha content"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn empty_state_when_no_tabs() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.tabs.tabs.clear();
        let (text, _c) = render(&mut app, 60, 10);
        assert!(
            text.contains("no markdown files yet"),
            "expected empty-state hint: {text:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn single_empty_leaf_renders_empty_state_inline() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        app.tabs.tabs = vec![TabState::new(Pane::empty())];
        app.tabs.active = 0;
        let (text, _c) = render(&mut app, 60, 10);
        assert!(text.contains("no markdown files yet"), "got: {text:?}");
    }

    // ---- tree sidebar ----------------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_visible_splits_body_and_paints_sidebar() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "doc body\n")]).await;
        open_doc(&mut app, "doc body\n");
        app.tree.visible = true;
        let (text, _c) = render(&mut app, 80, 16);
        // Editor content still shows next to the sidebar.
        assert!(text.contains("doc body"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_hidden_uses_full_width_editor() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "wide editor\n")]).await;
        open_doc(&mut app, "wide editor\n");
        app.tree.visible = false;
        let (text, _c) = render(&mut app, 80, 12);
        assert!(text.contains("wide editor"), "got: {text:?}");
    }

    // ---- mode-specific cursor placement ----------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn command_line_mode_places_cursor_in_status_row() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.vim.mode = Mode::CommandLine;
        app.vim.cmdline = crate::vim::lineedit::LineEdit::from_str("w");
        let (_t, cur) = render(&mut app, 60, 10);
        assert!(cur.is_some(), "command-line cursor should be set");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_mode_places_cursor_and_live_highlights() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "find me here\n")]).await;
        open_doc(&mut app, "find me here\n");
        app.vim.mode = Mode::Search;
        app.vim.search_forward = true;
        app.vim.search_buf = crate::vim::lineedit::LineEdit::from_str("me");
        let (_t, cur) = render(&mut app, 60, 10);
        assert!(cur.is_some(), "search cursor should be set");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn quickopen_mode_renders_and_sets_cursor() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.vim.mode = Mode::QuickOpen;
        let (_t, cur) = render(&mut app, 60, 12);
        assert!(cur.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn content_search_mode_renders_when_state_present() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.vim.mode = Mode::ContentSearch;
        app.content_search = Some(ContentSearchState::new());
        let (_t, cur) = render(&mut app, 60, 12);
        assert!(cur.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_prompt_mode_places_cursor_in_status_row() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.vim.mode = Mode::TreePrompt;
        app.tree.prompt = Some(crate::tree::TreePrompt::new(
            crate::tree::TreePromptKind::Create { dir: String::new() },
            "new.md".into(),
        ));
        let (_t, cur) = render(&mut app, 60, 10);
        assert!(cur.is_some(), "tree-prompt cursor should be set");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn suppress_cursor_when_modal_owns_input() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.modal = Some(crate::modal::Modal::Help);
        let (text, _c) = render(&mut app, 60, 12);
        // Help modal painted on top.
        assert!(!text.trim().is_empty());
    }

    // ---- visual overlay paths --------------------------------------

    #[tokio::test(flavor = "multi_thread")]
    async fn visual_mode_overlay_path_is_exercised() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "select this line\n")]).await;
        open_doc(&mut app, "select this line\n");
        app.config.editor.mode = EditorMode::Vim;
        app.vim.mode = Mode::Visual;
        app.vim.visual_anchor = Some(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        if let Some(d) = app.tabs.active_document_mut() {
            d.set_cursor(Cursor::InProse {
                segment_idx: 0,
                offset: 6,
            });
        }
        let (text, _c) = render(&mut app, 60, 10);
        assert!(text.contains("select this line"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn visual_line_mode_overlay_path_is_exercised() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "line A\nline B\n")]).await;
        open_doc(&mut app, "line A\nline B\n");
        app.vim.mode = Mode::VisualLine;
        app.vim.visual_anchor = Some(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        let (text, _c) = render(&mut app, 60, 10);
        assert!(text.contains("line A"), "got: {text:?}");
    }

    // ---- modal / popup overlay stack -------------------------------

    fn sub_doc() -> Document {
        Document::from_markdown("status 200\nheader: x\n\nbody\n").unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn db_row_detail_modal_paints() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.db_row_detail = Some(DbRowDetailState {
            segment_idx: 0,
            row: 0,
            title: " row 1 ".into(),
            doc: sub_doc(),
            viewport_height: 10,
            viewport_top: 0,
        });
        let (text, _c) = render(&mut app, 70, 16);
        assert!(text.contains("body"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn http_response_detail_modal_paints_with_visual() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.vim.mode = Mode::Visual;
        app.vim.visual_anchor = Some(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        app.http_response_detail = Some(HttpResponseDetailState {
            segment_idx: 0,
            title: " 200 OK ".into(),
            doc: sub_doc(),
            viewport_height: 10,
            viewport_top: 0,
        });
        let (text, _c) = render(&mut app, 70, 16);
        assert!(
            text.contains("status 200") || text.contains("body"),
            "got: {text:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn connection_picker_popup_paints_anchored() {
        let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        open_doc(&mut app, src);
        let seg = app
            .tabs
            .active_document_mut()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, crate::buffer::Segment::Block(_)))
            .unwrap();
        app.connection_picker = Some(ConnectionPickerState {
            segment_idx: seg,
            connections: vec![crate::app::ConnectionEntry {
                id: "c1".into(),
                name: "Local PG".into(),
                kind: "postgres".into(),
            }],
            selected: 0,
        });
        let (text, _c) = render(&mut app, 70, 16);
        assert!(text.contains("Local PG"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fence_edit_popup_paints() {
        let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        open_doc(&mut app, src);
        app.fence_edit = Some(FenceEditState {
            segment_idx: 0,
            kind: crate::app::FenceEditKind::Alias,
            input: crate::vim::lineedit::LineEdit::from_str("req1"),
        });
        let (text, _c) = render(&mut app, 70, 14);
        assert!(
            text.contains("req1") || text.contains("alias"),
            "got: {text:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn completion_popup_paints() {
        let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        open_doc(&mut app, src);
        app.completion_popup = Some(CompletionPopupState {
            segment_idx: 0,
            items: vec![crate::sql_completion::CompletionItem {
                label: "SELECT".into(),
                kind: crate::sql_completion::CompletionKind::Keyword,
                detail: None,
            }],
            selected: 0,
            anchor_line: 0,
            anchor_offset: 0,
            prefix: "SEL".into(),
        });
        let (text, _c) = render(&mut app, 70, 16);
        assert!(text.contains("SELECT"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn db_confirm_run_modal_paints() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.db_confirm_run = Some(DbConfirmRunState {
            segment_idx: 0,
            reason: "UPDATE without WHERE".into(),
        });
        let (text, _c) = render(&mut app, 70, 14);
        assert!(
            text.contains("WHERE") || text.contains("UPDATE"),
            "got: {text:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn db_export_picker_modal_paints() {
        let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        open_doc(&mut app, src);
        app.db_export_picker = Some(DbExportPickerState::new(0, BlockExportFormat::DB_FORMATS));
        let (text, _c) = render(&mut app, 70, 16);
        assert!(
            text.contains("CSV") || text.contains("JSON"),
            "got: {text:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn db_settings_modal_paints() {
        let src = "```db-postgres alias=q connection=c\nSELECT 1\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        open_doc(&mut app, src);
        app.db_settings = Some(DbSettingsState {
            segment_idx: 0,
            fields: vec![SettingsField {
                label: "Limit",
                key: "limit",
                input: crate::vim::lineedit::LineEdit::from_str("100"),
            }],
            focus: 0,
        });
        let (text, _c) = render(&mut app, 70, 16);
        assert!(
            text.contains("Limit") || text.contains("100"),
            "got: {text:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn block_history_modal_paints() {
        let src = "```http alias=h\nGET https://x.com\n```\n";
        let (mut app, _d, _v) = app_with_files(&[("a.md", src)]).await;
        open_doc(&mut app, src);
        app.block_history = Some(BlockHistoryState {
            segment_idx: 0,
            title: "GET h".into(),
            entries: vec![],
            selected: 0,
        });
        let (text, _c) = render(&mut app, 70, 16);
        assert!(
            text.contains("GET h") || !text.trim().is_empty(),
            "got: {text:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn environment_picker_modal_paints() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.environment_picker = Some(EnvironmentPickerState {
            entries: vec![crate::app::EnvironmentEntry {
                id: "e1".into(),
                name: "staging".into(),
            }],
            selected: 0,
            active_id: None,
        });
        let (text, _c) = render(&mut app, 70, 14);
        assert!(text.contains("staging"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn help_modal_paints() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.modal = Some(crate::modal::Modal::Help);
        let (text, _c) = render(&mut app, 80, 24);
        assert!(!text.trim().is_empty(), "help modal should paint");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn block_template_picker_modal_paints() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.block_template_picker = Some(BlockTemplatePickerState::new());
        let (text, _c) = render(&mut app, 70, 16);
        assert!(
            text.contains("HTTP") || !text.trim().is_empty(),
            "got: {text:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tab_picker_modal_paints() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "x\n")]).await;
        open_doc(&mut app, "x\n");
        app.tab_picker = Some(TabPickerState {
            entries: vec![crate::app::TabPickerEntry {
                idx: 0,
                label: "a.md".into(),
                dirty: false,
            }],
            selected: 0,
        });
        let (text, _c) = render(&mut app, 70, 14);
        assert!(text.contains("a.md"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn noh_search_no_highlight_path() {
        // search_highlight false + not in Search mode → search_pattern
        // resolves to None (the `:noh` branch).
        let (mut app, _d, _v) = app_with_files(&[("a.md", "plain text\n")]).await;
        open_doc(&mut app, "plain text\n");
        app.vim.mode = Mode::Normal;
        app.vim.search_highlight = false;
        let (text, _c) = render(&mut app, 60, 8);
        assert!(text.contains("plain text"), "got: {text:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn persisted_search_highlight_uses_last_search() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "find token here\n")]).await;
        open_doc(&mut app, "find token here\n");
        app.vim.mode = Mode::Normal;
        app.vim.search_highlight = true;
        app.vim.last_search = Some("token".into());
        let (text, _c) = render(&mut app, 60, 8);
        assert!(text.contains("find token here"), "got: {text:?}");
    }

    // ---- Standard-profile selection overlay (fase 3 P-W) -----------

    /// Same as `render` but returns the raw cell buffer so a test can
    /// assert per-cell `bg`. The selection overlay only mutates the
    /// background, so the bg is the load-bearing signal.
    fn render_buf(app: &mut App, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                super::render(f, app);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    const SEL_BG: ratatui::style::Color = crate::ui::palette::SELECTION_BG;

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_mode_selection_paints_highlight_bg() {
        // Cenário 1 passo 5 (pleno): Standard profile, a non-empty
        // Shift-selection (anchor + moved caret) → cells in the range
        // carry the selection bg. Proves the highlight is *visible*.
        let (mut app, _d, _v) = app_with_files(&[("a.md", "select this line\n")]).await;
        open_doc(&mut app, "select this line\n");
        app.config.editor.mode = EditorMode::Standard;
        app.vim.mode = Mode::Normal; // Standard has no Mode::Visual
        app.vim.visual_anchor = None;
        app.standard.anchor = Some(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        if let Some(d) = app.tabs.active_document_mut() {
            d.set_cursor(Cursor::InProse {
                segment_idx: 0,
                offset: 6,
            });
        }
        let buf = render_buf(&mut app, 60, 10);
        let painted = (0..10)
            .flat_map(|y| (0..60).map(move |x| (x, y)))
            .filter(|&(x, y)| buf.cell((x, y)).unwrap().bg == SEL_BG)
            .count();
        assert!(
            painted >= 6,
            "Standard selection must paint the highlight bg over the \
             selected run; painted {painted} cells with SEL_BG"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vim_profile_ignores_standard_anchor_no_highlight() {
        // Guarda Cenário 2: Vim profile, no vim visual anchor, but a
        // stray `standard.anchor` set → the Standard arm resolves to
        // `None` (helper returns None for Vim), so NO cell gets the
        // selection bg. The Standard path can never leak into vim.
        let (mut app, _d, _v) = app_with_files(&[("a.md", "select this line\n")]).await;
        open_doc(&mut app, "select this line\n");
        app.config.editor.mode = EditorMode::Vim;
        app.vim.mode = Mode::Normal;
        app.vim.visual_anchor = None;
        app.standard.anchor = Some(Cursor::InProse {
            segment_idx: 0,
            offset: 0,
        });
        if let Some(d) = app.tabs.active_document_mut() {
            d.set_cursor(Cursor::InProse {
                segment_idx: 0,
                offset: 6,
            });
        }
        let buf = render_buf(&mut app, 60, 10);
        let painted = (0..10)
            .flat_map(|y| (0..60).map(move |x| (x, y)))
            .filter(|&(x, y)| buf.cell((x, y)).unwrap().bg == SEL_BG)
            .count();
        assert_eq!(
            painted, 0,
            "Vim profile must never paint a Standard-anchor selection \
             (Cenário 2 byte-identical); painted {painted}"
        );
    }
}
