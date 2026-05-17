use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileEntry>>,
}

fn resolve_path(vault_path: &str, relative_path: &str) -> PathBuf {
    Path::new(vault_path).join(relative_path)
}

pub fn list_workspace(vault_path: &str) -> Result<Vec<FileEntry>, String> {
    let root = Path::new(vault_path);
    if !root.is_dir() {
        return Err(format!("Vault path is not a directory: {}", vault_path));
    }
    list_dir_recursive(root, root)
}

const IGNORED_DIRS: &[&str] = &[
    // Heavy build / dependency dirs.
    "node_modules",
    "target",
    "dist",
    "build",
    ".git",
    "__pycache__",
    ".next",
    ".nuxt",
    ".svelte-kit",
    "vendor",
    ".venv",
    "venv",
    // Vault internals — managed via dedicated UIs (EnvironmentManager
    // drawer, ConnectionsList panel) rather than the file tree.
    // The file is always under top-level `envs/` per ADR 0001.
    "envs",
];

fn list_dir_recursive(dir: &Path, root: &Path) -> Result<Vec<FileEntry>, String> {
    let mut entries: Vec<FileEntry> = Vec::new();

    let read_dir = std::fs::read_dir(dir).map_err(|e| e.to_string())?;

    for entry in read_dir {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files/dirs and known heavy directories
        if name.starts_with('.') || IGNORED_DIRS.contains(&name.as_str()) {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        if path.is_dir() {
            let children = list_dir_recursive(&path, root)?;
            entries.push(FileEntry {
                name,
                path: relative,
                is_dir: true,
                children: Some(children),
            });
        } else if name.ends_with(".md") {
            entries.push(FileEntry {
                name,
                path: relative,
                is_dir: false,
                children: None,
            });
        }
    }

    // Sort: dirs first, then files, alphabetically
    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}

pub fn read_note(vault_path: &str, file_path: &str) -> Result<String, String> {
    let path = resolve_path(vault_path, file_path);
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;
    // Normalize legacy JSON-bodied http blocks on read so every read
    // path (editor open, watcher reload, session restore) sees the
    // canonical HTTP-message form. Idempotent + early-outs when there
    // is no ```http fence, so it is safe on this hot path.
    Ok(crate::blocks::http_normalize::normalize_http_blocks(&raw))
}

pub fn write_note(vault_path: &str, file_path: &str, content: &str) -> Result<(), String> {
    let path = resolve_path(vault_path, file_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, content).map_err(|e| format!("Failed to write {}: {}", file_path, e))
}

pub fn create_note(vault_path: &str, file_path: &str) -> Result<(), String> {
    let path = resolve_path(vault_path, file_path);
    if path.exists() {
        return Err(format!("File already exists: {}", file_path));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, "").map_err(|e| format!("Failed to create {}: {}", file_path, e))
}

pub fn delete_note(vault_path: &str, file_path: &str) -> Result<(), String> {
    let path = resolve_path(vault_path, file_path);
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }
    trash::delete(&path).map_err(|e| format!("Failed to delete {}: {}", file_path, e))
}

pub fn rename_note(vault_path: &str, old_path: &str, new_path: &str) -> Result<(), String> {
    let from = resolve_path(vault_path, old_path);
    let to = resolve_path(vault_path, new_path);
    if !from.exists() {
        return Err(format!("File not found: {}", old_path));
    }
    if to.exists() {
        return Err(format!("Destination already exists: {}", new_path));
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&from, &to)
        .map_err(|e| format!("Failed to rename {} to {}: {}", old_path, new_path, e))
}

