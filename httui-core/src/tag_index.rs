//! Vault-wide frontmatter tag index (Epic 52 Story 04).
//!
//! Walks `.md` files in the vault, parses YAML frontmatter, and
//! extracts the `tags:` flow-list. Returns one `TagEntry` per file
//! that declares at least one tag. The frontend's
//! `useTagIndexStore` consumes the list on vault-open and on
//! file-watcher save events; missing files / unreadable files /
//! files without frontmatter are silently skipped — the index is
//! best-effort and self-heals on the next walk.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::frontmatter::parse_frontmatter;

/// Directory names that are common build / dependency / VCS noise.
/// Same shape as `crate::dotenv::SKIP_DIRS` but the lists diverge
/// on purpose: dotenv only walks depth-1; the tag walker recurses,
/// so it benefits from skipping a few more.
const SKIP_DIRS: &[&str] = &[
    ".git",
    ".httui",
    ".idea",
    ".vscode",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".turbo",
    ".venv",
    "venv",
    "__pycache__",
];

/// Public accessor — other vault walkers (e.g. connection_uses)
/// share the skip list so the same noise is filtered consistently.
pub fn skip_dirs() -> &'static [&'static str] {
    SKIP_DIRS
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TagEntry {
    /// Path relative to the vault root, normalized to forward
    /// slashes so the same key shape works on Windows + Unix.
    pub path: String,
    pub tags: Vec<String>,
}

/// Walk `vault_root` recursively and return one `TagEntry` per
/// `.md`/`.markdown` file whose frontmatter has a non-empty `tags`
/// list. Output is sorted by `path` for stable consumer behaviour.
pub fn scan_vault_tags(vault_root: &Path) -> Vec<TagEntry> {
    let mut out = Vec::new();
    walk(vault_root, vault_root, &mut out);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn walk(vault_root: &Path, dir: &Path, out: &mut Vec<TagEntry>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if file_type.is_dir() {
            if name.starts_with('.') {
                // Skip every dot-directory (covers `.git`, `.httui`,
                // anything user-hidden). SKIP_DIRS is checked too
                // so non-dot-prefixed noise still gets pruned.
                continue;
            }
            if SKIP_DIRS.contains(&name) {
                continue;
            }
            walk(vault_root, &path, out);
            continue;
        }
        if !file_type.is_file() {
            // Symlinks etc. — skip; we don't want to follow links
            // out of the vault.
            continue;
        }
        if !is_md_file(&path) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let Some((fm, _body)) = parse_frontmatter(&content) else {
            continue;
        };
        if fm.tags.is_empty() {
            continue;
        }
        let rel = path.strip_prefix(vault_root).unwrap_or(&path);
        out.push(TagEntry {
            path: rel_to_posix(rel),
            tags: fm.tags,
        });
    }
}

fn is_md_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("md") | Some("markdown")
    )
}

