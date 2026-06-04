//! Pick + load the initial markdown file when the TUI opens.
//!
//! Heuristic ordering, applied to a vault directory:
//! 1. `README.md` at the root (case-insensitive).
//! 2. `index.md` at the root (case-insensitive).
//! 3. The first `.md` returned by `httui_core::fs::list_workspace`,
//!    which already filters heavy/hidden directories and sorts
//!    deterministically.
//!
//! Returns `Ok(None)` when the vault has no markdown files yet — the
//! caller renders an empty-state screen and lets the user create one
//! when lands.

use std::path::{Path, PathBuf};

use httui_core::fs::FileEntry;

use crate::buffer::Document;
use crate::error::{TuiError, TuiResult};

pub fn pick_initial_file(vault: &Path) -> Option<PathBuf> {
    let entries = httui_core::fs::list_workspace(&vault.to_string_lossy()).ok()?;

    if let Some(p) = find_top_level(&entries, "readme.md") {
        return Some(PathBuf::from(p));
    }
    if let Some(p) = find_top_level(&entries, "index.md") {
        return Some(PathBuf::from(p));
    }
    first_markdown(&entries).map(PathBuf::from)
}

fn find_top_level(entries: &[FileEntry], target_lowercase: &str) -> Option<String> {
    entries
        .iter()
        .find(|e| !e.is_dir && e.name.to_lowercase() == target_lowercase)
        .map(|e| e.path.clone())
}

fn first_markdown(entries: &[FileEntry]) -> Option<String> {
    for e in entries {
        if !e.is_dir && e.name.ends_with(".md") {
            return Some(e.path.clone());
        }
        if e.is_dir {
            if let Some(children) = e.children.as_deref() {
                if let Some(found) = first_markdown(children) {
                    return Some(found);
                }
            }
        }
    }
    None
}

pub fn load_document(vault: &Path, file: &Path) -> TuiResult<Document> {
    let raw = httui_core::fs::read_note(&vault.to_string_lossy(), &file.to_string_lossy())
        .map_err(|e| TuiError::Config(format!("read {}: {e}", file.display())))?;
    Document::from_markdown(&raw)
}

/// Same as `load_document` but also rehydrates each block's
/// `cached_result` from the `block_results` SQLite table, so opening
/// a file restores the last successful response (HTTP/DB) the same
/// way the desktop does. Best-effort: any backend error is logged
/// and the document still returns, with `cached_result = None` on
/// the affected blocks.
pub fn load_and_hydrate(
    vault: &Path,
    file_rel: &Path,
    pool: &sqlx::sqlite::SqlitePool,
    env_store: &httui_core::vault_config::EnvironmentsStore,
) -> TuiResult<Document> {
    let mut doc = load_document(vault, file_rel)?;
    let env_vars = if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(crate::commands::db::load_active_env_vars(env_store))
        })
        .unwrap_or_default()
    } else {
        std::collections::HashMap::new()
    };
    let abs = vault.join(file_rel);
    crate::block_hydrate::hydrate_document_blocking(pool, &mut doc, &env_vars, &abs);
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn touch(dir: &Path, rel: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, "# hi\n").unwrap();
    }

    #[test]
    fn picks_readme_first() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "z-other.md");
        touch(v.path(), "README.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert_eq!(p, PathBuf::from("README.md"));
    }

    #[test]
    fn falls_back_to_index() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "a.md");
        touch(v.path(), "index.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert_eq!(p, PathBuf::from("index.md"));
    }

    #[test]
    fn falls_back_to_first_alphabetical() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "zebra.md");
        touch(v.path(), "alpha.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert_eq!(p, PathBuf::from("alpha.md"));
    }

    #[test]
    fn descends_into_subdir_when_root_empty() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "docs/notes.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert!(p.to_string_lossy().contains("notes.md"));
    }

    #[test]
    fn returns_none_for_empty_vault() {
        let v = TempDir::new().unwrap();
        assert!(pick_initial_file(v.path()).is_none());
    }

    #[test]
    fn case_insensitive_match() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "readme.md");
        let p = pick_initial_file(v.path()).unwrap();
        assert_eq!(p, PathBuf::from("readme.md"));
    }

    #[test]
    fn load_document_parses_file() {
        let v = TempDir::new().unwrap();
        touch(v.path(), "n.md");
        let doc = load_document(v.path(), Path::new("n.md")).unwrap();
        assert!(doc.segment_count() >= 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn load_and_hydrate_restores_persisted_block_response() {
        use crate::buffer::Segment;
        let v = TempDir::new().unwrap();
        let md = "```http alias=ping\nGET https://example.com/health\n```\n";
        std::fs::write(v.path().join("api.md"), md).unwrap();

        let data = TempDir::new().unwrap();
        let pool = httui_core::db::init_db(data.path()).await.unwrap();
        let user_cfg = v.path().join("user.toml");
        let env_store =
            httui_core::vault_config::EnvironmentsStore::new(v.path().to_path_buf(), user_cfg);

        // Save a fake response under the canonical hash so the
        // hydration step has something to find. File key is the
        // absolute path — same shape used by the HTTP runner.
        let abs = v.path().join("api.md").to_string_lossy().to_string();
        let envs = std::collections::HashMap::new();
        let hash = httui_core::block_results::compute_http_cache_hash(
            "GET",
            "https://example.com/health",
            &[],
            &[],
            "",
            &envs,
        );
        httui_core::block_results::save_block_result_with_alias(
            &pool,
            &abs,
            &hash,
            Some("ping"),
            "success",
            r#"{"status":200,"body":{"ok":true}}"#,
            12,
            None,
        )
        .await
        .unwrap();

        let doc = load_and_hydrate(v.path(), Path::new("api.md"), &pool, &env_store).unwrap();
        let cached = doc
            .segments()
            .iter()
            .find_map(|s| match s {
                Segment::Block(b) if b.alias.as_deref() == Some("ping") => b.cached_result.clone(),
                _ => None,
            })
            .expect("cached_result populated by hydrate");
        assert_eq!(cached.get("status").and_then(|v| v.as_i64()), Some(200));
    }
}
