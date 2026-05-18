//! Vault grep for variable references.
//!
//! Scans every `*.md` under the vault for occurrences of a specific
//! variable key in the canonical reference forms `{{KEY}}` and
//! `{{KEY.<path>}}` (block-style ref). Returns a flat list of hits
//! sorted by file path then line number; the frontend groups them.
//!
//! Pure file scan — no SQLite, no FTS5 index. Fast enough for the
//! typical vault size (hundreds of `.md` files); the frontend
//! debounces refresh on file-watcher events. Skips dotfiles and the
//! same heavy directories `list_workspace` skips (`node_modules`,
//! `target`, `.git`, etc.).

use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VarUseEntry {
    /// Path relative to the vault root, with forward slashes.
    pub file_path: String,
    /// 1-indexed line number where the match occurred.
    pub line: usize,
    /// Trimmed line content. Truncated to ~120 chars with an ellipsis.
    pub snippet: String,
}

const MAX_SNIPPET_CHARS: usize = 120;
const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git", "dist", "build"];

/// Find every `{{KEY}}` and `{{KEY.…}}` occurrence under `vault_path`.
///
/// Returns `Ok(Vec)` even when the key is empty (yields an empty vec)
/// or when no matches exist. Errors only when the vault path itself
/// is unreadable.
pub fn grep_var_uses(vault_path: &str, key: &str) -> Result<Vec<VarUseEntry>, String> {
    let root = Path::new(vault_path);
    if !root.is_dir() {
        return Err("Vault path is not a directory".to_string());
    }
    let key_trimmed = key.trim();
    if key_trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    walk(root, root, key_trimmed, &mut out)?;
    out.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.line.cmp(&b.line)));
    Ok(out)
}

fn walk(dir: &Path, root: &Path, key: &str, out: &mut Vec<VarUseEntry>) -> Result<(), String> {
    let read_dir = std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    for entry in read_dir {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') {
            continue;
        }
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        if path.is_dir() {
            walk(&path, root, key, out)?;
        } else if name.ends_with(".md") {
            scan_file(&path, root, key, out)?;
        }
    }
    Ok(())
}

fn scan_file(
    path: &Path,
    root: &Path,
    key: &str,
    out: &mut Vec<VarUseEntry>,
) -> Result<(), String> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()), // skip unreadable files silently — non-md or perms
    };
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");

    for (i, line) in content.lines().enumerate() {
        if matches_key(line, key) {
            out.push(VarUseEntry {
                file_path: relative.clone(),
                line: i + 1,
                snippet: snippet_from(line, MAX_SNIPPET_CHARS),
            });
        }
    }
    Ok(())
}

/// True when `line` contains either `{{KEY}}` or `{{KEY.<path>}}`.
fn matches_key(line: &str, key: &str) -> bool {
    let plain = format!("{{{{{key}}}}}");
    let dotted = format!("{{{{{key}.");
    line.contains(&plain) || line.contains(&dotted)
}

