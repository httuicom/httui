//! Block run-body cache.
//!
//! Per-run response bodies live on disk at
//! `<vault>/.httui/runs/<file_relpath>/<alias>/<run_id>.<ext>` —
//! gitignored by default (`.httui/` is part of the auto-block).
//! `block_run_history` (SQLite) stores metadata; this
//! module owns the body bytes that were too large to live there.
//!
//! Contract:
//! - Bodies cap at `MAX_RUN_BODY_BYTES` (1 MiB). When a write exceeds
//!   the cap we truncate to `cap - marker.len()` bytes and append a
//!   marker so the consumer can detect and warn.
//! - Writes are atomic (`tempfile -> rename`) so a crash can't leave
//!   a half-written file the next read mistakes for valid.
//! - Trim policy: keep last N runs per (file_path, alias); older
//!   files deleted by lexicographic sort over the run-id stems
//!   (caller passes ULID/timestamp-prefixed ids so newest > oldest).
//!
//! Path sanitization is strict:
//! - `file_path` is treated as a vault-relative POSIX path; leading
//!   slashes stripped, `..` segments rejected.
//! - `alias` and `run_id` must match `[A-Za-z0-9_.-]+`; other input
//!   returns `Err` so we never write outside `.httui/runs/`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

pub const MAX_RUN_BODY_BYTES: usize = 1_048_576; // 1 MiB
const TRUNCATE_MARKER: &[u8] = b"\n[run-body truncated]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunBodyKind {
    /// Text body — written as `.json`. The consumer is responsible
    /// for ensuring `body` is valid JSON; this layer is opaque.
    Json,
    /// Opaque bytes — written as `.bin`. Used for binary HTTP
    /// responses (images, archives, etc.).
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunBodyEntry {
    pub run_id: String,
    pub kind: String, // "json" | "bin"
    pub byte_size: u64,
    pub truncated: bool,
}

/// Write `body` to the run-body cache. Returns the absolute path of
/// the written file. Truncates with marker if `body.len()` exceeds
/// [`MAX_RUN_BODY_BYTES`]. Creates the directory tree as needed.
pub fn write_run_body(
    vault_root: &Path,
    file_path: &str,
    alias: &str,
    run_id: &str,
    kind: RunBodyKind,
    body: &[u8],
) -> Result<PathBuf, String> {
    let block_dir = block_dir_for(vault_root, file_path, alias)?;
    sanitize_id(run_id, "run_id")?;
    fs::create_dir_all(&block_dir).map_err(|e| format!("create_dir_all failed: {e}"))?;

    let filename = format!("{run_id}.{}", ext_for(kind));
    let final_path = block_dir.join(&filename);
    let tmp_path = block_dir.join(format!("{filename}.tmp"));

    let payload = if body.len() > MAX_RUN_BODY_BYTES {
        let keep = MAX_RUN_BODY_BYTES.saturating_sub(TRUNCATE_MARKER.len());
        let mut buf = Vec::with_capacity(MAX_RUN_BODY_BYTES);
        buf.extend_from_slice(&body[..keep]);
        buf.extend_from_slice(TRUNCATE_MARKER);
        buf
    } else {
        body.to_vec()
    };

    fs::write(&tmp_path, &payload).map_err(|e| format!("tmp write failed: {e}"))?;
    fs::rename(&tmp_path, &final_path).map_err(|e| format!("rename failed: {e}"))?;
    Ok(final_path)
}

/// Read the bytes for `run_id` if present. Returns `Ok(None)` when
/// the file doesn't exist (e.g. trimmed) — that's the normal path,
/// not an error.
pub fn read_run_body(
    vault_root: &Path,
    file_path: &str,
    alias: &str,
    run_id: &str,
) -> Result<Option<Vec<u8>>, String> {
    let block_dir = block_dir_for(vault_root, file_path, alias)?;
    sanitize_id(run_id, "run_id")?;
    for ext in ["json", "bin"] {
        let candidate = block_dir.join(format!("{run_id}.{ext}"));
        match fs::read(&candidate) {
            Ok(b) => return Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("read failed: {e}")),
        }
    }
    Ok(None)
}

/// List run entries for `(file_path, alias)`, newest first by
/// lexicographic `run_id` order.
pub fn list_run_bodies(
    vault_root: &Path,
    file_path: &str,
    alias: &str,
) -> Result<Vec<RunBodyEntry>, String> {
    let block_dir = block_dir_for(vault_root, file_path, alias)?;
    let entries = match fs::read_dir(&block_dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read_dir failed: {e}")),
    };
    let mut out: Vec<RunBodyEntry> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        // Ignore in-flight tmp files.
        if stem.ends_with(".tmp") || path.extension().and_then(|s| s.to_str()) == Some("tmp") {
            continue;
        }
        let ext = match path.extension().and_then(|s| s.to_str()) {
            Some("json") => "json",
            Some("bin") => "bin",
            _ => continue,
        };
        let meta = match path.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let byte_size = meta.len();
        let truncated = ext_truncation_check(&path, byte_size);
        out.push(RunBodyEntry {
            run_id: stem.to_string(),
            kind: ext.to_string(),
            byte_size,
            truncated,
        });
    }
    out.sort_by(|a, b| b.run_id.cmp(&a.run_id));
    Ok(out)
}

