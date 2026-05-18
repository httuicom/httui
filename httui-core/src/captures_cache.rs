//! Persistent captures cache.
//!
//! When auto-capture is ON for a file, the last-run capture map
//! survives restarts via a JSON file at
//! `<vault>/.httui/captures/<file_relpath>.json` (gitignored via
//! the `.httui/` auto-block).
//!
//! Secrets are NEVER persisted: the consumer must filter
//! `isSecret`-flagged entries out of the JSON BEFORE calling
//! `write_captures_file`. This module is opaque about the JSON
//! shape — it owns the disk layout, atomic write, and path
//! sanitization. The frontend's `useCaptureStore` defines the
//! shape (`Record<alias, Record<key, CaptureValue>>`).
//!
//! Path sanitization mirrors `run_bodies::sanitize_file_path`:
//! leading slashes stripped, `..` segments rejected. We never
//! write outside `.httui/captures/`.

use std::fs;
use std::path::{Path, PathBuf};

/// Read the captures JSON for `file_path`. Returns `Ok(None)` when
/// the file doesn't exist (the auto-capture was either OFF or
/// nothing was persisted yet) — that's normal, not an error.
pub fn read_captures_file(vault_root: &Path, file_path: &str) -> Result<Option<String>, String> {
    let path = captures_path_for(vault_root, file_path)?;
    match fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("read failed: {e}")),
    }
}

/// Atomically write `json` to the captures cache for `file_path`.
/// Creates parent dirs as needed. Returns the absolute path.
pub fn write_captures_file(
    vault_root: &Path,
    file_path: &str,
    json: &str,
) -> Result<PathBuf, String> {
    let path = captures_path_for(vault_root, file_path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create_dir_all failed: {e}"))?;
    }
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json.as_bytes()).map_err(|e| format!("tmp write failed: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename failed: {e}"))?;
    Ok(path)
}

/// Delete the captures cache for `file_path`. Returns `true` when
/// a file was removed, `false` when there was nothing to delete.
pub fn delete_captures_file(vault_root: &Path, file_path: &str) -> Result<bool, String> {
    let path = captures_path_for(vault_root, file_path)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("remove failed: {e}")),
    }
}

fn captures_path_for(vault_root: &Path, file_path: &str) -> Result<PathBuf, String> {
    let rel = sanitize_file_path(file_path)?;
    let mut p = vault_root.to_path_buf();
    p.push(".httui");
    p.push("captures");
    // Append `.json` to the *full* relative path so a runbook
    // `ops/incident.md` becomes `.httui/captures/ops/incident.md.json`
    // — preserves the source-file shape and dodges collisions when
    // two folders contain a file with the same stem.
    let mut leaf_with_ext = rel.clone();
    let new_name = match leaf_with_ext.file_name() {
        Some(name) => {
            let mut n = name.to_os_string();
            n.push(".json");
            n
        }
        None => return Err("file_path has no leaf name".into()),
    };
    leaf_with_ext.set_file_name(new_name);
    p.push(leaf_with_ext);
    Ok(p)
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

    const SAMPLE_JSON: &str = r#"{"fetchUser":{"token":"abc"}}"#;

    #[test]
    fn write_then_read_round_trips() {
        let dir = tempdir().unwrap();
        let path = write_captures_file(dir.path(), "runbook.md", SAMPLE_JSON).unwrap();
        assert!(path.ends_with(".httui/captures/runbook.md.json"));
        let read = read_captures_file(dir.path(), "runbook.md")
            .unwrap()
            .unwrap();
        assert_eq!(read, SAMPLE_JSON);
    }

    #[test]
    fn read_returns_none_when_missing() {
        let dir = tempdir().unwrap();
        let r = read_captures_file(dir.path(), "absent.md").unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn write_creates_parent_dirs_for_nested_path() {
        let dir = tempdir().unwrap();
        let path = write_captures_file(dir.path(), "ops/incident.md", "{}").unwrap();
        assert!(path.ends_with(".httui/captures/ops/incident.md.json"));
        assert!(path.parent().unwrap().is_dir());
    }

    #[test]
    fn delete_removes_existing_file_and_reports_true() {
        let dir = tempdir().unwrap();
        write_captures_file(dir.path(), "x.md", "{}").unwrap();
        assert!(delete_captures_file(dir.path(), "x.md").unwrap());
        assert!(read_captures_file(dir.path(), "x.md").unwrap().is_none());
    }

    #[test]
    fn delete_returns_false_when_nothing_to_remove() {
        let dir = tempdir().unwrap();
        assert!(!delete_captures_file(dir.path(), "absent.md").unwrap());
    }

    #[test]
    fn write_overwrites_atomically_with_no_partial_state() {
        let dir = tempdir().unwrap();
        let p1 = write_captures_file(dir.path(), "x.md", "{\"v\":1}").unwrap();
        let p2 = write_captures_file(dir.path(), "x.md", "{\"v\":2}").unwrap();
        assert_eq!(p1, p2);
        let final_text = read_captures_file(dir.path(), "x.md").unwrap().unwrap();
        assert_eq!(final_text, "{\"v\":2}");
        // No leftover .tmp file.
        let parent = p1.parent().unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(parent)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("tmp"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn dotdot_in_file_path_errors() {
        let dir = tempdir().unwrap();
        let r = write_captures_file(dir.path(), "../sneaky.md", "{}");
        assert!(r.is_err());
        let r = read_captures_file(dir.path(), "../sneaky.md");
        assert!(r.is_err());
        let r = delete_captures_file(dir.path(), "../sneaky.md");
        assert!(r.is_err());
    }

    #[test]
    fn empty_file_path_errors() {
        let dir = tempdir().unwrap();
        assert!(write_captures_file(dir.path(), "", "{}").is_err());
        assert!(read_captures_file(dir.path(), "").is_err());
        assert!(delete_captures_file(dir.path(), "").is_err());
    }

    #[test]
    fn leading_slashes_are_stripped() {
        let dir = tempdir().unwrap();
        let path = write_captures_file(dir.path(), "/runbook.md", "{}").unwrap();
        assert!(path.starts_with(dir.path().join(".httui/captures")));
        assert!(path.ends_with("runbook.md.json"));
    }

    #[test]
    fn body_is_opaque_string_not_json_validated() {
        // Backend doesn't parse — frontend owns shape.
        let dir = tempdir().unwrap();
        let raw = "not json on purpose";
        let path = write_captures_file(dir.path(), "x.md", raw).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);
    }

    #[test]
    fn captures_path_for_handles_only_dot_segments() {
        let dir = tempdir().unwrap();
        // `./x.md` and `././x.md` should both resolve to `x.md`.
        write_captures_file(dir.path(), "./x.md", "{}").unwrap();
        write_captures_file(dir.path(), "./././y.md", "{}").unwrap();
        assert!(read_captures_file(dir.path(), "x.md").unwrap().is_some());
        assert!(read_captures_file(dir.path(), "y.md").unwrap().is_some());
    }
}
