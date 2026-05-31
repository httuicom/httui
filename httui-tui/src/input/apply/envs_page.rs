//! V4 P2-P4 (2026-05-23): Vars + Envs page handlers.

use crate::app::{
    App, EnvFormState, EnvSummary, EnvsPageState, EnvsPaneFocus, StatusKind, VarFormFocus,
    VarFormState, VarRow,
};
use crate::input::action::Action;
use crate::vim::lineedit::LineEdit;
use crate::vim::mode::Mode;
use httui_core::vault_config::SetVarInput;

pub(crate) fn apply_envs(app: &mut App, action: Action) {
    match action {
        Action::OpenEnvsPage => {
            if let Err(e) = open_envs_page(app) {
                app.set_status(StatusKind::Error, e);
            }
        }
        Action::CloseEnvsPage => {
            if matches!(app.modal, Some(crate::modal::Modal::EnvsPage(_))) {
                app.modal = None;
            }
            app.vim.enter_normal();
        }
        Action::EnvsPageFocusToggle => with_page(app, |s| {
            s.focus = match s.focus {
                EnvsPaneFocus::Envs => EnvsPaneFocus::Vars,
                EnvsPaneFocus::Vars => EnvsPaneFocus::Envs,
            };
        }),
        Action::EnvsPageFocusEnvs => with_page(app, |s| s.focus = EnvsPaneFocus::Envs),
        Action::EnvsPageFocusVars => with_page(app, |s| s.focus = EnvsPaneFocus::Vars),
        Action::EnvsPageMoveEnvCursor(d) => {
            move_env_cursor(app, d);
            reload_vars(app);
        }
        Action::EnvsPageMoveVarCursor(d) => {
            with_page(app, |s| {
                if s.vars.is_empty() {
                    return;
                }
                let last = s.vars.len() as i64 - 1;
                s.selected_var = ((s.selected_var as i64 + d as i64).clamp(0, last)) as usize;
            });
            refresh_var_uses(app);
        }
        Action::EnvsPageActivateEnv => activate_selected_env(app),
        Action::OpenEnvForm => open_env_form(app, false),
        Action::OpenEnvEditForm => open_env_form(app, true),
        Action::CloseEnvForm => {
            close_form_and_reopen(app, |m| matches!(m, Some(crate::modal::Modal::EnvForm(_))));
        }
        Action::EnvFormChar(c) => with_env_form(app, |f| f.name.insert_char(c)),
        Action::EnvFormBackspace => with_env_form(app, |f| {
            f.name.delete_before();
        }),
        Action::EnvFormDelete => with_env_form(app, |f| {
            f.name.delete_after();
        }),
        Action::EnvFormCursorLeft => with_env_form(app, |f| f.name.move_left()),
        Action::EnvFormCursorRight => with_env_form(app, |f| f.name.move_right()),
        Action::EnvFormHome => with_env_form(app, |f| f.name.move_home()),
        Action::EnvFormEnd => with_env_form(app, |f| f.name.move_end()),
        Action::EnvFormSubmit => env_form_submit(app),
        Action::OpenVarForm => open_var_form(app, false),
        Action::OpenVarEditForm => open_var_form(app, true),
        Action::CloseVarForm => {
            close_form_and_reopen(app, |m| matches!(m, Some(crate::modal::Modal::VarForm(_))));
        }
        Action::VarFormChar(c) => with_var_form(app, |f| match f.focus {
            VarFormFocus::Key => f.key.insert_char(c),
            VarFormFocus::Value => f.value.insert_char(c),
            VarFormFocus::Secret => {}
        }),
        Action::VarFormBackspace => with_var_form(app, |f| {
            match f.focus {
                VarFormFocus::Key => {
                    f.key.delete_before();
                }
                VarFormFocus::Value => {
                    f.value.delete_before();
                }
                VarFormFocus::Secret => {}
            };
        }),
        Action::VarFormDelete => with_var_form(app, |f| {
            match f.focus {
                VarFormFocus::Key => {
                    f.key.delete_after();
                }
                VarFormFocus::Value => {
                    f.value.delete_after();
                }
                VarFormFocus::Secret => {}
            };
        }),
        Action::VarFormCursorLeft => with_var_form(app, |f| match f.focus {
            VarFormFocus::Key => f.key.move_left(),
            VarFormFocus::Value => f.value.move_left(),
            VarFormFocus::Secret => {}
        }),
        Action::VarFormCursorRight => with_var_form(app, |f| match f.focus {
            VarFormFocus::Key => f.key.move_right(),
            VarFormFocus::Value => f.value.move_right(),
            VarFormFocus::Secret => {}
        }),
        Action::VarFormHome => with_var_form(app, |f| match f.focus {
            VarFormFocus::Key => f.key.move_home(),
            VarFormFocus::Value => f.value.move_home(),
            VarFormFocus::Secret => {}
        }),
        Action::VarFormEnd => with_var_form(app, |f| match f.focus {
            VarFormFocus::Key => f.key.move_end(),
            VarFormFocus::Value => f.value.move_end(),
            VarFormFocus::Secret => {}
        }),
        Action::VarFormFocusNext => with_var_form(app, |f| f.focus = f.focus.next()),
        Action::VarFormFocusPrev => with_var_form(app, |f| f.focus = f.focus.prev()),
        Action::VarFormToggleSecret => with_var_form(app, |f| f.is_secret = !f.is_secret),
        Action::VarFormSubmit => var_form_submit(app),
        Action::OpenEnvDeleteConfirm => open_env_delete_confirm(app),
        Action::OpenVarDeleteConfirm => open_var_delete_confirm(app),
        Action::ConfirmEnvOrVarDelete => confirm_delete(app),
        Action::CancelEnvOrVarDelete => {
            // Reopen envs page (cheap reload).
            let _ = open_envs_page(app);
        }
        Action::OpenEnvCloneForm => super::envs_clone::open_env_clone_form(app),
        Action::CloseEnvCloneForm => {
            close_form_and_reopen(app, |m| {
                matches!(m, Some(crate::modal::Modal::EnvCloneForm(_)))
            });
        }
        Action::EnvCloneFormChar(c) => {
            super::envs_clone::with_clone_form(app, |f| f.name.insert_char(c))
        }
        Action::EnvCloneFormBackspace => super::envs_clone::with_clone_form(app, |f| {
            f.name.delete_before();
        }),
        Action::EnvCloneFormFocusToggle => super::envs_clone::with_clone_form(app, |f| {
            use crate::app::EnvCloneFormFocus;
            f.focus = match f.focus {
                EnvCloneFormFocus::Name => EnvCloneFormFocus::Vars,
                EnvCloneFormFocus::Vars => EnvCloneFormFocus::Name,
            };
        }),
        Action::EnvCloneFormMoveVarCursor(d) => super::envs_clone::with_clone_form(app, |f| {
            if f.vars.is_empty() {
                return;
            }
            let last = f.vars.len() as i64 - 1;
            f.selected_var = ((f.selected_var as i64 + d as i64).clamp(0, last)) as usize;
        }),
        Action::EnvCloneFormToggleVar => super::envs_clone::with_clone_form(app, |f| {
            if let Some(row) = f.vars.get_mut(f.selected_var) {
                row.checked = !row.checked;
            }
        }),
        Action::EnvCloneFormToggleAll => super::envs_clone::with_clone_form(app, |f| {
            let any_unchecked = f.vars.iter().any(|v| !v.checked);
            for row in &mut f.vars {
                row.checked = any_unchecked;
            }
        }),
        Action::EnvCloneFormSubmit => super::envs_clone::env_clone_form_submit(app),
        _ => unreachable!("apply_envs: variante fora do grupo"),
    }
}

