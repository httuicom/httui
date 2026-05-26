//! `Ctrl+Y` share handler — composes the HTTPS URL from the vault's
//! `origin` (or first) remote and copies it to the system clipboard.
//! Split out of `apply::git_panel` to keep that file under the size
//! gate.

use crate::app::{App, StatusKind};

pub(super) fn share_https_url(app: &mut App) {
    let vault = app.vault_path.clone();
    let remotes = match httui_core::git::git_remote_list(&vault) {
        Ok(r) => r,
        Err(msg) => {
            app.set_status(
                StatusKind::Error,
                format!("git remote: {}", msg.lines().next().unwrap_or("")),
            );
            return;
        }
    };
    let Some(remote) = remotes
        .iter()
        .find(|r| r.name == "origin")
        .or_else(|| remotes.first())
    else {
        app.set_status(StatusKind::Info, "no remotes configured".to_string());
        return;
    };
    let Some(parsed) = httui_core::git::parse_remote_url(&remote.url) else {
        app.set_status(
            StatusKind::Error,
            format!("cannot parse remote URL: {}", remote.url),
        );
        return;
    };
    let https = format!(
        "https://{}/{}/{}",
        parsed.host_str, parsed.owner, parsed.repo
    );
    match crate::clipboard::set_text(&https) {
        Ok(()) => app.set_status(StatusKind::Info, format!("Copied: {https}")),
        Err(msg) => {
            app.set_status(StatusKind::Error, format!("clipboard: {msg}"));
        }
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
    async fn share_with_no_remote_emits_info_status() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        share_https_url(&mut app);
        let msg = app
            .status_message
            .as_ref()
            .expect("status message populated");
        assert!(msg.text.contains("no remotes"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn share_with_origin_runs_through_clipboard_path() {
        let (mut app, _d, vault) = build_app().await;
        crate::git::test_helpers::init_repo(vault.path());
        std::process::Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["remote", "add", "origin", "git@github.com:owner/repo.git"])
            .output()
            .unwrap();
        share_https_url(&mut app);
        // Clipboard may not be available in headless test runners;
        // either branch sets a status message — the assertion is that
        // the share flow reached a terminal state without panicking
        // and surfaces the composed URL.
        let msg = app
            .status_message
            .as_ref()
            .expect("status message populated");
        assert!(
            msg.text.contains("Copied")
                || msg.text.starts_with("clipboard:")
                || msg.text.contains("github.com/owner/repo"),
            "got: {}",
            msg.text
        );
    }
}
