//! Tauri commands wrapping `httui_core::git`. Thin delegators —
//! the panel UI calls these, the substantive logic lives in core.

use httui_core::git::{
    git_branch_list, git_checkout, git_checkout_b, git_checkout_conflict_path, git_clone,
    git_commit, git_conflict_versions, git_diff, git_fetch, git_first_commit_author, git_log,
    git_pull, git_push, git_remote_list, git_status, stage_path, unstage_path, BranchInfo,
    CloneOutcome, CommitInfo, ConflictSide, ConflictVersions, GitStatus, Remote,
};
use std::path::PathBuf;

/// `git status --porcelain=v2 --branch` for the vault.
#[tauri::command]
pub async fn git_status_cmd(vault_path: String) -> Result<GitStatus, String> {
    git_status(&PathBuf::from(vault_path))
}

/// `git log -n <limit>` for the vault, optionally filtered to a path.
#[tauri::command]
pub async fn git_log_cmd(
    vault_path: String,
    limit: usize,
    path_filter: Option<String>,
) -> Result<Vec<CommitInfo>, String> {
    git_log(&PathBuf::from(vault_path), limit, path_filter.as_deref())
}

/// `git show <sha>` (or `git diff HEAD` when sha is `None`).
#[tauri::command]
pub async fn git_diff_cmd(
    vault_path: String,
    commit_sha: Option<String>,
) -> Result<String, String> {
    git_diff(&PathBuf::from(vault_path), commit_sha.as_deref())
}

/// Local + remote branches for the vault.
#[tauri::command]
pub async fn git_branch_list_cmd(vault_path: String) -> Result<Vec<BranchInfo>, String> {
    git_branch_list(&PathBuf::from(vault_path))
}

/// Configured remotes (`git remote -v` deduped to one entry per
/// `(name, url)`). Powers Epic 49's `<SharePopover>`.
#[tauri::command]
pub async fn git_remote_list_cmd(vault_path: String) -> Result<Vec<Remote>, String> {
    git_remote_list(&PathBuf::from(vault_path))
}

/// First-commit author of `path` (follows renames). `None` when the
/// path doesn't appear in history. Powers Epic 50 Story 03's
/// `<DocHeaderMetaStrip>` Author chip.
#[tauri::command]
pub async fn git_first_commit_author_cmd(
    vault_path: String,
    path: String,
) -> Result<Option<CommitInfo>, String> {
    git_first_commit_author(&PathBuf::from(vault_path), &path)
}

/// `git checkout <branch>` — switch to an existing branch. Powers
/// Epic 48 Story 04's `<GitBranchPicker>` selection flow. The
/// consumer handles the dirty-state stash precheck before calling.
#[tauri::command]
pub async fn git_checkout_cmd(vault_path: String, branch: String) -> Result<(), String> {
    git_checkout(&PathBuf::from(vault_path), &branch)
}

/// `git checkout -b <new>` — create branch + switch. Forks from the
/// current branch (git's default).
#[tauri::command]
pub async fn git_checkout_b_cmd(
    vault_path: String,
    new_branch: String,
) -> Result<(), String> {
    git_checkout_b(&PathBuf::from(vault_path), &new_branch)
}

/// `git checkout --ours|--theirs -- <path>` — replace a conflicted
/// working-tree file with one side of the merge. Powers Epic 48
/// Story 06's `<GitConflictBanner>` Accept-yours / Accept-theirs
/// row actions. Caller is expected to follow up with
/// `stage_path_cmd` to mark the conflict resolved.
#[tauri::command]
pub async fn git_checkout_conflict_path_cmd(
    vault_path: String,
    path: String,
    side: ConflictSide,
) -> Result<(), String> {
    git_checkout_conflict_path(&PathBuf::from(vault_path), &path, side)
}

/// `git show :1|:2|:3:<path>` — the three merge stages of a
/// conflicted file. Powers the V10 cenário 6 3-way resolver.
#[tauri::command]
pub async fn git_conflict_versions_cmd(
    vault_path: String,
    path: String,
) -> Result<ConflictVersions, String> {
    git_conflict_versions(&PathBuf::from(vault_path), &path)
}

/// `git add <path>` — stages a single vault-relative file. Powers
/// the `<GitFileList>` per-row staging checkbox (Epic 48 Story 02).
#[tauri::command]
pub async fn stage_path_cmd(vault_path: String, path: String) -> Result<(), String> {
    stage_path(&PathBuf::from(vault_path), &path)
}

/// `git reset HEAD -- <path>` — drops the staged version, keeps
/// working-tree edits.
#[tauri::command]
pub async fn unstage_path_cmd(vault_path: String, path: String) -> Result<(), String> {
    unstage_path(&PathBuf::from(vault_path), &path)
}

/// `git commit -m <message>` (or `--amend --no-edit` when amend).
/// Powers `<GitCommitForm>` submit. Empty messages are rejected
/// at the Rust layer; the form already validates client-side.
#[tauri::command]
pub async fn git_commit_cmd(
    vault_path: String,
    message: String,
    amend: bool,
) -> Result<(), String> {
    git_commit(&PathBuf::from(vault_path), &message, amend)
}

