//! Tauri commands wrapping `httui_core::captures_cache`.

use httui_core::captures_cache::{delete_captures_file, read_captures_file, write_captures_file};
use std::path::PathBuf;

#[tauri::command]
pub async fn read_captures_cache_cmd(
    vault_path: String,
    file_path: String,
) -> Result<Option<String>, String> {
    read_captures_file(&PathBuf::from(vault_path), &file_path)
}

#[tauri::command]
pub async fn write_captures_cache_cmd(
    vault_path: String,
    file_path: String,
    json: String,
) -> Result<String, String> {
    let path = write_captures_file(&PathBuf::from(vault_path), &file_path, &json)?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn delete_captures_cache_cmd(
    vault_path: String,
    file_path: String,
) -> Result<bool, String> {
    delete_captures_file(&PathBuf::from(vault_path), &file_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn write_then_read_round_trips() {
        let dir = tempdir().unwrap();
        write_captures_cache_cmd(
            dir.path().to_string_lossy().into_owned(),
            "x.md".into(),
            r#"{"a":{"k":"v"}}"#.into(),
        )
        .await
        .unwrap();
        let r = read_captures_cache_cmd(dir.path().to_string_lossy().into_owned(), "x.md".into())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r, r#"{"a":{"k":"v"}}"#);
    }

    #[tokio::test]
    async fn delete_round_trips_with_read() {
        let dir = tempdir().unwrap();
        write_captures_cache_cmd(
            dir.path().to_string_lossy().into_owned(),
            "x.md".into(),
            "{}".into(),
        )
        .await
        .unwrap();
        let removed =
            delete_captures_cache_cmd(dir.path().to_string_lossy().into_owned(), "x.md".into())
                .await
                .unwrap();
        assert!(removed);
        let r = read_captures_cache_cmd(dir.path().to_string_lossy().into_owned(), "x.md".into())
            .await
            .unwrap();
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn read_missing_returns_none() {
        let dir = tempdir().unwrap();
        let r = read_captures_cache_cmd(
            dir.path().to_string_lossy().into_owned(),
            "absent.md".into(),
        )
        .await
        .unwrap();
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn invalid_path_errors() {
        let dir = tempdir().unwrap();
        let r = write_captures_cache_cmd(
            dir.path().to_string_lossy().into_owned(),
            "../escape.md".into(),
            "{}".into(),
        )
        .await;
        assert!(r.is_err());
    }
}
