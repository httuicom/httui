//! Template registry — Epic 41 Story 04 carry.
//!
//! The empty-state Templates card needs a list of available
//! templates to render its `+ N templates →` count and to seed the
//! picker that copies a chosen template into a fresh vault. Two
//! sources contribute:
//!
//! - **Built-in** templates ship inside the binary (planned: a
//!   `httui-core/embedded-templates/` tree consumed via
//!   `include_str!` in a follow-up slice).
//! - **Vault-local** templates live at
//!   `<vault>/.httui/templates/*.md` — users drop their own
//!   templates here so they survive `git clone`s.
//!
//! This module ships the data model + the vault-local lister.
//! `list_builtin_templates()` returns an empty `Vec` until the
//! content slice lands; consumers compose the two lists when
//! presenting the picker.
//!
//! Frontmatter contract for templates (4-backtick `text` fence so
//! the nested 3-backtick http fence is literal, not a Rust doctest):
//!
//! ````text
//! ---
//! title: "Postgres health check"
//! description: "SELECT 1, current_database(), version()"
//! ---
//! ```http
//! GET https://...
//! ```
//! ````
//!
//! `title` becomes the display name (fallback: file stem). The
//! `description` extra key shows under the title in the picker;
//! falls back to the empty string when absent.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::frontmatter::parse_frontmatter;

/// Where a template comes from. Drives display ordering (built-ins
/// usually pinned first) and whether the template can be edited
/// in-app (vault-local can; built-in can't).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    Builtin,
    Vault,
}

/// A discoverable template. `body` is the full markdown including
/// the frontmatter fence — copied verbatim into the runbook the
/// picker creates, so the user sees the template author's intent
/// preserved (executable blocks, fences, info-string tokens).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Template {
    /// Stable id — file stem for vault templates, slug for built-ins.
    pub id: String,
    /// Display name. Falls back to `id` when frontmatter has no
    /// `title` field.
    pub name: String,
    /// One-line description from the frontmatter `description` key.
    /// Empty when absent.
    pub description: String,
    pub source: TemplateSource,
    /// Full markdown body, frontmatter included.
    pub body: String,
}

/// List built-in templates. Returns an empty `Vec` until the
/// content slice ships the embedded markdown files.
pub fn list_builtin_templates() -> Vec<Template> {
    Vec::new()
}

/// List vault-local templates discovered at
/// `<vault_root>/.httui/templates/*.md`. Subdirectories are
/// ignored (templates are flat at v1; nesting can ship later).
/// Returns an empty `Vec` when the directory doesn't exist or
/// can't be read — auto-discovery is best-effort, never an error
/// the UI surfaces.
pub fn list_vault_templates(vault_root: &Path) -> Vec<Template> {
    let dir = vault_root.join(".httui").join("templates");
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
            continue;
        };
        if ext != "md" && ext != "markdown" {
            continue;
        }
        let Ok(body) = fs::read_to_string(&path) else {
            continue;
        };
        out.push(template_from_body(stem, body, TemplateSource::Vault));
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

