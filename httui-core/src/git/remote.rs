//! `git remote -v` — list configured remotes.
//!
//! Used by Epic 49's `<SharePopover>` to populate the remotes list.
//! Each remote has multiple lines (one per fetch / push direction);
//! we deduplicate by name + URL so the popover sees one entry per
//! `(name, url)` pair.

use std::path::Path;

use serde::Serialize;

use super::run_git;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

pub fn git_remote_list(vault: &Path) -> Result<Vec<Remote>, String> {
    let raw = run_git(vault, &["remote", "-v"])?;
    Ok(parse_remote_list(&raw))
}

fn parse_remote_list(raw: &str) -> Vec<Remote> {
    let mut out: Vec<Remote> = Vec::new();
    for line in raw.lines() {
        // Format: `<name>\t<url> (fetch|push)`
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut tab_iter = trimmed.split('\t');
        let name = tab_iter.next().unwrap_or("").trim();
        let rest = tab_iter.next().unwrap_or("").trim();
        if name.is_empty() || rest.is_empty() {
            continue;
        }
        // Strip the trailing "(fetch)" / "(push)" suffix.
        let url = rest
            .rsplit_once(' ')
            .map(|(u, _suffix)| u.trim().to_string())
            .unwrap_or_else(|| rest.to_string());
        if url.is_empty() {
            continue;
        }
        if !out.iter().any(|r| r.name == name && r.url == url) {
            out.push(Remote {
                name: name.to_string(),
                url,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::init_repo;
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    #[test]
    fn parse_dedupes_fetch_and_push_pair() {
        let raw = "origin\tgit@github.com:owner/repo.git (fetch)\n\
                   origin\tgit@github.com:owner/repo.git (push)\n";
        let out = parse_remote_list(raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "origin");
        assert_eq!(out[0].url, "git@github.com:owner/repo.git");
    }

    #[test]
    fn parse_keeps_separate_entries_for_different_urls() {
        let raw = "origin\tgit@github.com:a/r.git (fetch)\n\
                   origin\thttps://github.com/a/r.git (push)\n";
        let out = parse_remote_list(raw);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn parse_handles_multiple_remotes() {
        let raw = "origin\tgit@host:o/r.git (fetch)\n\
                   origin\tgit@host:o/r.git (push)\n\
                   upstream\tgit@host:up/r.git (fetch)\n\
                   upstream\tgit@host:up/r.git (push)\n";
        let out = parse_remote_list(raw);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].name, "origin");
        assert_eq!(out[1].name, "upstream");
    }

    #[test]
    fn parse_skips_blank_lines() {
        let raw = "\norigin\tgit@host:o/r (fetch)\n\n\norigin\tgit@host:o/r (push)\n";
        assert_eq!(parse_remote_list(raw).len(), 1);
    }

    #[test]
    fn parse_skips_lines_without_tab_separator() {
        let raw = "garbage line\norigin\tgit@host:o/r (fetch)\n";
        assert_eq!(parse_remote_list(raw).len(), 1);
    }

    #[test]
    fn parse_handles_url_without_direction_suffix() {
        // Defensive — git always emits the suffix, but the parser
        // shouldn't blow up if it's missing.
        let raw = "origin\tgit@host:o/r\n";
        let out = parse_remote_list(raw);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].url, "git@host:o/r");
    }

    #[test]
    fn list_remotes_returns_empty_for_repo_with_no_remotes() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        let out = git_remote_list(dir.path()).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn list_remotes_returns_origin_after_remote_add() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        let mut cmd = Command::new("git");
        super::super::scrub_git_env(&mut cmd);
        let r = cmd
            .arg("-C")
            .arg(dir.path())
            .args(["remote", "add", "origin", "git@github.com:owner/repo.git"])
            .output()
            .unwrap();
        assert!(r.status.success(), "git remote add failed");
        let out = git_remote_list(dir.path()).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "origin");
        assert_eq!(out[0].url, "git@github.com:owner/repo.git");
    }
}
