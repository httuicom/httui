//! `impl App` — file open + tab management.
//!
//! Mechanically extracted from the monolithic `impl App` in `app.rs`
//! (tui-v2 vertical 1, fase 2 p2-file_tab) — pure code move, no
//! behavior change. Sibling `impl App {}` block; methods stay `pub fn`
//! so every `app.foo()` call site keeps resolving unchanged.

use std::path::PathBuf;

use crate::document_loader;
use crate::pane::{Pane, TabState};

use super::{file_name, tab_has_dirty, App};

impl App {
    // ----- file open / tab management ------------------------------------

    /// Replace the focused pane's document with the file at
    /// `relative_path`. If that file is the focused leaf of another
    /// tab, switches to that tab instead. Refuses to clobber a dirty
    /// buffer unless `force` is true.
    pub fn open_document(&mut self, relative_path: PathBuf, force: bool) -> Result<String, String> {
        if let Some(idx) = self.tabs.find_focused(&relative_path) {
            self.tabs.active = idx;
            return Ok(format!("\"{}\"", file_name(&relative_path)));
        }
        if !force
            && self
                .active_pane()
                .and_then(|p| p.document.as_ref())
                .is_some_and(|d| d.is_dirty())
        {
            return Err("no write since last change (add ! to override)".into());
        }
        let doc = document_loader::load_and_hydrate(
            &self.vault_path,
            &relative_path,
            self.pool_manager.app_pool(),
            &self.environments_store,
        )
        .map_err(|e| format!("E484: Can't open file: {e}"))?;
        let name = file_name(&relative_path);
        // No tab yet (e.g. last close left us empty)? Open as new tab.
        if self.tabs.is_empty() {
            self.tabs
                .tabs
                .push(TabState::new(Pane::new(doc, relative_path)));
            self.tabs.active = 0;
            return Ok(format!("\"{name}\""));
        }
        // Replace the focused pane's document in-place.
        if let Some(p) = self.active_pane_mut() {
            p.document = Some(doc);
            p.document_path = Some(relative_path);
            p.viewport_top = 0;
        }
        Ok(format!("\"{name}\""))
    }

    /// Open `relative_path` in a brand-new tab. If already focused in
    /// another tab, switches to it instead.
    pub fn open_in_new_tab(&mut self, relative_path: PathBuf) -> Result<String, String> {
        if let Some(idx) = self.tabs.find_focused(&relative_path) {
            self.tabs.active = idx;
            return Ok(format!("\"{}\"", file_name(&relative_path)));
        }
        let doc = document_loader::load_and_hydrate(
            &self.vault_path,
            &relative_path,
            self.pool_manager.app_pool(),
            &self.environments_store,
        )
        .map_err(|e| format!("E484: Can't open file: {e}"))?;
        let name = file_name(&relative_path);
        let new_tab = TabState::new(Pane::new(doc, relative_path));
        self.tabs.tabs.push(new_tab);
        self.tabs.active = self.tabs.tabs.len() - 1;
        Ok(format!("\"{name}\""))
    }