fn template_from_body(id_stem: &str, body: String, source: TemplateSource) -> Template {
    let (name, description) = match parse_frontmatter(&body) {
        Some((fm, _)) => {
            let name = fm
                .title
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| id_stem.to_string());
            let description = fm
                .extra
                .get("description")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            (name, description)
        }
        None => (id_stem.to_string(), String::new()),
    };
    Template {
        id: id_stem.to_string(),
        name,
        description,
        source,
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as stdfs;
    use tempfile::tempdir;

    fn mk_template_dir(root: &Path) {
        stdfs::create_dir_all(root.join(".httui").join("templates")).unwrap();
    }

    fn write_template(root: &Path, name: &str, body: &str) {
        stdfs::write(root.join(".httui").join("templates").join(name), body).unwrap();
    }

    #[test]
    fn builtins_starts_empty() {
        assert!(list_builtin_templates().is_empty());
    }

    #[test]
    fn vault_returns_empty_when_dir_missing() {
        let dir = tempdir().unwrap();
        // No .httui/templates created.
        assert!(list_vault_templates(dir.path()).is_empty());
    }

    #[test]
    fn vault_returns_empty_when_dir_present_but_empty() {
        let dir = tempdir().unwrap();
        mk_template_dir(dir.path());
        assert!(list_vault_templates(dir.path()).is_empty());
    }

    #[test]
    fn vault_lists_md_files_with_frontmatter_metadata() {
        let dir = tempdir().unwrap();
        mk_template_dir(dir.path());
        write_template(
            dir.path(),
            "pg-health.md",
            "---\ntitle: \"Postgres health\"\ndescription: heartbeat\n---\n```http\nGET /\n```\n",
        );

        let list = list_vault_templates(dir.path());
        assert_eq!(list.len(), 1);
        let t = &list[0];
        assert_eq!(t.id, "pg-health");
        assert_eq!(t.name, "Postgres health");
        assert_eq!(t.description, "heartbeat");
        assert_eq!(t.source, TemplateSource::Vault);
        assert!(t.body.starts_with("---\n"), "body keeps frontmatter");
    }

    #[test]
    fn vault_falls_back_to_stem_when_frontmatter_missing() {
        let dir = tempdir().unwrap();
        mk_template_dir(dir.path());
        write_template(dir.path(), "raw.md", "# raw runbook\n\nno frontmatter\n");

        let list = list_vault_templates(dir.path());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "raw");
        assert_eq!(list[0].description, "");
    }

    #[test]
    fn vault_falls_back_to_stem_when_title_blank() {
        let dir = tempdir().unwrap();
        mk_template_dir(dir.path());
        write_template(
            dir.path(),
            "blank-title.md",
            "---\ntitle: \"   \"\n---\nbody\n",
        );

        let list = list_vault_templates(dir.path());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "blank-title");
    }

    #[test]
    fn vault_accepts_markdown_extension_alias() {
        let dir = tempdir().unwrap();
        mk_template_dir(dir.path());
        write_template(dir.path(), "alt.markdown", "body\n");

        let list = list_vault_templates(dir.path());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "alt");
    }

    #[test]
    fn vault_skips_non_markdown_files() {
        let dir = tempdir().unwrap();
        mk_template_dir(dir.path());
        write_template(dir.path(), "skip.txt", "not a runbook\n");
        write_template(dir.path(), "skip.json", "{}\n");
        write_template(dir.path(), "real.md", "# real\n");

        let list = list_vault_templates(dir.path());
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "real");
    }

    #[test]
    fn vault_does_not_recurse_into_subdirs() {
        let dir = tempdir().unwrap();
        mk_template_dir(dir.path());
        let nested = dir.path().join(".httui").join("templates").join("nested");
        stdfs::create_dir_all(&nested).unwrap();
        stdfs::write(nested.join("hidden.md"), "body").unwrap();

        let list = list_vault_templates(dir.path());
        assert!(list.is_empty(), "subdir templates not surfaced (yet)");
    }

    #[test]
    fn vault_results_sort_by_id() {
        let dir = tempdir().unwrap();
        mk_template_dir(dir.path());
        write_template(dir.path(), "zebra.md", "");
        write_template(dir.path(), "apple.md", "");
        write_template(dir.path(), "mango.md", "");

        let list = list_vault_templates(dir.path());
        let ids: Vec<&str> = list.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["apple", "mango", "zebra"]);
    }

    #[test]
    fn template_source_serializes_snake_case() {
        let v = serde_json::to_string(&TemplateSource::Vault).unwrap();
        let b = serde_json::to_string(&TemplateSource::Builtin).unwrap();
        assert_eq!(v, "\"vault\"");
        assert_eq!(b, "\"builtin\"");
    }
}
