//! Vault-wide grep for "which runbook uses connection X" (V4
//! cenário 7).
//!
//! Walks `.md` files in the vault, scans for db-block fenced-code
//! info strings, and records every fence whose `connection=<name>`
//! token matches the requested connection. Returns one
//! [`ConnectionUse`] per match — files with multiple matches surface
//! as multiple entries so the UI can deep-link to each fence
//! independently.
//!
//! On-demand only — no cache, no indexing. The frontend hook is
//! debounced and the typical vault is well under 10k files; if a
//! larger vault hits a measurable gap, we add a cache + invalidation
//! by file watcher (Epic 11). Same skip-dirs convention as
//! `tag_index` so build artifacts don't pollute the scan.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::tag_index::skip_dirs;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectionUse {
    /// Path relative to the vault root, normalized to forward slashes.
    pub file: String,
    /// 1-based line number of the fence opener (matches what editors
    /// jump to with `path:line`).
    pub line: u32,
}

/// Scan the vault for db-block fences referencing `connection=<name>`.
/// Returns matches sorted by `(file, line)` for stable UI rendering.
pub fn find_connection_uses(vault_root: &Path, connection_name: &str) -> Vec<ConnectionUse> {
    let mut out = Vec::new();
    if connection_name.is_empty() {
        return out;
    }
    walk(vault_root, vault_root, connection_name, &mut out);
    out.sort_by(|a, b| a.file.cmp(&b.file).then(a.line.cmp(&b.line)));
    out
}

fn walk(vault_root: &Path, dir: &Path, connection_name: &str, out: &mut Vec<ConnectionUse>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if file_type.is_dir() {
            if name.starts_with('.') {
                continue;
            }
            if skip_dirs().contains(&name) {
                continue;
            }
            walk(vault_root, &path, connection_name, out);
            continue;
        }
        if !file_type.is_file() || !is_md_file(&path) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let rel = path.strip_prefix(vault_root).unwrap_or(&path);
        let rel_posix = rel_to_posix(rel);
        for (idx, line) in content.lines().enumerate() {
            if line_matches_connection(line, connection_name) {
                out.push(ConnectionUse {
                    file: rel_posix.clone(),
                    line: (idx as u32) + 1,
                });
            }
        }
    }
}

fn is_md_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md") || e.eq_ignore_ascii_case("markdown"))
}

fn rel_to_posix(rel: &Path) -> String {
    rel.to_string_lossy().replace('\\', "/")
}

/// True iff `line` is a db-block fence opener whose info-string
/// carries `connection=<connection_name>` as a whitespace-delimited
/// token. Matches at the start of the line only (markdown fence
/// openers begin column 0 in the V1 vault format) and does prefix
/// matching on `db-` so all dialects (`db`, `db-postgres`,
/// `db-mysql`, `db-sqlite`) are covered.
fn line_matches_connection(line: &str, connection_name: &str) -> bool {
    let Some(rest) = line.strip_prefix("```") else {
        return false;
    };
    let Some(head) = rest.split_whitespace().next() else {
        return false;
    };
    if head != "db" && !head.starts_with("db-") {
        return false;
    }
    let needle = format!("connection={connection_name}");
    rest.split_whitespace().skip(1).any(|tok| tok == needle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    #[test]
    fn empty_name_returns_empty() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "a.md",
            "```db-postgres connection=x\nSELECT 1;\n```\n",
        );
        assert!(find_connection_uses(tmp.path(), "").is_empty());
    }

    #[test]
    fn matches_single_fence_in_single_file() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "rollout.md",
            "# Rollout\n\n```db-postgres alias=q1 connection=payments-db\nSELECT 1;\n```\n",
        );
        let r = find_connection_uses(tmp.path(), "payments-db");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].file, "rollout.md");
        assert_eq!(r[0].line, 3);
    }

    #[test]
    fn matches_multiple_fences_same_file() {
        let tmp = TempDir::new().unwrap();
        let body = "```db-mysql connection=foo\nSELECT 1;\n```\n\nbody\n\n```db connection=foo\nQUERY 2;\n```\n";
        write(tmp.path(), "x.md", body);
        let r = find_connection_uses(tmp.path(), "foo");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].line, 1);
        assert_eq!(r[1].line, 7);
    }

    #[test]
    fn matches_across_files_sorted() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "z.md", "```db-postgres connection=p\n;\n```\n");
        write(tmp.path(), "a.md", "```db-postgres connection=p\n;\n```\n");
        let r = find_connection_uses(tmp.path(), "p");
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].file, "a.md");
        assert_eq!(r[1].file, "z.md");
    }

    #[test]
    fn ignores_other_connections() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "x.md",
            "```db-postgres connection=other\n;\n```\n",
        );
        assert!(find_connection_uses(tmp.path(), "wanted").is_empty());
    }

    #[test]
    fn ignores_http_blocks_with_same_token() {
        let tmp = TempDir::new().unwrap();
        // HTTP blocks don't carry connection=; even if a body line
        // mentions it accidentally, we only match opener fences.
        write(tmp.path(), "x.md", "```http\nGET /api?connection=p\n```\n");
        assert!(find_connection_uses(tmp.path(), "p").is_empty());
    }

    #[test]
    fn name_prefix_does_not_false_match() {
        // `connection=payments` must not match `payments-db`. We split
        // on whitespace and compare equality, so this is structural.
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "x.md",
            "```db-postgres connection=payments\n;\n```\n",
        );
        assert!(find_connection_uses(tmp.path(), "payments-db").is_empty());
    }

    #[test]
    fn skips_dot_dirs_and_build_dirs() {
        let tmp = TempDir::new().unwrap();
        write(
            tmp.path(),
            "node_modules/foo/x.md",
            "```db connection=p\n;\n```\n",
        );
        write(tmp.path(), ".git/HEAD/x.md", "```db connection=p\n;\n```\n");
        write(tmp.path(), "ok.md", "```db connection=p\n;\n```\n");
        let r = find_connection_uses(tmp.path(), "p");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].file, "ok.md");
    }

    #[test]
    fn uses_posix_separators_in_output() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "notes/sub/x.md", "```db connection=p\n;\n```\n");
        let r = find_connection_uses(tmp.path(), "p");
        assert_eq!(r.len(), 1);
        assert!(r[0].file.contains("notes/sub/x.md"));
    }

    #[test]
    fn ignores_non_md_files() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "x.txt", "```db connection=p\n;\n```\n");
        assert!(find_connection_uses(tmp.path(), "p").is_empty());
    }

    #[test]
    fn line_matches_helper_handles_edge_cases() {
        assert!(line_matches_connection("```db connection=foo", "foo"));
        assert!(line_matches_connection(
            "```db-postgres alias=a connection=foo limit=10",
            "foo"
        ));
        assert!(!line_matches_connection("```db connection=bar", "foo"));
        assert!(!line_matches_connection("not a fence", "foo"));
        assert!(!line_matches_connection("```http", "foo"));
        assert!(!line_matches_connection("```db", "foo"));
        assert!(!line_matches_connection("  ```db connection=foo", "foo")); // indented fence — outside our spec
    }
}
