// coverage:exclude file — Tauri command shells with no testable
// logic without spinning up the Tauri runtime. Substantive code is
// in `crate::fs::*` (covered there). Same shape as
// `commands/connections.rs` and `commands/environments.rs`
// (audit-016). Justified in `audit-018-files-commands-coverage-exclude.md`.
// Retires when the integration harness lands, mirroring the
// retirement schedule for the other two opt-outs.

//! Vault file Tauri commands — list / read / write / create / rename /
//! delete notes and folders.
//!
//! Extracted from `main.rs` The substantive
//! logic lives in `crate::fs`; these wrappers just thread Tauri
//! state (the file-watcher ignore list, the SQLite pool for cascading
//! per-block cache cleanup) into the call.

use std::sync::{Arc, Mutex};

use sqlx::sqlite::SqlitePool;
use tauri::State;

#[tauri::command]
pub fn list_workspace(vault_path: String) -> Result<Vec<crate::fs::FileEntry>, String> {
    crate::fs::list_workspace(&vault_path)
}

/// Read a markdown note from disk. `file_path` is resolved relative to
/// `vault_path`; resolved paths must stay inside the vault.
#[tauri::command]
pub fn read_note(vault_path: String, file_path: String) -> Result<String, String> {
    crate::fs::read_note(&vault_path, &file_path)
}

/// Save markdown content to a vault-relative path. Adds the path to a
/// short-lived ignore list so the file watcher does not echo our own
/// write back to the frontend as an external change.
#[tauri::command]
pub fn write_note(
    vault_path: String,
    file_path: String,
    content: String,
    ignore_paths: State<'_, Arc<Mutex<Vec<String>>>>,
) -> Result<(), String> {
    // Add to ignore list so file watcher skips this event
    {
        let mut ignored = ignore_paths.lock().unwrap();
        if !ignored.contains(&file_path) {
            ignored.push(file_path.clone());
        }
    }

    let result = crate::fs::write_note(&vault_path, &file_path, &content);

    // Remove from ignore list after a short delay
    let ignore = ignore_paths.inner().clone();
    let fp = file_path.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let mut ignored = ignore.lock().unwrap();
        ignored.retain(|p| p != &fp);
    });

    result
}

/// Create an empty markdown note. Errors if the file already exists.
#[tauri::command]
pub fn create_note(vault_path: String, file_path: String) -> Result<(), String> {
    crate::fs::create_note(&vault_path, &file_path)
}

/// Move a note to the OS trash (recoverable) and clear any related
/// per-block cache rows.
#[tauri::command]
pub async fn delete_note(
    vault_path: String,
    file_path: String,
    pool: State<'_, SqlitePool>,
) -> Result<(), String> {
    // Move the file to trash first; only purge SQLite state if the FS
    // operation succeeded so we don't drop history/examples for a file
    // that's still on disk.
    crate::fs::delete_note(&vault_path, &file_path)?;

    // Cascade purge across every per-block table (Onda 1-3). Each call is
    // best-effort — a failure here doesn't undo the trash operation.
    let absolute = format!("{vault_path}/{file_path}");
    for path_variant in [&file_path, &absolute] {
        let _ = crate::block_history::purge_history_for_file(&pool, path_variant).await;
        let _ = crate::block_settings::purge_settings_for_file(&pool, path_variant).await;
        let _ = crate::block_examples::purge_examples_for_file(&pool, path_variant).await;
        let _ = crate::block_results::delete_block_results_for_file(&pool, path_variant).await;
    }
    Ok(())
}

/// Rename / move a note within the vault. Errors if `new_path` already
/// exists or escapes the vault.
#[tauri::command]
pub fn rename_note(vault_path: String, old_path: String, new_path: String) -> Result<(), String> {
    crate::fs::rename_note(&vault_path, &old_path, &new_path)
}

/// Create a folder under `vault_path`. Idempotent — succeeds if the
/// folder already exists.
#[tauri::command]
pub fn create_folder(vault_path: String, folder_path: String) -> Result<(), String> {
    crate::fs::create_folder(&vault_path, &folder_path)
}

/// Last modification timestamp for a vault note, in **epoch
/// milliseconds**. Returns `None` if the file is absent or its mtime
/// can't be read. Wraps the existing `httui_core::vault_config::merge
/// ::mtime_or_none` helper so the same source of truth backs both
/// vault-config cache invalidation and the editor toolbar timestamp.
///
/// Carry-over from feeds the `useFileMtime` hook
/// that drives "edited Xm ago" in the toolbar. Polled on focus / save
/// rather than continuously.
#[tauri::command]
pub fn get_file_mtime(vault_path: String, file_path: String) -> Option<i64> {
    let absolute = std::path::Path::new(&vault_path).join(&file_path);
    httui_core::vault_config::merge::mtime_or_none(&absolute).and_then(|t| {
        t.duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_millis() as i64)
    })
}