pub(crate) fn open_envs_page(app: &mut App) -> Result<(), String> {
    let store = app.environments_store.clone();
    let (envs_pub, active) = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let envs = store
                .list_envs()
                .await
                .map_err(|e| format!("env list failed: {e}"))?;
            let active = store.active_env().await.ok().flatten();
            Ok::<_, String>((envs, active))
        })
    })?;
    let envs: Vec<EnvSummary> = envs_pub
        .into_iter()
        .map(|e| EnvSummary { name: e.name })
        .collect();
    // Pre-select o env ativo (falls back pro primeiro se nada ativo).
    let selected_env = active
        .as_deref()
        .and_then(|name| envs.iter().position(|e| e.name == name))
        .unwrap_or(0);
    let vars = if let Some(env) = envs.get(selected_env) {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(store.list_vars(&env.name))
                .map(|vs| {
                    vs.into_iter()
                        .map(|v| VarRow {
                            key: v.key,
                            value: v.value,
                            is_secret: v.is_secret,
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
    } else {
        Vec::new()
    };
    // Foco default vai pras vars quando há env (caso comum:
    // user quer editar valores), e cai pra envs quando vazio
    // (pra ter o "press n to add" sob a barra de foco).
    let focus = if envs.is_empty() {
        EnvsPaneFocus::Envs
    } else {
        EnvsPaneFocus::Vars
    };
    app.modal = Some(crate::modal::Modal::EnvsPage(EnvsPageState {
        envs,
        active,
        selected_env,
        vars,
        selected_var: 0,
        focus,
        var_uses: Vec::new(),
    }));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
    // V4 P7: dispara grep inicial para a primeira var (se houver).
    refresh_var_uses(app);
    Ok(())
}

fn with_page(app: &mut App, f: impl FnOnce(&mut EnvsPageState)) {
    if let Some(crate::modal::Modal::EnvsPage(s)) = app.modal.as_mut() {
        f(s);
    }
}

fn with_env_form(app: &mut App, f: impl FnOnce(&mut EnvFormState)) {
    if let Some(crate::modal::Modal::EnvForm(s)) = app.modal.as_mut() {
        f(s);
    }
}

fn with_var_form(app: &mut App, f: impl FnOnce(&mut VarFormState)) {
    if let Some(crate::modal::Modal::VarForm(s)) = app.modal.as_mut() {
        f(s);
    }
}

fn move_env_cursor(app: &mut App, d: i32) {
    with_page(app, |s| {
        if s.envs.is_empty() {
            return;
        }
        let last = s.envs.len() as i64 - 1;
        s.selected_env = ((s.selected_env as i64 + d as i64).clamp(0, last)) as usize;
        s.selected_var = 0;
    });
}

fn reload_vars(app: &mut App) {
    let store = app.environments_store.clone();
    let name = if let Some(crate::modal::Modal::EnvsPage(s)) = app.modal.as_ref() {
        s.envs.get(s.selected_env).map(|e| e.name.clone())
    } else {
        None
    };
    if let Some(name) = name {
        let vars = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(store.list_vars(&name))
                .unwrap_or_default()
        });
        with_page(app, |s| {
            s.vars = vars
                .into_iter()
                .map(|v| VarRow {
                    key: v.key,
                    value: v.value,
                    is_secret: v.is_secret,
                })
                .collect();
            // V4 P7: clamp cursor + dispara reload do used-in.
            if s.selected_var >= s.vars.len() {
                s.selected_var = s.vars.len().saturating_sub(1);
            }
        });
        refresh_var_uses(app);
    }
}

/// V4 P7: vault-grep da var selecionada. Pular se sem vault path ou
/// var inválida. Falha de IO produz lista vazia (não-fatal).
pub(super) fn refresh_var_uses(app: &mut App) {
    let key = if let Some(crate::modal::Modal::EnvsPage(s)) = app.modal.as_ref() {
        s.vars.get(s.selected_var).map(|v| v.key.clone())
    } else {
        None
    };
    let uses = match (key, app.vault_path.to_str()) {
        (Some(k), Some(p)) => httui_core::var_uses::grep_var_uses(p, &k).unwrap_or_default(),
        _ => Vec::new(),
    };
    with_page(app, |s| s.var_uses = uses);
}

fn activate_selected_env(app: &mut App) {
    let name = if let Some(crate::modal::Modal::EnvsPage(s)) = app.modal.as_ref() {
        s.envs.get(s.selected_env).map(|e| e.name.clone())
    } else {
        None
    };
    let Some(name) = name else { return };
    let store = app.environments_store.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.set_active_env(Some(&name)))
    });
    if let Err(e) = result {
        app.set_status(StatusKind::Error, format!("activate failed: {e}"));
        return;
    }
    app.refresh_active_env_name();
    with_page(app, |s| s.active = Some(name.clone()));
    app.set_status(StatusKind::Info, format!("env: {name}"));
}

