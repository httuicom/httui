//! `git clone` — V1 vertical 1, cenário 2.
//!
//! Shells out to `git clone <url> <parent>/<repo-name>` using the
//! system `git`. Auth (HTTPS PAT, SSH keys) is delegated to the
//! user's credential helper / ssh-agent. We don't parse stderr —
//! when git fails we surface its message verbatim so the UI shows
//! what `git` actually said.
//!
//! `parent` semantics — the caller picks the **container folder**,
//! never the leaf. The repo name is always derived from the URL so
//! the user gets `<parent>/Hello-World/` instead of having to type
//! the leaf name themselves:
//! - `Some(path)` — use that as the parent. Final path is
//!   `path/<repo-name>`.
//! - `None` — use `~/Documents` (or `~/` if Documents isn't
//!   available) as the parent.
//!
//! Pre-flight: refuses to clone if `<parent>/<repo-name>` already
//! exists and is non-empty. The friendly message reads like the
//! user expects — names the leaf, not just "destination".

use super::scrub_git_env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Outcome of a successful clone — the absolute path of the cloned
/// repository, ready to hand back to the frontend for `switchVault`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CloneOutcome {
    pub destination: PathBuf,
}

/// Clone `url` into `<parent>/<repo-name>` (parent defaults to
/// `~/Documents` when None). Returns the absolute path of the clone
/// on success, or git's stderr message on failure.
pub fn git_clone(url: &str, parent: Option<&Path>) -> Result<CloneOutcome, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("URL não pode ser vazia".into());
    }

    let name = derive_repo_name(url);
    if name.is_empty() {
        return Err("não consegui derivar nome de pasta a partir da URL".into());
    }

    let parent_dir = match parent {
        Some(p) => p.to_path_buf(),
        None => default_parent_dir()?,
    };

    if !parent_dir.exists() {
        return Err(format!(
            "pasta pai '{}' não existe",
            parent_dir.display()
        ));
    }
    if !parent_dir.is_dir() {
        return Err(format!(
            "'{}' não é uma pasta",
            parent_dir.display()
        ));
    }

    let dest = parent_dir.join(&name);

    if dest.exists() {
        let is_empty = dest
            .read_dir()
            .map(|mut it| it.next().is_none())
            .unwrap_or(false);
        if !is_empty {
            return Err(format!(
                "'{}' já existe e não está vazio",
                dest.display()
            ));
        }
    }

    let mut cmd = Command::new("git");
    cmd.args(["clone", "--", url]).arg(&dest);
    scrub_git_env(&mut cmd);
    let output = cmd
        .output()
        .map_err(|e| format!("git invocation failed: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(if stderr.trim().is_empty() {
            format!(
                "git clone exited with code {}",
                output.status.code().unwrap_or(-1)
            )
        } else {
            stderr.trim().to_string()
        });
    }

    Ok(CloneOutcome { destination: dest })
}

/// Default parent folder when the user didn't pick one — prefers
/// `~/Documents`, falls back to `~/` if Documents isn't available.
fn default_parent_dir() -> Result<PathBuf, String> {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "não consegui localizar o diretório do usuário".to_string())
}

