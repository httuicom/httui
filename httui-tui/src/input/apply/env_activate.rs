//! V4 P6 (2026-05-23): activate-env-by-index handler. Extraído de
//! `apply/pickers.rs` pra não engrossar mais o monolito pré-existente.

use crate::app::{App, StatusKind};

/// Ativa o env de índice `idx` (1-based, 1..9). Resolve via
/// `EnvironmentsStore::list_envs` na ordem de disco; no-op silencioso
/// se índice fora dos limites. Dismissa o picker/page se aberto.
pub(crate) fn apply_activate_env_by_index(app: &mut App, idx: usize) {
    if !(1..=9).contains(&idx) {
        return;
    }
    let store = app.environments_store.clone();
    let envs = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(store.list_envs())
            .unwrap_or_default()
    });
    let Some(env) = envs.get(idx - 1) else {
        app.set_status(
            StatusKind::Info,
            format!("env #{idx} não existe ({} envs total)", envs.len()),
        );
        return;
    };
    let name = env.name.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.set_active_env(Some(&name)))
    });
    if let Err(e) = result {
        app.set_status(StatusKind::Error, format!("set active env failed: {e}"));
        return;
    }
    app.refresh_active_env_name();
    // Dismiss picker/page (matches o comportamento de ConfirmEnvironmentPicker).
    let was_page = matches!(app.modal, Some(crate::modal::Modal::EnvsPage(_)));
    let was_picker = matches!(app.modal, Some(crate::modal::Modal::EnvironmentPicker(_)));
    if was_page || was_picker {
        app.modal = None;
        if was_page {
            // Reabre EnvsPage com active atualizado pra que o user veja o
            // novo ● (picker comportamento padrão = fecha + volta normal).
            let _ = super::envs_page::open_envs_page(app);
        } else {
            app.vim.enter_normal();
        }
    }
    app.set_status(StatusKind::Info, format!("env: {name}"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use tempfile::TempDir;

    async fn app_fixture() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        std::fs::write(vault.path().join("note.md"), "stub\n").unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        (App::new(Config::default(), resolved, pool), data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn idx_zero_is_noop() {
        let (mut app, _d, _v) = app_fixture().await;
        apply_activate_env_by_index(&mut app, 0);
        assert!(app.active_env_name.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn idx_out_of_range_sets_info_status() {
        let (mut app, _d, _v) = app_fixture().await;
        apply_activate_env_by_index(&mut app, 3);
        assert!(app.status_message.is_some());
        assert!(app.active_env_name.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn activate_existing_env_by_index() {
        let (mut app, _d, _v) = app_fixture().await;
        app.environments_store.create_env("alpha").await.unwrap();
        app.environments_store.create_env("beta").await.unwrap();
        apply_activate_env_by_index(&mut app, 2);
        assert_eq!(app.active_env_name.as_deref(), Some("beta"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn activate_dismisses_envs_page_modal() {
        let (mut app, _d, _v) = app_fixture().await;
        app.environments_store.create_env("alpha").await.unwrap();
        // Abre a page primeiro.
        crate::input::apply::envs_page::apply_envs(
            &mut app,
            crate::input::action::Action::OpenEnvsPage,
        );
        assert!(matches!(app.modal, Some(crate::modal::Modal::EnvsPage(_))));
        apply_activate_env_by_index(&mut app, 1);
        // Reabre EnvsPage com active atualizado (não fica None).
        assert!(matches!(app.modal, Some(crate::modal::Modal::EnvsPage(_))));
        assert_eq!(app.active_env_name.as_deref(), Some("alpha"));
    }
}