fn open_env_form(app: &mut App, edit: bool) {
    let editing_name = if edit {
        if let Some(crate::modal::Modal::EnvsPage(s)) = app.modal.as_ref() {
            s.envs.get(s.selected_env).map(|e| e.name.clone())
        } else {
            None
        }
    } else {
        None
    };
    if edit && editing_name.is_none() {
        return;
    }
    let state = EnvFormState {
        name: editing_name
            .as_deref()
            .map(LineEdit::from_str)
            .unwrap_or_default(),
        editing: editing_name,
        error: None,
    };
    app.modal = Some(crate::modal::Modal::EnvForm(state));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
}

fn env_form_submit(app: &mut App) {
    let (name, editing) = if let Some(crate::modal::Modal::EnvForm(s)) = app.modal.as_ref() {
        (s.name.as_str().trim().to_string(), s.editing.clone())
    } else {
        return;
    };
    if name.is_empty() {
        if let Some(crate::modal::Modal::EnvForm(s)) = app.modal.as_mut() {
            s.error = Some("name is required".into());
        }
        return;
    }
    let store = app.environments_store.clone();
    let result = match editing.clone() {
        Some(old) if old != name => tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(store.rename_env(&old, &name))
        }),
        Some(_) => Ok(()),
        None => tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(store.create_env(&name))
                .map(|_| ())
        }),
    };
    match result {
        Ok(()) => {
            app.modal = None;
            let _ = open_envs_page(app);
            app.refresh_active_env_name();
            app.set_status(
                StatusKind::Info,
                format!(
                    "{} env \"{name}\"",
                    if editing.is_some() {
                        "renamed"
                    } else {
                        "created"
                    }
                ),
            );
        }
        Err(e) => {
            if let Some(crate::modal::Modal::EnvForm(s)) = app.modal.as_mut() {
                s.error = Some(e);
            }
        }
    }
}

