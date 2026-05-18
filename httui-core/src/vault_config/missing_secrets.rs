//! Scan a vault for `{{keychain:...}}` references whose values are
//! missing from the local keychain.
//!
//! On a freshly cloned vault every `{{keychain:...}}` ref points at
//! a key that doesn't yet exist on this machine. The first-run flow
//! batch-prompts the user once instead of forcing a
//! prompt-per-execution. This module walks `connections.toml` and
//! `envs/*.toml`, collects every reference, and asks the
//! [`SecretBackend`] which ones are missing.

use std::path::Path;

use crate::secrets::parser;
use crate::secrets::SecretBackend;

use super::layout::{CONNECTIONS_FILE, ENVS_DIR};

/// One missing reference. The grouping is what the UI uses to label
/// the form sections ("3 connections need passwords", "5 env vars
/// need values").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct MissingRef {
    /// Vault-relative path of the file that holds the reference.
    pub source_file: String,
    /// Where in the file the ref came from. Surfaces as the form
    /// label ("connection `pg-staging` / password",
    /// "env `staging` / DB_URL").
    pub label: String,
    /// The keychain address the reference points at (e.g.
    /// `conn:pg-staging:password`). Caller passes this back when
    /// the user submits the value.
    pub keychain_key: String,
    /// `connections` or `env`.
    pub kind: MissingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingKind {
    Connection,
    Env,
}

/// Walk `vault_root` and return every `{{keychain:...}}` reference
/// that doesn't have a corresponding entry in `backend`. Already-
/// populated refs are filtered out.
pub fn scan_missing_secrets<B: SecretBackend>(
    vault_root: &Path,
    backend: &B,
) -> Result<Vec<MissingRef>, String> {
    let mut all = Vec::new();
    collect_connections(vault_root, &mut all)?;
    collect_envs(vault_root, &mut all)?;

    let mut missing = Vec::new();
    for r in all {
        match backend.get(&r.keychain_key) {
            Ok(Some(_)) => {} // present — skip
            Ok(None) => missing.push(r),
            // A backend error here would block the whole scan; surface
            // the first one with context.
            Err(e) => return Err(format!("backend.get('{}'): {e}", r.keychain_key)),
        }
    }
    Ok(missing)
}

fn collect_connections(vault_root: &Path, out: &mut Vec<MissingRef>) -> Result<(), String> {
    let path = vault_root.join(CONNECTIONS_FILE);
    if !path.exists() {
        return Ok(());
    }
    let raw =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let Some(connections) = value.get("connections").and_then(|v| v.as_table()) else {
        return Ok(());
    };
    for (name, conn) in connections {
        let Some(table) = conn.as_table() else {
            continue;
        };
        for (field, value) in table {
            if let Some(s) = value.as_str() {
                if let Ok(("keychain", addr)) = parser::parse_secret_ref(s) {
                    out.push(MissingRef {
                        source_file: CONNECTIONS_FILE.to_string(),
                        label: format!("connection `{name}` / {field}"),
                        keychain_key: addr.to_string(),
                        kind: MissingKind::Connection,
                    });
                }
            }
        }
    }
    Ok(())
}

fn collect_envs(vault_root: &Path, out: &mut Vec<MissingRef>) -> Result<(), String> {
    let dir = vault_root.join(ENVS_DIR);
    if !dir.is_dir() {
        return Ok(());
    }
    let entries =
        std::fs::read_dir(&dir).map_err(|e| format!("read dir {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        // Skip *.local.toml — references in those mirror the base
        // and would surface as duplicates. Per ADR 0004 the merged
        // view is what the UI cares about; the base file already
        // contains the reference for the missing-secrets prompt.
        if stem.ends_with(".local") {
            continue;
        }
        scan_env_file(&path, stem, out)?;
    }
    Ok(())
}

fn scan_env_file(path: &Path, env_name: &str, out: &mut Vec<MissingRef>) -> Result<(), String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let value: toml::Value =
        toml::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    let Some(secrets) = value.get("secrets").and_then(|v| v.as_table()) else {
        return Ok(());
    };
    for (key, value) in secrets {
        if let Some(s) = value.as_str() {
            if let Ok(("keychain", addr)) = parser::parse_secret_ref(s) {
                let rel =
                    path_to_vault_relative(path).unwrap_or_else(|| path.display().to_string());
                out.push(MissingRef {
                    source_file: rel,
                    label: format!("env `{env_name}` / {key}"),
                    keychain_key: addr.to_string(),
                    kind: MissingKind::Env,
                });
            }
        }
    }
    Ok(())
}

