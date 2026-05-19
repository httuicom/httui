//! `impl App` — vault-relative file CRUD (create / folder / rename /
//! delete).
//!
//! Mechanically extracted from the monolithic `impl App` in `app.rs`
//! (tui-v2 vertical 1, fase 2 p2-file_crud) — pure code move, no
//! behavior change. Rust permits multiple `impl App {}` blocks across
//! sibling modules of the same crate; the methods stay `pub fn` so
//! every `app.foo()` call site keeps resolving unchanged.

use std::path::PathBuf;

use super::{file_name, for_each_leaf_mut, App};

impl App {
    // ----- file CRUD (vault-relative) ------------------------------------

    pub fn create_document(
        &mut self,
        relative_path: PathBuf,
        force: bool,
    ) -> Result<String, String> {
        if !force
            && self
                .active_pane()
                .and_then(|p| p.document.as_ref())
                .is_some_and(|d| d.is_dirty())
        {
            return Err("no write since last change (add ! to override)".into());
        }
        let vault = self.vault_path.to_string_lossy().into_owned();
        let path_str = relative_path.to_string_lossy().into_owned();
        httui_core::fs::create_note(&vault, &path_str)
            .map_err(|e| format!("create failed: {e}"))?;
        self.open_document(relative_path, true)
    }

    pub fn create_folder(&mut self, relative_path: PathBuf) -> Result<String, String> {
        let abs = self.vault_path.join(&relative_path);
        if abs.exists() {
            return Err(format!(
                "create folder failed: path already exists: {}",
                relative_path.display()
            ));
        }
        std::fs::create_dir_all(&abs).map_err(|e| format!("create folder failed: {e}"))?;
        Ok(format!("created folder \"{}\"", file_name(&relative_path)))
    }

