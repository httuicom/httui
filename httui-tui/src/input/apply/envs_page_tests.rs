//! V4 débito-de-cobertura (2026-05-23): tests integrados pros
//! handlers de `apply/envs_page.rs` e `apply/envs_clone.rs`. Usa um
//! `App` real construído sobre tempdir vault + tempdir data, então
//! exercita `EnvironmentsStore` (TOML real) e `block_in_place` (tokio
//! multi-thread). Padrão idêntico ao `impl_lifecycle::tests`.
//!
//! Cobre os caminhos críticos do V4:
//! - open/move/focus na EnvsPage
//! - env form: create + rename + erro empty
//! - var form: set + erro empty
//! - delete env/var + cancel
//! - clone env form: open + submit + erro empty + erro dest=source
//!   + toggle var + toggle-all + move cursor + focus toggle
//! - refresh_var_uses (sem vault / com matches)

#![cfg(test)]

use crate::app::{
    App, EnvCloneFormFocus, EnvFormState, EnvsPaneFocus, VarFormFocus, VarFormState,
};
use crate::config::Config;
use crate::input::action::Action;
use crate::modal::Modal;
use crate::vault::ResolvedVault;
use crate::vim::lineedit::LineEdit;
use httui_core::db::init_db;
use tempfile::TempDir;

use super::envs_page::apply_envs;

async fn app_fixture() -> (App, TempDir, TempDir) {
    let data = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    std::fs::write(vault.path().join("note.md"), "stub\n").unwrap();
    let pool = init_db(data.path()).await.unwrap();
    let resolved = ResolvedVault {
        vault: vault.path().to_path_buf(),
    };
    let app = App::new(Config::default(), resolved, pool);
    (app, data, vault)
}

fn page(app: &App) -> Option<&crate::app::EnvsPageState> {
    if let Some(Modal::EnvsPage(s)) = app.modal.as_ref() {
        Some(s)
    } else {
        None
    }
}

// ----- open + navigate -----

#[tokio::test(flavor = "multi_thread")]
async fn open_envs_page_with_no_envs() {
    let (mut app, _d, _v) = app_fixture().await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    let s = page(&app).expect("EnvsPage open");
    assert!(s.envs.is_empty());
    assert!(s.vars.is_empty());
    assert_eq!(s.focus, EnvsPaneFocus::Envs);
}

#[tokio::test(flavor = "multi_thread")]
async fn close_envs_page_clears_modal() {
    let (mut app, _d, _v) = app_fixture().await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::CloseEnvsPage);
    assert!(app.modal.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn focus_toggle_switches_pane() {
    let (mut app, _d, _v) = app_fixture().await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusToggle);
    assert_eq!(page(&app).unwrap().focus, EnvsPaneFocus::Vars);
    apply_envs(&mut app, Action::EnvsPageFocusToggle);
    assert_eq!(page(&app).unwrap().focus, EnvsPaneFocus::Envs);
}

#[tokio::test(flavor = "multi_thread")]
async fn move_var_cursor_clamps_when_empty() {
    let (mut app, _d, _v) = app_fixture().await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageMoveVarCursor(5));
    assert_eq!(page(&app).unwrap().selected_var, 0);
}

// ----- env form: create + rename -----

#[tokio::test(flavor = "multi_thread")]
async fn create_env_via_form_persists_and_reopens_page() {
    let (mut app, _d, _v) = app_fixture().await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvForm);
    for c in "staging".chars() {
        apply_envs(&mut app, Action::EnvFormChar(c));
    }
    apply_envs(&mut app, Action::EnvFormSubmit);
    let s = page(&app).expect("page reopens");
    assert_eq!(s.envs.len(), 1);
    assert_eq!(s.envs[0].name, "staging");
}

#[tokio::test(flavor = "multi_thread")]
async fn env_form_submit_empty_name_shows_error() {
    let (mut app, _d, _v) = app_fixture().await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvForm);
    apply_envs(&mut app, Action::EnvFormSubmit);
    let Some(Modal::EnvForm(f)) = app.modal.as_ref() else {
        panic!("ainda no EnvForm");
    };
    assert!(f.error.as_deref().unwrap().contains("name"));
}

#[tokio::test(flavor = "multi_thread")]
async fn env_form_char_and_backspace_edit_field() {
    let (mut app, _d, _v) = app_fixture().await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvForm);
    apply_envs(&mut app, Action::EnvFormChar('a'));
    apply_envs(&mut app, Action::EnvFormChar('b'));
    apply_envs(&mut app, Action::EnvFormBackspace);
    let Some(Modal::EnvForm(f)) = app.modal.as_ref() else {
        panic!("EnvForm aberto");
    };
    assert_eq!(f.name.as_str(), "a");
}