/// Delete run-body files beyond the newest `keep_n` for
/// `(file_path, alias)`. No-op when fewer entries exist or the dir
/// is missing.
pub fn trim_run_bodies(
    vault_root: &Path,
    file_path: &str,
    alias: &str,
    keep_n: usize,
) -> Result<usize, String> {
    let entries = list_run_bodies(vault_root, file_path, alias)?;
    if entries.len() <= keep_n {
        return Ok(0);
    }
    let block_dir = block_dir_for(vault_root, file_path, alias)?;
    let mut deleted = 0usize;
    for entry in &entries[keep_n..] {
        let path = block_dir.join(format!("{}.{}", entry.run_id, entry.kind));
        if fs::remove_file(&path).is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// Move every cached run body for `(file_path, old_alias)` to
/// `(file_path, new_alias)`. Powers the "alias rename" carry from
/// when the user renames a block's `alias=` info-
/// string token, the on-disk run history follows so older diffs
/// stay reachable from the new alias.
///
/// Best-effort:
/// - Returns `Ok(false)` when the source dir doesn't exist (the
///   block has never run with that alias — nothing to move).
/// - Returns `Err` when the destination dir already exists. The
///   consumer should refuse the rename (or pick a different new
///   alias) rather than have us merge two histories.
/// - Returns `Err` when either alias fails the same sanitization
///   used by `write_run_body`.
pub fn rename_alias_runs(
    vault_root: &Path,
    file_path: &str,
    old_alias: &str,
    new_alias: &str,
) -> Result<bool, String> {
    let old_dir = block_dir_for(vault_root, file_path, old_alias)?;
    let new_dir = block_dir_for(vault_root, file_path, new_alias)?;
    if !old_dir.exists() {
        return Ok(false);
    }
    if new_dir.exists() {
        return Err(format!(
            "destination alias `{new_alias}` already has cached runs",
        ));
    }
    if let Some(parent) = new_dir.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create_dir_all failed: {e}"))?;
    }
    fs::rename(&old_dir, &new_dir).map_err(|e| format!("rename failed: {e}"))?;
    Ok(true)
}

fn ext_for(kind: RunBodyKind) -> &'static str {
    match kind {
        RunBodyKind::Json => "json",
        RunBodyKind::Binary => "bin",
    }
}

fn ext_truncation_check(path: &Path, byte_size: u64) -> bool {
    // Cheap heuristic — only inspect the trailing chunk of files
    // that COULD plausibly have been truncated (size at the cap).
    if byte_size as usize != MAX_RUN_BODY_BYTES {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    bytes.ends_with(TRUNCATE_MARKER)
}

fn block_dir_for(vault_root: &Path, file_path: &str, alias: &str) -> Result<PathBuf, String> {
    sanitize_id(alias, "alias")?;
    let rel = sanitize_file_path(file_path)?;
    let mut p = vault_root.to_path_buf();
    p.push(".httui");
    p.push("runs");
    p.push(rel);
    p.push(alias);
    Ok(p)
}

fn sanitize_id(value: &str, label: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{label} is empty"));
    }
    for ch in value.chars() {
        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.') {
            return Err(format!(
                "{label} contains invalid char `{ch}` (allowed: alnum, _ - .)",
            ));
        }
    }
    if value == "." || value == ".." {
        return Err(format!("{label} cannot be `.` or `..`"));
    }
    Ok(())
}

