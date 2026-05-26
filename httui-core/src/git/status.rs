//! `git status` / `git diff` / `git branch` wrappers.
//!
//! `status` parses `git status --porcelain=v2 --branch` so we get a
//! stable machine-readable format. `diff` returns raw unified-diff
//! text — the frontend's existing diff viewer renders it. `branch`
//! lists local branches and reports the current one.

use std::path::Path;

use serde::Serialize;

use super::run_git;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct GitStatus {
    /// Current branch. `None` for a detached HEAD.
    pub branch: Option<String>,
    /// Upstream branch (`origin/main`), if any.
    pub upstream: Option<String>,
    /// Commits ahead of upstream. `0` when no upstream.
    pub ahead: u32,
    /// Commits behind upstream. `0` when no upstream.
    pub behind: u32,
    /// Files in the working tree with any kind of change (staged,
    /// modified, untracked).
    pub changed: Vec<FileChange>,
    /// True when the working tree is clean (no changed entries).
    pub clean: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileChange {
    pub path: String,
    pub status: String,
    /// True when this entry is in the staging area only (not yet
    /// committed).
    pub staged: bool,
    /// True when the file is not tracked by git.
    pub untracked: bool,
}

/// Read working-tree state via `git status --porcelain=v2 --branch`.
pub fn git_status(vault: &Path) -> Result<GitStatus, String> {
    let raw = run_git(vault, &["status", "--porcelain=v2", "--branch"])?;
    parse_status(&raw)
}

fn parse_status(raw: &str) -> Result<GitStatus, String> {
    let mut out = GitStatus::default();
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            if rest != "(detached)" {
                out.branch = Some(rest.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            out.upstream = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            // Format: "+<ahead> -<behind>"
            for token in rest.split_whitespace() {
                if let Some(n) = token.strip_prefix('+') {
                    out.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = token.strip_prefix('-') {
                    out.behind = n.parse().unwrap_or(0);
                }
            }
        } else if let Some(rest) = line.strip_prefix("? ") {
            out.changed.push(FileChange {
                path: rest.to_string(),
                status: "??".to_string(),
                staged: false,
                untracked: true,
            });
        } else if let Some(rest) = line.strip_prefix("1 ") {
            // Ordinary changed entry: `<XY> <sub> <mH> <mI> <mW> <hH> <hI> <path>`
            // We only care about XY + path here. XY is two chars: staged + worktree.
            let mut fields = rest.splitn(8, ' ');
            let xy = fields.next().unwrap_or("..");
            let path = fields.nth(6).unwrap_or("").to_string();
            if path.is_empty() {
                continue;
            }
            let staged = xy.chars().next().map(|c| c != '.').unwrap_or(false);
            out.changed.push(FileChange {
                path,
                status: xy.to_string(),
                staged,
                untracked: false,
            });
        } else if let Some(rest) = line.strip_prefix("2 ") {
            // Renamed/copied entry: `<XY> ... <X><score> <path>\t<orig>`
            // Take the path up to the tab.
            let mut fields = rest.splitn(9, ' ');
            let xy = fields.next().unwrap_or("..");
            let tail = fields.nth(7).unwrap_or("");
            let path = tail.split('\t').next().unwrap_or("").to_string();
            if path.is_empty() {
                continue;
            }
            out.changed.push(FileChange {
                path,
                status: xy.to_string(),
                staged: true,
                untracked: false,
            });
        } else if let Some(rest) = line.strip_prefix("u ") {
            // Unmerged (conflict) entry:
            // `<XY> <sub> <m1> <m2> <m3> <mW> <h1> <h2> <h3> <path>`
            // XY is e.g. `UU`/`AA`/`DD`/`AU`/`UD`. Not staged + not
            // untracked so the frontend's `labelFileStatus` maps it to
            // "conflicted".
            let mut fields = rest.splitn(10, ' ');
            let xy = fields.next().unwrap_or("..");
            let path = fields.nth(8).unwrap_or("").to_string();
            if path.is_empty() {
                continue;
            }
            out.changed.push(FileChange {
                path,
                status: xy.to_string(),
                staged: false,
                untracked: false,
            });
        }
    }
    out.clean = out.changed.is_empty();
    Ok(out)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub current: bool,
    pub remote: bool,
}

/// List local branches plus their `remotes/origin/*` counterparts.
pub fn git_branch_list(vault: &Path) -> Result<Vec<BranchInfo>, String> {
    let raw = run_git(
        vault,
        &[
            "for-each-ref",
            "--format=%(refname:short)\x1f%(HEAD)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;
    let mut out = Vec::new();
    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\x1f');
        let name = parts.next().unwrap_or("").to_string();
        let head_marker = parts.next().unwrap_or("");
        if name.is_empty() {
            continue;
        }
        // refs/remotes/origin/HEAD shows up as a symbolic alias —
        // skip it; it isn't a branch the user can check out.
        if name.ends_with("/HEAD") {
            continue;
        }
        out.push(BranchInfo {
            current: head_marker == "*",
            remote: name.starts_with("origin/")
                || name.starts_with("remotes/")
                || name.contains('/'),
            name,
        });
    }
    Ok(out)
}

/// Return the unified diff for a single commit (or `HEAD..workdir`
/// when `commit_sha` is `None`).
pub fn git_diff(vault: &Path, commit_sha: Option<&str>) -> Result<String, String> {
    match commit_sha {
        Some(sha) => run_git(vault, &["show", "--no-color", sha]),
        None => run_git(vault, &["diff", "--no-color", "HEAD"]),
    }
}

/// Aggregate +/- counts of the working tree against `HEAD`. Untracked
/// files are not counted — they have no diff base. `files` matches
/// `git diff --shortstat`'s tracked-only count; UI surfaces that
/// alongside the untracked count from [`git_status`] for completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
pub struct DiffMetrics {
    pub files: u32,
    pub insertions: u32,
    pub deletions: u32,
}

/// Run `git diff HEAD --shortstat --no-color` and parse the summary
/// line. Returns zeroes when the working tree is clean (empty stdout)
/// or when only untracked files exist.
pub fn git_diff_shortstat(vault: &Path) -> Result<DiffMetrics, String> {
    let raw = run_git(vault, &["diff", "HEAD", "--shortstat", "--no-color"])?;
    Ok(parse_shortstat(&raw))
}

fn parse_shortstat(raw: &str) -> DiffMetrics {
    let mut out = DiffMetrics::default();
    let line = raw.trim();
    if line.is_empty() {
        return out;
    }
    for part in line.split(',') {
        let p = part.trim();
        let parsed_into = |suffixes: &[&str]| -> Option<u32> {
            for s in suffixes {
                if let Some(rest) = p.strip_suffix(s) {
                    return rest.trim().parse().ok();
                }
            }
            None
        };
        if let Some(n) = parsed_into(&[" files changed", " file changed"]) {
            out.files = n;
        } else if let Some(n) = parsed_into(&[" insertions(+)", " insertion(+)"]) {
            out.insertions = n;
        } else if let Some(n) = parsed_into(&[" deletions(-)", " deletion(-)"]) {
            out.deletions = n;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{commit_all, init_repo};
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn status_clean_repo_after_commit() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1").unwrap();
        commit_all(dir.path(), "init");
        let s = git_status(dir.path()).unwrap();
        assert!(s.clean);
        assert!(s.changed.is_empty());
        assert_eq!(s.ahead, 0);
        assert_eq!(s.behind, 0);
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert!(s.upstream.is_none());
    }

    #[test]
    fn status_reports_untracked_file() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("new"), "x").unwrap();
        let s = git_status(dir.path()).unwrap();
        assert!(!s.clean);
        assert_eq!(s.changed.len(), 1);
        assert_eq!(s.changed[0].path, "new");
        assert!(s.changed[0].untracked);
    }

    #[test]
    fn status_reports_modified_file() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("a"), "2").unwrap();
        let s = git_status(dir.path()).unwrap();
        assert!(!s.clean);
        assert_eq!(s.changed.len(), 1);
        assert_eq!(s.changed[0].path, "a");
        assert!(!s.changed[0].untracked);
    }

    #[test]
    fn parse_status_reports_unmerged_conflict_line() {
        // porcelain=v2 `u` line for a both-modified conflict.
        let raw = "# branch.head main\nu UU N... 100644 100644 100644 100644 aaa bbb ccc runbooks/okl.md\n";
        let s = parse_status(raw).unwrap();
        assert!(!s.clean, "conflicted tree must not read as clean");
        assert_eq!(s.changed.len(), 1);
        assert_eq!(s.changed[0].path, "runbooks/okl.md");
        assert_eq!(s.changed[0].status, "UU");
        assert!(!s.changed[0].staged);
        assert!(!s.changed[0].untracked);
    }

    #[test]
    fn status_reports_real_merge_conflict_as_unmerged() {
        use std::process::Command;
        let dir = TempDir::new().unwrap();
        let p = dir.path();
        init_repo(p);
        std::fs::write(p.join("f.txt"), "base\n").unwrap();
        commit_all(p, "base");
        let run = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(p)
                .args(args)
                .output()
                .unwrap()
        };
        run(&["checkout", "-b", "feature"]);
        std::fs::write(p.join("f.txt"), "theirs\n").unwrap();
        commit_all(p, "theirs");
        run(&["checkout", "main"]);
        std::fs::write(p.join("f.txt"), "ours\n").unwrap();
        commit_all(p, "ours");
        let _ = run(&["merge", "feature"]); // conflicts
        let s = git_status(p).unwrap();
        assert!(!s.clean);
        let f = s
            .changed
            .iter()
            .find(|c| c.path == "f.txt")
            .expect("conflicted f.txt must appear in status");
        assert!(f.status.contains('U'), "got status {}", f.status);
        assert!(!f.untracked);
    }

    #[test]
    fn parse_status_handles_renamed_entry() {
        // porcelain=v2 `2` line: rename, path is `<new>\t<orig>`.
        let raw = "# branch.head main\n2 R. N... 100644 100644 100644 aaaa bbbb R100 docs/new.md\tdocs/old.md\n";
        let s = parse_status(raw).unwrap();
        assert_eq!(s.changed.len(), 1);
        assert_eq!(s.changed[0].path, "docs/new.md");
        assert_eq!(s.changed[0].status, "R.");
        assert!(s.changed[0].staged);
        assert!(!s.clean);
    }

    #[test]
    fn parse_status_counts_ahead_and_behind() {
        let raw = "# branch.head main\n# branch.ab +5 -2\n";
        let s = parse_status(raw).unwrap();
        assert_eq!(s.ahead, 5);
        assert_eq!(s.behind, 2);
    }

    #[test]
    fn parse_status_skips_entries_with_empty_paths() {
        // Truncated `1`/`2`/`u` lines (no path field) must be
        // skipped, not panic, leaving the tree clean.
        let raw = "1 .M N... 100644\n2 R. N... 100644\nu UU N... 100644\n";
        let s = parse_status(raw).unwrap();
        assert!(s.clean);
        assert!(s.changed.is_empty());
    }

    #[test]
    fn branch_list_skips_origin_head_alias() {
        use std::process::Command;
        let remote = TempDir::new().unwrap();
        Command::new("git")
            .args(["init", "--bare", "--initial-branch=main"])
            .arg(remote.path())
            .output()
            .unwrap();
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1").unwrap();
        commit_all(dir.path(), "init");
        let run = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(args)
                .output()
                .unwrap()
        };
        run(&["remote", "add", "origin"]);
        Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["remote", "set-url", "origin"])
            .arg(remote.path())
            .output()
            .unwrap();
        run(&["push", "-u", "origin", "main"]);
        run(&["remote", "set-head", "origin", "main"]);
        let branches = git_branch_list(dir.path()).unwrap();
        // origin/HEAD symbolic alias must be filtered out.
        assert!(branches.iter().all(|b| !b.name.ends_with("/HEAD")));
        assert!(branches.iter().any(|b| b.name == "main" && b.current));
    }

    #[test]
    fn git_diff_workdir_and_commit_modes() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1\n").unwrap();
        let sha = commit_all(dir.path(), "init");
        // Commit mode: `git show <sha>` includes the subject.
        let commit_diff = git_diff(dir.path(), Some(&sha)).unwrap();
        assert!(commit_diff.contains("init"));
        // Workdir mode: HEAD..worktree diff of an edit.
        std::fs::write(dir.path().join("a"), "2\n").unwrap();
        let work_diff = git_diff(dir.path(), None).unwrap();
        assert!(work_diff.contains("+2"));
    }

    #[test]
    fn parse_status_handles_branch_ab_format() {
        let raw = "# branch.head main\n# branch.upstream origin/main\n# branch.ab +3 -1\n";
        let s = parse_status(raw).unwrap();
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!(s.ahead, 3);
        assert_eq!(s.behind, 1);
    }

    #[test]
    fn parse_status_handles_detached_head() {
        let raw = "# branch.head (detached)\n";
        let s = parse_status(raw).unwrap();
        assert!(s.branch.is_none());
    }

    #[test]
    fn branch_list_contains_main_after_init_commit() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1").unwrap();
        commit_all(dir.path(), "init");
        let bs = git_branch_list(dir.path()).unwrap();
        assert!(bs.iter().any(|b| b.name == "main" && b.current));
    }

    #[test]
    fn diff_for_commit_returns_show_output() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1\n").unwrap();
        let sha = commit_all(dir.path(), "init");
        let d = git_diff(dir.path(), Some(&sha)).unwrap();
        assert!(d.contains("init"));
        assert!(d.contains("+1"));
    }

    #[test]
    fn diff_workdir_when_no_sha() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1\n").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("a"), "2\n").unwrap();
        let d = git_diff(dir.path(), None).unwrap();
        assert!(d.contains("-1"));
        assert!(d.contains("+2"));
    }

    #[test]
    fn parse_shortstat_plural_files_inserts_and_deletes() {
        let raw = " 3 files changed, 12 insertions(+), 5 deletions(-)\n";
        let m = parse_shortstat(raw);
        assert_eq!(
            m,
            DiffMetrics {
                files: 3,
                insertions: 12,
                deletions: 5,
            }
        );
    }

    #[test]
    fn parse_shortstat_singular_forms() {
        let raw = " 1 file changed, 1 insertion(+), 1 deletion(-)\n";
        let m = parse_shortstat(raw);
        assert_eq!(
            m,
            DiffMetrics {
                files: 1,
                insertions: 1,
                deletions: 1,
            }
        );
    }

    #[test]
    fn parse_shortstat_only_insertions() {
        let raw = " 2 files changed, 4 insertions(+)\n";
        let m = parse_shortstat(raw);
        assert_eq!(m.files, 2);
        assert_eq!(m.insertions, 4);
        assert_eq!(m.deletions, 0);
    }

    #[test]
    fn parse_shortstat_only_deletions() {
        let raw = " 1 file changed, 7 deletions(-)\n";
        let m = parse_shortstat(raw);
        assert_eq!(m.files, 1);
        assert_eq!(m.deletions, 7);
        assert_eq!(m.insertions, 0);
    }

    #[test]
    fn parse_shortstat_empty_returns_zeroes() {
        assert_eq!(parse_shortstat(""), DiffMetrics::default());
        assert_eq!(parse_shortstat("   \n"), DiffMetrics::default());
    }

    #[test]
    fn diff_shortstat_reports_modified_file_counts() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1\n2\n3\n").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("a"), "1\n2\n3\n4\n5\n").unwrap();
        let m = git_diff_shortstat(dir.path()).unwrap();
        assert_eq!(m.files, 1);
        assert_eq!(m.insertions, 2);
        assert_eq!(m.deletions, 0);
    }

    #[test]
    fn diff_shortstat_clean_tree_is_all_zeros() {
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1\n").unwrap();
        commit_all(dir.path(), "init");
        let m = git_diff_shortstat(dir.path()).unwrap();
        assert_eq!(m, DiffMetrics::default());
    }

    #[test]
    fn diff_shortstat_ignores_untracked_files() {
        // Untracked files have no HEAD baseline → shortstat must
        // report zeroes (the UI gets the untracked count from
        // `git_status` instead).
        let dir = TempDir::new().unwrap();
        init_repo(dir.path());
        std::fs::write(dir.path().join("a"), "1\n").unwrap();
        commit_all(dir.path(), "init");
        std::fs::write(dir.path().join("new.md"), "hi\n").unwrap();
        let m = git_diff_shortstat(dir.path()).unwrap();
        assert_eq!(m, DiffMetrics::default());
    }
}
