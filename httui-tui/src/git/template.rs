//! Commit-message prefill.
//!
//! - Empty template → "Update <stem>" for one change, "Update N
//!   notes" otherwise.
//! - Configured (`user.toml [ui].git_commit_template`) → placeholders
//!   `{{notes}}` (stems comma-joined), `{{count}}`, `{{date}}`
//!   (`YYYY-MM-DD`).

use httui_core::git::status::GitStatus;

/// Render the prefill for `status`. Empty `template` uses the
/// built-in default; empty `status.changed` collapses to `""`.
pub fn commit_template(status: &GitStatus, template: &str) -> String {
    render_template(
        &status
            .changed
            .iter()
            .map(|c| c.path.clone())
            .collect::<Vec<_>>(),
        template,
        today_iso(),
    )
}

fn render_template(paths: &[String], template: &str, today: String) -> String {
    let stems: Vec<String> = paths.iter().map(|p| note_stem(p)).collect();
    let count = stems.len();
    let tpl = template.trim();
    if !tpl.is_empty() {
        return tpl
            .replace("{{notes}}", &stems.join(", "))
            .replace("{{count}}", &count.to_string())
            .replace("{{date}}", &today);
    }
    if count == 0 {
        return String::new();
    }
    if count == 1 {
        return format!("Update {}", stems[0]);
    }
    format!("Update {count} notes")
}

/// Basename without a trailing `.md`. Other extensions keep their
/// full filename so non-note files read sensibly.
fn note_stem(path: &str) -> String {
    let last = path.rsplit('/').next().unwrap_or(path);
    if let Some(stripped) = last.strip_suffix(".md") {
        if !stripped.is_empty() {
            return stripped.to_string();
        }
    }
    last.to_string()
}

fn today_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    iso_date_from_unix(secs)
}

/// `YYYY-MM-DD` for a Unix timestamp using the proleptic Gregorian
/// calendar — avoids pulling in `chrono` for just this.
fn iso_date_from_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant's `days_from_civil` inverse. Returns
/// `(year, month_1_12, day_1_31)`.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 }.div_euclid(146_097);
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y as i32, m, d)
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

    // ---- built-in default (empty template) -------------------------

    #[test]
    fn default_clean_tree_yields_empty() {
        assert_eq!(commit_template(&status_with(&[]), ""), "");
    }

    #[test]
    fn default_single_file_uses_stem() {
        assert_eq!(
            commit_template(&status_with(&["notes/runbook.md"]), ""),
            "Update runbook"
        );
    }

    #[test]
    fn default_multiple_files_use_count() {
        assert_eq!(
            commit_template(&status_with(&["a.md", "b.md", "c.md"]), ""),
            "Update 3 notes"
        );
    }

    #[test]
    fn default_keeps_non_md_extension() {
        assert_eq!(
            commit_template(&status_with(&["Makefile"]), ""),
            "Update Makefile"
        );
    }

    #[test]
    fn default_preserves_dotfile_stem() {
        // `.md` only stripped from real `.md` suffix; `.gitignore`
        // keeps its leading dot.
        assert_eq!(
            commit_template(&status_with(&[".gitignore"]), ""),
            "Update .gitignore"
        );
    }

    // ---- configurable template -------------------------------------

    #[test]
    fn template_substitutes_notes_placeholder() {
        let s = status_with(&["a.md", "sub/b.md"]);
        assert_eq!(commit_template(&s, "wip: {{notes}}"), "wip: a, b");
    }

    #[test]
    fn template_substitutes_count_placeholder() {
        let s = status_with(&["a.md", "b.md"]);
        assert_eq!(
            commit_template(&s, "docs({{count}}): batch"),
            "docs(2): batch"
        );
    }

    #[test]
    fn template_substitutes_date_placeholder() {
        let paths = ["a.md".to_string()];
        // 2026-05-26 noon UTC ≈ unix 1779700800.
        let rendered = render_template(&paths, "daily-{{date}}", "2026-05-26".into());
        assert_eq!(rendered, "daily-2026-05-26");
    }

    #[test]
    fn template_trims_whitespace_before_checking_empty() {
        // A template that's just whitespace falls back to default.
        assert_eq!(commit_template(&status_with(&["a.md"]), "   "), "Update a");
    }

    #[test]
    fn template_with_no_placeholders_is_used_verbatim() {
        assert_eq!(
            commit_template(&status_with(&["a.md", "b.md"]), "chore: sync"),
            "chore: sync"
        );
    }

    // ---- helpers ---------------------------------------------------

    #[test]
    fn note_stem_strips_md_and_path_prefix() {
        assert_eq!(note_stem("notes/runbook.md"), "runbook");
        assert_eq!(note_stem("a.md"), "a");
        assert_eq!(note_stem("Makefile"), "Makefile");
        assert_eq!(note_stem(".gitignore"), ".gitignore");
        assert_eq!(note_stem("deep/sub/dir/file.md"), "file");
    }

    #[test]
    fn iso_date_from_unix_matches_known_dates() {
        // 2026-05-26 UTC = days from 1970-01-01: 20_599 (validated
        // out-of-band).
        assert_eq!(iso_date_from_unix(20_599 * 86_400), "2026-05-26");
        // 1970-01-01 ↔ 0.
        assert_eq!(iso_date_from_unix(0), "1970-01-01");
        // 2000-01-01.
        assert_eq!(iso_date_from_unix(946_684_800), "2000-01-01");
    }
}
