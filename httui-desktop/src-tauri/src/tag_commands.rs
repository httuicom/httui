//! Tauri command wrapping `httui_core::tag_index`. Powers
//! the frontend `useTagIndexStore` calls this on vault-open
//! and after file-watcher save events.

use httui_core::tag_index::{scan_vault_tags, TagEntry};
use std::path::PathBuf;

/// Walk `vault_path` and return one entry per `.md` file with a
/// non-empty `tags:` frontmatter list. Best-effort — IO errors
/// surface as an empty result for that file, never a hard error,
/// so the consumer can refresh on file save and self-heal.
#[tauri::command]
pub async fn scan_vault_tags_cmd(vault_path: String) -> Result<Vec<TagEntry>, String> {
    Ok(scan_vault_tags(&PathBuf::from(vault_path)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn scan_returns_empty_for_empty_vault() {
        let dir = tempdir().unwrap();
        let r = scan_vault_tags_cmd(dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        assert!(r.is_empty());
    }

    #[tokio::test]
    async fn scan_picks_up_md_with_tags() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("a.md"),
            "---\ntitle: \"x\"\ntags: [foo, bar]\n---\n# body\n",
        )
        .unwrap();
        let r = scan_vault_tags_cmd(dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "a.md");
        assert_eq!(r[0].tags, vec!["foo", "bar"]);
    }

    #[tokio::test]
    async fn scan_handles_nonexistent_path_gracefully() {
        let r = scan_vault_tags_cmd("/path/that/does/not/exist".into())
            .await
            .unwrap();
        assert!(r.is_empty());
    }
}
