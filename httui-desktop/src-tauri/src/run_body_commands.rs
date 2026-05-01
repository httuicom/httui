//! Tauri commands wrapping `httui_core::run_bodies`. Powers Epic 47
//! Story 01 — the consumer (HTTP/DB streamed-execution path) calls
//! `write_run_body_cmd` after a successful run + `trim_run_bodies_cmd`
//! to keep the cache bounded; the run-diff viewer (Epic 47 Stories
//! 02-04) reads via `read_run_body_cmd` / `list_run_bodies_cmd`.

use httui_core::run_bodies::{
    list_run_bodies, read_run_body, rename_alias_runs, trim_run_bodies, write_run_body,
    RunBodyEntry, RunBodyKind,
};
use std::path::PathBuf;

fn parse_kind(kind: &str) -> Result<RunBodyKind, String> {
    match kind {
        "json" => Ok(RunBodyKind::Json),
        "bin" | "binary" => Ok(RunBodyKind::Binary),
        other => Err(format!("unknown body kind `{other}` (want json|bin)")),
    }
}

#[tauri::command]
pub async fn write_run_body_cmd(
    vault_path: String,
    file_path: String,
    alias: String,
    run_id: String,
    kind: String,
    body: Vec<u8>,
) -> Result<String, String> {
    let parsed = parse_kind(&kind)?;
    let path = write_run_body(
        &PathBuf::from(vault_path),
        &file_path,
        &alias,
        &run_id,
        parsed,
        &body,
    )?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn read_run_body_cmd(
    vault_path: String,
    file_path: String,
    alias: String,
    run_id: String,
) -> Result<Option<Vec<u8>>, String> {
    read_run_body(
        &PathBuf::from(vault_path),
        &file_path,
        &alias,
        &run_id,
    )
}

#[tauri::command]
pub async fn list_run_bodies_cmd(
    vault_path: String,
    file_path: String,
    alias: String,
) -> Result<Vec<RunBodyEntry>, String> {
    list_run_bodies(&PathBuf::from(vault_path), &file_path, &alias)
}

#[tauri::command]
pub async fn trim_run_bodies_cmd(
    vault_path: String,
    file_path: String,
    alias: String,
    keep_n: usize,
) -> Result<usize, String> {
    trim_run_bodies(
        &PathBuf::from(vault_path),
        &file_path,
        &alias,
        keep_n,
    )
}

/// Move every cached run body for `(file_path, old_alias)` to
/// `(file_path, new_alias)`. Powers Epic 47 Story 05's alias-rename
/// flow. Best-effort:
/// - `Ok(false)` when the source dir doesn't exist
/// - `Err` when the destination dir already has cached runs
#[tauri::command]
pub async fn rename_alias_runs_cmd(
    vault_path: String,
    file_path: String,
    old_alias: String,
    new_alias: String,
) -> Result<bool, String> {
    rename_alias_runs(
        &PathBuf::from(vault_path),
        &file_path,
        &old_alias,
        &new_alias,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn write_then_read_round_trips() {
        let dir = tempdir().unwrap();
        write_run_body_cmd(
            dir.path().to_string_lossy().into_owned(),
            "x.md".into(),
            "a".into(),
            "r1".into(),
            "json".into(),
            br#"{"id":1}"#.to_vec(),
        )
        .await
        .unwrap();
        let read = read_run_body_cmd(
            dir.path().to_string_lossy().into_owned(),
            "x.md".into(),
            "a".into(),
            "r1".into(),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(read, br#"{"id":1}"#);
    }

    #[tokio::test]
    async fn list_returns_what_was_written() {
        let dir = tempdir().unwrap();
        for id in ["01a", "01b"] {
            write_run_body_cmd(
                dir.path().to_string_lossy().into_owned(),
                "x.md".into(),
                "a".into(),
                id.into(),
                "json".into(),
                b"{}".to_vec(),
            )
            .await
            .unwrap();
        }
        let entries = list_run_bodies_cmd(
            dir.path().to_string_lossy().into_owned(),
            "x.md".into(),
            "a".into(),
        )
        .await
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].run_id, "01b");
    }

    #[tokio::test]
    async fn trim_drops_older_entries() {
        let dir = tempdir().unwrap();
        for id in ["01a", "01b", "01c"] {
            write_run_body_cmd(
                dir.path().to_string_lossy().into_owned(),
                "x.md".into(),
                "a".into(),
                id.into(),
                "json".into(),
                b"{}".to_vec(),
            )
            .await
            .unwrap();
        }
        let deleted = trim_run_bodies_cmd(
            dir.path().to_string_lossy().into_owned(),
            "x.md".into(),
            "a".into(),
            1,
        )
        .await
        .unwrap();
        assert_eq!(deleted, 2);
    }

    #[tokio::test]
    async fn rename_alias_round_trip() {
        let dir = tempdir().unwrap();
        // Write under "src" alias.
        write_run_body_cmd(
            dir.path().to_string_lossy().into_owned(),
            "rb.md".into(),
            "src".into(),
            "r1".into(),
            "json".into(),
            b"{}".to_vec(),
        )
        .await
        .unwrap();
        // Rename to "dst" — succeeds.
        let moved = rename_alias_runs_cmd(
            dir.path().to_string_lossy().into_owned(),
            "rb.md".into(),
            "src".into(),
            "dst".into(),
        )
        .await
        .unwrap();
        assert!(moved);
        // Old alias dir gone, new one has the run.
        let from_old = list_run_bodies_cmd(
            dir.path().to_string_lossy().into_owned(),
            "rb.md".into(),
            "src".into(),
        )
        .await
        .unwrap();
        let from_new = list_run_bodies_cmd(
            dir.path().to_string_lossy().into_owned(),
            "rb.md".into(),
            "dst".into(),
        )
        .await
        .unwrap();
        assert!(from_old.is_empty());
        assert_eq!(from_new.len(), 1);
        // Renaming a never-run alias returns Ok(false).
        let moved2 = rename_alias_runs_cmd(
            dir.path().to_string_lossy().into_owned(),
            "rb.md".into(),
            "ghost".into(),
            "fresh".into(),
        )
        .await
        .unwrap();
        assert!(!moved2);
    }

    #[tokio::test]
    async fn unknown_kind_returns_error() {
        let dir = tempdir().unwrap();
        let r = write_run_body_cmd(
            dir.path().to_string_lossy().into_owned(),
            "x.md".into(),
            "a".into(),
            "r1".into(),
            "yaml".into(),
            b"{}".to_vec(),
        )
        .await;
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("yaml"));
    }
}