fn open_var_form(app: &mut App, edit: bool) {
    let (env_name, editing_var) = if let Some(crate::modal::Modal::EnvsPage(s)) = app.modal.as_ref()
    {
        let env = match s.envs.get(s.selected_env) {
            Some(e) => e.name.clone(),
            None => return,
        };
        let var = if edit {
            s.vars.get(s.selected_var).cloned()
        } else {
            None
        };
        (env, var)
    } else {
        return;
    };
    if edit && editing_var.is_none() {
        return;
    }
    let state = match editing_var {
        Some(v) => VarFormState {
            env_name: env_name.clone(),
            key: LineEdit::from_str(v.key.clone()),
            value: LineEdit::from_str(v.value.clone()),
            is_secret: v.is_secret,
            focus: VarFormFocus::Value,
            editing: Some(v.key),
            error: None,
        },
        None => VarFormState {
            env_name,
            ..Default::default()
        },
    };
    app.modal = Some(crate::modal::Modal::VarForm(state));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
}

fn var_form_submit(app: &mut App) {
    let (env_name, key, value, is_secret) =
        if let Some(crate::modal::Modal::VarForm(s)) = app.modal.as_ref() {
            (
                s.env_name.clone(),
                s.key.as_str().trim().to_string(),
                s.value.as_str().to_string(),
                s.is_secret,
            )
        } else {
            return;
        };
    if key.is_empty() {
        if let Some(crate::modal::Modal::VarForm(s)) = app.modal.as_mut() {
            s.error = Some("key is required".into());
        }
        return;
    }
    let store = app.environments_store.clone();
    let input = SetVarInput {
        env_name: env_name.clone(),
        key: key.clone(),
        value,
        is_secret,
    };
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(store.set_var(input))
    });
    match result {
        Ok(_) => {
            app.modal = None;
            let _ = open_envs_page(app);
            // Restore env selection on the same env name.
            with_page(app, |s| {
                if let Some(idx) = s.envs.iter().position(|e| e.name == env_name) {
                    s.selected_env = idx;
                }
            });
            reload_vars(app);
            app.set_status(StatusKind::Info, format!("set var \"{key}\" in {env_name}"));
        }
        Err(e) => {
            if let Some(crate::modal::Modal::VarForm(s)) = app.modal.as_mut() {
                s.error = Some(e);
            }
        }
    }
}

