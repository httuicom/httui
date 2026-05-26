//! Exit-time safety-net flush.
//!
//! No tick-driven auto-save: external writers (git, MCP, scripts)
//! race with watchers, so disk-state is owned exclusively by
//! explicit `Ctrl+S` / `:w`. `flush_all_dirty` is a crash-avoidance
//! hook for graceful exit only.

use super::App;

/// Serialize + write every dirty document across all tabs/panes, then
/// `mark_clean`. Safety-net invoked on every exit route (incl. the
/// channel-closed path that historically dropped unsaved edits).
/// Idempotent: a clean doc serializes to the same bytes and is skipped.
/// FTS index updates are intentionally NOT mirrored here — that's a
/// best-effort freshness hook in the interactive `:w`, and a full
/// rebuild reconciles on next search-modal open (see `ex.rs:342-360`).
// Wired by `event_loop::main_loop` (flush on quit).
pub(crate) fn flush_all_dirty(app: &mut App) {
    let vault = app.vault_path.to_string_lossy().into_owned();
    for tab in &mut app.tabs.tabs {
        super::for_each_leaf_mut(&mut tab.root, &mut |pane| {
            let Some(path) = pane.document_path.clone() else {
                return;
            };
            let Some(doc) = pane.document.as_mut() else {
                return;
            };
            if !doc.is_dirty() {
                return;
            }
            let body = doc.to_markdown();
            let file_str = path.to_string_lossy().into_owned();
            match httui_core::fs::write_note(&vault, &file_str, &body) {
                Ok(()) => doc.mark_clean(),
                Err(e) => tracing::warn!("auto-save flush failed for {file_str}: {e}"),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Document;
    use crate::config::Config;
    use crate::pane::{Pane, TabState};
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_fixture(body: &str) -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), body).unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn flush_all_dirty_writes_every_dirty_tab_and_marks_clean() {
        let (mut app, _d, vault) = app_fixture("first\n").await;
        // Tab 0 (note.md, from the fixture) — dirty it.
        {
            let doc = app.tabs.active_document_mut().expect("tab0 doc");
            for ch in "EDIT0".chars() {
                doc.insert_char_at_cursor(ch);
            }
            assert!(doc.is_dirty());
        }
        // Add a second tab with its own dirty doc.
        std::fs::write(vault.path().join("two.md"), "second\n").unwrap();
        let mut d2 = Document::from_markdown("second\n").unwrap();
        for ch in "EDIT1".chars() {
            d2.insert_char_at_cursor(ch);
        }
        assert!(d2.is_dirty());
        app.tabs
            .tabs
            .push(TabState::new(Pane::new(d2, "two.md".into())));

        flush_all_dirty(&mut app);

        // Both files now hold the edited content on disk.
        let f0 = std::fs::read_to_string(vault.path().join("note.md")).unwrap();
        let f1 = std::fs::read_to_string(vault.path().join("two.md")).unwrap();
        assert!(f0.contains("EDIT0"), "tab0 flushed: {f0:?}");
        assert!(f1.contains("EDIT1"), "tab1 flushed: {f1:?}");
        // And both docs are marked clean.
        assert!(!app.tabs.tabs[0]
            .active_leaf()
            .document
            .as_ref()
            .unwrap()
            .is_dirty());
        assert!(!app.tabs.tabs[1]
            .active_leaf()
            .document
            .as_ref()
            .unwrap()
            .is_dirty());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn flush_all_dirty_is_a_noop_for_clean_docs() {
        let (mut app, _d, vault) = app_fixture("untouched\n").await;
        // Doc loaded clean — flush must not rewrite it / error.
        assert!(!app.tabs.active_document_mut().unwrap().is_dirty());
        flush_all_dirty(&mut app);
        let body = std::fs::read_to_string(vault.path().join("note.md")).unwrap();
        assert_eq!(body, "untouched\n");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn flush_all_dirty_skips_pane_without_path() {
        let (mut app, _d, _v) = app_fixture("body\n").await;
        // A pane with a dirty document but no path — must be skipped
        // (no panic, no write target).
        let mut d = Document::from_markdown("orphan\n").unwrap();
        d.insert_char_at_cursor('X');
        let pane = Pane {
            document: Some(d),
            document_path: None,
            viewport_top: 0,
            viewport_height: 0,
        };
        app.tabs.tabs.push(TabState::new(pane));
        flush_all_dirty(&mut app);
        // The orphan pane's doc stays dirty (never written).
        assert!(app.tabs.tabs[1]
            .active_leaf()
            .document
            .as_ref()
            .unwrap()
            .is_dirty());
    }
}