#[tokio::test(flavor = "multi_thread")]
async fn close_env_form_reopens_page() {
    let (mut app, _d, _v) = app_fixture().await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvForm);
    apply_envs(&mut app, Action::CloseEnvForm);
    assert!(matches!(app.modal, Some(Modal::EnvsPage(_))));
}

// ----- var form -----

async fn seed_env(app: &mut App, name: &str) {
    let _ = app.environments_store.create_env(name).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn set_var_via_form() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "dev").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusVars);
    apply_envs(&mut app, Action::OpenVarForm);
    // Modal::VarForm aberto agora; preenche key + value.
    if let Some(Modal::VarForm(s)) = app.modal.as_mut() {
        s.focus = VarFormFocus::Key;
    }
    for c in "API".chars() {
        apply_envs(&mut app, Action::VarFormChar(c));
    }
    apply_envs(&mut app, Action::VarFormFocusNext);
    for c in "https://x".chars() {
        apply_envs(&mut app, Action::VarFormChar(c));
    }
    apply_envs(&mut app, Action::VarFormSubmit);
    let s = page(&app).expect("page reabriu");
    assert_eq!(s.vars.len(), 1);
    assert_eq!(s.vars[0].key, "API");
}

#[tokio::test(flavor = "multi_thread")]
async fn var_form_submit_empty_key_shows_error() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "dev").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusVars);
    apply_envs(&mut app, Action::OpenVarForm);
    apply_envs(&mut app, Action::VarFormSubmit);
    let Some(Modal::VarForm(f)) = app.modal.as_ref() else {
        panic!("var form aberto");
    };
    assert!(f.error.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn var_form_toggle_secret_and_focus_cycle() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "dev").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusVars);
    apply_envs(&mut app, Action::OpenVarForm);
    apply_envs(&mut app, Action::VarFormFocusNext);
    apply_envs(&mut app, Action::VarFormFocusNext);
    apply_envs(&mut app, Action::VarFormToggleSecret);
    apply_envs(&mut app, Action::VarFormFocusPrev);
    let Some(Modal::VarForm(f)) = app.modal.as_ref() else {
        panic!("var form aberto");
    };
    assert!(f.is_secret);
    assert_eq!(f.focus, VarFormFocus::Value);
}

#[tokio::test(flavor = "multi_thread")]
async fn close_var_form_reopens_page() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "dev").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusVars);
    apply_envs(&mut app, Action::OpenVarForm);
    apply_envs(&mut app, Action::CloseVarForm);
    assert!(matches!(app.modal, Some(Modal::EnvsPage(_))));
}

// ----- delete env / var -----

#[tokio::test(flavor = "multi_thread")]
async fn delete_env_with_confirm_removes_it() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "to-delete").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvDeleteConfirm);
    apply_envs(&mut app, Action::ConfirmEnvOrVarDelete);
    let s = page(&app).expect("reabriu");
    assert!(s.envs.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_env_delete_keeps_env() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "keep").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvDeleteConfirm);
    apply_envs(&mut app, Action::CancelEnvOrVarDelete);
    let s = page(&app).expect("reabriu");
    assert_eq!(s.envs.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_var_with_confirm_removes_it() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "dev").await;
    app.environments_store
        .set_var(httui_core::vault_config::SetVarInput {
            env_name: "dev".into(),
            key: "TARGET".into(),
            value: "v".into(),
            is_secret: false,
        })
        .await
        .unwrap();
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusVars);
    apply_envs(&mut app, Action::OpenVarDeleteConfirm);
    apply_envs(&mut app, Action::ConfirmEnvOrVarDelete);
    let s = page(&app).expect("reabriu");
    assert!(s.vars.is_empty());
}

// ----- env activate (a chord) -----

#[tokio::test(flavor = "multi_thread")]
async fn activate_env_via_page() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "prod").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageActivateEnv);
    let s = page(&app).unwrap();
    assert_eq!(s.active.as_deref(), Some("prod"));
}

// ----- clone form -----