    pub fn next_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.active = (self.tabs.active + 1) % self.tabs.len();
    }

    pub fn prev_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.active = if self.tabs.active == 0 {
            self.tabs.len() - 1
        } else {
            self.tabs.active - 1
        };
    }

    /// Switch to the 1-indexed tab number `n`. Out-of-range no-ops.
    pub fn goto_tab(&mut self, n: usize) {
        if n == 0 || n > self.tabs.len() {
            return;
        }
        self.tabs.active = n - 1;
    }

    /// Close the active tab (drops every pane inside it). With dirty
    /// content in any pane and `force == false`, refuses.
    pub fn close_tab(&mut self, force: bool) -> Result<String, String> {
        if self.tabs.is_empty() {
            return Err("no tab to close".into());
        }
        let active = self.tabs.active;
        if !force && tab_has_dirty(&self.tabs.tabs[active]) {
            return Err("no write since last change (add ! to override)".into());
        }
        let removed = self.tabs.tabs.remove(active);
        let removed_path = removed
            .active_leaf()
            .document_path
            .clone()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(no name)".into());
        if self.tabs.tabs.is_empty() {
            self.tabs.active = 0;
            return Ok(format!("closed \"{removed_path}\""));
        }
        if active >= self.tabs.tabs.len() {
            self.tabs.active = self.tabs.tabs.len() - 1;
        }
        Ok(format!("closed \"{removed_path}\""))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    /// Build an `App` over a vault seeded with the given
    /// `(relative_path, contents)` files. The first file (if any) is
    /// what `App::new`'s initial-document picker lands on. `App::new`
    /// uses `block_in_place` → multi-thread runtime required.
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

    #[tokio::test(flavor = "multi_thread")]
    async fn open_document_replaces_focused_pane_in_place() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n"), ("b.md", "bravo\n")]).await;
        // App::new loaded a.md (alphabetical first). Open b.md → same
        // single tab, focused pane now points at b.md.
        let msg = app.open_document(PathBuf::from("b.md"), false).unwrap();
        assert_eq!(msg, "\"b.md\"");
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(
            app.document_path().map(|p| p.to_string_lossy().to_string()),
            Some("b.md".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_document_switches_to_existing_focused_tab() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n"), ("b.md", "bravo\n")]).await;
        app.open_in_new_tab(PathBuf::from("b.md")).unwrap();
        assert_eq!(app.tabs.len(), 2);
        app.tabs.active = 1;
        // a.md is the focused leaf of tab 0 → open switches, no new tab.
        let msg = app.open_document(PathBuf::from("a.md"), false).unwrap();
        assert_eq!(msg, "\"a.md\"");
        assert_eq!(app.tabs.active, 0);
        assert_eq!(app.tabs.len(), 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_document_refuses_to_clobber_a_dirty_buffer() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n"), ("b.md", "bravo\n")]).await;
        app.document_mut().unwrap().mark_dirty();
        let err = app.open_document(PathBuf::from("b.md"), false).unwrap_err();
        assert!(err.contains("no write since last change"));
        // `force` overrides the guard.
        assert!(app.open_document(PathBuf::from("b.md"), true).is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_document_errors_on_missing_file() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n")]).await;
        let err = app
            .open_document(PathBuf::from("nope.md"), true)
            .unwrap_err();
        assert!(err.starts_with("E484: Can't open file"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_document_opens_as_new_tab_when_tabbar_empty() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n")]).await;
        // Drop every tab so the "no tab yet" branch fires.
        app.tabs.tabs.clear();
        app.tabs.active = 0;
        let msg = app.open_document(PathBuf::from("a.md"), false).unwrap();
        assert_eq!(msg, "\"a.md\"");
        assert_eq!(app.tabs.len(), 1);
        assert_eq!(app.tabs.active, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_in_new_tab_appends_and_focuses_new_tab() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n"), ("b.md", "bravo\n")]).await;
        let msg = app.open_in_new_tab(PathBuf::from("b.md")).unwrap();
        assert_eq!(msg, "\"b.md\"");
        assert_eq!(app.tabs.len(), 2);
        assert_eq!(app.tabs.active, 1);
        // Re-opening the same path switches instead of duplicating.
        app.tabs.active = 0;
        let msg = app.open_in_new_tab(PathBuf::from("b.md")).unwrap();
        assert_eq!(msg, "\"b.md\"");
        assert_eq!(app.tabs.active, 1);
        assert_eq!(app.tabs.len(), 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_in_new_tab_errors_on_missing_file() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n")]).await;
        let err = app.open_in_new_tab(PathBuf::from("ghost.md")).unwrap_err();
        assert!(err.starts_with("E484: Can't open file"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn next_and_prev_tab_wrap_around() {
        let (mut app, _d, _v) =
            app_with_files(&[("a.md", "a\n"), ("b.md", "b\n"), ("c.md", "c\n")]).await;
        app.open_in_new_tab(PathBuf::from("b.md")).unwrap();
        app.open_in_new_tab(PathBuf::from("c.md")).unwrap();
        assert_eq!(app.tabs.active, 2);
        app.next_tab();
        assert_eq!(app.tabs.active, 0); // wrapped past the end
        app.prev_tab();
        assert_eq!(app.tabs.active, 2); // wrapped past the start
        app.prev_tab();
        assert_eq!(app.tabs.active, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn next_prev_tab_noop_with_one_or_zero_tabs() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "a\n")]).await;
        assert_eq!(app.tabs.len(), 1);
        app.next_tab();
        app.prev_tab();
        assert_eq!(app.tabs.active, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn goto_tab_is_one_indexed_and_clamps_out_of_range() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "a\n"), ("b.md", "b\n")]).await;
        app.open_in_new_tab(PathBuf::from("b.md")).unwrap();
        app.goto_tab(1);
        assert_eq!(app.tabs.active, 0);
        app.goto_tab(2);
        assert_eq!(app.tabs.active, 1);
        // 0 and out-of-range no-op (stay on 1).
        app.goto_tab(0);
        assert_eq!(app.tabs.active, 1);
        app.goto_tab(99);
        assert_eq!(app.tabs.active, 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_tab_removes_active_and_reclamps_index() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "a\n"), ("b.md", "b\n")]).await;
        app.open_in_new_tab(PathBuf::from("b.md")).unwrap();
        assert_eq!(app.tabs.active, 1);
        let msg = app.close_tab(false).unwrap();
        assert!(msg.contains("closed"));
        assert_eq!(app.tabs.len(), 1);
        // active was the last index → reclamped down to len-1.
        assert_eq!(app.tabs.active, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_tab_refuses_dirty_then_force_closes_last_tab() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "a\n")]).await;
        app.document_mut().unwrap().mark_dirty();
        let err = app.close_tab(false).unwrap_err();
        assert!(err.contains("no write since last change"));
        // Force closes; last tab gone → active resets to 0, empty bar.
        let msg = app.close_tab(true).unwrap();
        assert!(msg.contains("closed"));
        assert!(app.tabs.is_empty());
        assert_eq!(app.tabs.active, 0);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_tab_errors_when_no_tabs() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "a\n")]).await;
        app.tabs.tabs.clear();
        let err = app.close_tab(false).unwrap_err();
        assert_eq!(err, "no tab to close");
    }
}
