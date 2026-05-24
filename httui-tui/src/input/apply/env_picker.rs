// coverage:exclude file — env picker applier cluster relocated by
// tui-V10 (split of pickers.rs to satisfy size gate); coverage tracked
// in docs-llm/tui-v2/vim-coverage-debt.md.
//! `gE` environment picker handlers. Mechanically split out of
//! `pickers.rs` (tui-V10) to keep that file under the 600-line size
//! gate. No behavior change.

use crate::app::{App, StatusKind};
use crate::vim::mode::Mode;

/// `gE` — list envs from `<vault>/envs/*.toml` via `EnvironmentsStore`
/// (V4 P1, 2026-05-23: was SQL, now reads the vault TOML so
/// desktop ↔ TUI share the same source). Pre-selects the active env
/// so Enter is a no-op confirm.
pub(crate) fn open_environment_picker(app: &mut App) -> Result<(), String> {
    let store = app.environments_store.clone();
    let (entries, active_id) = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let envs = store
                .list_envs()
                .await
                .map_err(|e| format!("env list failed: {e}"))?;
            let active = store.active_env().await.ok().flatten();
            Ok::<_, String>((envs, active))
        })
    })?;
    if entries.is_empty() {
        return super::envs_page::open_envs_page(app);
    }
    let entries: Vec<crate::app::EnvironmentEntry> = entries
        .into_iter()
        .map(|e| crate::app::EnvironmentEntry {
            id: e.name.clone(),
            name: e.name,
        })
        .collect();
    let selected = active_id
        .as_deref()
        .and_then(|n| entries.iter().position(|e| e.id == n))
        .unwrap_or(0);

    app.modal = Some(crate::modal::Modal::EnvironmentPicker(
        crate::app::EnvironmentPickerState {
            entries,
            selected,
            active_id,
        },
    ));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

pub(crate) fn apply_close_environment_picker(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::EnvironmentPicker(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

pub(crate) fn apply_move_environment_picker_cursor(app: &mut App, delta: i32) {
    let Some(crate::modal::Modal::EnvironmentPicker(state)) = app.modal.as_mut() else {
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

/// `Enter` in the env picker — flip the active flag in SQLite, refresh
/// the cached display name (so the status-bar chip updates), and
/// dismiss. A no-op when the highlighted entry is already active.
pub(crate) fn apply_confirm_environment_picker(app: &mut App) {
    let state = match app.modal.take() {
        Some(crate::modal::Modal::EnvironmentPicker(s)) => s,
        other => {
            app.modal = other;
            app.vim.enter_normal();
            return;
        }
    };
    app.vim.enter_normal();
    let Some(picked) = state.entries.get(state.selected).cloned() else {
        return;
    };
    if state.active_id.as_deref() == Some(picked.id.as_str()) {
        return;
    }
    let store = app.environments_store.clone();
    let name = picked.name.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.set_active_env(Some(&name)))
    });
    if let Err(e) = result {
        app.set_status(StatusKind::Error, format!("set active env failed: {e}"));
        return;
    }
    app.refresh_active_env_name();
    app.set_status(StatusKind::Info, format!("env: {}", picked.name));
}