#[tokio::test(flavor = "multi_thread")]
async fn clone_env_form_copies_marked_vars() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "src").await;
    for (k, v) in [("A", "1"), ("B", "2"), ("C", "3")] {
        app.environments_store
            .set_var(httui_core::vault_config::SetVarInput {
                env_name: "src".into(),
                key: k.into(),
                value: v.into(),
                is_secret: false,
            })
            .await
            .unwrap();
    }
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvCloneForm);
    // 3 vars todas marcadas por default. Desmarca a 2ª (index 1).
    apply_envs(&mut app, Action::EnvCloneFormFocusToggle); // Name → Vars
    apply_envs(&mut app, Action::EnvCloneFormMoveVarCursor(1));
    apply_envs(&mut app, Action::EnvCloneFormToggleVar);
    apply_envs(&mut app, Action::EnvCloneFormSubmit);
    // Esperado: novo env "src-copy" com A e C, sem B.
    let s = page(&app).expect("page reaberta");
    assert!(s.envs.iter().any(|e| e.name == "src-copy"));
    let vars = app.environments_store.list_vars("src-copy").await.unwrap();
    let keys: Vec<_> = vars.iter().map(|v| v.key.as_str()).collect();
    assert!(keys.contains(&"A"));
    assert!(keys.contains(&"C"));
    assert!(!keys.contains(&"B"));
}

#[tokio::test(flavor = "multi_thread")]
async fn clone_env_form_empty_name_shows_error() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "src").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvCloneForm);
    // Apaga o default `src-copy`.
    if let Some(Modal::EnvCloneForm(s)) = app.modal.as_mut() {
        s.name = LineEdit::default();
    }
    apply_envs(&mut app, Action::EnvCloneFormSubmit);
    let Some(Modal::EnvCloneForm(s)) = app.modal.as_ref() else {
        panic!("clone form aberto");
    };
    assert!(s.error.as_deref().unwrap().contains("name"));
}

#[tokio::test(flavor = "multi_thread")]
async fn clone_env_form_dest_equals_source_shows_error() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "src").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvCloneForm);
    if let Some(Modal::EnvCloneForm(s)) = app.modal.as_mut() {
        s.name = LineEdit::from_str("src");
    }
    apply_envs(&mut app, Action::EnvCloneFormSubmit);
    let Some(Modal::EnvCloneForm(s)) = app.modal.as_ref() else {
        panic!("clone form aberto");
    };
    assert!(s.error.as_deref().unwrap().contains("differ"));
}

#[tokio::test(flavor = "multi_thread")]
async fn clone_env_form_toggle_all_inverts_marks() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "src").await;
    app.environments_store
        .set_var(httui_core::vault_config::SetVarInput {
            env_name: "src".into(),
            key: "K".into(),
            value: "v".into(),
            is_secret: false,
        })
        .await
        .unwrap();
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvCloneForm);
    // Default: K marcada. Toggle-all desmarca tudo.
    apply_envs(&mut app, Action::EnvCloneFormToggleAll);
    if let Some(Modal::EnvCloneForm(s)) = app.modal.as_ref() {
        assert!(s.vars.iter().all(|v| !v.checked));
    } else {
        panic!("clone form aberto");
    }
    // Toggle-all de novo marca tudo.
    apply_envs(&mut app, Action::EnvCloneFormToggleAll);
    if let Some(Modal::EnvCloneForm(s)) = app.modal.as_ref() {
        assert!(s.vars.iter().all(|v| v.checked));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn clone_env_form_char_backspace_edit_name() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "src").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvCloneForm);
    // Default name = "src-copy"; backspace 4× remove "copy".
    for _ in 0..4 {
        apply_envs(&mut app, Action::EnvCloneFormBackspace);
    }
    apply_envs(&mut app, Action::EnvCloneFormChar('v'));
    apply_envs(&mut app, Action::EnvCloneFormChar('2'));
    if let Some(Modal::EnvCloneForm(s)) = app.modal.as_ref() {
        assert_eq!(s.name.as_str(), "src-v2");
    } else {
        panic!("clone form aberto");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn clone_env_form_focus_toggle_and_move_cursor() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "src").await;
    for k in ["A", "B"] {
        app.environments_store
            .set_var(httui_core::vault_config::SetVarInput {
                env_name: "src".into(),
                key: k.into(),
                value: "v".into(),
                is_secret: false,
            })
            .await
            .unwrap();
    }
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvCloneForm);
    apply_envs(&mut app, Action::EnvCloneFormFocusToggle);
    apply_envs(&mut app, Action::EnvCloneFormMoveVarCursor(1));
    apply_envs(&mut app, Action::EnvCloneFormMoveVarCursor(99)); // clamped
    if let Some(Modal::EnvCloneForm(s)) = app.modal.as_ref() {
        assert_eq!(s.focus, EnvCloneFormFocus::Vars);
        assert_eq!(s.selected_var, 1);
    } else {
        panic!("clone form aberto");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn close_clone_form_reopens_page() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "src").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvCloneForm);
    apply_envs(&mut app, Action::CloseEnvCloneForm);
    assert!(matches!(app.modal, Some(Modal::EnvsPage(_))));
}

