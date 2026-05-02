//! Atomic file write helper.
//!
//! Implements the atomic-write contract from ADR 0003:
//!
//! 1. Write to a sibling `<file>.tmp.<random>` in the same directory
//!    (so the rename stays on the same filesystem and is atomic).
//! 2. `fsync` the temp file.
//! 3. `rename` over the target.
//! 4. Preserve the target's mode if it already existed.
//!
//! Parent directories are created on demand. Errors at any step leave
//! the original file untouched (the temp file is removed on failure).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Atomically write `content` to `path`. See module docs for the contract.
pub fn write_atomic(path: &Path, content: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "write_atomic: path must include a file name",
        )
    })?;

    let tmp_name = {
        let mut name = std::ffi::OsString::from(".");
        name.push(file_name);
        name.push(".tmp.");
        name.push(unique_suffix());
        name
    };
    let tmp_path = match parent {
        Some(p) => p.join(&tmp_name),
        None => Path::new(&tmp_name).to_path_buf(),
    };

    let preexisting_mode = preexisting_mode(path);

    let result = (|| -> io::Result<()> {
        let mut tmp = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp_path)?;
        tmp.write_all(content.as_bytes())?;
        tmp.flush()?;
        tmp.sync_all()?;
        drop(tmp);

        if let Some(mode) = preexisting_mode {
            apply_mode(&tmp_path, mode)?;
        }

        fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

#[cfg(unix)]
fn preexisting_mode(path: &Path) -> Option<u32> {
    fs::metadata(path).ok().map(|m| m.permissions().mode())
}

#[cfg(not(unix))]
fn preexisting_mode(_path: &Path) -> Option<u32> {
    None
}

#[cfg(unix)]
fn apply_mode(path: &Path, mode: u32) -> io::Result<()> {
    let perms = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn apply_mode(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

fn unique_suffix() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    format!("{nanos:x}-{pid:x}-{count:x}")
}

// Convenience: read a TOML file and deserialize into T.
pub fn read_toml<T>(path: &Path) -> io::Result<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    let content = fs::read_to_string(path)?;
    toml::from_str::<T>(&content).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

// Convenience: serialize T and atomically write to path.
pub fn write_toml<T>(path: &Path, value: &T) -> io::Result<()>
where
    T: serde::Serialize,
{
    let content =
        toml::to_string_pretty(value).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_atomic(path, &content)
}

// Quick existence check, useful before write to decide whether to
// preserve mode.
pub fn file_exists(path: &Path) -> bool {
    File::open(path).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault_config::workspace::WorkspaceFile;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn writes_new_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("foo.toml");
        write_atomic(&path, "version = \"1\"\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "version = \"1\"\n");
    }

    #[test]
    fn creates_parent_dirs() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nested/deep/file.toml");
        write_atomic(&path, "x = 1\n").unwrap();
        assert!(path.exists());
    }

    #[test]
    fn overwrites_existing() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("foo.toml");
        fs::write(&path, "old\n").unwrap();
        write_atomic(&path, "new\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new\n");
    }

    #[test]
    fn no_temp_file_left_after_success() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("foo.toml");
        write_atomic(&path, "x\n").unwrap();
        let entries: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(entries.len(), 1, "expected only the target file");
    }

    #[cfg(unix)]
    #[test]
    fn preserves_unix_mode() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("foo.toml");
        fs::write(&path, "old\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        write_atomic(&path, "new\n").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn round_trip_workspace_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("workspace.toml");
        let original = WorkspaceFile {
            version: crate::vault_config::Version::V1,
            defaults: crate::vault_config::workspace::WorkspaceDefaults {
                environment: Some("staging".into()),
                git_remote: Some("origin".into()),
                git_branch: Some("main".into()),
                display_name: None,
            },
            files: Default::default(),
        };
        write_toml(&path, &original).unwrap();
        let loaded: WorkspaceFile = read_toml(&path).unwrap();
        assert_eq!(loaded.defaults.environment.as_deref(), Some("staging"));
        assert_eq!(loaded.defaults.git_remote.as_deref(), Some("origin"));
    }

    #[test]
    fn round_trip_connections_file_preserves_variants() {
        use crate::vault_config::connections::{
            Connection, ConnectionsFile, HttpConfig, PostgresConfig,
        };
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("connections.toml");

        let mut connections = std::collections::BTreeMap::new();
        connections.insert(
            "pg".to_string(),
            Connection::Postgres(PostgresConfig {
                host: "h".into(),
                port: 5432,
                database: "d".into(),
                user: "{{keychain:pg:user}}".into(),
                password: "{{keychain:pg:password}}".into(),
                ssl_mode: Some("require".into()),
                common: Default::default(),
            }),
        );
        connections.insert(
            "api".to_string(),
            Connection::Http(HttpConfig {
                base_url: "https://x".into(),
                default_headers: Default::default(),
                timeout_ms: Some(30000),
                common: Default::default(),
            }),
        );
        let original = ConnectionsFile {
            version: crate::vault_config::Version::V1,
            connections,
        };

        write_toml(&path, &original).unwrap();
        let loaded: ConnectionsFile = read_toml(&path).unwrap();
        assert_eq!(loaded.connections.len(), 2);
        assert!(matches!(
            loaded.connections.get("pg"),
            Some(Connection::Postgres(_))
        ));
        assert!(matches!(
            loaded.connections.get("api"),
            Some(Connection::Http(_))
        ));
    }

    #[test]
    fn read_toml_invalid_returns_invalid_data_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.toml");
        fs::write(&path, "this is = = not valid toml [[[").unwrap();
        let err = read_toml::<WorkspaceFile>(&path).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn unique_suffix_is_actually_unique() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..1000 {
            assert!(seen.insert(unique_suffix()), "collision");
        }
    }
}