/// `git fetch [<remote>]`. Powers Story 05's `<GitSyncButtons>`
/// fetch action. Returns the combined stdout+stderr for the toast.
#[tauri::command]
pub async fn git_fetch_cmd(
    vault_path: String,
    remote: Option<String>,
) -> Result<String, String> {
    git_fetch(&PathBuf::from(vault_path), remote.as_deref())
}

/// `git pull [--ff-only] [<remote> <branch>]`. `ff_only` is the
/// V10.1 cenário 3 Sync path (never auto-merge on pull).
#[tauri::command]
pub async fn git_pull_cmd(
    vault_path: String,
    remote: Option<String>,
    branch: Option<String>,
    ff_only: bool,
) -> Result<String, String> {
    git_pull(
        &PathBuf::from(vault_path),
        remote.as_deref(),
        branch.as_deref(),
        ff_only,
    )
}

/// `git push [-u] [<remote> <branch>]`. `set_upstream` is the V10
/// no-upstream confirm path (`git push -u origin <branch>`).
#[tauri::command]
pub async fn git_push_cmd(
    vault_path: String,
    remote: Option<String>,
    branch: Option<String>,
    set_upstream: bool,
) -> Result<String, String> {
    git_push(
        &PathBuf::from(vault_path),
        remote.as_deref(),
        branch.as_deref(),
        set_upstream,
    )
}

/// `git clone <url> <parent>/<repo-name>` — V1 vertical 1, cenário 2.
///
/// Auth (HTTPS PAT, SSH keys) is delegated to the user's git
/// credential helper / ssh-agent. The `parent` arg is the *container*
/// folder the user picked (or `None` to default to `~/Documents`).
/// The repo's leaf name is always derived from the URL — never the
/// caller's responsibility — so picking `/tmp` clones into
/// `/tmp/<repo>` rather than overwriting `/tmp` itself.
/// Returns the absolute path of the clone so the frontend can
/// `switchVault` straight in.
#[tauri::command]
pub async fn clone_vault_cmd(
    url: String,
    parent: Option<String>,
) -> Result<CloneOutcome, String> {
    let parent = parent.map(PathBuf::from);
    git_clone(&url, parent.as_deref())
}

#[cfg(test)]
mod tests {
    //! Smoke-tests only — `git` CLI behaviour is exhaustively
    //! covered in `httui_core::git::*::tests`. Here we just confirm
    //! the wrappers forward correctly.

    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_with_commit(dir: &TempDir) {
        let p = dir.path();
        let _ = Command::new("git").arg("init").arg(p).output();
        for (k, v) in [
            ("user.email", "t@t"),
            ("user.name", "t"),
            ("commit.gpgsign", "false"),
            ("init.defaultBranch", "main"),
        ] {
            let _ = Command::new("git")
                .arg("-C")
                .arg(p)
                .args(["config", k, v])
                .output();
        }
        std::fs::write(p.join("a"), "x").unwrap();
        let _ = Command::new("git")
            .arg("-C")
            .arg(p)
            .args(["add", "-A"])
            .output();
        let _ = Command::new("git")
            .arg("-C")
            .arg(p)
            .args(["commit", "-m", "init"])
            .output();
    }

