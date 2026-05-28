//! Sidebar last-run badges. Two operations over the BLOCKS indices:
//! bulk-enrich from `block_run_history` when the view loads, and patch
//! a single block's badge after a run completes.

use std::path::Path;

use crate::app::{App, BlockIndex, BlockLastRun};

/// Fill each block's [`crate::app::BlockMeta::last_run`] from
/// `block_run_history`. History is keyed by the absolute on-disk path
/// (matches what the run path records via `vault.join(rel)`), so each
/// file's vault-relative path is resolved against `vault` before
/// querying. Queried per file (`list_history_for_file` returns rows
/// DESC); the first row seen per alias is the most recent run.
/// Anonymous blocks carry no history (it's keyed by alias) and stay
/// `None`. Query failures are skipped silently — badges are
/// best-effort and never block the view.
pub async fn enrich_last_runs(index: &mut BlockIndex, vault: &Path, pool: &sqlx::SqlitePool) {
    use std::collections::HashMap;
    for file in index.files.iter_mut() {
        if file.blocks.iter().all(|b| b.alias.is_none()) {
            continue;
        }
        let abs = vault.join(&file.path);
        let limit = (file.blocks.len() as i64 * 10).max(50);
        let entries = match httui_core::block_history::list_history_for_file(
            pool,
            &abs.to_string_lossy(),
            limit,
        )
        .await
        {
            Ok(e) => e,
            Err(_) => continue,
        };
        let mut latest: HashMap<&str, &httui_core::block_history::HistoryEntry> = HashMap::new();
        for e in &entries {
            latest.entry(e.block_alias.as_str()).or_insert(e);
        }
        for block in file.blocks.iter_mut() {
            let Some(alias) = block.alias.as_deref() else {
                continue;
            };
            if let Some(e) = latest.get(alias) {
                block.last_run = Some(BlockLastRun {
                    status: e.status,
                    outcome: e.outcome.clone(),
                });
            }
        }
    }
}

