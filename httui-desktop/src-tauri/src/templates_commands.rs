//! Tauri commands wrapping `httui_core::templates`.

use httui_core::templates::{list_builtin_templates, list_vault_templates, Template};
use std::path::PathBuf;

/// Built-ins first, then vault-local templates sorted by id.
#[tauri::command]
pub async fn list_templates_cmd(vault_path: String) -> Result<Vec<Template>, String> {
    let mut out = list_builtin_templates();
    out.extend(list_vault_templates(&PathBuf::from(vault_path)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as stdfs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn returns_empty_for_vault_with_no_templates() {
        let dir = tempdir().unwrap();
        let list = list_templates_cmd(dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn surfaces_vault_template_with_metadata() {
        let dir = tempdir().unwrap();
        let templates = dir.path().join(".httui").join("templates");
        stdfs::create_dir_all(&templates).unwrap();
        stdfs::write(
            templates.join("pg-health.md"),
            "---\ntitle: \"Postgres health\"\ndescription: heartbeat\n---\n```http\nGET /\n```\n",
        )
        .unwrap();

        let list = list_templates_cmd(dir.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "pg-health");
        assert_eq!(list[0].name, "Postgres health");
    }
}