/// Best-effort repo-name extractor for git clone URLs.
///
/// `https://github.com/owner/repo.git` → `repo`
/// `https://github.com/owner/repo` → `repo`
/// `git@github.com:owner/repo.git` → `repo`
/// `ssh://git@host/owner/repo` → `repo`
/// `/local/path/repo.git` → `repo`
pub(crate) fn derive_repo_name(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    let without_git = trimmed
        .strip_suffix(".git")
        .unwrap_or(trimmed);
    let last = without_git
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("");
    last.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn make_bare_remote() -> TempDir {
        let dir = TempDir::new().unwrap();
        let mut cmd = Command::new("git");
        cmd.arg("init").arg("--bare").arg(dir.path());
        scrub_git_env(&mut cmd);
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "init --bare failed");
        dir
    }

    fn make_seeded_remote() -> TempDir {
        let bare = make_bare_remote();

        let work = TempDir::new().unwrap();
        for args in [
            vec!["init", work.path().to_str().unwrap()],
            vec!["-C", work.path().to_str().unwrap(), "config", "user.email", "t@t"],
            vec!["-C", work.path().to_str().unwrap(), "config", "user.name", "t"],
            vec!["-C", work.path().to_str().unwrap(), "config", "commit.gpgsign", "false"],
        ] {
            let mut c = Command::new("git");
            c.args(args);
            scrub_git_env(&mut c);
            c.output().unwrap();
        }
        std::fs::write(work.path().join("README.md"), "hello").unwrap();
        for args in [
            vec!["-C", work.path().to_str().unwrap(), "add", "-A"],
            vec!["-C", work.path().to_str().unwrap(), "commit", "-m", "init"],
            vec!["-C", work.path().to_str().unwrap(), "remote", "add", "origin", bare.path().to_str().unwrap()],
            vec!["-C", work.path().to_str().unwrap(), "push", "origin", "HEAD:main"],
        ] {
            let mut c = Command::new("git");
            c.args(args);
            scrub_git_env(&mut c);
            c.output().unwrap();
        }
        bare
    }

    #[test]
    fn derive_repo_name_from_https() {
        assert_eq!(derive_repo_name("https://github.com/o/repo.git"), "repo");
        assert_eq!(derive_repo_name("https://github.com/o/repo"), "repo");
        assert_eq!(derive_repo_name("https://github.com/o/repo/"), "repo");
    }

    #[test]
    fn derive_repo_name_from_ssh_short() {
        assert_eq!(derive_repo_name("git@github.com:o/repo.git"), "repo");
        assert_eq!(derive_repo_name("git@github.com:o/repo"), "repo");
    }

    #[test]
    fn derive_repo_name_from_ssh_protocol() {
        assert_eq!(derive_repo_name("ssh://git@host/o/repo.git"), "repo");
    }

    #[test]
    fn derive_repo_name_from_local_path() {
        assert_eq!(derive_repo_name("/tmp/foo/repo.git"), "repo");
        assert_eq!(derive_repo_name("repo.git"), "repo");
    }

    #[test]
    fn derive_repo_name_empty_for_malformed() {
        assert_eq!(derive_repo_name(""), "");
        assert_eq!(derive_repo_name("/"), "");
    }

    #[test]
    fn empty_url_returns_error() {
        let parent = TempDir::new().unwrap();
        let err = git_clone("", Some(parent.path())).unwrap_err();
        assert!(err.contains("vazia"));
    }

    #[test]
    fn invalid_url_returns_git_stderr() {
        let parent = TempDir::new().unwrap();
        let err = git_clone("not-a-url", Some(parent.path())).unwrap_err();
        // git's error message varies by version, but should contain
        // "fatal" or "repository" or similar.
        assert!(
            err.contains("fatal") || err.contains("repository") || err.contains("not"),
            "got: {err}"
        );
    }

    #[test]
    fn collision_with_non_empty_leaf_returns_friendly_error() {
        // URL → leaf "x"; pre-create `<parent>/x` non-empty.
        let parent = TempDir::new().unwrap();
        let blocked_leaf = parent.path().join("x");
        std::fs::create_dir(&blocked_leaf).unwrap();
        std::fs::write(blocked_leaf.join("blocker"), b"x").unwrap();
        let err = git_clone("https://example.invalid/x.git", Some(parent.path())).unwrap_err();
        assert!(err.contains("já existe e não está vazio"), "got: {err}");
    }

    #[test]
    fn parent_does_not_exist_returns_friendly_error() {
        let parent = TempDir::new().unwrap();
        let missing_parent = parent.path().join("missing");
        let err =
            git_clone("https://example.invalid/x.git", Some(&missing_parent)).unwrap_err();
        assert!(err.contains("pasta pai"), "got: {err}");
    }

    #[test]
    fn parent_is_a_file_returns_friendly_error() {
        let parent = TempDir::new().unwrap();
        let file = parent.path().join("not-a-dir");
        std::fs::write(&file, b"x").unwrap();
        let err = git_clone("https://example.invalid/x.git", Some(&file)).unwrap_err();
        assert!(err.contains("não é uma pasta"), "got: {err}");
    }

    #[test]
    fn happy_path_clones_into_parent_under_derived_leaf() {
        let bare = make_seeded_remote();
        let parent = TempDir::new().unwrap();

        let leaf = derive_repo_name(bare.path().to_str().unwrap());
        let expected_dest = parent.path().join(&leaf);

        let outcome = git_clone(
            bare.path().to_str().unwrap(),
            Some(parent.path()),
        )
        .unwrap();
        assert_eq!(outcome.destination, expected_dest);
        assert!(expected_dest.join(".git").is_dir(), ".git folder should exist");
        assert!(
            expected_dest.join("README.md").is_file(),
            "seeded file should be present"
        );
    }

    #[test]
    fn happy_path_clones_when_leaf_dir_pre_exists_and_is_empty() {
        let bare = make_seeded_remote();
        let parent = TempDir::new().unwrap();
        let leaf = derive_repo_name(bare.path().to_str().unwrap());
        let leaf_dir = parent.path().join(&leaf);
        std::fs::create_dir(&leaf_dir).unwrap();

        let outcome = git_clone(
            bare.path().to_str().unwrap(),
            Some(parent.path()),
        )
        .unwrap();
        assert_eq!(outcome.destination, leaf_dir);
        assert!(leaf_dir.join(".git").is_dir());
    }
}