    /// Rename a vault-relative path. With `src == None` the focused
    /// pane's path is used. Updates every pane (across all tabs) that
    /// currently shows the renamed path.
    pub fn rename_path(&mut self, src: Option<PathBuf>, dst: PathBuf) -> Result<String, String> {
        let src_rel = match src {
            Some(p) => p,
            None => self
                .document_path()
                .cloned()
                .ok_or_else(|| "no file name".to_string())?,
        };
        let vault = self.vault_path.clone();
        let src_abs = vault.join(&src_rel);
        let dst_abs = vault.join(&dst);
        if dst_abs.exists() {
            return Err(format!(
                "E13: File exists (add ! to override): {}",
                dst.display()
            ));
        }
        if let Some(parent) = dst_abs.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("rename failed: {e}"))?;
        }
        std::fs::rename(&src_abs, &dst_abs).map_err(|e| format!("rename failed: {e}"))?;
        // Update every pane referencing the old path.
        for tab in self.tabs.tabs.iter_mut() {
            for_each_leaf_mut(&mut tab.root, &mut |pane| {
                if pane.document_path.as_deref() == Some(src_rel.as_path()) {
                    pane.document_path = Some(dst.clone());
                }
            });
        }

        // Move the search-index row to the new path. Cheapest
        // correct thing: drop the old key + reinsert the file's
        // current content under the new key. We re-read from disk
        // (the move just happened) so the indexed body matches the
        // file even if the user renamed without saving first. Best-
        // effort — index can always be rebuilt via the next `<C-f>`
        // open in the worst case.
        if self.content_search_index_built
            && src_rel.extension().and_then(|s| s.to_str()) == Some("md")
        {
            let pool = self.pool_manager.app_pool().clone();
            let src_key = src_rel.to_string_lossy().to_string();
            let dst_key = dst.to_string_lossy().to_string();
            let dst_abs_for_read = dst_abs.clone();
            tokio::spawn(async move {
                if let Err(e) = httui_core::search::remove_search_entry(&pool, &src_key).await {
                    tracing::warn!("search index rename (drop old) failed: {e}");
                }
                let body = std::fs::read_to_string(&dst_abs_for_read).unwrap_or_default();
                if let Err(e) =
                    httui_core::search::update_search_entry(&pool, &dst_key, &body).await
                {
                    tracing::warn!("search index rename (insert new) failed: {e}");
                }
            });
        }

        let name = file_name(&dst);
        Ok(format!("renamed to \"{name}\""))
    }

    /// Delete a path under the vault. Panes pointing at the deleted
    /// path are emptied (document/path → None); a tab containing only
    /// empty leaves is collapsed to a single empty leaf.
    pub fn delete_path(&mut self, target: Option<PathBuf>, force: bool) -> Result<String, String> {
        let target_rel = match target {
            Some(p) => p,
            None => self
                .document_path()
                .cloned()
                .ok_or_else(|| "no file name".to_string())?,
        };
        let opens_current = self.document_path() == Some(&target_rel);
        if opens_current && !force && self.document().is_some_and(|d| d.is_dirty()) {
            return Err("no write since last change (add ! to override)".into());
        }
        let vault = self.vault_path.clone();
        let abs = vault.join(&target_rel);
        let metadata = std::fs::metadata(&abs).map_err(|e| format!("delete failed: {e}"))?;
        let was_dir = metadata.is_dir();
        if was_dir {
            std::fs::remove_dir_all(&abs).map_err(|e| format!("delete failed: {e}"))?;
        } else {
            std::fs::remove_file(&abs).map_err(|e| format!("delete failed: {e}"))?;
        }
        // Empty out any pane whose path matched the deleted target.
        for tab in self.tabs.tabs.iter_mut() {
            for_each_leaf_mut(&mut tab.root, &mut |pane| {
                if pane.document_path.as_deref() == Some(target_rel.as_path()) {
                    pane.document = None;
                    pane.document_path = None;
                    pane.viewport_top = 0;
                }
            });
        }

        // Drop search-index rows pointing at the deleted file (or any
        // file under the deleted directory). Without this, `<C-f>`
        // would surface a result that opens to a missing file. Only
        // runs after the index has been built — pre-build the rows
        // don't exist anyway.
        if self.content_search_index_built {
            let pool = self.pool_manager.app_pool().clone();
            let target_str = target_rel.to_string_lossy().to_string();
            tokio::spawn(async move {
                if was_dir {
                    let prefix = format!("{}/%", target_str.trim_end_matches('/'));
                    if let Err(e) = sqlx::query(
                        "DELETE FROM search_index WHERE file_path = ? OR file_path LIKE ?",
                    )
                    .bind(&target_str)
                    .bind(&prefix)
                    .execute(&pool)
                    .await
                    {
                        tracing::warn!("search index purge (dir) failed: {e}");
                    }
                } else if let Err(e) =
                    httui_core::search::remove_search_entry(&pool, &target_str).await
                {
                    tracing::warn!("search index purge (file) failed: {e}");
                }
            });
        }

        Ok(format!("deleted \"{}\"", file_name(&target_rel)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    /// Build an `App` over a vault seeded with `(rel, body)` files.
    /// `App::new` calls `block_in_place` → multi-thread runtime needed.
    /// Temp dirs are returned so they outlive (and clean up) the vault.
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
    async fn create_document_writes_file_and_opens_it() {
        let (mut app, _d, vault) = app_with_files(&[("a.md", "alpha\n")]).await;
        let msg = app
            .create_document(PathBuf::from("sub/new.md"), false)
            .unwrap();
        assert_eq!(msg, "\"new.md\"");
        assert!(vault.path().join("sub/new.md").is_file());
        // It became the focused pane (open_document(force=true)).
        assert_eq!(
            app.document_path().map(|p| p.to_string_lossy().to_string()),
            Some("sub/new.md".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_document_refuses_over_dirty_buffer_then_force_works() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n")]).await;
        app.document_mut().unwrap().mark_dirty();
        let err = app
            .create_document(PathBuf::from("x.md"), false)
            .unwrap_err();
        assert!(err.contains("no write since last change"));
        assert!(app.create_document(PathBuf::from("x.md"), true).is_ok());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_document_surfaces_core_error_on_existing_path() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n")]).await;
        // a.md already exists → httui_core::fs::create_note errors.
        let err = app
            .create_document(PathBuf::from("a.md"), true)
            .unwrap_err();
        assert!(err.starts_with("create failed:"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn create_folder_makes_dir_and_rejects_existing() {
        let (mut app, _d, vault) = app_with_files(&[("a.md", "a\n")]).await;
        let msg = app.create_folder(PathBuf::from("notes/sub")).unwrap();
        assert_eq!(msg, "created folder \"sub\"");
        assert!(vault.path().join("notes/sub").is_dir());
        // Second attempt on the now-existing path errors.
        let err = app.create_folder(PathBuf::from("notes/sub")).unwrap_err();
        assert!(err.contains("path already exists"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rename_path_moves_file_and_updates_open_panes() {
        let (mut app, _d, vault) =
            app_with_files(&[("a.md", "alpha\n"), ("b.md", "bravo\n")]).await;
        // a.md is the active pane (alphabetical first).
        let msg = app
            .rename_path(Some(PathBuf::from("a.md")), PathBuf::from("renamed/a2.md"))
            .unwrap();
        assert_eq!(msg, "renamed to \"a2.md\"");
        assert!(!vault.path().join("a.md").exists());
        assert!(vault.path().join("renamed/a2.md").is_file());
        // The pane that showed a.md now points at the new path.
        assert_eq!(
            app.document_path().map(|p| p.to_string_lossy().to_string()),
            Some("renamed/a2.md".to_string())
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rename_path_uses_active_path_when_src_none() {
        let (mut app, _d, vault) = app_with_files(&[("a.md", "alpha\n")]).await;
        let msg = app.rename_path(None, PathBuf::from("b.md")).unwrap();
        assert_eq!(msg, "renamed to \"b.md\"");
        assert!(vault.path().join("b.md").is_file());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rename_path_errors_when_destination_exists() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "alpha\n"), ("b.md", "bravo\n")]).await;
        let err = app
            .rename_path(Some(PathBuf::from("a.md")), PathBuf::from("b.md"))
            .unwrap_err();
        assert!(err.starts_with("E13: File exists"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rename_path_errors_with_no_file_name_when_no_src_and_no_active() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "a\n")]).await;
        if let Some(p) = app.active_pane_mut() {
            p.document_path = None;
        }
        let err = app.rename_path(None, PathBuf::from("z.md")).unwrap_err();
        assert_eq!(err, "no file name");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rename_path_drops_search_index_branch_runs() {
        let (mut app, _d, vault) = app_with_files(&[("a.md", "alpha\n")]).await;
        // Flip the gate so the index-maintenance spawn path executes
        // (best-effort fire-and-forget; we only need the sync code +
        // the move itself to succeed).
        app.content_search_index_built = true;
        let msg = app
            .rename_path(Some(PathBuf::from("a.md")), PathBuf::from("a2.md"))
            .unwrap();
        assert_eq!(msg, "renamed to \"a2.md\"");
        assert!(vault.path().join("a2.md").is_file());
        tokio::task::yield_now().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_path_removes_file_and_empties_matching_panes() {
        let (mut app, _d, vault) =
            app_with_files(&[("a.md", "alpha\n"), ("b.md", "bravo\n")]).await;
        // a.md is active; delete it.
        let msg = app.delete_path(Some(PathBuf::from("a.md")), false).unwrap();
        assert_eq!(msg, "deleted \"a.md\"");
        assert!(!vault.path().join("a.md").exists());
        // The pane that showed a.md was emptied.
        assert!(app.active_pane().unwrap().document.is_none());
        assert!(app.active_pane().unwrap().document_path.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_path_removes_a_directory_recursively() {
        let (mut app, _d, vault) = app_with_files(&[("a.md", "a\n"), ("dir/c.md", "c\n")]).await;
        app.content_search_index_built = true;
        let msg = app.delete_path(Some(PathBuf::from("dir")), true).unwrap();
        assert_eq!(msg, "deleted \"dir\"");
        assert!(!vault.path().join("dir").exists());
        tokio::task::yield_now().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_path_refuses_dirty_active_then_force_works() {
        let (mut app, _d, vault) = app_with_files(&[("a.md", "alpha\n")]).await;
        app.document_mut().unwrap().mark_dirty();
        let err = app
            .delete_path(Some(PathBuf::from("a.md")), false)
            .unwrap_err();
        assert!(err.contains("no write since last change"));
        let msg = app.delete_path(Some(PathBuf::from("a.md")), true).unwrap();
        assert_eq!(msg, "deleted \"a.md\"");
        assert!(!vault.path().join("a.md").exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_path_errors_on_missing_target_and_no_file_name() {
        let (mut app, _d, _v) = app_with_files(&[("a.md", "a\n")]).await;
        let err = app
            .delete_path(Some(PathBuf::from("ghost.md")), true)
            .unwrap_err();
        assert!(err.starts_with("delete failed:"));
        // No target + no active path → "no file name".
        if let Some(p) = app.active_pane_mut() {
            p.document_path = None;
        }
        let err = app.delete_path(None, true).unwrap_err();
        assert_eq!(err, "no file name");
    }
}
