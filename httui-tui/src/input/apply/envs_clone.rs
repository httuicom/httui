//! V4 P5 (2026-05-23): clone-env form handlers. Extraído de
//! `apply/envs_page.rs` pra manter cada arquivo abaixo do limite
//! 600L do DoD (TUI Vim Monolith Split). Sem mudança de comportamento.

use crate::app::{App, CloneVarRow, EnvCloneFormFocus, EnvCloneFormState, StatusKind};
use crate::vim::lineedit::LineEdit;
use crate::vim::mode::Mode;
use httui_core::vault_config::SetVarInput;

pub(crate) fn with_clone_form(app: &mut App, f: impl FnOnce(&mut EnvCloneFormState)) {
    if let Some(crate::modal::Modal::EnvCloneForm(s)) = app.modal.as_mut() {
        f(s);
    }
}

/// Abre o clone form. Source = env selecionado na EnvsPage; nome
/// destino pré-preenchido como `<source>-copy`; lista de vars vem do
/// source com `checked = true` (default copia tudo).
pub(crate) fn open_env_clone_form(app: &mut App) {
    let source = if let Some(crate::modal::Modal::EnvsPage(s)) = app.modal.as_ref() {
        s.envs.get(s.selected_env).map(|e| e.name.clone())
    } else {
        None
    };
    let Some(source) = source else { return };
    let store = app.environments_store.clone();
    let vars = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(store.list_vars(&source))
            .unwrap_or_default()
    });
    let rows: Vec<CloneVarRow> = vars
        .into_iter()
        .map(|v| CloneVarRow {
            key: v.key,
            value: v.value,
            is_secret: v.is_secret,
            checked: true,
        })
        .collect();
    let default_name = format!("{source}-copy");
    app.modal = Some(crate::modal::Modal::EnvCloneForm(EnvCloneFormState {
        source,
        name: LineEdit::from_str(default_name),
        vars: rows,
        selected_var: 0,
        focus: EnvCloneFormFocus::Name,
        error: None,
    }));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
}

/// Cria env destino + bulk set_var apenas das vars marcadas.
/// Per-var failures acumulam e são reportadas no status; o env já foi
/// criado, então fechamos a modal mesmo no caso parcial.
pub(crate) fn env_clone_form_submit(app: &mut App) {
    let (name, source, picks) =
        if let Some(crate::modal::Modal::EnvCloneForm(s)) = app.modal.as_ref() {
            let picks: Vec<(String, String, bool)> = s
                .vars
                .iter()
                .filter(|v| v.checked)
                .map(|v| (v.key.clone(), v.value.clone(), v.is_secret))
                .collect();
            (s.name.as_str().trim().to_string(), s.source.clone(), picks)
        } else {
            return;
        };
    if name.is_empty() {
        if let Some(crate::modal::Modal::EnvCloneForm(s)) = app.modal.as_mut() {
            s.error = Some("name is required".into());
        }
        return;
    }
    if name == source {
        if let Some(crate::modal::Modal::EnvCloneForm(s)) = app.modal.as_mut() {
            s.error = Some("destination must differ from source".into());
        }
        return;
    }
    let store = app.environments_store.clone();
    let create_res = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.create_env(&name))
    });
    if let Err(e) = create_res {
        if let Some(crate::modal::Modal::EnvCloneForm(s)) = app.modal.as_mut() {
            s.error = Some(e);
        }
        return;
    }
    let total = picks.len();
    let mut copied = 0usize;
    let mut failed: Vec<String> = Vec::new();
    for (key, value, is_secret) in picks {
        let input = SetVarInput {
            env_name: name.clone(),
            key: key.clone(),
            value,
            is_secret,
        };
        let r = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(store.set_var(input))
        });
        match r {
            Ok(_) => copied += 1,
            Err(_) => failed.push(key),
        }
    }
    app.modal = None;
    let _ = super::envs_page::open_envs_page(app);
    super::envs_page::with_page_select_env(app, &name);
    super::envs_page::reload_vars_export(app);
    if failed.is_empty() {
        app.set_status(
            StatusKind::Info,
            format!("cloned \"{source}\" → \"{name}\" ({copied}/{total} vars)"),
        );
    } else {
        app.set_status(
            StatusKind::Error,
            format!(
                "cloned \"{name}\" but {} var(s) failed: {}",
                failed.len(),
                failed.join(", ")
            ),
        );
    }
}
