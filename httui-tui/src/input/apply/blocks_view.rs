use crate::app::{App, BlocksViewKind, BlocksViewState, StatusKind};
use crate::buffer::{Cursor, Segment};
use crate::input::action::Action;
use crate::modal::Modal;
use crate::vim::mode::Mode;

pub(crate) fn apply_blocks_view(app: &mut App, action: Action) {
    match action {
        Action::OpenBlocksView => open_blocks_view(app),
        Action::CloseBlocksView => close_blocks_view(app),
        Action::BlocksViewNextRegion => with_view(app, |s| s.next_region()),
        Action::BlocksViewPrevRegion => with_view(app, |s| s.prev_region()),
        Action::BlocksViewJumpRegion(n) => with_view(app, move |s| {
            s.set_region(n.saturating_sub(1));
        }),
        _ => {}
    }
}

fn open_blocks_view(app: &mut App) {
    let Some(target) = block_under_cursor(app) else {
        app.set_status(
            StatusKind::Error,
            "BLOCKS view: cursor must be on an HTTP block",
        );
        return;
    };
    let Some(path) = app.document_path().map(|p| p.to_path_buf()) else {
        app.set_status(StatusKind::Error, "BLOCKS view: no file open");
        return;
    };
    app.modal = Some(Modal::BlocksView(BlocksViewState::new(
        path,
        target.segment_idx,
        target.kind,
    )));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
}

fn close_blocks_view(app: &mut App) {
    if matches!(app.modal, Some(Modal::BlocksView(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

fn with_view(app: &mut App, f: impl FnOnce(&mut BlocksViewState)) {
    if let Some(Modal::BlocksView(s)) = app.modal.as_mut() {
        f(s);
    }
}

struct BlockTarget {
    segment_idx: usize,
    kind: BlocksViewKind,
}

fn block_under_cursor(app: &App) -> Option<BlockTarget> {
    let doc = app.document()?;
    let segment_idx = match doc.cursor() {
        Cursor::InBlock { segment_idx, .. } | Cursor::InBlockResult { segment_idx, .. } => {
            segment_idx
        }
        Cursor::InProse { .. } => return None,
    };
    match doc.segments().get(segment_idx)? {
        Segment::Block(b) if b.is_http() => Some(BlockTarget {
            segment_idx,
            kind: BlocksViewKind::Http,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_with(file: &str, content: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join(file), content).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    fn http_md() -> &'static str {
        "# notes\n\n```http alias=q\nGET https://x.com\n```\n"
    }

    fn park_cursor_in_block(app: &mut App) {
        let doc = app.document_mut().unwrap();
        let idx = doc
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("seeded doc has a block");
        doc.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: 0,
        });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_with_cursor_in_prose_sets_error_status() {
        let (mut app, _d, _v) = app_with("api.md", http_md()).await;
        apply_blocks_view(&mut app, Action::OpenBlocksView);
        assert!(app.modal.is_none());
        let msg = app.status_message.as_ref().expect("error status set");
        assert!(matches!(msg.kind, StatusKind::Error));
        assert!(msg.text.contains("HTTP block"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_with_cursor_on_http_block_creates_modal() {
        let (mut app, _d, _v) = app_with("api.md", http_md()).await;
        park_cursor_in_block(&mut app);
        apply_blocks_view(&mut app, Action::OpenBlocksView);
        let state = app
            .modal
            .as_ref()
            .and_then(|m| m.as_blocks_view())
            .expect("BlocksView modal open");
        assert_eq!(state.kind, BlocksViewKind::Http);
        assert_eq!(state.region, 0);
        assert_eq!(app.vim.mode, Mode::Modal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_clears_modal_and_restores_normal_mode() {
        let (mut app, _d, _v) = app_with("api.md", http_md()).await;
        park_cursor_in_block(&mut app);
        apply_blocks_view(&mut app, Action::OpenBlocksView);
        apply_blocks_view(&mut app, Action::CloseBlocksView);
        assert!(app.modal.is_none());
        assert_eq!(app.vim.mode, Mode::Normal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn next_and_prev_region_cycle_focus() {
        let (mut app, _d, _v) = app_with("api.md", http_md()).await;
        park_cursor_in_block(&mut app);
        apply_blocks_view(&mut app, Action::OpenBlocksView);
        apply_blocks_view(&mut app, Action::BlocksViewNextRegion);
        assert_eq!(
            app.modal.as_ref().unwrap().as_blocks_view().unwrap().region,
            1
        );
        apply_blocks_view(&mut app, Action::BlocksViewPrevRegion);
        assert_eq!(
            app.modal.as_ref().unwrap().as_blocks_view().unwrap().region,
            0
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn jump_region_is_one_indexed_and_clamps() {
        let (mut app, _d, _v) = app_with("api.md", http_md()).await;
        park_cursor_in_block(&mut app);
        apply_blocks_view(&mut app, Action::OpenBlocksView);
        apply_blocks_view(&mut app, Action::BlocksViewJumpRegion(3));
        assert_eq!(
            app.modal.as_ref().unwrap().as_blocks_view().unwrap().region,
            2
        );
        apply_blocks_view(&mut app, Action::BlocksViewJumpRegion(9));
        assert_eq!(
            app.modal.as_ref().unwrap().as_blocks_view().unwrap().region,
            3
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_when_no_modal_is_open_is_a_safe_no_op() {
        let (mut app, _d, _v) = app_with("api.md", http_md()).await;
        apply_blocks_view(&mut app, Action::CloseBlocksView);
        assert!(app.modal.is_none());
        assert_eq!(app.vim.mode, Mode::Normal);
    }
}
