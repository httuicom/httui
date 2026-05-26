//! Git side-panel action handlers. Owns the toggle + (later) commit
//! form / list navigation. Reads from `httui_core::git::status` via
//! the [`refresh_git_status`](crate::commands::git::refresh_git_status)
//! helper so the renderer just projects state.

use crate::app::App;
use crate::input::action::Action;

pub(crate) fn apply_git_panel(app: &mut App, action: Action) {
    match action {
        Action::GitPanelToggle => {
            let now_visible = app.git_panel.toggle_visible();
            if now_visible {
                crate::commands::git::refresh_git_status(app);
            }
        }
        _ => unreachable!("apply_git_panel: variante fora do grupo"),
    }
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
    async fn toggle_flips_visibility_and_refreshes_on_open() {
        let (mut app, _d, _v) = build_app().await;
        assert!(!app.git_panel.visible);

        apply_git_panel(&mut app, Action::GitPanelToggle);
        assert!(app.git_panel.visible);
        // Vault isn't a git repo → status_error populated, status stays None.
        // Either branch is acceptable here; what matters is that a
        // refresh attempt happened (no panic, panel transitioned).

        apply_git_panel(&mut app, Action::GitPanelToggle);
        assert!(!app.git_panel.visible);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_on_a_git_repo_populates_status() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::fs::write(vault.path().join("note.md"), "hi\n").unwrap();

        apply_git_panel(&mut app, Action::GitPanelToggle);
        assert!(app.git_panel.visible);
        let status = app
            .git_panel
            .status
            .as_ref()
            .expect("status populated for a real repo");
        assert_eq!(status.branch.as_deref(), Some("main"));
        // `note.md` is untracked → shows up in `changed`.
        assert!(status.changed.iter().any(|c| c.path == "note.md"));
    }

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic(expected = "apply_git_panel: variante fora do grupo")]
    async fn unexpected_variant_panics() {
        let (mut app, _d, _v) = build_app().await;
        apply_git_panel(&mut app, Action::Quit);
    }
}
