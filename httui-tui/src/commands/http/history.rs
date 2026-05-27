//! HTTP/DB block history modal (`gh` chord). Read-only browser over
//! `httui-core::block_history` rows keyed by `(file_path, alias)`.

use crate::app::{App, BlockHistoryState};
use crate::buffer::{Cursor, Segment};
use crate::modal::Modal;

/// `gh` on an HTTP block — open the read-only history modal. Reads
/// `httui-core::block_history::list_history(file, alias)` synchronously
/// (the read is cheap; opening can briefly block). Validates:
///   1. cursor sits on an HTTP block
///   2. block has an alias (history rows are keyed by alias)
///   3. active doc has a file path on disk
///   4. there's at least one row to show
///
/// Each failure surfaces a status hint instead of opening an empty
/// modal — empty modals waste a keystroke.
pub fn open_block_history(app: &mut App) -> Result<(), String> {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => return Err("place the cursor on a block first".into()),
    };
    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => return Err("place the cursor on a block first".into()),
    };
    if !block.is_http() && !block.is_db() {
        return Err(format!(
            "`{}` blocks don't record runs yet",
            block.block_type
        ));
    }
    let alias = match block.alias.as_deref().filter(|s| !s.is_empty()) {
        Some(a) => a.to_string(),
        None => return Err("anonymous block has no history (give it an `alias=`)".into()),
    };
    let file_path = super::active_file_path_string(app)
        .ok_or_else(|| "save the file first — history is keyed by file path".to_string())?;

    let pool = app.pool_manager.app_pool().clone();
    let entries: Vec<httui_core::block_history::HistoryEntry> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async move {
            httui_core::block_history::list_history(&pool, &file_path, &alias).await
        })
    })
    .map_err(|e| format!("history read failed: {e}"))?;

    if entries.is_empty() {
        return Err("no history yet — run the block at least once".into());
    }

    // Title format differs by block type so users see context
    // matching their mental model:
    // - HTTP: `<METHOD> <alias>` (`GET myreq`)
    // - DB:   `DB <alias>`       (`DB userlist`)
    // The driver kind is already encoded in each row's `method`
    // column so we don't repeat it in the title.
    let title = if block.is_http() {
        format!(
            "{} {}",
            block
                .params
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("GET"),
            block.alias.as_deref().unwrap_or(""),
        )
    } else {
        format!("DB {}", block.alias.as_deref().unwrap_or(""))
    };

    app.modal = Some(Modal::BlockHistory(BlockHistoryState {
        segment_idx,
        title,
        entries,
        selected: 0,
    }));
    app.vim.mode = crate::vim::mode::Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub fn close_block_history(app: &mut App) {
    if matches!(app.modal, Some(Modal::BlockHistory(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub fn move_block_history_cursor(app: &mut App, delta: i32) {
    let Some(Modal::BlockHistory(state)) = app.modal.as_mut() else {
        return;
    };
    if state.entries.is_empty() {
        return;
    }
    let last = state.entries.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::buffer::Document;
    use crate::config::Config;
    use crate::pane::{Pane, TabState};
    use crate::vault::ResolvedVault;
    use httui_core::block_history::{insert_history_entry, InsertEntry};
    use httui_core::db::init_db;
    use std::path::PathBuf;
    use tempfile::TempDir;

    async fn app_with_doc(md: &str, path: Option<PathBuf>) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(Config::default(), resolved, pool);
        let doc = Document::from_markdown(md).unwrap();
        let pane = match path {
            Some(p) => Pane::new(doc, p),
            None => Pane {
                document: Some(doc),
                ..Pane::empty()
            },
        };
        // App::new auto-loads an initial pane; replace it with ours so
        // the test sees the requested doc as the active leaf.
        app.tabs.tabs.clear();
        app.tabs.tabs.push(TabState::new(pane));
        app.tabs.active = 0;
        (app, data, vault)
    }

    fn place_cursor_in_block(app: &mut App) {
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .expect("block");
        app.document_mut().unwrap().set_cursor(Cursor::InBlock {
            segment_idx: block_idx,
            offset: 0,
        });
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_block_history_no_block_at_cursor_errors() {
        let (mut app, _d, _v) = app_with_doc("prose\n", None).await;
        let err = open_block_history(&mut app).unwrap_err();
        assert!(err.contains("cursor"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_block_history_anonymous_block_errors() {
        let md = "```http\nGET /x\n```\n";
        let (mut app, _d, _v) = app_with_doc(md, None).await;
        place_cursor_in_block(&mut app);
        let err = open_block_history(&mut app).unwrap_err();
        assert!(
            err.contains("anonymous") || err.contains("alias"),
            "got {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_block_history_unsaved_doc_errors() {
        let md = "```http alias=a\nGET /x\n```\n";
        let (mut app, _d, _v) = app_with_doc(md, None).await;
        place_cursor_in_block(&mut app);
        let err = open_block_history(&mut app).unwrap_err();
        assert!(
            err.contains("save the file") || err.contains("history"),
            "got {err:?}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_block_history_unsupported_block_type_errors() {
        // Use http parse then mutate type to something non-http/non-db.
        let md = "```http alias=a\nGET /x\n```\n";
        let path = std::env::temp_dir().join("ht-history-bad-type.md");
        let (mut app, _d, _v) = app_with_doc(md, Some(path)).await;
        place_cursor_in_block(&mut app);
        let block_idx = app
            .document()
            .unwrap()
            .segments()
            .iter()
            .position(|s| matches!(s, Segment::Block(_)))
            .unwrap();
        if let Some(b) = app.document_mut().unwrap().block_at_mut(block_idx) {
            b.block_type = "mystery".into();
        }
        let err = open_block_history(&mut app).unwrap_err();
        assert!(err.contains("don't record"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_block_history_no_entries_errors() {
        let md = "```http alias=a\nGET /x\n```\n";
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("note.md");
        std::fs::write(&path, md).unwrap();
        let (mut app, _d, _v) = app_with_doc(md, Some(path)).await;
        place_cursor_in_block(&mut app);
        let err = open_block_history(&mut app).unwrap_err();
        assert!(err.contains("no history yet"), "got {err:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_block_history_with_entries_opens_modal_http_title() {
        let md = "```http alias=a method=POST\nPOST https://x.com\n```\n";
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("note.md");
        std::fs::write(&path, md).unwrap();
        let (mut app, _d, _v) = app_with_doc(md, Some(path.clone())).await;
        // Seed one row.
        let pool = app.pool_manager.app_pool().clone();
        let path_str = path.to_string_lossy().to_string();
        insert_history_entry(
            &pool,
            InsertEntry {
                file_path: path_str,
                block_alias: "a".into(),
                method: "POST".into(),
                url_canonical: "https://x.com".into(),
                status: Some(200),
                request_size: None,
                response_size: None,
                elapsed_ms: Some(50),
                outcome: "ok".into(),
                plan: None,
            },
        )
        .await
        .unwrap();
        place_cursor_in_block(&mut app);
        open_block_history(&mut app).unwrap();
        let Some(Modal::BlockHistory(state)) = app.modal.as_ref() else {
            panic!("expected BlockHistory modal");
        };
        assert!(
            state.title.contains("POST") || state.title.contains("a"),
            "title: {}",
            state.title
        );
        assert_eq!(state.entries.len(), 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_block_history_with_db_block_uses_db_title() {
        let md = "```db-sqlite alias=q\nSELECT 1;\n```\n";
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("note.md");
        std::fs::write(&path, md).unwrap();
        let (mut app, _d, _v) = app_with_doc(md, Some(path.clone())).await;
        let pool = app.pool_manager.app_pool().clone();
        let path_str = path.to_string_lossy().to_string();
        insert_history_entry(
            &pool,
            InsertEntry {
                file_path: path_str,
                block_alias: "q".into(),
                method: "sqlite".into(),
                url_canonical: "SELECT 1".into(),
                status: None,
                request_size: None,
                response_size: None,
                elapsed_ms: None,
                outcome: "ok".into(),
                plan: None,
            },
        )
        .await
        .unwrap();
        place_cursor_in_block(&mut app);
        open_block_history(&mut app).unwrap();
        let Some(Modal::BlockHistory(state)) = app.modal.as_ref() else {
            panic!("expected modal");
        };
        assert!(state.title.starts_with("DB "), "title: {}", state.title);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_block_history_clears_modal_and_returns_to_normal() {
        let md = "prose\n";
        let (mut app, _d, _v) = app_with_doc(md, None).await;
        app.modal = Some(Modal::BlockHistory(BlockHistoryState {
            segment_idx: 0,
            title: "t".into(),
            entries: Vec::new(),
            selected: 0,
        }));
        app.vim.mode = crate::vim::mode::Mode::Modal;
        close_block_history(&mut app);
        assert!(app.modal.is_none());
        assert!(matches!(app.vim.mode, crate::vim::mode::Mode::Normal));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_block_history_no_op_when_other_modal_active() {
        let md = "prose\n";
        let (mut app, _d, _v) = app_with_doc(md, None).await;
        app.modal = Some(Modal::Help);
        close_block_history(&mut app);
        assert!(matches!(app.modal, Some(Modal::Help)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn move_block_history_cursor_no_modal_is_noop() {
        let md = "prose\n";
        let (mut app, _d, _v) = app_with_doc(md, None).await;
        move_block_history_cursor(&mut app, 1); // no panic, no effect
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn move_block_history_cursor_with_entries_clamps_to_last() {
        let md = "prose\n";
        let (mut app, _d, _v) = app_with_doc(md, None).await;
        let entry = httui_core::block_history::HistoryEntry {
            id: 1,
            file_path: "x".into(),
            block_alias: "a".into(),
            method: "GET".into(),
            url_canonical: "".into(),
            status: None,
            request_size: None,
            response_size: None,
            elapsed_ms: None,
            outcome: "ok".into(),
            ran_at: "".into(),
            plan: None,
        };
        app.modal = Some(Modal::BlockHistory(BlockHistoryState {
            segment_idx: 0,
            title: "t".into(),
            entries: vec![entry.clone(), entry.clone(), entry],
            selected: 0,
        }));
        move_block_history_cursor(&mut app, 5);
        let Some(Modal::BlockHistory(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.selected, 2);
        move_block_history_cursor(&mut app, -10);
        let Some(Modal::BlockHistory(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.selected, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn move_block_history_cursor_empty_entries_is_noop() {
        let md = "prose\n";
        let (mut app, _d, _v) = app_with_doc(md, None).await;
        app.modal = Some(Modal::BlockHistory(BlockHistoryState {
            segment_idx: 0,
            title: "t".into(),
            entries: Vec::new(),
            selected: 0,
        }));
        move_block_history_cursor(&mut app, 1);
        let Some(Modal::BlockHistory(s)) = app.modal.as_ref() else {
            panic!()
        };
        assert_eq!(s.selected, 0);
    }
}
