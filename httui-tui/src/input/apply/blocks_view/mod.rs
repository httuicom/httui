use crate::app::{
    App, AppView, BlockDraft, BlockIndex, BlocksUnsavedPromptFocus, BlocksUnsavedPromptState,
    BlocksWorkspace, EditField, EditSubMode, RegionEdit, StatusKind,
};
use crate::config::EditorMode;
use crate::input::action::Action;
use crate::modal::Modal;
use crate::vim::mode::Mode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

mod draft;
mod edit;
mod keys;
mod nav;
mod tree;

pub(crate) use self::draft::*;
pub(crate) use self::edit::*;
pub(crate) use self::keys::*;
pub(crate) use self::nav::*;
pub(crate) use self::tree::*;

pub(crate) fn apply_blocks_view(app: &mut App, action: Action) {
    match action {
        Action::ToggleAppView => request_toggle_view(app),
        Action::BlocksPaneNextRegion => cycle_band_subtab(app, 1),
        Action::BlocksPanePrevRegion => cycle_band_subtab(app, -1),
        Action::BlocksPaneJumpRegion(n) => {
            let target = active_block_type(app)
                .map(|bt| jump_target_region(&bt, n))
                .unwrap_or(0);
            set_region(app, target);
        }
        Action::BlocksPanePickerChoose(n) => choose_picker(app, n.saturating_sub(1)),
        Action::BlocksPanePickerCancel => cancel_picker(app),
        Action::BlocksPaneRowUp => shift_row(app, -1),
        Action::BlocksPaneRowDown => shift_row(app, 1),
        Action::BlocksPaneColLeft => shift_col(app, -1),
        Action::BlocksPaneColRight => shift_col(app, 1),
        Action::BlocksRegionEnterEdit => enter_edit(app, EnterMode::Auto),
        Action::BlocksRegionEnterEditInsert => enter_edit(app, EnterMode::Insert),
        Action::BlocksRegionCommitEdit => commit_edit(app),
        Action::BlocksRegionCancelEdit => cancel_edit(app),
        Action::BlocksSaveDraft => save_draft(app),
        Action::BlocksNextBlockMotion => shift_block(app, 1),
        Action::BlocksPrevBlockMotion => shift_block(app, -1),
        Action::BlocksRunFocused => run_focused_block(app),
        Action::BlocksCancelRun => {
            crate::commands::db::cancel_running_query(app);
        }
        Action::BlocksHeaderInsertRow => insert_header_row(app),
        Action::BlocksHeaderDeleteRow => delete_header_row(app),
        Action::BlocksHeaderToggleEnabled => toggle_header_enabled(app),
        Action::BlocksHeaderDeleteConfirm => apply_header_delete_confirm(app),
        Action::BlocksHeaderDeleteCancel => {
            app.modal = None;
        }
        Action::BlocksFieldAdvanceNext => field_advance_next(app),
        Action::BlocksFieldOpenBelow => field_open_below(app),
        Action::BlocksFieldOpenAbove => field_open_above(app),
        Action::BlocksResponseNextTab => shift_response_subtab(app, 1),
        Action::BlocksResponsePrevTab => shift_response_subtab(app, -1),
        Action::BlocksTreeNewBlock => tree_new_block(app),
        Action::BlocksTreeReorderUp => tree_reorder_block(app, -1),
        Action::BlocksTreeReorderDown => tree_reorder_block(app, 1),
        Action::BlocksTreeDeleteBlock => tree_delete_block(app),
        Action::BlocksTreeOpenSplitVertical => {
            tree_open_in_split(app, crate::pane::SplitDir::Vertical);
        }
        Action::BlocksTreeOpenSplitHorizontal => {
            tree_open_in_split(app, crate::pane::SplitDir::Horizontal);
        }
        Action::BlocksUnsavedPromptSave => {
            close_unsaved_prompt(app);
            save_draft(app);
            toggle_view(app);
        }
        Action::BlocksUnsavedPromptDiscard => {
            close_unsaved_prompt(app);
            discard_all_drafts(app);
            toggle_view(app);
        }
        Action::BlocksUnsavedPromptCancel => {
            close_unsaved_prompt(app);
        }
        Action::BlocksTabNew => tab_new(app),
        Action::BlocksTabClose => tab_close(app),
        Action::BlocksTabNext => tab_cycle(app, 1),
        Action::BlocksTabPrev => tab_cycle(app, -1),
        _ => {}
    }
}

