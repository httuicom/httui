//! Handlers for the 3-way conflict resolver modal.

use std::path::PathBuf;

use crate::app::{App, StatusKind};
use crate::git::{ConflictVersion, GitConflictResolverState};
use crate::modal::Modal;
use crate::vim::mode::Mode;

pub(super) fn open(app: &mut App) {
    crate::commands::git::refresh_git_status(app);
    let files: Vec<String> = match app.git_panel.status.as_ref() {
        Some(s) => s
            .changed
            .iter()
            .filter(|c| c.status.contains('U'))
            .map(|c| c.path.clone())
            .collect(),
        None => Vec::new(),
    };
    if files.is_empty() {
        app.set_status(
            StatusKind::Info,
            "no unmerged files to resolve".to_string(),
        );
        return;
    }
    app.modal = Some(Modal::GitConflictResolver(GitConflictResolverState::new(
        files,
    )));
    app.vim.mode = Mode::Modal;
}

pub(super) fn close(app: &mut App) {
    if matches!(app.modal, Some(Modal::GitConflictResolver(_))) {
        app.modal = None;
        app.vim.mode = Mode::Git;
    }
}

pub(super) fn move_file(app: &mut App, delta: i32) {
    if let Some(Modal::GitConflictResolver(state)) = app.modal.as_mut() {
        state.move_file_cursor(delta);
    }
}

pub(super) fn resolve(app: &mut App, version: ConflictVersion) {
    let Some((path, body)) = pick_body(app, version) else {
        return;
    };
    let abs = PathBuf::from(&app.vault_path).join(&path);
    if let Err(e) = std::fs::write(&abs, body) {
        if let Some(Modal::GitConflictResolver(state)) = app.modal.as_mut() {
            state.error = Some(format!("write {path}: {e}"));
        }
        return;
    }
    if let Err(e) = httui_core::git::staging::stage_path(&app.vault_path, &path) {
        if let Some(Modal::GitConflictResolver(state)) = app.modal.as_mut() {
            state.error = Some(format!("stage {path}: {e}"));
        }
        return;
    }
    // Drop the resolved file from the list; if it was the last one,
    // close the modal and refresh status.
    let resolved_label = version_label(version);
    if let Some(Modal::GitConflictResolver(state)) = app.modal.as_mut() {
        state.files.retain(|f| f != &path);
        if state.selected_file >= state.files.len() {
            state.selected_file = state.files.len().saturating_sub(1);
        }
        state.versions = None;
        state.error = None;
    }
    let close_modal = matches!(
        app.modal.as_ref(),
        Some(Modal::GitConflictResolver(s)) if s.files.is_empty()
    );
    if close_modal {
        app.modal = None;
        app.vim.mode = Mode::Git;
    }
    crate::commands::git::refresh_git_status(app);
    app.set_status(
        StatusKind::Info,
        format!("Resolved {path} with {resolved_label}"),
    );
}

/// Lazy-fetch the three versions for the current file before reading
/// the chosen one. Called by the renderer too — idempotent on hit.
pub fn ensure_versions_loaded(app: &mut App) {
    let needs_load = matches!(
        app.modal.as_ref(),
        Some(Modal::GitConflictResolver(s))
            if s.versions.is_none() && !s.files.is_empty()
    );
    if !needs_load {
        return;
    }
    let path = match app.modal.as_ref() {
        Some(Modal::GitConflictResolver(s)) => s.current_file().map(|p| p.to_string()),
        _ => None,
    };
    let Some(path) = path else { return };
    let res = httui_core::git::git_conflict_versions(&app.vault_path, &path);
    if let Some(Modal::GitConflictResolver(state)) = app.modal.as_mut() {
        match res {
            Ok(versions) => {
                state.versions = Some(versions);
                state.error = None;
            }
            Err(msg) => {
                state.versions = None;
                state.error = Some(msg);
            }
        }
    }
}

fn pick_body(app: &mut App, version: ConflictVersion) -> Option<(String, String)> {
    ensure_versions_loaded(app);
    let Some(Modal::GitConflictResolver(state)) = app.modal.as_ref() else {
        return None;
    };
    let path = state.current_file()?.to_string();
    let v = state.versions.as_ref()?;
    let body = match version {
        ConflictVersion::Base => v.base.clone(),
        ConflictVersion::Ours => v.ours.clone(),
        ConflictVersion::Theirs => v.theirs.clone(),
    };
    Some((path, body))
}