/// Update one block's [`crate::app::BlockMeta::last_run`] in the live
/// BLOCKS indices right after a run completes, so the sidebar badge
/// reflects the fresh result without waiting for the next view toggle
/// (the `block_run_history` insert is fire-and-forget, so re-querying
/// here would race it — we feed the in-hand values instead). No-op
/// outside BLOCKS view (`block_index` is `None`) and for anonymous
/// blocks (matched by alias). `file_abs` is the on-disk path the run
/// recorded; it's resolved back to the vault-relative form the indices
/// use.
pub fn refresh_block_badge(app: &mut App, file_abs: &str, alias: &str, last_run: BlockLastRun) {
    if app.tree.block_index.is_none() {
        return;
    }
    let vault = app.vault_path.clone();
    let rel = Path::new(file_abs)
        .strip_prefix(&vault)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| file_abs.to_string());
    let apply = |index: &mut BlockIndex| {
        for f in index.files.iter_mut() {
            if f.display != rel && f.path.to_string_lossy() != rel {
                continue;
            }
            for b in f.blocks.iter_mut() {
                if b.alias.as_deref() == Some(alias) {
                    b.last_run = Some(last_run.clone());
                }
            }
        }
    };
    if let Some(index) = app.tree.block_index.as_mut() {
        apply(index);
    }
    if let Some(ws) = app.blocks_workspace.as_mut() {
        apply(&mut ws.index);
    }
    app.tree.refresh(&vault);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{BlockMeta, FileBlocks};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn seed() -> TempDir {
        let v = TempDir::new().unwrap();
        std::fs::write(
            v.path().join("api.md"),
            "# api\n\n```http alias=login\nGET https://x.com\n```\n",
        )
        .unwrap();
        v
    }

    async fn seed_history(
        vault: &Path,
        rel: &str,
        alias: &str,
        method: &str,
        status: Option<i64>,
        outcome: &str,
    ) -> (sqlx::SqlitePool, TempDir) {
        let data = TempDir::new().unwrap();
        let pool = httui_core::db::init_db(data.path()).await.unwrap();
        let abs = vault.join(rel);
        httui_core::block_history::insert_history_entry(
            &pool,
            httui_core::block_history::InsertEntry {
                file_path: abs.to_string_lossy().to_string(),
                block_alias: alias.into(),
                method: method.into(),
                url_canonical: "/".into(),
                status,
                request_size: None,
                response_size: None,
                elapsed_ms: Some(10),
                outcome: outcome.into(),
                plan: None,
            },
        )
        .await
        .unwrap();
        (pool, data)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enrich_fills_http_status_for_aliased_block() {
        let v = seed();
        let (pool, _data) =
            seed_history(v.path(), "api.md", "login", "GET", Some(200), "ok").await;
        let mut index = BlockIndex::build(v.path());
        enrich_last_runs(&mut index, v.path(), &pool).await;
        let api = index.files.iter().find(|f| f.display == "api.md").unwrap();
        let lr = api.blocks[0].last_run.as_ref().expect("login has history");
        assert_eq!(lr.status, Some(200));
        assert_eq!(lr.outcome, "ok");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enrich_carries_error_outcome() {
        let v = seed();
        let (pool, _data) =
            seed_history(v.path(), "api.md", "login", "GET", Some(500), "error").await;
        let mut index = BlockIndex::build(v.path());
        enrich_last_runs(&mut index, v.path(), &pool).await;
        let api = index.files.iter().find(|f| f.display == "api.md").unwrap();
        let lr = api.blocks[0].last_run.as_ref().unwrap();
        assert_eq!(lr.status, Some(500));
        assert_eq!(lr.outcome, "error");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enrich_keeps_latest_run_per_alias() {
        let v = seed();
        let (pool, _data) =
            seed_history(v.path(), "api.md", "login", "GET", Some(500), "error").await;
        // A newer success must win over the earlier failure.
        let abs = v.path().join("api.md");
        httui_core::block_history::insert_history_entry(
            &pool,
            httui_core::block_history::InsertEntry {
                file_path: abs.to_string_lossy().to_string(),
                block_alias: "login".into(),
                method: "GET".into(),
                url_canonical: "/".into(),
                status: Some(200),
                request_size: None,
                response_size: None,
                elapsed_ms: Some(5),
                outcome: "ok".into(),
                plan: None,
            },
        )
        .await
        .unwrap();
        let mut index = BlockIndex::build(v.path());
        enrich_last_runs(&mut index, v.path(), &pool).await;
        let api = index.files.iter().find(|f| f.display == "api.md").unwrap();
        let lr = api.blocks[0].last_run.as_ref().unwrap();
        assert_eq!(lr.status, Some(200), "newest run should win");
        assert_eq!(lr.outcome, "ok");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enrich_leaves_block_without_history_as_none() {
        let v = seed();
        // Pool has a row for a different alias only.
        let (pool, _data) =
            seed_history(v.path(), "api.md", "other", "GET", Some(200), "ok").await;
        let mut index = BlockIndex::build(v.path());
        enrich_last_runs(&mut index, v.path(), &pool).await;
        let api = index.files.iter().find(|f| f.display == "api.md").unwrap();
        assert!(api.blocks[0].last_run.is_none());
    }

    async fn blocks_app() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = httui_core::db::init_db(data.path()).await.unwrap();
        let resolved = crate::vault::ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let mut app = App::new(crate::config::Config::default(), resolved, pool);
        let index = BlockIndex {
            files: vec![FileBlocks {
                path: PathBuf::from("api.md"),
                display: "api.md".into(),
                blocks: vec![BlockMeta {
                    alias: Some("login".into()),
                    block_type: "http".into(),
                    line_start: 1,
                    last_run: None,
                }],
            }],
        };
        app.tree.block_index = Some(index);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refresh_block_badge_sets_last_run_on_matching_block() {
        let (mut app, _d, vault) = blocks_app().await;
        let abs = vault.path().join("api.md");
        refresh_block_badge(
            &mut app,
            &abs.to_string_lossy(),
            "login",
            BlockLastRun {
                status: Some(200),
                outcome: "ok".into(),
            },
        );
        let lr = app.tree.block_index.as_ref().unwrap().files[0].blocks[0]
            .last_run
            .as_ref()
            .expect("badge applied");
        assert_eq!(lr.status, Some(200));
        assert_eq!(lr.outcome, "ok");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refresh_block_badge_noop_when_not_in_blocks_view() {
        let (mut app, _d, vault) = blocks_app().await;
        app.tree.block_index = None;
        let abs = vault.path().join("api.md");
        refresh_block_badge(
            &mut app,
            &abs.to_string_lossy(),
            "login",
            BlockLastRun {
                status: Some(500),
                outcome: "error".into(),
            },
        );
        assert!(app.tree.block_index.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refresh_block_badge_ignores_unknown_alias() {
        let (mut app, _d, vault) = blocks_app().await;
        let abs = vault.path().join("api.md");
        refresh_block_badge(
            &mut app,
            &abs.to_string_lossy(),
            "nope",
            BlockLastRun {
                status: Some(200),
                outcome: "ok".into(),
            },
        );
        assert!(app.tree.block_index.as_ref().unwrap().files[0].blocks[0]
            .last_run
            .is_none());
    }
}
