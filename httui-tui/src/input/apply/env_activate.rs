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