fn path_to_vault_relative(p: &Path) -> Option<String> {
    let parent = p.parent()?.file_name()?.to_str()?;
    let file = p.file_name()?.to_str()?;
    Some(format!("{parent}/{file}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{Keychain, SecretResult};
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use tempfile::TempDir;

    /// Lightweight stub that lets tests pre-populate "stored" entries
    /// without going through the OS keychain — keeps the test suite
    /// out of `cargo test`'s macOS-Keychain prompts.
    struct StubBackend {
        store: Mutex<HashMap<String, String>>,
    }

    impl StubBackend {
        fn with(entries: &[(&str, &str)]) -> Self {
            let mut m = HashMap::new();
            for (k, v) in entries {
                m.insert((*k).to_string(), (*v).to_string());
            }
            Self {
                store: Mutex::new(m),
            }
        }
    }

    impl SecretBackend for StubBackend {
        fn store(&self, key: &str, value: &str) -> SecretResult<()> {
            self.store
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
            Ok(())
        }
        fn get(&self, key: &str) -> SecretResult<Option<String>> {
            Ok(self.store.lock().unwrap().get(key).cloned())
        }
        fn delete(&self, key: &str) -> SecretResult<()> {
            self.store.lock().unwrap().remove(key);
            Ok(())
        }
        fn id(&self) -> &'static str {
            "stub"
        }
    }

    fn write_vault(dir: &TempDir, files: &[(&str, &str)]) -> PathBuf {
        let root = dir.path().join("vault");
        std::fs::create_dir_all(root.join("envs")).unwrap();
        for (rel, content) in files {
            let p = root.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, content).unwrap();
        }
        root
    }

    #[test]
    fn empty_vault_has_no_missing_refs() {
        let dir = TempDir::new().unwrap();
        let root = write_vault(&dir, &[]);
        let backend = StubBackend::with(&[]);
        let missing = scan_missing_secrets(&root, &backend).unwrap();
        assert!(missing.is_empty());
    }

    #[test]
    fn finds_missing_connection_password() {
        let dir = TempDir::new().unwrap();
        let root = write_vault(
            &dir,
            &[(
                "connections.toml",
                r#"version = "1"
[connections.pg-staging]
type = "postgres"
host = "h"
port = 5432
database = "d"
user = "u"
password = "{{keychain:conn:pg-staging:password}}"
"#,
            )],
        );
        let backend = StubBackend::with(&[]);
        let missing = scan_missing_secrets(&root, &backend).unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].kind, MissingKind::Connection);
        assert_eq!(missing[0].keychain_key, "conn:pg-staging:password");
        assert_eq!(missing[0].label, "connection `pg-staging` / password");
        assert_eq!(missing[0].source_file, "connections.toml");
    }

    #[test]
    fn skips_already_populated_refs() {
        let dir = TempDir::new().unwrap();
        let root = write_vault(
            &dir,
            &[(
                "connections.toml",
                r#"version = "1"
[connections.pg]
type = "postgres"
host = "h"
port = 5432
database = "d"
user = "u"
password = "{{keychain:conn:pg:password}}"
"#,
            )],
        );
        let backend = StubBackend::with(&[("conn:pg:password", "secret")]);
        let missing = scan_missing_secrets(&root, &backend).unwrap();
        assert!(missing.is_empty(), "populated ref should be filtered out");
    }

    #[test]
    fn finds_missing_env_secrets() {
        let dir = TempDir::new().unwrap();
        let root = write_vault(
            &dir,
            &[(
                "envs/staging.toml",
                r#"version = "1"
[vars]
PUBLIC = "shared"
[secrets]
DB_URL = "{{keychain:env:staging:DB_URL}}"
API_TOKEN = "{{keychain:env:staging:API_TOKEN}}"
"#,
            )],
        );
        let backend = StubBackend::with(&[]);
        let missing = scan_missing_secrets(&root, &backend).unwrap();
        assert_eq!(missing.len(), 2);
        for r in &missing {
            assert_eq!(r.kind, MissingKind::Env);
            assert_eq!(r.source_file, "envs/staging.toml");
        }
    }

    #[test]
    fn skips_local_env_files() {
        let dir = TempDir::new().unwrap();
        let root = write_vault(
            &dir,
            &[(
                "envs/staging.local.toml",
                r#"version = "1"
[secrets]
DB_URL = "{{keychain:env:staging:DB_URL}}"
"#,
            )],
        );
        let backend = StubBackend::with(&[]);
        let missing = scan_missing_secrets(&root, &backend).unwrap();
        assert!(missing.is_empty(), ".local files must not be scanned");
    }

    #[test]
    fn unknown_backends_are_silently_skipped() {
        // Non-keychain refs (1password, pass, env) aren't this
        // resolver's responsibility — they get their own handlers.
        let dir = TempDir::new().unwrap();
        let root = write_vault(
            &dir,
            &[(
                "envs/staging.toml",
                r#"version = "1"
[secrets]
TOKEN = "{{1password:op://Personal/x}}"
"#,
            )],
        );
        let backend = StubBackend::with(&[]);
        let missing = scan_missing_secrets(&root, &backend).unwrap();
        assert!(missing.is_empty());
    }

    #[test]
    fn missing_files_are_silently_ignored() {
        let dir = TempDir::new().unwrap();
        // No connections.toml, no envs/ — a fresh vault.
        let root = dir.path().to_path_buf();
        let backend = StubBackend::with(&[]);
        let missing = scan_missing_secrets(&root, &backend).unwrap();
        assert!(missing.is_empty());
    }

    #[test]
    fn invalid_toml_returns_error() {
        let dir = TempDir::new().unwrap();
        let root = write_vault(&dir, &[("connections.toml", "this is = = not valid")]);
        let backend = StubBackend::with(&[]);
        let err = scan_missing_secrets(&root, &backend).unwrap_err();
        assert!(err.contains("parse"), "got {err}");
    }

    #[test]
    fn smoke_test_with_real_keychain_backend() {
        // Cheap smoke: the real `Keychain` impl plugs into the same
        // function. We don't assert on contents — just that the call
        // succeeds against an empty vault. Validates the trait
        // boundary.
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        let result = scan_missing_secrets(&root, &Keychain);
        assert!(result.is_ok());
    }
}