fn version_label(version: ConflictVersion) -> &'static str {
    match version {
        ConflictVersion::Base => "base",
        ConflictVersion::Ours => "ours",
        ConflictVersion::Theirs => "theirs",
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

    /// Build a real merge conflict on `f.txt`.
    fn make_conflict(vault: &std::path::Path) {
        crate::git::test_helpers::init_repo(vault);
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(vault)
                .args(args)
                .output()
                .unwrap();
        };
        std::fs::write(vault.join("f.txt"), "base\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "base"]);
        git(&["checkout", "-b", "feature"]);
        std::fs::write(vault.join("f.txt"), "theirs\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "theirs"]);
        git(&["checkout", "main"]);
        std::fs::write(vault.join("f.txt"), "ours\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-m", "ours"]);
        git(&["merge", "feature"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_with_conflicts_populates_modal() {
        let (mut app, _d, vault) = build_app().await;
        make_conflict(vault.path());
        open(&mut app);
        match app.modal.as_ref() {
            Some(Modal::GitConflictResolver(s)) => {
                assert_eq!(s.files, vec!["f.txt"]);
            }
            other => panic!("expected resolver modal, got {other:?}"),
        }
        assert_eq!(app.vim.mode, Mode::Modal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn open_without_conflicts_emits_status_info() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        open(&mut app);
        assert!(app.modal.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn close_drops_modal_and_returns_to_git_mode() {
        let (mut app, _d, vault) = build_app().await;
        make_conflict(vault.path());
        open(&mut app);
        close(&mut app);
        assert!(app.modal.is_none());
        assert_eq!(app.vim.mode, Mode::Git);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ensure_versions_loaded_populates_three_stages() {
        let (mut app, _d, vault) = build_app().await;
        make_conflict(vault.path());
        open(&mut app);
        ensure_versions_loaded(&mut app);
        match app.modal.as_ref() {
            Some(Modal::GitConflictResolver(s)) => {
                let v = s.versions.as_ref().expect("versions populated");
                assert_eq!(v.base, "base\n");
                assert_eq!(v.ours, "ours\n");
                assert_eq!(v.theirs, "theirs\n");
            }
            _ => panic!("expected resolver modal"),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resolve_with_ours_writes_and_stages_and_closes_when_done() {
        let (mut app, _d, vault) = build_app().await;
        make_conflict(vault.path());
        open(&mut app);
        resolve(&mut app, ConflictVersion::Ours);
        // Single-file conflict → resolving the only entry closes
        // the modal.
        assert!(app.modal.is_none());
        assert_eq!(app.vim.mode, Mode::Git);
        // File on disk is "ours\n" and staged.
        let body = std::fs::read_to_string(vault.path().join("f.txt")).unwrap();
        assert_eq!(body, "ours\n");
        let status = httui_core::git::git_status(vault.path()).unwrap();
        // No more unmerged entries.
        assert!(!status.changed.iter().any(|c| c.status.contains('U')));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resolve_with_theirs_writes_their_body() {
        let (mut app, _d, vault) = build_app().await;
        make_conflict(vault.path());
        open(&mut app);
        resolve(&mut app, ConflictVersion::Theirs);
        let body = std::fs::read_to_string(vault.path().join("f.txt")).unwrap();
        assert_eq!(body, "theirs\n");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn resolve_with_base_writes_base_body() {
        let (mut app, _d, vault) = build_app().await;
        make_conflict(vault.path());
        open(&mut app);
        resolve(&mut app, ConflictVersion::Base);
        let body = std::fs::read_to_string(vault.path().join("f.txt")).unwrap();
        assert_eq!(body, "base\n");
    }

    #[test]
    fn version_label_is_human_readable() {
        assert_eq!(version_label(ConflictVersion::Base), "base");
        assert_eq!(version_label(ConflictVersion::Ours), "ours");
        assert_eq!(version_label(ConflictVersion::Theirs), "theirs");
    }
}