// ----- refresh_var_uses -----

#[tokio::test(flavor = "multi_thread")]
async fn var_uses_finds_md_references() {
    let (mut app, _d, vault) = app_fixture().await;
    seed_env(&mut app, "dev").await;
    app.environments_store
        .set_var(httui_core::vault_config::SetVarInput {
            env_name: "dev".into(),
            key: "API".into(),
            value: "v".into(),
            is_secret: false,
        })
        .await
        .unwrap();
    std::fs::write(
        vault.path().join("runbook.md"),
        "url: {{API}}/users\nbody: {{API.body}}\n",
    )
    .unwrap();
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusVars);
    let s = page(&app).unwrap();
    assert_eq!(s.var_uses.len(), 2);
    assert!(s.var_uses.iter().any(|u| u.snippet.contains("{{API}}")));
}

#[tokio::test(flavor = "multi_thread")]
async fn var_uses_empty_when_no_matches() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "dev").await;
    app.environments_store
        .set_var(httui_core::vault_config::SetVarInput {
            env_name: "dev".into(),
            key: "UNUSED".into(),
            value: "v".into(),
            is_secret: false,
        })
        .await
        .unwrap();
    apply_envs(&mut app, Action::OpenEnvsPage);
    let s = page(&app).unwrap();
    assert!(s.var_uses.is_empty());
}

// ----- focus shortcuts -----

#[tokio::test(flavor = "multi_thread")]
async fn focus_envs_then_vars_then_envs() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "x").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusEnvs);
    assert_eq!(page(&app).unwrap().focus, EnvsPaneFocus::Envs);
    apply_envs(&mut app, Action::EnvsPageFocusVars);
    assert_eq!(page(&app).unwrap().focus, EnvsPaneFocus::Vars);
}

// ----- env form rename path -----

#[tokio::test(flavor = "multi_thread")]
async fn env_form_rename_existing_env() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "old-name").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::OpenEnvEditForm);
    // Manualmente substitui: apaga e digita.
    if let Some(Modal::EnvForm(s)) = app.modal.as_mut() {
        s.name = LineEdit::from_str("new-name");
        // Manter editing field intacto.
        let _: &EnvFormState = s;
    }
    apply_envs(&mut app, Action::EnvFormSubmit);
    let s = page(&app).unwrap();
    assert!(s.envs.iter().any(|e| e.name == "new-name"));
    assert!(!s.envs.iter().any(|e| e.name == "old-name"));
}

#[tokio::test(flavor = "multi_thread")]
async fn open_var_edit_form_prefills_existing_var() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "dev").await;
    app.environments_store
        .set_var(httui_core::vault_config::SetVarInput {
            env_name: "dev".into(),
            key: "KEY".into(),
            value: "VAL".into(),
            is_secret: false,
        })
        .await
        .unwrap();
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusVars);
    apply_envs(&mut app, Action::OpenVarEditForm);
    let Some(Modal::VarForm(f)) = app.modal.as_ref() else {
        panic!("var form aberto");
    };
    assert_eq!(f.key.as_str(), "KEY");
    assert_eq!(f.value.as_str(), "VAL");
    assert!(f.editing.is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn var_form_backspace_in_each_focus_field() {
    let (mut app, _d, _v) = app_fixture().await;
    seed_env(&mut app, "dev").await;
    apply_envs(&mut app, Action::OpenEnvsPage);
    apply_envs(&mut app, Action::EnvsPageFocusVars);
    apply_envs(&mut app, Action::OpenVarForm);
    apply_envs(&mut app, Action::VarFormChar('A'));
    apply_envs(&mut app, Action::VarFormChar('B'));
    apply_envs(&mut app, Action::VarFormBackspace);
    apply_envs(&mut app, Action::VarFormFocusNext);
    apply_envs(&mut app, Action::VarFormChar('1'));
    apply_envs(&mut app, Action::VarFormBackspace);
    let Some(Modal::VarForm(f)) = app.modal.as_ref() else {
        panic!("var form");
    };
    assert_eq!(f.key.as_str(), "A");
    assert_eq!(f.value.as_str(), "");
}

// Suppress unused-import warning when only some paths are exercised.
#[allow(dead_code)]
fn _ensure_imports(_: VarFormState) {}