pub fn create_folder(vault_path: &str, folder_path: &str) -> Result<(), String> {
    let path = resolve_path(vault_path, folder_path);
    if path.exists() {
        return Err(format!("Folder already exists: {}", folder_path));
    }
    std::fs::create_dir_all(&path)
        .map_err(|e| format!("Failed to create folder {}: {}", folder_path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_vault() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create test structure
        std::fs::write(root.join("README.md"), "# Hello").unwrap();
        std::fs::write(root.join("notes.md"), "Some notes").unwrap();
        std::fs::create_dir_all(root.join("subfolder")).unwrap();
        std::fs::write(root.join("subfolder/nested.md"), "Nested").unwrap();
        std::fs::write(root.join("not-markdown.txt"), "ignored").unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();

        tmp
    }

    #[test]
    fn test_list_workspace() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        let entries = list_workspace(vault).unwrap();

        // Should have: subfolder (dir), README.md, notes.md
        // .hidden should be excluded, not-markdown.txt should be excluded
        assert_eq!(entries.len(), 3);
        assert!(entries[0].is_dir); // subfolder first
        assert_eq!(entries[0].name, "subfolder");
        assert!(entries[0].children.as_ref().unwrap().len() == 1);
        assert!(!entries[1].is_dir);
    }

    #[test]
    fn list_workspace_hides_envs_internal_dir() {
        // Scaffolded vaults always include `envs/` for env TOML files
        // (ADR 0001). Those are managed via the EnvironmentManager
        // drawer / TopBar, not the file tree, so the sidebar must
        // hide them — otherwise the user sees a confusing folder
        // they can't usefully edit as markdown.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("envs")).unwrap();
        std::fs::write(root.join("envs/local.toml"), "[vars]\n").unwrap();
        std::fs::create_dir_all(root.join("runbooks")).unwrap();
        std::fs::write(root.join("runbooks/auth.md"), "# Auth").unwrap();

        let entries = list_workspace(root.to_str().unwrap()).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(
            !names.contains(&"envs"),
            "envs should be hidden, got: {names:?}"
        );
        assert!(
            names.contains(&"runbooks"),
            "runbooks should appear, got: {names:?}"
        );
    }

    #[test]
    fn test_read_note() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        let content = read_note(vault, "README.md").unwrap();
        assert_eq!(content, "# Hello");
    }

    #[test]
    fn read_note_normalizes_legacy_http_block() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        let legacy = concat!(
            "```http alias=req1\n",
            "{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n",
            "```\n",
        );
        write_note(vault, "legacy.md", legacy).unwrap();
        let content = read_note(vault, "legacy.md").unwrap();
        assert_eq!(content, "```http alias=req1\nGET https://x.com\n```\n");
    }

    #[test]
    fn read_note_leaves_plain_markdown_untouched() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        let content = read_note(vault, "notes.md").unwrap();
        assert_eq!(content, "Some notes");
    }

    #[test]
    fn test_write_note() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        write_note(vault, "README.md", "# Updated").unwrap();
        let content = read_note(vault, "README.md").unwrap();
        assert_eq!(content, "# Updated");
    }

    #[test]
    fn test_create_note() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        create_note(vault, "new-note.md").unwrap();
        let content = read_note(vault, "new-note.md").unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_create_note_in_subfolder() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        create_note(vault, "new-folder/deep.md").unwrap();
        let content = read_note(vault, "new-folder/deep.md").unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_create_note_already_exists() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        let result = create_note(vault, "README.md");
        assert!(result.is_err());
    }

    #[test]
    fn test_rename_note() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        rename_note(vault, "README.md", "RENAMED.md").unwrap();
        assert!(read_note(vault, "README.md").is_err());
        assert_eq!(read_note(vault, "RENAMED.md").unwrap(), "# Hello");
    }

    #[test]
    fn test_create_folder() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        create_folder(vault, "new-folder").unwrap();
        assert!(Path::new(vault).join("new-folder").is_dir());
    }

    #[test]
    fn test_create_folder_already_exists() {
        let tmp = setup_vault();
        let vault = tmp.path().to_str().unwrap();
        let result = create_folder(vault, "subfolder");
        assert!(result.is_err());
    }
}