    #[tokio::test]
    async fn status_round_trip() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        let s = git_status_cmd(dir.path().to_string_lossy().into())
            .await
            .unwrap();
        assert!(s.clean);
    }

    #[tokio::test]
    async fn log_round_trip() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        let l = git_log_cmd(dir.path().to_string_lossy().into(), 10, None)
            .await
            .unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].subject, "init");
    }

    #[tokio::test]
    async fn branches_round_trip() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        let b = git_branch_list_cmd(dir.path().to_string_lossy().into())
            .await
            .unwrap();
        assert!(b.iter().any(|x| x.name == "main"));
    }

    #[tokio::test]
    async fn diff_round_trip() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        let d = git_diff_cmd(dir.path().to_string_lossy().into(), None)
            .await
            .unwrap();
        // No working-tree changes after init.
        assert_eq!(d, "");
    }

    #[tokio::test]
    async fn remote_list_round_trip() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        let r = git_remote_list_cmd(dir.path().to_string_lossy().into())
            .await
            .unwrap();
        assert!(r.is_empty());
        // Add a remote and re-check.
        let _ = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["remote", "add", "origin", "git@github.com:o/r.git"])
            .output();
        let r2 = git_remote_list_cmd(dir.path().to_string_lossy().into())
            .await
            .unwrap();
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].name, "origin");
    }

    #[tokio::test]
    async fn checkout_round_trip() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        // Create + switch to feat/x.
        git_checkout_b_cmd(
            dir.path().to_string_lossy().into(),
            "feat/x".into(),
        )
        .await
        .unwrap();
        // Switch back to main.
        git_checkout_cmd(
            dir.path().to_string_lossy().into(),
            "main".into(),
        )
        .await
        .unwrap();
        let head = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&head.stdout).trim(), "main");
    }

    #[tokio::test]
    async fn stage_unstage_round_trip() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        std::fs::write(dir.path().join("new"), "x").unwrap();
        stage_path_cmd(
            dir.path().to_string_lossy().into(),
            "new".into(),
        )
        .await
        .unwrap();
        unstage_path_cmd(
            dir.path().to_string_lossy().into(),
            "new".into(),
        )
        .await
        .unwrap();
        // After unstage, "new" should still be untracked.
        let s = Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["status", "--porcelain"])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&s.stdout).contains("?? new"));
    }

    #[tokio::test]
    async fn commit_round_trip() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        std::fs::write(dir.path().join("b"), "x").unwrap();
        stage_path_cmd(dir.path().to_string_lossy().into(), "b".into())
            .await
            .unwrap();
        git_commit_cmd(
            dir.path().to_string_lossy().into(),
            "second".into(),
            false,
        )
        .await
        .unwrap();
        let l = git_log_cmd(dir.path().to_string_lossy().into(), 10, None)
            .await
            .unwrap();
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].subject, "second");
    }

    #[tokio::test]
    async fn checkout_conflict_path_rejects_empty_path() {
        // Smoke-only — exhaustive merge-conflict scenarios are covered
        // in `httui_core::git::checkout::tests`. The wrapper just
        // needs to forward the arg + side enum.
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        let err = git_checkout_conflict_path_cmd(
            dir.path().to_string_lossy().into(),
            "  ".into(),
            ConflictSide::Ours,
        )
        .await
        .unwrap_err();
        assert!(err.contains("empty"));
    }

    #[tokio::test]
    async fn first_commit_author_round_trip() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        let info = git_first_commit_author_cmd(
            dir.path().to_string_lossy().into(),
            "a".into(),
        )
        .await
        .unwrap()
        .expect("`a` was added by init_with_commit");
        assert_eq!(info.subject, "init");
        // Path that was never committed → None.
        let none = git_first_commit_author_cmd(
            dir.path().to_string_lossy().into(),
            "ghost".into(),
        )
        .await
        .unwrap();
        assert!(none.is_none());
    }

    #[tokio::test]
    async fn fetch_pull_push_no_remote_dont_panic() {
        let dir = TempDir::new().unwrap();
        init_with_commit(&dir);
        // No remote configured — fetch behaviour is git-version-
        // dependent (silent success on some, error on others).
        // The wrapper just must not panic.
        let _ = git_fetch_cmd(dir.path().to_string_lossy().into(), None).await;
        // pull/push without a remote always error.
        assert!(git_pull_cmd(
            dir.path().to_string_lossy().into(),
            None,
            None,
            false,
        )
        .await
        .is_err());
        assert!(git_push_cmd(
            dir.path().to_string_lossy().into(),
            None,
            None,
            false,
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn clone_vault_cmd_rejects_empty_url() {
        let dir = TempDir::new().unwrap();
        let dest = dir.path().join("repo");
        let err = clone_vault_cmd(
            "".into(),
            Some(dest.to_string_lossy().into()),
        )
        .await
        .unwrap_err();
        assert!(err.contains("vazia"), "got: {err}");
    }

    #[tokio::test]
    async fn clone_vault_cmd_round_trip_against_local_bare() {
        // Set up a local bare remote with one seeded commit.
        let bare = TempDir::new().unwrap();
        let mut init = Command::new("git");
        init.arg("init").arg("--bare").arg(bare.path());
        let _ = init.output();

        let work = TempDir::new().unwrap();
        for args in [
            vec!["init", work.path().to_str().unwrap()],
            vec!["-C", work.path().to_str().unwrap(), "config", "user.email", "t@t"],
            vec!["-C", work.path().to_str().unwrap(), "config", "user.name", "t"],
            vec!["-C", work.path().to_str().unwrap(), "config", "commit.gpgsign", "false"],
        ] {
            let _ = Command::new("git").args(args).output();
        }
        std::fs::write(work.path().join("README.md"), "hi").unwrap();
        for args in [
            vec!["-C", work.path().to_str().unwrap(), "add", "-A"],
            vec!["-C", work.path().to_str().unwrap(), "commit", "-m", "init"],
            vec![
                "-C",
                work.path().to_str().unwrap(),
                "remote",
                "add",
                "origin",
                bare.path().to_str().unwrap(),
            ],
            vec!["-C", work.path().to_str().unwrap(), "push", "origin", "HEAD:main"],
        ] {
            let _ = Command::new("git").args(args).output();
        }

        // Pass the parent dir; backend derives the leaf from the URL.
        let parent = TempDir::new().unwrap();
        let outcome = clone_vault_cmd(
            bare.path().to_string_lossy().into(),
            Some(parent.path().to_string_lossy().into()),
        )
        .await
        .unwrap();
        // Outcome destination is `<parent>/<leaf>`, where leaf is
        // derived from the bare repo's path.
        assert!(
            outcome.destination.starts_with(parent.path()),
            "destination should sit under the picked parent",
        );
        assert!(outcome.destination.join(".git").is_dir());
        assert!(outcome.destination.join("README.md").is_file());
    }
}