/// `Ctrl+T` (BLOCKS view). Push a blank tab onto the focused pane's
/// tab strip and activate it. The greeter region shows a hint until
/// the user picks a block in the sidebar (Enter replaces the active
/// tab in place; Ctrl+Enter would add another new tab).
pub(crate) fn tab_new(app: &mut App) {
    if !matches!(app.view, AppView::Blocks) {
        return;
    }
    // Editing must be committed before a swap so the buffer doesn't
    // get stranded on the now-inactive tab.
    commit_edit(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    pane.push_blank_tab();
}

/// `Ctrl+W` in standard BLOCKS view (also bound to `:bd` / `Ctrl+Q`
/// in vim). Closes the focused pane's active tab. When the closed tab
/// was the last one in the pane, the pane itself is collapsed via the
/// same path as `Ctrl+W q`.
pub(crate) fn tab_close(app: &mut App) {
    if !matches!(app.view, AppView::Blocks) {
        return;
    }
    commit_edit(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    match pane.close_active_tab() {
        crate::pane::CloseResult::Closed => {}
        crate::pane::CloseResult::LastLeaf => {
            // Last tab in the pane → collapse the pane itself. Reuses
            // the same path as `Ctrl+W q` so layout invariants stay
            // identical.
            crate::input::apply::window::apply_window_cmd(
                app,
                crate::input::types::WindowCmd::Close,
            );
        }
    }
}

/// `gt` / `gT` (vim NORMAL) and `Ctrl+PgDn` / `Ctrl+PgUp` (standard).
/// `dir = 1` → next tab, `dir = -1` → previous. Wraps both ways.
pub(crate) fn tab_cycle(app: &mut App, dir: i32) {
    if !matches!(app.view, AppView::Blocks) {
        return;
    }
    commit_edit(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let count = pane.tab_count();
    if count <= 1 {
        return;
    }
    let current = pane.block_tab_active as i32;
    let next = (current + dir).rem_euclid(count as i32) as usize;
    pane.swap_to_tab(next);
}

/// `Alt+M` entry point. If any pane carries a draft, open the
/// Save/Discard/Cancel modal instead of toggling immediately. The
/// modal's emit re-enters this applier with the resolved action.
fn request_toggle_view(app: &mut App) {
    let dirty = collect_dirty_panes(app);
    if dirty.is_empty() {
        toggle_view(app);
        return;
    }
    let files: Vec<std::path::PathBuf> = dirty.iter().map(|(p, _)| p.clone()).collect();
    app.modal = Some(Modal::BlocksUnsavedPrompt(BlocksUnsavedPromptState {
        dirty: files,
        focus: BlocksUnsavedPromptFocus::default(),
    }));
}

fn close_unsaved_prompt(app: &mut App) {
    if let Some(Modal::BlocksUnsavedPrompt(_)) = app.modal {
        app.modal = None;
    }
}

fn choose_picker(app: &mut App, leaf_idx: usize) {
    let Some(intent) = app.blocks_workspace.as_ref().and_then(|w| w.pane_picker) else {
        return;
    };
    let Some(tab) = app.active_tab_mut() else {
        cancel_picker(app);
        return;
    };
    let leaves = tab.leaf_count();
    if leaves == 0 {
        cancel_picker(app);
        return;
    }
    let idx = leaf_idx.min(leaves - 1);
    match intent.action {
        crate::app::PanePickerAction::Open => {
            // Smart Enter, picker variant: an empty target tab takes
            // the block in place; a populated target pushes it as a
            // new tab so the user accumulates blocks per pane.
            let mut visited = 0usize;
            apply_to_nth_leaf(&mut tab.root, idx, &mut visited, &mut |pane| {
                if pane.block_selected.is_none() {
                    pane.block_selected = Some(intent.target);
                    pane.block_region = 0;
                } else {
                    let new_tab = crate::pane_tabs::BlockTab {
                        block_selected: Some(intent.target),
                        block_region: 0,
                        ..crate::pane_tabs::BlockTab::empty()
                    };
                    pane.push_block_tab(new_tab);
                }
            });
            let mut path: Vec<u8> = Vec::new();
            if path_to_nth_leaf(&tab.root, idx, &mut 0, &mut path) {
                tab.focused = path;
            }
        }
        crate::app::PanePickerAction::SplitVertical
        | crate::app::PanePickerAction::SplitHorizontal => {
            // Focus the picked pane so `tab.split` (which acts on the
            // currently-focused leaf) clones the right one.
            let mut path: Vec<u8> = Vec::new();
            if path_to_nth_leaf(&tab.root, idx, &mut 0, &mut path) {
                tab.focused = path;
            }
            let dir = match intent.action {
                crate::app::PanePickerAction::SplitVertical => {
                    crate::pane::SplitDir::Vertical
                }
                _ => crate::pane::SplitDir::Horizontal,
            };
            let mut new_pane = tab.active_leaf().snapshot_clone();
            new_pane.block_selected = Some(intent.target);
            new_pane.block_region = 0;
            new_pane.block_row = 0;
            new_pane.block_col = 0;
            tab.split(dir, new_pane);
        }
    }
    cancel_picker(app);
    app.vim.enter_normal();
}

/// Walk the pane tree to find the path (`0`/`1` per split level) of
/// the leaf at index `target`. Returns true when found and writes
/// the path into `out`.
fn path_to_nth_leaf(
    node: &crate::pane::PaneNode,
    target: usize,
    counter: &mut usize,
    out: &mut Vec<u8>,
) -> bool {
    match node {
        crate::pane::PaneNode::Leaf(_) => {
            if *counter == target {
                return true;
            }
            *counter += 1;
            false
        }
        crate::pane::PaneNode::Split { first, second, .. } => {
            out.push(0);
            if path_to_nth_leaf(first, target, counter, out) {
                return true;
            }
            out.pop();
            out.push(1);
            if path_to_nth_leaf(second, target, counter, out) {
                return true;
            }
            out.pop();
            false
        }
    }
}

fn cancel_picker(app: &mut App) {
    if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.pane_picker = None;
    }
}

fn apply_to_nth_leaf(
    node: &mut crate::pane::PaneNode,
    target: usize,
    counter: &mut usize,
    f: &mut impl FnMut(&mut crate::pane::Pane),
) -> bool {
    match node {
        crate::pane::PaneNode::Leaf(pane) => {
            if *counter == target {
                f(pane);
                return true;
            }
            *counter += 1;
            false
        }
        crate::pane::PaneNode::Split { first, second, .. } => {
            if apply_to_nth_leaf(first, target, counter, f) {
                return true;
            }
            apply_to_nth_leaf(second, target, counter, f)
        }
    }
}

/// Cycle the result/response sub-tab on the focused pane via the
/// shared, BlockId-keyed `ResultPanelTab` — the same state the DOC view
/// cycles, so a choice carries across views/panes for the same block.
/// Applies only on the result band (HTTP region 3, DB region 2).
fn shift_response_subtab(app: &mut App, delta: isize) {
    let target = app
        .blocks_workspace
        .as_ref()
        .zip(app.active_pane())
        .and_then(|(ws, pane)| {
            let sel = pane.block_selected?;
            let file = ws.index.files.get(sel.file_idx)?;
            let block = file.blocks.get(sel.block_idx)?;
            let on_result = (block.block_type == "http" && pane.block_region == 3)
                || (block.block_type.starts_with("db") && pane.block_region == 2);
            if !on_result {
                return None;
            }
            Some((
                file.display.clone(),
                block.line_start,
                block.block_type.clone(),
            ))
        });
    let Some((file, line_start, block_type)) = target else {
        return;
    };
    let id = result_tab_block_id(&file, line_start);
    let current = app
        .result_tabs
        .get(&id)
        .copied()
        .unwrap_or(crate::app::ResultPanelTab::Result);
    let next = if delta >= 0 {
        current.next_for(&block_type)
    } else {
        current.prev_for(&block_type)
    };
    app.result_tabs.insert(id, next);
}

/// Mirrors `block_node_id` in `ui::blocks_view::pane`. The two MUST
/// agree on the hash inputs or the renderer reads a different tab
/// state from what `shift_response_subtab` wrote.
fn result_tab_block_id(file_display: &str, line_start: usize) -> crate::buffer::block::BlockId {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    file_display.hash(&mut h);
    line_start.hash(&mut h);
    crate::buffer::block::BlockId(h.finish())
}

fn toggle_view(app: &mut App) {
    match app.view {
        AppView::Doc => enter_blocks(app),
        AppView::Blocks => exit_blocks(app),
    }
}

fn enter_blocks(app: &mut App) {
    let index = BlockIndex::build(&app.vault_path);
    let vault = app.vault_path.clone();
    if app.blocks_workspace.is_none() {
        app.blocks_workspace = Some(BlocksWorkspace::new(index.clone()));
    } else if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.index = index.clone();
        if let Some(sel) = ws.selected {
            let still_valid = ws
                .index
                .files
                .get(sel.file_idx)
                .map(|f| sel.block_idx < f.blocks.len())
                .unwrap_or(false);
            if !still_valid {
                ws.selected = None;
            }
        }
    }
    app.view = AppView::Blocks;
    app.tree.block_index = Some(index);
    app.tree.visible = true;
    app.tree.refresh(&vault);
    app.vim.mode = Mode::Tree;
    app.vim.reset_pending();
}

fn exit_blocks(app: &mut App) {
    app.view = AppView::Doc;
    app.tree.block_index = None;
    app.tree.expanded.clear();
    let vault = app.vault_path.clone();
    app.tree.refresh(&vault);
    app.tree.selected = 0;
    if matches!(app.vim.mode, Mode::Tree | Mode::TreePrompt) {
        app.vim.enter_normal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppView;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn band_neighbor_walks_http_bands() {
        // URL(0) ↓ Request(1) ; Request ↑ URL, ↓ Response(3) ;
        // Response(3) ↑ Request(1). Edges return None.
        assert_eq!(band_neighbor("http", 0, 1), Some(1));
        assert_eq!(band_neighbor("http", 0, -1), None);
        assert_eq!(band_neighbor("http", 1, -1), Some(0));
        assert_eq!(band_neighbor("http", 2, 1), Some(3));
        assert_eq!(band_neighbor("http", 3, -1), Some(1));
        assert_eq!(band_neighbor("http", 3, 1), None);
    }

    #[test]
    fn band_neighbor_walks_db_bands() {
        assert_eq!(band_neighbor("db-postgres", 0, 1), Some(1));
        assert_eq!(band_neighbor("db-postgres", 1, 1), Some(2));
        assert_eq!(band_neighbor("db-postgres", 2, 1), None);
        assert_eq!(band_neighbor("db-postgres", 1, -1), Some(0));
    }

    #[test]
    fn jump_targets_band_entry_regions() {
        // HTTP 3 bands: 1→URL(0), 2→Request(1), 3→Response(3); clamp.
        assert_eq!(jump_target_region("http", 1), 0);
        assert_eq!(jump_target_region("http", 2), 1);
        assert_eq!(jump_target_region("http", 3), 3);
        assert_eq!(jump_target_region("http", 9), 3);
        // DB 3 bands: 1→Connection(0), 2→Query(1), 3→Result(2).
        assert_eq!(jump_target_region("db-postgres", 1), 0);
        assert_eq!(jump_target_region("db-postgres", 2), 1);
        assert_eq!(jump_target_region("db-postgres", 3), 2);
    }

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, body).unwrap();
    }

    async fn app_with_blocks() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        write(
            vault.path(),
            "api.md",
            "# api\n\n```http alias=login\nGET https://x.com\n```\n",
        );
        write(
            vault.path(),
            "users.md",
            "# users\n\n```http alias=list\nGET https://x.com/users\n```\n",
        );
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_enters_blocks_with_index_loaded() {
        let (mut app, _d, _v) = app_with_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        assert!(matches!(app.view, AppView::Blocks));
        assert!(app.blocks_workspace.is_some());
        assert!(app.tree.block_index.is_some());
        assert!(app.tree.visible);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_back_restores_doc_view() {
        let (mut app, _d, _v) = app_with_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        apply_blocks_view(&mut app, Action::ToggleAppView);
        assert!(matches!(app.view, AppView::Doc));
        assert!(app.tree.block_index.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_preserves_workspace_state_across_round_trips() {
        let (mut app, _d, _v) = app_with_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        if let Some(ws) = app.blocks_workspace.as_mut() {
            ws.selected = Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
        }
        apply_blocks_view(&mut app, Action::ToggleAppView);
        apply_blocks_view(&mut app, Action::ToggleAppView);
        let ws = app.blocks_workspace.as_ref().unwrap();
        assert_eq!(
            ws.selected,
            Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 0
            })
        );
    }

    /// Drive the focused pane into BLOCKS view with the first HTTP
    /// block selected — every Story 5 test starts here. Returns the
    /// vault dir so the test can read what got written.
    async fn enter_blocks_on_first_http() -> (App, TempDir, TempDir) {
        let (mut app, data, vault) = app_with_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        if let Some(ws) = app.blocks_workspace.as_mut() {
            ws.selected = Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
        }
        if let Some(pane) = app.active_pane_mut() {
            pane.block_selected = Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
            pane.block_region = 0;
            pane.block_row = 0;
            pane.block_col = 1;
        }
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn response_next_prev_cycles_shared_result_panel_tab() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        if let Some(p) = app.active_pane_mut() {
            p.block_region = 3;
        }
        let (display, line_start) = {
            let ws = app.blocks_workspace.as_ref().unwrap();
            let sel = app.active_pane().unwrap().block_selected.unwrap();
            let f = &ws.index.files[sel.file_idx];
            (f.display.clone(), f.blocks[sel.block_idx].line_start)
        };
        let id = result_tab_block_id(&display, line_start);
        apply_blocks_view(&mut app, Action::BlocksResponseNextTab);
        assert_eq!(
            app.result_tabs.get(&id).copied(),
            Some(crate::app::ResultPanelTab::Messages)
        );
        apply_blocks_view(&mut app, Action::BlocksResponsePrevTab);
        assert_eq!(
            app.result_tabs.get(&id).copied(),
            Some(crate::app::ResultPanelTab::Result)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn response_tab_chord_is_noop_off_the_result_band() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        if let Some(p) = app.active_pane_mut() {
            p.block_region = 0;
        }
        apply_blocks_view(&mut app, Action::BlocksResponseNextTab);
        assert!(app.result_tabs.is_empty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn http_response_segment_none_without_result_or_off_band() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        // URL band (region 0) — never the response detail trigger.
        assert!(focused_http_response_segment(&app).is_none());
        // Response band but no loaded document / cached result yet.
        if let Some(p) = app.active_pane_mut() {
            p.block_region = 3;
        }
        assert!(focused_http_response_segment(&app).is_none());
    }

    /// Stand-in for the engine wiring: simulate the user typing into
    /// the sub-Document by writing directly via the redirect, the
    /// same path the vim/standard route uses in production.
    fn type_into_active_edit(app: &mut App, text: &str) {
        let doc = app.document_mut().expect("EDIT must be active");
        for c in text.chars() {
            if c == '\n' {
                doc.insert_newline_at_cursor();
            } else {
                doc.insert_char_at_cursor(c);
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enter_edit_url_hydrates_draft_and_seeds_subdoc() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        let pane = app.active_pane().unwrap();
        assert!(pane.block_draft.is_some(), "draft hydrated on first edit");
        let edit = pane.block_edit.as_ref().expect("edit state allocated");
        assert!(matches!(edit.field, EditField::HttpUrl));
        assert_eq!(edit.current_text(), "https://x.com");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_writes_subdoc_text_into_draft() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, " /test");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        let pane = app.active_pane().unwrap();
        assert!(pane.block_edit.is_none(), "edit cleared after commit");
        assert_eq!(
            pane.block_draft.as_ref().unwrap().url(),
            "https://x.com /test"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_sync_applies_committed_unsaved_draft_to_doc() {
        let (mut app, _d, v) = enter_blocks_on_first_http().await;
        // Edit the URL and commit with Esc, but DON'T save (no Ctrl+S).
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "/test");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        assert!(app.active_pane().unwrap().block_edit.is_none());

        // Load the on-disk doc into the pane — disk still has the
        // un-edited URL (mirrors run_focused_block's load step).
        let text = httui_core::fs::read_note(&v.path().to_string_lossy(), "api.md").unwrap();
        let doc = crate::buffer::Document::from_markdown(&text).unwrap();
        if let Some(p) = app.active_pane_mut() {
            p.document = Some(doc);
            p.document_path = Some(v.path().join("api.md"));
        }

        sync_draft_to_doc_in_memory(&mut app);

        // The segment now carries the edited URL, so a run executes the
        // unsaved value instead of the stale on-disk one.
        let pane = app.active_pane().unwrap();
        let url = pane
            .document
            .as_ref()
            .unwrap()
            .segments()
            .iter()
            .find_map(|s| match s {
                crate::buffer::Segment::Block(b) if b.block_type == "http" => b
                    .params
                    .get("url")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
                _ => None,
            });
        assert_eq!(url.as_deref(), Some("https://x.com/test"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_discards_subdoc_without_touching_draft() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "xxxx");
        apply_blocks_view(&mut app, Action::BlocksRegionCancelEdit);
        let pane = app.active_pane().unwrap();
        assert!(pane.block_edit.is_none(), "edit cleared after cancel");
        // Draft was hydrated by enter_edit (so it's Some), but the
        // URL is unchanged because the sub-doc was discarded.
        assert_eq!(pane.block_draft.as_ref().unwrap().url(), "https://x.com");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn save_writes_canonical_fence_to_disk() {
        let (mut app, _d, vault) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "/edited");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        apply_blocks_view(&mut app, Action::BlocksSaveDraft);
        let on_disk = std::fs::read_to_string(vault.path().join("api.md")).unwrap();
        assert!(
            on_disk.contains("https://x.com/edited"),
            "saved file should contain edited URL, got: {on_disk:?}"
        );
        let pane = app.active_pane().unwrap();
        assert!(pane.block_draft.is_none(), "draft cleared after save");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_view_with_dirty_opens_unsaved_prompt() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "z");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        apply_blocks_view(&mut app, Action::ToggleAppView);
        assert!(matches!(
            app.modal,
            Some(crate::modal::Modal::BlocksUnsavedPrompt(_))
        ));
        assert!(matches!(app.view, AppView::Blocks));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unsaved_prompt_discard_drops_drafts_and_toggles() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "z");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        apply_blocks_view(&mut app, Action::ToggleAppView);
        apply_blocks_view(&mut app, Action::BlocksUnsavedPromptDiscard);
        assert!(app.modal.is_none());
        assert!(matches!(app.view, AppView::Doc));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multiline_body_inserts_newline_then_commits_via_esc() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        // Jump to the Request band (Headers), then Tab toggles its
        // sub-tab to Body.
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        apply_blocks_view(&mut app, Action::BlocksPaneNextRegion);
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "line1\nline2");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        let pane = app.active_pane().unwrap();
        assert_eq!(pane.block_draft.as_ref().unwrap().body(), "line1\nline2");
    }

    // ---- Story 6: vim opt-in (via Document) ----

    async fn enter_blocks_vim() -> (App, TempDir, TempDir) {
        let (mut app, data, vault) = enter_blocks_on_first_http().await;
        app.config.editor.mode = crate::config::EditorMode::Vim;
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vim_enter_lands_with_engine_in_normal() {
        let (mut app, _d, _v) = enter_blocks_vim().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        assert!(app.active_pane().unwrap().block_edit.is_some());
        // Engine pinned to Normal so vim chords parse correctly.
        assert_eq!(app.vim.mode, Mode::Normal);
        assert_eq!(effective_sub_mode(&app), EditSubMode::Normal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vim_i_action_lands_engine_in_insert() {
        let (mut app, _d, _v) = enter_blocks_vim().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEditInsert);
        assert_eq!(app.vim.mode, Mode::Insert);
        assert_eq!(effective_sub_mode(&app), EditSubMode::Insert);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_enter_lands_engine_in_insert() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        // Standard profile always reports Insert; vim.mode is the
        // same so the engine inserts directly.
        assert_eq!(effective_sub_mode(&app), EditSubMode::Insert);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn insert_header_row_grows_array_and_moves_cursor() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        let before = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .map(|d| d.header_count())
            .unwrap_or(0);
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        let pane = app.active_pane().unwrap();
        let after = pane.block_draft.as_ref().unwrap().header_count();
        assert_eq!(after, before + 1);
        assert_eq!(pane.block_col, 0, "cursor moved to new key cell");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_header_row_shrinks_array() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        let after_insert = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .unwrap()
            .header_count();
        // `dd` now opens a confirm prompt; the actual delete fires on confirm.
        apply_blocks_view(&mut app, Action::BlocksHeaderDeleteRow);
        assert!(
            matches!(app.modal, Some(crate::modal::Modal::ConfirmPrompt(_))),
            "delete row opens the confirm prompt"
        );
        apply_blocks_view(&mut app, Action::BlocksHeaderDeleteConfirm);
        let after_delete = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .unwrap()
            .header_count();
        assert_eq!(after_delete, after_insert - 1);
        assert!(app.modal.is_none(), "modal closes on confirm");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_header_row_cancel_keeps_row() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        let before = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .unwrap()
            .header_count();
        apply_blocks_view(&mut app, Action::BlocksHeaderDeleteRow);
        apply_blocks_view(&mut app, Action::BlocksHeaderDeleteCancel);
        let after = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .unwrap()
            .header_count();
        assert_eq!(after, before, "cancel keeps the row");
        assert!(app.modal.is_none(), "modal closes on cancel");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn space_toggles_focused_header_enabled() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        let row = app.active_pane().unwrap().block_row;
        apply_blocks_view(&mut app, Action::BlocksHeaderToggleEnabled);
        let disabled = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .map(|d| {
                d.block.params["headers"][row]
                    .get("enabled")
                    .and_then(|v| v.as_bool())
            });
        assert_eq!(disabled, Some(Some(false)), "row disabled after toggle");
        // Second toggle re-enables → flag removed (omit-when-true).
        apply_blocks_view(&mut app, Action::BlocksHeaderToggleEnabled);
        let flag_absent = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .map(|d| d.block.params["headers"][row].get("enabled").is_none());
        assert_eq!(flag_absent, Some(true), "flag removed after re-enable");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enter_in_header_key_advances_to_value() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        // BlocksHeaderInsertRow creates a row + enters EDIT INSERT on key.
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        let field = app
            .active_pane()
            .and_then(|p| p.block_edit.as_ref())
            .map(|e| e.field.clone());
        assert!(
            matches!(field, Some(crate::app::EditField::HttpHeaderKey(_))),
            "started on header key: {field:?}"
        );
        // Advance: key → value, still in EDIT.
        apply_blocks_view(&mut app, Action::BlocksFieldAdvanceNext);
        let field = app
            .active_pane()
            .and_then(|p| p.block_edit.as_ref())
            .map(|e| e.field.clone());
        assert!(
            matches!(field, Some(crate::app::EditField::HttpHeaderValue(_))),
            "advanced to header value: {field:?}"
        );
        assert_eq!(app.active_pane().unwrap().block_col, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enter_in_last_header_value_appends_new_row() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow); // row 0 key
        let count_before = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .map(|d| d.header_count())
            .unwrap_or(0);
        // Move to value cell on the same (last) row.
        apply_blocks_view(&mut app, Action::BlocksFieldAdvanceNext);
        // Advance from last-row value → appends a new row + INSERT on its key.
        apply_blocks_view(&mut app, Action::BlocksFieldAdvanceNext);
        let count_after = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .map(|d| d.header_count())
            .unwrap_or(0);
        assert_eq!(count_after, count_before + 1, "appended a new row");
        let field = app
            .active_pane()
            .and_then(|p| p.block_edit.as_ref())
            .map(|e| e.field.clone());
        assert!(
            matches!(field, Some(crate::app::EditField::HttpHeaderKey(_))),
            "back on a key cell: {field:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_below_and_above_insert_new_rows_in_edit() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow); // row 0 key, EDIT
        let count0 = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .map(|d| d.header_count())
            .unwrap_or(0);
        // vim `o` from EDIT: commit + new row below + INSERT on its key.
        apply_blocks_view(&mut app, Action::BlocksFieldOpenBelow);
        let count1 = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .map(|d| d.header_count())
            .unwrap_or(0);
        assert_eq!(count1, count0 + 1, "open below appended a row");
        assert_eq!(app.active_pane().unwrap().block_col, 0, "on key cell");
        assert!(app.active_pane().unwrap().block_edit.is_some(), "in EDIT");
        // vim `O` from EDIT: commit + new row above + INSERT on its key. The
        // cursor stays at the same index (the previous row is pushed down).
        let row_before = app.active_pane().unwrap().block_row;
        apply_blocks_view(&mut app, Action::BlocksFieldOpenAbove);
        let count2 = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .map(|d| d.header_count())
            .unwrap_or(0);
        assert_eq!(count2, count1 + 1, "open above also inserted");
        assert_eq!(
            app.active_pane().unwrap().block_row,
            row_before,
            "cursor index unchanged on `O` (now on the new row)"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resolve_pane_key_routes_enter_in_header_key_to_advance() {
        use crate::input::apply::blocks_view::keys::resolve_pane_key;
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        // Now in EDIT INSERT on a header key. Enter and Tab both advance.
        assert!(matches!(
            resolve_pane_key(&app, knone(KeyCode::Enter)),
            Some(Action::BlocksFieldAdvanceNext)
        ));
        assert!(matches!(
            resolve_pane_key(&app, knone(KeyCode::Tab)),
            Some(Action::BlocksFieldAdvanceNext)
        ));
        // A plain character key still falls through to the engine (None here).
        assert!(resolve_pane_key(&app, knone(KeyCode::Char('x'))).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn next_block_motion_wraps_workspace() {
        let (mut app, _d, _v) = enter_blocks_vim().await;
        let total: usize = app
            .blocks_workspace
            .as_ref()
            .unwrap()
            .index
            .files
            .iter()
            .map(|f| f.blocks.len())
            .sum();
        assert!(total >= 2, "fixture has >= 2 blocks");
        apply_blocks_view(&mut app, Action::BlocksNextBlockMotion);
        let after_one = app.active_pane().unwrap().block_selected;
        for _ in 0..total {
            apply_blocks_view(&mut app, Action::BlocksNextBlockMotion);
        }
        assert_eq!(app.active_pane().unwrap().block_selected, after_one);
    }

    fn knone(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn nav_keys_map_to_region_and_motion_actions() {
        use Action::*;
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Tab), false, false),
            Some(BlocksPaneNextRegion)
        ));
        assert!(matches!(
            resolve_nav_key(
                KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT),
                false,
                false
            ),
            Some(BlocksPanePrevRegion)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('2')), false, false),
            Some(BlocksPaneJumpRegion(2))
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('k')), false, false),
            Some(BlocksPaneRowUp)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('j')), false, false),
            Some(BlocksPaneRowDown)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('h')), false, false),
            Some(BlocksPaneColLeft)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('l')), false, false),
            Some(BlocksPaneColRight)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Enter), false, false),
            Some(BlocksRegionEnterEdit)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::PageDown), false, false),
            Some(BlocksNextBlockMotion)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::PageUp), false, false),
            Some(BlocksPrevBlockMotion)
        ));
        assert!(resolve_nav_key(knone(KeyCode::Char('z')), false, false).is_none());
    }

    #[test]
    fn nav_vim_only_and_headers_table_chords() {
        use Action::*;
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('i')), true, false),
            Some(BlocksRegionEnterEditInsert)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('a')), true, false),
            Some(BlocksRegionEnterEditInsert)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char(']')), true, false),
            Some(BlocksNextBlockMotion)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('[')), true, false),
            Some(BlocksPrevBlockMotion)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('o')), false, true),
            Some(BlocksHeaderInsertRow)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Insert), false, true),
            Some(BlocksHeaderInsertRow)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char('d')), false, true),
            Some(BlocksHeaderDeleteRow)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Delete), false, true),
            Some(BlocksHeaderDeleteRow)
        ));
        assert!(matches!(
            resolve_nav_key(knone(KeyCode::Char(' ')), false, true),
            Some(BlocksHeaderToggleEnabled)
        ));
        assert!(resolve_nav_key(knone(KeyCode::Char(' ')), false, false).is_none());
    }

    #[test]
    fn nav_save_and_response_tab_chords() {
        use Action::*;
        assert!(matches!(
            resolve_nav_key(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
                false,
                false
            ),
            Some(BlocksSaveDraft)
        ));
        assert!(matches!(
            resolve_nav_key(
                KeyEvent::new(KeyCode::Char('t'), KeyModifiers::ALT),
                false,
                false
            ),
            Some(BlocksResponseNextTab)
        ));
        assert!(matches!(
            resolve_nav_key(
                KeyEvent::new(KeyCode::Char('T'), KeyModifiers::ALT | KeyModifiers::SHIFT),
                false,
                false
            ),
            Some(BlocksResponsePrevTab)
        ));
    }

    #[test]
    fn edit_key_lifecycle_chords() {
        use Action::*;
        assert!(resolve_edit_key(knone(KeyCode::Esc), EditSubMode::Insert, true).is_none());
        assert!(matches!(
            resolve_edit_key(knone(KeyCode::Esc), EditSubMode::Normal, true),
            Some(BlocksRegionCommitEdit)
        ));
        assert!(matches!(
            resolve_edit_key(knone(KeyCode::Esc), EditSubMode::Insert, false),
            Some(BlocksRegionCommitEdit)
        ));
        assert!(matches!(
            resolve_edit_key(
                KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
                EditSubMode::Insert,
                false
            ),
            Some(BlocksRegionCancelEdit)
        ));
        assert!(matches!(
            resolve_edit_key(
                KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
                EditSubMode::Insert,
                false
            ),
            Some(BlocksSaveDraft)
        ));
        assert!(matches!(
            resolve_edit_key(
                KeyEvent::new(KeyCode::Char('r'), KeyModifiers::ALT),
                EditSubMode::Insert,
                false
            ),
            Some(BlocksRunFocused)
        ));
        assert!(matches!(
            resolve_edit_key(
                KeyEvent::new(KeyCode::Char('.'), KeyModifiers::ALT),
                EditSubMode::Insert,
                false
            ),
            Some(BlocksCancelRun)
        ));
        assert!(matches!(
            resolve_edit_key(knone(KeyCode::Char('r')), EditSubMode::Normal, true),
            Some(BlocksRunFocused)
        ));
        assert!(matches!(
            resolve_edit_key(knone(KeyCode::Char('.')), EditSubMode::Normal, true),
            Some(BlocksCancelRun)
        ));
        assert!(resolve_edit_key(knone(KeyCode::Char('x')), EditSubMode::Insert, false).is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resolve_pane_key_nav_modal_guard_and_submode() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        assert!(matches!(
            resolve_pane_key(&app, knone(KeyCode::Tab)),
            Some(Action::BlocksPaneNextRegion)
        ));
        assert_eq!(effective_sub_mode(&app), EditSubMode::Insert);
        app.modal = Some(Modal::Help);
        assert!(resolve_pane_key(&app, knone(KeyCode::Tab)).is_none());
    }

    async fn app_with_mixed_blocks() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        write(
            vault.path(),
            "api.md",
            "# api\n\n```http alias=login\nGET https://x.com\nAuthorization: Bearer {{T}}\n```\n",
        );
        write(
            vault.path(),
            "data.md",
            "# data\n\n```db-postgres alias=q1\nSELECT 1\n```\n\n```db-postgres alias=q2\nSELECT 2\n```\n",
        );
        let pool = init_db(data.path()).await.unwrap();
        let app = App::new(
            Config::default(),
            ResolvedVault {
                vault: vault.path().to_path_buf(),
            },
            pool,
        );
        (app, data, vault)
    }

    fn enter_and_select(app: &mut App, file_idx: usize, block_idx: usize) {
        apply_blocks_view(app, Action::ToggleAppView);
        let sel = crate::app::BlockRef {
            file_idx,
            block_idx,
        };
        if let Some(ws) = app.blocks_workspace.as_mut() {
            ws.selected = Some(sel);
        }
        if let Some(p) = app.active_pane_mut() {
            p.block_selected = Some(sel);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn nav_http_band_and_col_motion() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneRowDown);
        assert_eq!(app.active_pane().unwrap().block_region, 1);
        apply_blocks_view(&mut app, Action::BlocksPaneColLeft);
        apply_blocks_view(&mut app, Action::BlocksPaneColRight);
        apply_blocks_view(&mut app, Action::BlocksPaneRowUp);
        assert_eq!(app.active_pane().unwrap().block_region, 0);
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(3));
        assert_eq!(app.active_pane().unwrap().block_region, 3);
        apply_blocks_view(&mut app, Action::BlocksPaneRowUp);
        assert_eq!(app.active_pane().unwrap().block_region, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn nav_db_band_motion_and_row_count() {
        let (mut app, _d, _v) = app_with_mixed_blocks().await;
        enter_and_select(&mut app, 1, 0);
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        assert_eq!(app.active_pane().unwrap().block_region, 1);
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(3));
        assert_eq!(app.active_pane().unwrap().block_region, 2);
        apply_blocks_view(&mut app, Action::BlocksPaneRowDown);
        apply_blocks_view(&mut app, Action::BlocksPaneRowUp);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enter_edit_resolves_body_and_header_fields() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        if let Some(p) = app.active_pane_mut() {
            p.block_region = 2;
        }
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        assert!(matches!(
            app.active_pane()
                .unwrap()
                .block_edit
                .as_ref()
                .unwrap()
                .field,
            EditField::HttpBody
        ));
        apply_blocks_view(&mut app, Action::BlocksRegionCancelEdit);
        if let Some(p) = app.active_pane_mut() {
            p.block_region = 1;
            p.block_col = 1;
        }
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        assert!(matches!(
            app.active_pane()
                .unwrap()
                .block_edit
                .as_ref()
                .unwrap()
                .field,
            EditField::HttpHeaderValue(_) | EditField::HttpHeaderKey(_)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn run_focused_sets_up_doc_then_defers_to_running_guard() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        // Open an edit so the run path also exercises sync-to-doc.
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        // Pre-arm a running query so start_run_chain early-returns; we
        // only want run_focused_block's load/segment/cursor setup, not a
        // real network/DB dispatch.
        app.running_query = Some(crate::app::RunningQuery {
            segment_idx: 0,
            cancel: tokio_util::sync::CancellationToken::new(),
            started_at: std::time::Instant::now(),
            kind: crate::app::RunningKind::Run,
            cache_key: None,
            bytes_received: 0,
            http_cache_meta: None,
        });
        apply_blocks_view(&mut app, Action::BlocksRunFocused);
        assert!(app.active_pane().unwrap().document.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_delete_block_confirmed_removes_block_from_disk() {
        let (mut app, _d, v) = app_with_mixed_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        tree_delete_block_confirmed(&mut app, "data.md", 0);
        let text = httui_core::fs::read_note(&v.path().to_string_lossy(), "data.md").unwrap();
        let parsed = httui_core::blocks::parse_blocks(&text);
        assert_eq!(parsed.len(), 1, "one db block removed");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resolve_tree_key_block_and_file_rows() {
        let (mut app, _d, _v) = app_with_mixed_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        app.tree.expanded.insert("data.md".to_string());
        let vp = app.vault_path.clone();
        app.tree.refresh(&vp);
        let bidx = app
            .tree
            .entries
            .iter()
            .position(|n| n.block.is_some())
            .expect("a block row");
        app.tree.selected = bidx;
        assert!(matches!(
            resolve_tree_key(&app, KeyEvent::new(KeyCode::Up, KeyModifiers::SHIFT)),
            Some(Action::BlocksTreeReorderUp)
        ));
        assert!(matches!(
            resolve_tree_key(&app, knone(KeyCode::Char('n'))),
            Some(Action::BlocksTreeNewBlock)
        ));
        assert!(matches!(
            resolve_tree_key(&app, knone(KeyCode::Char('d'))),
            Some(Action::BlocksTreeDeleteBlock)
        ));
        let fidx = app
            .tree
            .entries
            .iter()
            .position(|n| n.block.is_none() && n.path.ends_with(".md"))
            .expect("a file row");
        app.tree.selected = fidx;
        assert!(matches!(
            resolve_tree_key(&app, knone(KeyCode::Char('n'))),
            Some(Action::BlocksTreeNewBlock)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_new_block_appends_and_reorder_swaps() {
        let (mut app, _d, v) = app_with_mixed_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        // New block on the api.md file row.
        let fidx = app
            .tree
            .entries
            .iter()
            .position(|n| n.path.ends_with("api.md") && n.block.is_none())
            .expect("api.md row");
        app.tree.selected = fidx;
        apply_blocks_view(&mut app, Action::BlocksTreeNewBlock);
        let api = httui_core::fs::read_note(&v.path().to_string_lossy(), "api.md").unwrap();
        assert_eq!(httui_core::blocks::parse_blocks(&api).len(), 2);

        // Reorder the first block of data.md down → q2 lands first.
        app.tree.expanded.insert("data.md".to_string());
        let vp = app.vault_path.clone();
        app.tree.refresh(&vp);
        let bidx = app
            .tree
            .entries
            .iter()
            .position(|n| {
                n.path.ends_with("data.md")
                    && n.block.as_ref().map(|b| b.block_idx == 0).unwrap_or(false)
            })
            .expect("data.md first block row");
        app.tree.selected = bidx;
        apply_blocks_view(&mut app, Action::BlocksTreeReorderDown);
        let data = httui_core::fs::read_note(&v.path().to_string_lossy(), "data.md").unwrap();
        let parsed = httui_core::blocks::parse_blocks(&data);
        assert_eq!(parsed[0].alias.as_deref(), Some("q2"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_edit_persists_body_and_header_fields() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        if let Some(p) = app.active_pane_mut() {
            p.block_region = 2;
        }
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "BODY");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        assert!(app.active_pane().unwrap().block_edit.is_none());
        if let Some(p) = app.active_pane_mut() {
            p.block_region = 1;
            p.block_col = 1;
            p.block_row = 0;
        }
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "Z");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        assert!(app.active_pane().unwrap().block_draft.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_edit_db_query_field() {
        let (mut app, _d, _v) = app_with_mixed_blocks().await;
        enter_and_select(&mut app, 1, 0);
        if let Some(p) = app.active_pane_mut() {
            p.block_region = 1;
        }
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, " WHERE 1=1");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        assert!(app.active_pane().unwrap().block_draft.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn save_draft_writes_edited_url_to_disk() {
        let (mut app, _d, v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "/edited");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        apply_blocks_view(&mut app, Action::BlocksSaveDraft);
        let api = httui_core::fs::read_note(&v.path().to_string_lossy(), "api.md").unwrap();
        assert!(api.contains("/edited"), "save persisted the edit: {api}");
        assert!(
            app.active_pane().unwrap().block_draft.is_none(),
            "draft cleared after save"
        );
        // A second save with no draft is a no-op.
        apply_blocks_view(&mut app, Action::BlocksSaveDraft);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_new_on_block_row_and_delete_opens_prompt() {
        let (mut app, _d, v) = app_with_mixed_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        app.tree.expanded.insert("data.md".to_string());
        let vp = app.vault_path.clone();
        app.tree.refresh(&vp);
        let bidx = app
            .tree
            .entries
            .iter()
            .position(|n| n.block.is_some() && n.path.ends_with("data.md"))
            .expect("data.md block row");
        app.tree.selected = bidx;
        apply_blocks_view(&mut app, Action::BlocksTreeNewBlock);
        let data = httui_core::fs::read_note(&v.path().to_string_lossy(), "data.md").unwrap();
        assert_eq!(httui_core::blocks::parse_blocks(&data).len(), 3);
        // Delete on a block row opens the confirm prompt (no removal yet).
        app.tree.expanded.insert("data.md".to_string());
        let vp = app.vault_path.clone();
        app.tree.refresh(&vp);
        let bidx2 = app
            .tree
            .entries
            .iter()
            .position(|n| n.block.is_some())
            .expect("a block row");
        app.tree.selected = bidx2;
        apply_blocks_view(&mut app, Action::BlocksTreeDeleteBlock);
        assert!(
            app.tree.prompt.is_some(),
            "delete-block confirm prompt opened"
        );
    }

    /// Load `data.md` into the focused pane with cached result rows on
    /// its first DB block, parked on the Result band.
    fn load_db_result_rows(app: &mut App, vault: &std::path::Path) {
        let text = httui_core::fs::read_note(&vault.to_string_lossy(), "data.md").unwrap();
        let mut doc = crate::buffer::Document::from_markdown(&text).unwrap();
        let idx = doc
            .segments()
            .iter()
            .position(
                |s| matches!(s, crate::buffer::Segment::Block(b) if b.block_type.starts_with("db")),
            )
            .unwrap();
        if let Some(b) = doc.block_at_mut(idx) {
            b.cached_result = Some(serde_json::json!({
                "results": [{
                    "kind": "select",
                    "columns": ["id"],
                    "rows": [{"id": 1}, {"id": 2}, {"id": 3}]
                }]
            }));
        }
        if let Some(p) = app.active_pane_mut() {
            p.document = Some(doc);
            p.document_path = Some(vault.join("data.md"));
            p.block_region = 2;
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn nav_db_result_rows_and_col_clamp() {
        let (mut app, _d, v) = app_with_mixed_blocks().await;
        enter_and_select(&mut app, 1, 0);
        load_db_result_rows(&mut app, v.path());
        apply_blocks_view(&mut app, Action::BlocksPaneRowDown);
        apply_blocks_view(&mut app, Action::BlocksPaneRowDown);
        assert_eq!(app.active_pane().unwrap().block_row, 2);
        apply_blocks_view(&mut app, Action::BlocksPaneColLeft);
        apply_blocks_view(&mut app, Action::BlocksPaneColRight);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enter_edit_on_db_result_opens_row_detail_modal() {
        let (mut app, _d, v) = app_with_mixed_blocks().await;
        enter_and_select(&mut app, 1, 0);
        load_db_result_rows(&mut app, v.path());
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        assert!(matches!(app.modal, Some(Modal::DbRowDetail(_))));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn save_draft_restores_on_write_failure() {
        let (mut app, _d, v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "/x");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        // Make the target path unwritable: replace the file with a dir.
        std::fs::remove_file(v.path().join("api.md")).unwrap();
        std::fs::create_dir(v.path().join("api.md")).unwrap();
        apply_blocks_view(&mut app, Action::BlocksSaveDraft);
        // Write failed → the draft is restored, not silently lost.
        assert!(app.active_pane().unwrap().block_draft.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_reorder_at_top_edge_is_noop() {
        let (mut app, _d, v) = app_with_mixed_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        app.tree.expanded.insert("data.md".to_string());
        let vp = app.vault_path.clone();
        app.tree.refresh(&vp);
        let b0 = app
            .tree
            .entries
            .iter()
            .position(|n| {
                n.path.ends_with("data.md")
                    && n.block.as_ref().map(|b| b.block_idx == 0).unwrap_or(false)
            })
            .expect("first data.md block row");
        app.tree.selected = b0;
        apply_blocks_view(&mut app, Action::BlocksTreeReorderUp);
        let data = httui_core::fs::read_note(&v.path().to_string_lossy(), "data.md").unwrap();
        let parsed = httui_core::blocks::parse_blocks(&data);
        assert_eq!(
            parsed[0].alias.as_deref(),
            Some("q1"),
            "top-edge reorder is a no-op"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_new_block_dedups_alias_across_calls() {
        let (mut app, _d, v) = app_with_mixed_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        for _ in 0..2 {
            let i = app
                .tree
                .entries
                .iter()
                .position(|n| n.path.ends_with("api.md") && n.block.is_none())
                .expect("api.md row");
            app.tree.selected = i;
            apply_blocks_view(&mut app, Action::BlocksTreeNewBlock);
        }
        let api = httui_core::fs::read_note(&v.path().to_string_lossy(), "api.md").unwrap();
        assert_eq!(httui_core::blocks::parse_blocks(&api).len(), 3);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn header_row_insert_synthesises_and_delete_empties() {
        // The `login` block has no headers, so the first insert must
        // synthesise the `headers` array.
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        if let Some(p) = app.active_pane_mut() {
            p.block_region = 1;
        }
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        assert!(app
            .active_pane()
            .unwrap()
            .block_draft
            .as_ref()
            .map(|d| d.header_count() >= 1)
            .unwrap_or(false));
        apply_blocks_view(&mut app, Action::BlocksHeaderDeleteRow);
        // Deleting again on an empty list is a safe no-op.
        apply_blocks_view(&mut app, Action::BlocksHeaderDeleteRow);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_new_block_no_trailing_newline_then_read_failure() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        // No trailing newline → exercises the `else` append branch.
        write(vault.path(), "x.md", "# x\n\n```http alias=a\nGET /1\n```");
        let pool = init_db(data.path()).await.unwrap();
        let mut app = App::new(
            Config::default(),
            ResolvedVault {
                vault: vault.path().to_path_buf(),
            },
            pool,
        );
        apply_blocks_view(&mut app, Action::ToggleAppView);
        let fi = app
            .tree
            .entries
            .iter()
            .position(|n| n.path.ends_with("x.md") && n.block.is_none())
            .expect("x.md row");
        app.tree.selected = fi;
        apply_blocks_view(&mut app, Action::BlocksTreeNewBlock);
        let t = httui_core::fs::read_note(&vault.path().to_string_lossy(), "x.md").unwrap();
        assert_eq!(httui_core::blocks::parse_blocks(&t).len(), 2);

        // Replace the file with a directory so the next read fails — the
        // new-block and reorder handlers must bail with a status, not panic.
        std::fs::remove_file(vault.path().join("x.md")).unwrap();
        std::fs::create_dir(vault.path().join("x.md")).unwrap();
        apply_blocks_view(&mut app, Action::BlocksTreeNewBlock);
        app.tree.expanded.insert("x.md".to_string());
        let vp = app.vault_path.clone();
        app.tree.refresh(&vp);
        if let Some(bi) = app.tree.entries.iter().position(|n| n.block.is_some()) {
            app.tree.selected = bi;
            apply_blocks_view(&mut app, Action::BlocksTreeReorderDown);
            app.tree.selected = bi;
            apply_blocks_view(&mut app, Action::BlocksTreeNewBlock);
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_alias_collision_and_adjacent_reorder() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        // `untitled3` already taken (collides with the len+1 candidate),
        // and the two blocks are adjacent (no prose between fences).
        write(
            vault.path(),
            "y.md",
            "```http alias=untitled3\nGET /1\n```\n```http alias=x\nGET /2\n```\n",
        );
        let pool = init_db(data.path()).await.unwrap();
        let mut app = App::new(
            Config::default(),
            ResolvedVault {
                vault: vault.path().to_path_buf(),
            },
            pool,
        );
        apply_blocks_view(&mut app, Action::ToggleAppView);
        let fi = app
            .tree
            .entries
            .iter()
            .position(|n| n.path.ends_with("y.md") && n.block.is_none())
            .expect("y.md row");
        app.tree.selected = fi;
        apply_blocks_view(&mut app, Action::BlocksTreeNewBlock);
        let t = httui_core::fs::read_note(&vault.path().to_string_lossy(), "y.md").unwrap();
        let aliases: Vec<_> = httui_core::blocks::parse_blocks(&t)
            .iter()
            .filter_map(|p| p.alias.clone())
            .collect();
        assert!(
            aliases.iter().any(|a| a == "untitled4"),
            "dedup skipped the collision: {aliases:?}"
        );

        // Reorder the two adjacent blocks (no prose between them).
        app.tree.expanded.insert("y.md".to_string());
        let vp = app.vault_path.clone();
        app.tree.refresh(&vp);
        let bi = app
            .tree
            .entries
            .iter()
            .position(|n| {
                n.path.ends_with("y.md")
                    && n.block.as_ref().map(|b| b.block_idx == 0).unwrap_or(false)
            })
            .expect("y.md first block");
        app.tree.selected = bi;
        apply_blocks_view(&mut app, Action::BlocksTreeReorderDown);
        let t = httui_core::fs::read_note(&vault.path().to_string_lossy(), "y.md").unwrap();
        assert_eq!(
            httui_core::blocks::parse_blocks(&t)[0].alias.as_deref(),
            Some("x")
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn tree_reorder_and_delete_on_file_without_trailing_newline() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        write(
            vault.path(),
            "z.md",
            "```http alias=a\nGET /1\n```\n\n```http alias=b\nGET /2\n```",
        );
        let pool = init_db(data.path()).await.unwrap();
        let mut app = App::new(
            Config::default(),
            ResolvedVault {
                vault: vault.path().to_path_buf(),
            },
            pool,
        );
        apply_blocks_view(&mut app, Action::ToggleAppView);
        app.tree.expanded.insert("z.md".to_string());
        let vp = app.vault_path.clone();
        app.tree.refresh(&vp);
        let bi = app
            .tree
            .entries
            .iter()
            .position(|n| {
                n.path.ends_with("z.md")
                    && n.block.as_ref().map(|b| b.block_idx == 0).unwrap_or(false)
            })
            .expect("z.md first block");
        app.tree.selected = bi;
        apply_blocks_view(&mut app, Action::BlocksTreeReorderDown);
        tree_delete_block_confirmed(&mut app, "z.md", 0);
        let t = httui_core::fs::read_note(&vault.path().to_string_lossy(), "z.md").unwrap();
        assert_eq!(httui_core::blocks::parse_blocks(&t).len(), 1);
    }
}
