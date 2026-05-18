//! Structured errors for the `vault_config` layer.
//!
//! Replaces (incrementally) the `Result<_, String>` shape that
//! flattens errors at every internal boundary. Each variant carries
//! a stable `code()` so the Tauri IPC layer can serialize
//! `{ code, message, details? }` and the frontend can switch on the
//! code for retry / display logic.
//!
//! defined this surface; the migration of existing call sites
//! is incremental and lands across follow-up
//! commits. New code should reach for `VaultConfigError` /
//! `ConnectionsError` directly; legacy callers continue with
//! `Result<_, String>` until rewired.

use std::io;

use thiserror::Error;

/// Errors raised by the vault_config layer (TOML CRUD, validation,
/// keychain, scaffold, migration).
#[derive(Debug, Error)]
pub enum VaultConfigError {
    /// Filesystem error — read, write, mkdir, atomic rename, fsync.
    #[error("io error: {0}")]
    Io(#[from] io::Error),

    /// TOML parse error (deserialization). The TOML file is malformed
    /// or doesn't match the expected schema.
    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// TOML serialize error — should be rare (always a programmer
    /// error, never a user input issue).
    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    /// Schema-level validation rejected the parsed file (e.g.
    /// `[secrets]` value isn't a `{{...}}` reference, sensitive
    /// field holds plaintext).
    #[error("validation: {0}")]
    Validation(String),

    /// Keychain backend reported an error — service unavailable,
    /// permission denied, locked, etc.
    #[error("keychain: {0}")]
    Keychain(String),

    /// Migration / cutover-related error — wrap the migration
    /// dispatcher's structured failure.
    #[error("cutover: {0}")]
    Cutover(String),

    /// A `{{keychain:KEY}}` reference resolved to an empty keychain
    /// slot — the user hasn't supplied the secret yet (typically
    /// after a fresh clone).
    #[error("missing secret for key: {key}")]
    MissingSecret { key: String },

    /// Two-way conflict between base + local TOML files, or between
    /// in-memory state and disk after an external edit.
    #[error("conflict: {message}")]
    Conflict { message: String },

    /// Resource not found — connection / environment / variable.
    /// `kind` is the human-facing category (`"connection"`, `"environment"`,
    /// `"variable"`); `name` is the user-supplied key.
    #[error("{kind} not found: {name}")]
    NotFound { kind: String, name: String },
}

impl VaultConfigError {
    /// Stable error code. The frontend keys retry / display logic
    /// off this value, so the mapping is API surface — adding a
    /// variant means adding a new code; renumbering existing codes
    /// is a breaking change.
    pub fn code(&self) -> &'static str {
        match self {
            VaultConfigError::Io(_) => "VC-001",
            VaultConfigError::TomlParse(_) => "VC-002",
            VaultConfigError::TomlSerialize(_) => "VC-003",
            VaultConfigError::Validation(_) => "VC-004",
            VaultConfigError::Keychain(_) => "VC-005",
            VaultConfigError::Cutover(_) => "VC-006",
            VaultConfigError::MissingSecret { .. } => "VC-007",
            VaultConfigError::Conflict { .. } => "VC-008",
            VaultConfigError::NotFound { .. } => "VC-009",
        }
    }
}