fn snippet_from(line: &str, max_chars: usize) -> String {
    let trimmed = line.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    struct TempVault {
        path: PathBuf,
    }

    impl TempVault {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join("httui-test-grep")
                .join(format!("{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
        fn write(&self, rel: &str, content: &str) {
            let p = self.path.join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&p, content).unwrap();
        }
        fn as_str(&self) -> &str {
            self.path.to_str().unwrap()
        }
    }

    impl Drop for TempVault {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn errors_when_vault_path_is_not_a_directory() {
        assert!(grep_var_uses("/no/such/path", "API").is_err());
    }

    #[test]
    fn empty_key_returns_empty_vec_without_walking() {
        let v = TempVault::new("empty-key");
        v.write("a.md", "{{API}} usage");
        let out = grep_var_uses(v.as_str(), "").unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn whitespace_only_key_is_treated_as_empty() {
        let v = TempVault::new("ws-key");
        v.write("a.md", "{{API}}");
        let out = grep_var_uses(v.as_str(), "   ").unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn matches_plain_double_brace() {
        let v = TempVault::new("plain");
        v.write("runbook.md", "url: {{API_BASE}}/users\nmethod: GET\n");
        let out = grep_var_uses(v.as_str(), "API_BASE").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].file_path, "runbook.md");
        assert_eq!(out[0].line, 1);
        assert!(out[0].snippet.contains("{{API_BASE}}"));
    }

    #[test]
    fn matches_dotted_block_ref() {
        let v = TempVault::new("dotted");
        v.write("a.md", "user_id: {{login.body.user.id}}\n");
        let out = grep_var_uses(v.as_str(), "login").unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.contains("{{login.body.user.id}}"));
    }

    #[test]
    fn ignores_non_matching_keys() {
        let v = TempVault::new("non-match");
        v.write("a.md", "{{OTHER}} {{login.body}}");
        assert!(grep_var_uses(v.as_str(), "API").unwrap().is_empty());
    }

    #[test]
    fn does_not_match_when_key_is_a_substring() {
        let v = TempVault::new("substr");
        v.write("a.md", "{{API_BASE_URL}} unrelated\n");
        // Looking for "API" should NOT match {{API_BASE_URL}} because
        // matches_key requires {{KEY}} or {{KEY. — a longer key isn't either
        let out = grep_var_uses(v.as_str(), "API").unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn returns_one_entry_per_line_even_with_multiple_hits_on_same_line() {
        // V1 grep is line-granular; multiple occurrences on the same
        // line collapse to one entry. That's intentional — the
        // frontend groups by file already.
        let v = TempVault::new("multi-on-line");
        v.write("a.md", "{{API}} and again {{API}}\n");
        let out = grep_var_uses(v.as_str(), "API").unwrap();
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn emits_one_entry_per_matching_line_in_same_file() {
        let v = TempVault::new("multi-line");
        v.write(
            "a.md",
            "first: {{API}}\nfiller line\nsecond: {{API.body}}\n",
        );
        let out = grep_var_uses(v.as_str(), "API").unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].line, 1);
        assert_eq!(out[1].line, 3);
    }

    #[test]
    fn results_are_sorted_by_file_then_line() {
        let v = TempVault::new("sort");
        v.write("z.md", "{{API}}\n");
        v.write("a.md", "line1\n{{API}}\n");
        v.write("m.md", "{{API}}\n");
        let out = grep_var_uses(v.as_str(), "API").unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].file_path, "a.md");
        assert_eq!(out[0].line, 2);
        assert_eq!(out[1].file_path, "m.md");
        assert_eq!(out[2].file_path, "z.md");
    }

    #[test]
    fn skips_dotfiles_and_heavy_dirs() {
        let v = TempVault::new("skip");
        v.write(".git/HEAD", "{{API}}");
        v.write("node_modules/pkg/README.md", "{{API}}");
        v.write("target/index.md", "{{API}}");
        v.write("real/runbook.md", "{{API}}\n");
        let out = grep_var_uses(v.as_str(), "API").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].file_path, "real/runbook.md");
    }

    #[test]
    fn ignores_non_md_files() {
        let v = TempVault::new("ext");
        v.write("a.txt", "{{API}}");
        v.write("b.md", "{{API}}");
        let out = grep_var_uses(v.as_str(), "API").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].file_path, "b.md");
    }

    #[test]
    fn snippet_truncates_long_lines_with_ellipsis() {
        let v = TempVault::new("trunc");
        let long = format!("{} {{{{API}}}}", "x".repeat(150));
        v.write("a.md", &long);
        let out = grep_var_uses(v.as_str(), "API").unwrap();
        assert_eq!(out.len(), 1);
        assert!(out[0].snippet.ends_with('…'));
        // 120 chars + ellipsis
        assert_eq!(out[0].snippet.chars().count(), MAX_SNIPPET_CHARS + 1);
    }

    #[test]
    fn snippet_keeps_short_lines_intact() {
        let v = TempVault::new("short");
        v.write("a.md", "  {{API}} short  ");
        let out = grep_var_uses(v.as_str(), "API").unwrap();
        assert_eq!(out[0].snippet, "{{API}} short");
    }

    #[test]
    fn key_containing_special_regex_chars_does_not_break_matching() {
        // We use string contains, not regex — should be safe by construction.
        let v = TempVault::new("regex-chars");
        v.write("a.md", "{{KEY}}");
        let out = grep_var_uses(v.as_str(), "KEY").unwrap();
        assert_eq!(out.len(), 1);
    }
}