fn rel_to_posix(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, body).unwrap();
    }

    fn fm(tags: &[&str]) -> String {
        let tags_csv = tags
            .iter()
            .map(|t| (*t).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("---\ntitle: \"Doc\"\ntags: [{tags_csv}]\n---\n# body\n",)
    }

    #[test]
    fn empty_dir_returns_empty_vec() {
        let dir = tempdir().unwrap();
        assert!(scan_vault_tags(dir.path()).is_empty());
    }

    #[test]
    fn picks_up_md_with_tags_at_root() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a.md", &fm(&["payments", "debug"]));
        let r = scan_vault_tags(dir.path());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "a.md");
        assert_eq!(r[0].tags, vec!["payments", "debug"]);
    }

    #[test]
    fn recurses_into_nested_dirs() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a.md", &fm(&["alpha"]));
        write(dir.path(), "sub/b.md", &fm(&["beta"]));
        write(dir.path(), "sub/deeper/c.md", &fm(&["gamma"]));
        let r = scan_vault_tags(dir.path());
        let paths: Vec<&str> = r.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["a.md", "sub/b.md", "sub/deeper/c.md"]);
    }

    #[test]
    fn skips_md_without_frontmatter() {
        let dir = tempdir().unwrap();
        write(dir.path(), "no-fm.md", "# just markdown\n");
        write(dir.path(), "with-fm.md", &fm(&["t"]));
        let r = scan_vault_tags(dir.path());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "with-fm.md");
    }

    #[test]
    fn skips_md_with_frontmatter_but_no_tags() {
        let dir = tempdir().unwrap();
        write(dir.path(), "no-tags.md", "---\ntitle: \"x\"\n---\n# body\n");
        let r = scan_vault_tags(dir.path());
        assert!(r.is_empty());
    }

    #[test]
    fn skips_md_with_empty_tag_list() {
        let dir = tempdir().unwrap();
        write(
            dir.path(),
            "empty-tags.md",
            "---\ntitle: \"x\"\ntags: []\n---\n# body\n",
        );
        let r = scan_vault_tags(dir.path());
        assert!(r.is_empty());
    }

    #[test]
    fn skips_known_noise_dirs() {
        let dir = tempdir().unwrap();
        write(dir.path(), "node_modules/lib/x.md", &fm(&["junk"]));
        write(dir.path(), "target/x.md", &fm(&["junk"]));
        write(dir.path(), "dist/x.md", &fm(&["junk"]));
        write(dir.path(), "real.md", &fm(&["real"]));
        let r = scan_vault_tags(dir.path());
        let paths: Vec<&str> = r.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["real.md"]);
    }

    #[test]
    fn skips_dot_directories() {
        let dir = tempdir().unwrap();
        write(dir.path(), ".git/x.md", &fm(&["junk"]));
        write(dir.path(), ".httui/x.md", &fm(&["junk"]));
        write(dir.path(), ".obsidian/x.md", &fm(&["junk"]));
        write(dir.path(), "real.md", &fm(&["real"]));
        let r = scan_vault_tags(dir.path());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "real.md");
    }

    #[test]
    fn picks_up_markdown_extension_alias() {
        let dir = tempdir().unwrap();
        write(dir.path(), "doc.markdown", &fm(&["t"]));
        let r = scan_vault_tags(dir.path());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "doc.markdown");
    }

    #[test]
    fn ignores_non_md_extensions() {
        let dir = tempdir().unwrap();
        write(dir.path(), "x.txt", &fm(&["t"]));
        write(dir.path(), "x.mdx", &fm(&["t"]));
        write(dir.path(), "y.md", &fm(&["yes"]));
        let r = scan_vault_tags(dir.path());
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "y.md");
    }

    #[test]
    fn output_is_sorted_by_path() {
        let dir = tempdir().unwrap();
        write(dir.path(), "z.md", &fm(&["z"]));
        write(dir.path(), "m.md", &fm(&["m"]));
        write(dir.path(), "a.md", &fm(&["a"]));
        let r = scan_vault_tags(dir.path());
        let paths: Vec<&str> = r.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, vec!["a.md", "m.md", "z.md"]);
    }

    #[test]
    fn returns_empty_when_root_does_not_exist() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        assert!(scan_vault_tags(&missing).is_empty());
    }

    #[test]
    fn rel_path_uses_forward_slashes() {
        let dir = tempdir().unwrap();
        write(dir.path(), "a/b/c.md", &fm(&["t"]));
        let r = scan_vault_tags(dir.path());
        assert_eq!(r[0].path, "a/b/c.md"); // not "a\\b\\c.md" on Windows
    }

    #[test]
    fn tag_entry_serializes_with_path_and_tags() {
        let entry = TagEntry {
            path: "a.md".into(),
            tags: vec!["x".into(), "y".into()],
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"path\":\"a.md\""));
        assert!(json.contains("\"tags\":[\"x\",\"y\"]"));
    }
}