fn open_env_delete_confirm(app: &mut App) {
    let name = if let Some(crate::modal::Modal::EnvsPage(s)) = app.modal.as_ref() {
        s.envs.get(s.selected_env).map(|e| e.name.clone())
    } else {
        None
    };
    let Some(name) = name else { return };
    app.modal = Some(crate::modal::Modal::ConfirmPrompt(
        crate::app::ConfirmPromptState {
            title: "Delete env".to_string(),
            body: format!("Delete env \"{name}\"?"),
            on_confirm: crate::input::action::Action::ConfirmEnvOrVarDelete,
            on_cancel: crate::input::action::Action::CancelEnvOrVarDelete,
            payload: crate::app::ConfirmPayload::EnvName(name),
        },
    ));
}

fn open_var_delete_confirm(app: &mut App) {
    let pair = if let Some(crate::modal::Modal::EnvsPage(s)) = app.modal.as_ref() {
        match (s.envs.get(s.selected_env), s.vars.get(s.selected_var)) {
            (Some(e), Some(v)) => Some((e.name.clone(), v.key.clone())),
            _ => None,
        }
    } else {
        None
    };
    let Some((env_name, key)) = pair else { return };
    app.modal = Some(crate::modal::Modal::ConfirmPrompt(
        crate::app::ConfirmPromptState {
            title: "Delete var".to_string(),
            body: format!("Delete var \"{key}\" from \"{env_name}\"?"),
            on_confirm: crate::input::action::Action::ConfirmEnvOrVarDelete,
            on_cancel: crate::input::action::Action::CancelEnvOrVarDelete,
            payload: crate::app::ConfirmPayload::Var { env_name, key },
        },
    ));
}

fn confirm_delete(app: &mut App) {
    let store = app.environments_store.clone();
    enum Op {
        Env(String),
        Var(String, String),
    }
    let op = match app.modal.as_ref() {
        Some(crate::modal::Modal::ConfirmPrompt(state)) => match &state.payload {
            crate::app::ConfirmPayload::EnvName(name) => Op::Env(name.clone()),
            crate::app::ConfirmPayload::Var { env_name, key } => {
                Op::Var(env_name.clone(), key.clone())
            }
            _ => return,
        },
        _ => return,
    };
    let (result, msg) = match op {
        Op::Env(ref name) => {
            let r = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(store.delete_env(name))
            });
            (r, format!("deleted env \"{name}\""))
        }
        Op::Var(ref env, ref key) => {
            let r = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(store.delete_var(env, key))
            });
            (r, format!("deleted var \"{key}\" from {env}"))
        }
    };
    match result {
        Ok(()) => {
            let _ = open_envs_page(app);
            app.refresh_active_env_name();
            app.set_status(StatusKind::Info, msg);
        }
        Err(e) => {
            let _ = open_envs_page(app);
            app.set_status(StatusKind::Error, format!("delete failed: {e}"));
        }
    }
}

fn close_form_and_reopen(app: &mut App, is_form: impl Fn(&Option<crate::modal::Modal>) -> bool) {
    if !is_form(&app.modal) {
        return;
    }
    app.modal = None;
    let _ = open_envs_page(app);
}

/// Helper compartilhado com `envs_clone`: re-focus o env recém-criado
/// na EnvsPage por nome (no-op se não encontrar).
pub(super) fn with_page_select_env(app: &mut App, name: &str) {
    with_page(app, |s| {
        if let Some(idx) = s.envs.iter().position(|e| e.name == name) {
            s.selected_env = idx;
        }
    });
}

pub(super) fn reload_vars_export(app: &mut App) {
    reload_vars(app);
}
