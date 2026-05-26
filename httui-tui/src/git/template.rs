//! Commit-message template — prefill when the user submits an
//! empty draft. Mirrors the desktop's `lib/blocks/commit-template.ts`
//! pattern: "Update <stem>" for a single change, "Update N notes"
//! otherwise. Stems strip the parent path and the extension so the
//! message looks like prose, not a path.

use httui_core::git::status::GitStatus;

/// Render the prefill string for `status`. Returns an empty string
/// when the working tree is clean — the caller is responsible for
/// not committing in that case.
pub fn commit_template(status: &GitStatus) -> String {
    let changed = &status.changed;
    if changed.is_empty() {
        return String::new();
    }
    if changed.len() == 1 {
        let stem = path_stem(&changed[0].path);
        if stem.is_empty() {
            format!("Update {}", changed[0].path)
        } else {
            format!("Update {stem}")
        }
    } else {
        format!("Update {} notes", changed.len())
    }
}

/// Last path segment with its `.md` (or other single) extension
/// stripped. Falls back to the segment as-is when there's no
/// extension dot or when stripping would leave an empty string
/// (e.g. `.gitignore`).
fn path_stem(path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or(path);
    match last.rsplit_once('.') {
        Some((stem, _ext)) if !stem.is_empty() => stem.to_string(),
        _ => last.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httui_core::git::status::FileChange;

    fn change(path: &str) -> FileChange {
        FileChange {
            path: path.to_string(),
            status: "M.".to_string(),
            staged: false,
            untracked: false,
        }
    }

    fn status_with(paths: &[&str]) -> GitStatus {
        let changed: Vec<_> = paths.iter().map(|p| change(p)).collect();
        let clean = changed.is_empty();
        GitStatus {
            branch: Some("main".into()),
            upstream: None,
            ahead: 0,
            behind: 0,
            changed,
            clean,
        }
    }

    #[test]
    fn clean_tree_yields_empty_template() {
        assert_eq!(commit_template(&status_with(&[])), "");
    }

    #[test]
    fn single_file_uses_stem() {
        assert_eq!(
            commit_template(&status_with(&["notes/runbook.md"])),
            "Update runbook"
        );
    }

    #[test]
    fn single_file_with_no_extension_keeps_segment() {
        assert_eq!(
            commit_template(&status_with(&["Makefile"])),
            "Update Makefile"
        );
    }

    #[test]
    fn single_dotfile_keeps_leading_dot() {
        // `.gitignore` has only the extension dot — stripping would
        // leave an empty stem. Falls back to the segment.
        assert_eq!(
            commit_template(&status_with(&[".gitignore"])),
            "Update .gitignore"
        );
    }

    #[test]
    fn multiple_files_use_count() {
        assert_eq!(
            commit_template(&status_with(&["a.md", "b.md", "c.md"])),
            "Update 3 notes"
        );
    }
}
