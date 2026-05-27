use crate::app::{App, AppView, BlockIndex, BlocksWorkspace};
use crate::input::action::Action;
use crate::vim::mode::Mode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(crate) fn resolve_pane_key(key: KeyEvent) -> Option<Action> {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (KeyModifiers::NONE, KeyCode::Tab) => Some(Action::BlocksPaneNextRegion),
        (KeyModifiers::SHIFT, KeyCode::BackTab) | (_, KeyCode::BackTab) => {
            Some(Action::BlocksPanePrevRegion)
        }
        (KeyModifiers::NONE, KeyCode::Char(c)) if c.is_ascii_digit() && c != '0' => {
            let n = (c as u8 - b'0') as usize;
            Some(Action::BlocksPaneJumpRegion(n))
        }
        _ => None,
    }
}

pub(crate) fn apply_blocks_view(app: &mut App, action: Action) {
    match action {
        Action::ToggleAppView => toggle_view(app),
        Action::BlocksPaneNextRegion => shift_region(app, 1),
        Action::BlocksPanePrevRegion => shift_region(app, -1),
        Action::BlocksPaneJumpRegion(n) => set_region(app, n.saturating_sub(1)),
        _ => {}
    }
}

fn shift_region(app: &mut App, delta: isize) {
    let count = active_block_region_count(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    if count == 0 {
        pane.block_region = 0;
        return;
    }
    let current = pane.block_region as isize;
    let next = (current + delta).rem_euclid(count as isize);
    pane.block_region = next as usize;
}

fn set_region(app: &mut App, index: usize) {
    let count = active_block_region_count(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    if count == 0 {
        pane.block_region = 0;
        return;
    }
    pane.block_region = index.min(count - 1);
}

fn active_block_region_count(app: &App) -> usize {
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

fn toggle_view(app: &mut App) {
    match app.view {
        AppView::Doc => enter_blocks(app),
        AppView::Blocks => exit_blocks(app),
    }
}

fn enter_blocks(app: &mut App) {
    let index = BlockIndex::build(&app.vault_path);
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
    let vault = app.vault_path.clone();
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
}