fn sanitize_file_path(file_path: &str) -> Result<PathBuf, String> {
    if file_path.is_empty() {
        return Err("file_path is empty".into());
    }
    let trimmed = file_path.trim_start_matches('/').trim_start_matches('\\');
    if trimmed.is_empty() {
        return Err("file_path is empty after stripping leading slashes".into());
    }
    let mut out = PathBuf::new();
    for seg in trimmed.split(['/', '\\']) {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg == ".." {
            return Err("file_path may not contain `..` segments".into());
        }
        out.push(seg);
    }
    if out.as_os_str().is_empty() {
        return Err("file_path is empty after normalization".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_then_read_round_trips_json_body() {
        let dir = tempdir().unwrap();
        let path = write_run_body(
            dir.path(),
            "runbook.md",
            "fetchUser",
            "01H8XK00000000000000000000",
            RunBodyKind::Json,
            br#"{"id":42}"#,
        )
        .unwrap();
        assert!(path.ends_with(".httui/runs/runbook.md/fetchUser/01H8XK00000000000000000000.json"));
        let read = read_run_body(
            dir.path(),
            "runbook.md",
            "fetchUser",
            "01H8XK00000000000000000000",
        )
        .unwrap()
        .unwrap();
        assert_eq!(read, br#"{"id":42}"#);
    }

    #[test]
    fn write_creates_nested_dirs_for_path_with_subfolders() {
        let dir = tempdir().unwrap();
        let path = write_run_body(
            dir.path(),
            "ops/incident.md",
            "a",
            "r1",
            RunBodyKind::Json,
            b"{}",
        )
        .unwrap();
        assert!(path.starts_with(dir.path().join(".httui/runs/ops/incident.md/a")));
    }

    #[test]
    fn read_returns_none_when_file_missing() {
        let dir = tempdir().unwrap();
        let r = read_run_body(dir.path(), "x.md", "a", "missing").unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn body_over_cap_is_truncated_with_marker() {
        let dir = tempdir().unwrap();
        let big = vec![b'A'; MAX_RUN_BODY_BYTES + 1024];
        let path =
            write_run_body(dir.path(), "x.md", "a", "r1", RunBodyKind::Binary, &big).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), MAX_RUN_BODY_BYTES);
        assert!(bytes.ends_with(TRUNCATE_MARKER));
    }

    #[test]
    fn body_at_cap_is_not_truncated() {
        let dir = tempdir().unwrap();
        let exact = vec![b'A'; MAX_RUN_BODY_BYTES];
        let path =
            write_run_body(dir.path(), "x.md", "a", "r1", RunBodyKind::Binary, &exact).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), MAX_RUN_BODY_BYTES);
        assert!(!bytes.ends_with(TRUNCATE_MARKER));
    }

    #[test]
    fn list_run_bodies_returns_newest_first() {
        let dir = tempdir().unwrap();
        for id in ["01a", "01b", "01c"] {
            write_run_body(dir.path(), "x.md", "a", id, RunBodyKind::Json, b"{}").unwrap();
        }
        let entries = list_run_bodies(dir.path(), "x.md", "a").unwrap();
        let ids: Vec<&str> = entries.iter().map(|e| e.run_id.as_str()).collect();
        assert_eq!(ids, vec!["01c", "01b", "01a"]);
    }

    #[test]
    fn list_returns_empty_when_block_dir_missing() {
        let dir = tempdir().unwrap();
        let entries = list_run_bodies(dir.path(), "x.md", "missing").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn list_skips_non_recognized_extensions() {
        let dir = tempdir().unwrap();
        write_run_body(dir.path(), "x.md", "a", "01a", RunBodyKind::Json, b"{}").unwrap();
        // Sneak in a stray file that should be ignored.
        let block_dir = dir.path().join(".httui/runs/x.md/a");
        std::fs::write(block_dir.join("something.txt"), "noise").unwrap();
        let entries = list_run_bodies(dir.path(), "x.md", "a").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, "json");
    }

    #[test]
    fn trim_keeps_newest_n_and_returns_count_deleted() {
        let dir = tempdir().unwrap();
        for id in ["01a", "01b", "01c", "01d", "01e"] {
            write_run_body(dir.path(), "x.md", "a", id, RunBodyKind::Json, b"{}").unwrap();
        }
        let deleted = trim_run_bodies(dir.path(), "x.md", "a", 2).unwrap();
        assert_eq!(deleted, 3);
        let remaining: Vec<String> = list_run_bodies(dir.path(), "x.md", "a")
            .unwrap()
            .into_iter()
            .map(|e| e.run_id)
            .collect();
        assert_eq!(remaining, vec!["01e", "01d"]);
    }

    #[test]
    fn trim_no_op_when_under_cap() {
        let dir = tempdir().unwrap();
        for id in ["01a", "01b"] {
            write_run_body(dir.path(), "x.md", "a", id, RunBodyKind::Json, b"{}").unwrap();
        }
        let deleted = trim_run_bodies(dir.path(), "x.md", "a", 10).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_run_bodies(dir.path(), "x.md", "a").unwrap().len(), 2);
    }

    #[test]
    fn alias_with_invalid_chars_errors() {
        let dir = tempdir().unwrap();
        let r = write_run_body(
            dir.path(),
            "x.md",
            "bad alias", // space not allowed
            "r1",
            RunBodyKind::Json,
            b"{}",
        );
        assert!(r.is_err());
        let err = r.unwrap_err();
        assert!(err.contains("alias"));
    }

    #[test]
    fn run_id_with_invalid_chars_errors() {
        let dir = tempdir().unwrap();
        let r = write_run_body(
            dir.path(),
            "x.md",
            "a",
            "../escape",
            RunBodyKind::Json,
            b"{}",
        );
        assert!(r.is_err());
    }

    #[test]
    fn file_path_with_dotdot_segment_errors() {
        let dir = tempdir().unwrap();
        let r = write_run_body(
            dir.path(),
            "../sneaky.md",
            "a",
            "r1",
            RunBodyKind::Json,
            b"{}",
        );
        assert!(r.is_err());
    }

    #[test]
    fn empty_inputs_error() {
        let dir = tempdir().unwrap();
        assert!(write_run_body(dir.path(), "", "a", "r1", RunBodyKind::Json, b"{}").is_err());
        assert!(write_run_body(dir.path(), "x.md", "", "r1", RunBodyKind::Json, b"{}").is_err());
        assert!(write_run_body(dir.path(), "x.md", "a", "", RunBodyKind::Json, b"{}").is_err());
    }

    #[test]
    fn leading_slashes_stripped_from_file_path() {
        let dir = tempdir().unwrap();
        let path =
            write_run_body(dir.path(), "/x.md", "a", "r1", RunBodyKind::Json, b"{}").unwrap();
        assert!(path.starts_with(dir.path().join(".httui/runs/x.md/a")));
    }

    #[test]
    fn truncated_flag_surfaces_via_list() {
        let dir = tempdir().unwrap();
        let big = vec![b'A'; MAX_RUN_BODY_BYTES + 1024];
        write_run_body(dir.path(), "x.md", "a", "r1", RunBodyKind::Binary, &big).unwrap();
        let entries = list_run_bodies(dir.path(), "x.md", "a").unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].truncated);
    }

    #[test]
    fn write_atomic_no_partial_state_on_replace() {
        let dir = tempdir().unwrap();
        let path1 = write_run_body(
            dir.path(),
            "x.md",
            "a",
            "r1",
            RunBodyKind::Json,
            b"{\"v\":1}",
        )
        .unwrap();
        // Second write to same run_id should overwrite cleanly.
        let path2 = write_run_body(
            dir.path(),
            "x.md",
            "a",
            "r1",
            RunBodyKind::Json,
            b"{\"v\":2}",
        )
        .unwrap();
        assert_eq!(path1, path2);
        let bytes = std::fs::read(&path1).unwrap();
        assert_eq!(bytes, b"{\"v\":2}");
        // No leftover .tmp file.
        let entries = list_run_bodies(dir.path(), "x.md", "a").unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn rename_alias_moves_existing_runs() {
        let dir = tempdir().unwrap();
        for run_id in ["r1", "r2", "r3"] {
            write_run_body(dir.path(), "rb.md", "old", run_id, RunBodyKind::Json, b"{}").unwrap();
        }
        let moved = rename_alias_runs(dir.path(), "rb.md", "old", "new").unwrap();
        assert!(moved);
        let from_old = list_run_bodies(dir.path(), "rb.md", "old").unwrap();
        let from_new = list_run_bodies(dir.path(), "rb.md", "new").unwrap();
        assert!(from_old.is_empty(), "old alias dir should be gone");
        assert_eq!(from_new.len(), 3);
        // Files keep the same names; only the parent dir changed.
        let stems: Vec<_> = from_new.iter().map(|e| e.run_id.as_str()).collect();
        assert!(stems.contains(&"r1"));
        assert!(stems.contains(&"r2"));
        assert!(stems.contains(&"r3"));
    }

    #[test]
    fn rename_alias_returns_false_when_source_missing() {
        let dir = tempdir().unwrap();
        // Nothing has run yet — no source dir.
        let moved = rename_alias_runs(dir.path(), "rb.md", "missing", "fresh").unwrap();
        assert!(!moved);
    }

    #[test]
    fn rename_alias_errors_when_destination_already_has_runs() {
        let dir = tempdir().unwrap();
        write_run_body(dir.path(), "rb.md", "src", "r1", RunBodyKind::Json, b"{}").unwrap();
        write_run_body(dir.path(), "rb.md", "dst", "r1", RunBodyKind::Json, b"{}").unwrap();
        let err = rename_alias_runs(dir.path(), "rb.md", "src", "dst").unwrap_err();
        assert!(err.contains("already"));
        // Source still in place — caller can resolve and retry.
        let from_src = list_run_bodies(dir.path(), "rb.md", "src").unwrap();
        assert_eq!(from_src.len(), 1);
    }

    #[test]
    fn rename_alias_rejects_invalid_alias_names() {
        let dir = tempdir().unwrap();
        let err = rename_alias_runs(dir.path(), "rb.md", "ok", "bad/slash").unwrap_err();
        assert!(err.contains("invalid char"));
    }

    #[test]
    fn rename_alias_rejects_traversal_in_file_path() {
        let dir = tempdir().unwrap();
        let err = rename_alias_runs(dir.path(), "../escape.md", "a", "b").unwrap_err();
        assert!(err.contains(".."));
    }
}
