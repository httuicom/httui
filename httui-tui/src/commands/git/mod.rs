//! Git operations consumed by the side panel and the status bar.
//! Every git invocation lives in `httui_core::git`; this module owns
//! the wiring (sync calls, error propagation onto the panel state, /
//! eventually status-bar refresh cadence).

use crate::app::App;
use httui_core::git::staging::{git_commit, stage_path};
use httui_core::git::status::DiffMetrics;

/// Refresh the panel's `git status` snapshot plus diff metrics.
/// Errors (not a git repo, `git` missing) are surfaced through
/// `status_error` so the renderer can show a friendly message; both
/// `status` and `metrics` are reset in that case so stale data never
/// bleeds across vaults. Shortstat is best-effort: a failure there is
/// silently downgraded to zero counts (the status snapshot is the
/// load-bearing piece — UI never gates on metrics).
pub fn refresh_git_status(app: &mut App) {
    let vault = app.vault_path.clone();
    match httui_core::git::git_status(&vault) {
        Ok(status) => {
            let max = status.changed.len().saturating_sub(1);
            if app.git_panel.selected > max {
                app.git_panel.selected = max;
            }
            app.git_panel.status = Some(status);
            app.git_panel.status_error = None;
            app.git_panel.metrics =
                httui_core::git::git_diff_shortstat(&vault).unwrap_or_default();
        }
        Err(msg) => {
            app.git_panel.status = None;
            app.git_panel.status_error = Some(msg);
            app.git_panel.metrics = DiffMetrics::default();
        }
    }
}

/// Stage every changed file from the panel's last snapshot and run
/// `git commit -m <message>`. Returns `Ok(())` on success, `Err(msg)`
/// when there's nothing to commit, no fresh status snapshot, or git
/// rejects the commit (hook failure, identity unset, etc.). Refreshes
/// the panel's status snapshot regardless of outcome so the user
/// sees the new state.
pub fn commit_changes(app: &mut App, message: &str) -> Result<(), String> {
    let vault = app.vault_path.clone();
    let Some(status) = app.git_panel.status.as_ref() else {
        return Err("no git status snapshot — refresh first".to_string());
    };
    if status.changed.is_empty() {
        return Err("nothing to commit".to_string());
    }
    let paths: Vec<String> = status.changed.iter().map(|c| c.path.clone()).collect();
    for path in &paths {
        stage_path(&vault, path)?;
    }
    let result = git_commit(&vault, message, false);
    refresh_git_status(app);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn build_app() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn non_git_vault_sets_status_error() {
        let (mut app, _d, _v) = build_app().await;
        refresh_git_status(&mut app);
        assert!(app.git_panel.status.is_none());
        let err = app
            .git_panel
            .status_error
            .as_ref()
            .expect("error populated for non-git vault");
        assert!(err.contains("not a git repository") || err.contains("fatal"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn git_repo_clears_previous_error() {
        let (mut app, _d, vault) = build_app().await;
        // Prime with a stale error.
        app.git_panel.status_error = Some("stale".to_string());
        crate::git::test_helpers::init_repo(vault.path());

        refresh_git_status(&mut app);
        assert!(app.git_panel.status_error.is_none());
        assert!(app.git_panel.status.is_some());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refresh_clamps_selection_when_list_shrinks() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("a.md"), "x\n").unwrap();
        refresh_git_status(&mut app);
        // 1 untracked file → selected may be 0; force it past the end
        // and ensure refresh clamps.
        app.git_panel.selected = 9;
        refresh_git_status(&mut app);
        let len = app.git_panel.status.as_ref().unwrap().changed.len();
        assert!(app.git_panel.selected <= len.saturating_sub(1));
    }
}