/// Errors specific to the connection-management surface — the
/// vault_config core errors plus connection-specific cases that
/// don't fit the more general categories.
#[derive(Debug, Error)]
pub enum ConnectionsError {
    /// Wraps any `VaultConfigError` so `?` chains compose without
    /// repeating the boilerplate at every call site.
    #[error("{0}")]
    Vault(#[from] VaultConfigError),

    /// User asked for a driver this build doesn't support (`weirdb`
    /// etc.). Replaces the legacy `"unsupported driver: <name>"`
    /// stringly-typed error.
    #[error("unsupported driver: {driver}")]
    UnsupportedDriver { driver: String },

    /// `create_pool` returned a sqlx error after sanitization. The
    /// `driver` carries the typed-enum name for the frontend toast.
    #[error("pool build failed for {driver}: {message}")]
    PoolBuildFailed { driver: String, message: String },

    /// Connection ping (`pool.test()`) failed.
    #[error("test failed for {name}: {message}")]
    TestFailed { name: String, message: String },
}

impl ConnectionsError {
    /// Stable error code. Inherits the `VC-*` codes via the `Vault`
    /// variant; connection-specific codes live in the `CN-*` namespace.
    pub fn code(&self) -> &'static str {
        match self {
            ConnectionsError::Vault(inner) => inner.code(),
            ConnectionsError::UnsupportedDriver { .. } => "CN-001",
            ConnectionsError::PoolBuildFailed { .. } => "CN-002",
            ConnectionsError::TestFailed { .. } => "CN-003",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vc_codes() -> Vec<VaultConfigError> {
        vec![
            VaultConfigError::Io(io::Error::new(io::ErrorKind::NotFound, "missing")),
            VaultConfigError::Validation("bad shape".into()),
            VaultConfigError::Keychain("locked".into()),
            VaultConfigError::Cutover("phase 3 dropped".into()),
            VaultConfigError::MissingSecret {
                key: "conn:pg:password".into(),
            },
            VaultConfigError::Conflict {
                message: "external edit".into(),
            },
            VaultConfigError::NotFound {
                kind: "connection".into(),
                name: "no-such".into(),
            },
        ]
    }

    #[test]
    fn vault_config_error_codes_are_stable_and_unique() {
        let codes: Vec<&'static str> = vc_codes().iter().map(|e| e.code()).collect();
        // Every code starts with the `VC-` prefix.
        for c in &codes {
            assert!(c.starts_with("VC-"), "got: {c}");
        }
        // And no duplicates among the variants we constructed.
        let mut sorted = codes.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            codes.len(),
            sorted.len(),
            "duplicate codes in vault_config error set: {codes:?}"
        );
    }

    #[test]
    fn vault_config_error_io_from_impl() {
        // `?` should bubble io::Error into the typed error without
        // an explicit map_err.
        fn read() -> Result<(), VaultConfigError> {
            Err(io::Error::other("boom"))?;
            Ok(())
        }
        let err = read().unwrap_err();
        assert!(matches!(err, VaultConfigError::Io(_)));
        assert_eq!(err.code(), "VC-001");
    }

    #[test]
    fn vault_config_error_toml_parse_from_impl() {
        let toml_err: toml::de::Error =
            toml::from_str::<toml::Value>("not = = valid").expect_err("must fail");
        let vc: VaultConfigError = toml_err.into();
        assert!(matches!(vc, VaultConfigError::TomlParse(_)));
        assert_eq!(vc.code(), "VC-002");
    }

    #[test]
    fn vault_config_error_display_includes_message() {
        let err = VaultConfigError::Validation("missing host".into());
        let msg = err.to_string();
        assert!(msg.contains("validation"));
        assert!(msg.contains("missing host"));
    }

    #[test]
    fn vault_config_error_not_found_displays_kind_and_name() {
        let err = VaultConfigError::NotFound {
            kind: "connection".into(),
            name: "pg-staging".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("connection"));
        assert!(msg.contains("pg-staging"));
        assert_eq!(err.code(), "VC-009");
    }

    #[test]
    fn connections_error_inherits_vault_codes() {
        // The `Vault` wrap-variant must surface the inner code, not
        // a fresh `CN-*` shadow.
        let inner = VaultConfigError::Validation("bad".into());
        let outer: ConnectionsError = inner.into();
        assert_eq!(outer.code(), "VC-004");
    }

    #[test]
    fn connections_error_codes_are_stable_and_namespaced() {
        let cases = vec![
            ConnectionsError::UnsupportedDriver {
                driver: "weirdb".into(),
            },
            ConnectionsError::PoolBuildFailed {
                driver: "postgres".into(),
                message: "auth failed".into(),
            },
            ConnectionsError::TestFailed {
                name: "pg-prod".into(),
                message: "timeout".into(),
            },
        ];
        for case in &cases {
            assert!(
                case.code().starts_with("CN-"),
                "got: {} for {case:?}",
                case.code()
            );
        }
    }

    #[test]
    fn connections_error_unsupported_driver_displays() {
        let err = ConnectionsError::UnsupportedDriver {
            driver: "weirdb".into(),
        };
        assert!(err.to_string().contains("weirdb"));
        assert_eq!(err.code(), "CN-001");
    }

    #[test]
    fn connections_error_pool_build_failed_displays() {
        let err = ConnectionsError::PoolBuildFailed {
            driver: "postgres".into(),
            message: "auth failed".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("postgres"));
        assert!(msg.contains("auth failed"));
        assert_eq!(err.code(), "CN-002");
    }
}
