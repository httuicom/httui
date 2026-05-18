//! Connection variant views — converting `Connection` enum values
//! into the public DTO (`ConnectionPublic`) and into the legacy
//! `db::connections::Connection` struct that the pool manager still
//! consumes.
//!
//! Extracted from `connections_store.rs`. The pure variant
//! accessors (`driver_string_for`, `host_of`, …) live
//! here too — they're shared between the public view and the legacy
//! adapter, and they're the OCP target replaces with `trait
//! DbConnection`. Keeping them in one module makes the trait
//! migration a one-file refactor.

use serde::Serialize;

use crate::db::connections::Connection as LegacyConnection;
use crate::db::keychain::KEYCHAIN_SENTINEL;

use super::connections::Connection;
use super::secret_resolver::{keychain_entry_exists, resolve_value};
use super::validate::is_secret_ref;

/// Public-facing connection DTO (no secrets — `has_password` instead).
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionPublic {
    pub name: String,
    pub driver: String,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub database_name: Option<String>,
    pub username: Option<String>,
    pub has_password: bool,
    pub ssl_mode: Option<String>,
    pub is_readonly: bool,
    pub description: Option<String>,
}

// --- variant accessors (delegate to DbConnection trait) ------------------

pub(super) fn driver_string_for(c: &Connection) -> &'static str {
    c.as_dyn().driver()
}

pub(super) fn host_of(c: &Connection) -> Option<String> {
    c.as_dyn().host().map(str::to_owned)
}

pub(super) fn port_of(c: &Connection) -> Option<u16> {
    c.as_dyn().port()
}

pub(super) fn database_name_of(c: &Connection) -> Option<String> {
    c.as_dyn().database_name().map(str::to_owned)
}

pub(super) fn username_of(c: &Connection) -> Option<String> {
    c.as_dyn().username().map(str::to_owned)
}

pub(super) fn ssl_mode_of(c: &Connection) -> Option<String> {
    c.as_dyn().ssl_mode().map(str::to_owned)
}

pub(super) fn existing_readonly(c: &Connection) -> bool {
    c.as_dyn().common().read_only
}

pub(super) fn description_of(c: &Connection) -> Option<String> {
    c.as_dyn().common().description.clone()
}

/// Returns the password reference verbatim for an "unchanged" update.
/// Variants without password support return `None`.
pub(super) fn carry_password_ref(c: &Connection) -> Option<String> {
    c.as_dyn().password().map(str::to_owned)
}

/// True when the variant carries a non-empty password and either:
/// - it's a plaintext value (legacy), OR
/// - it's a `{{keychain:…}}` reference whose entry actually exists.
fn password_present(c: &Connection) -> bool {
    let Some(password) = c.as_dyn().password() else {
        return false;
    };
    if password.is_empty() {
        return false;
    }
    if is_secret_ref(password) {
        // A reference counts as "has password" only when the keychain
        // actually has the entry. Best-effort lookup; transient
        // keychain failures surface as `false` (caller can re-prompt).
        keychain_entry_exists(password)
    } else {
        // Legacy plaintext value (pre-migration) or sentinel.
        password != KEYCHAIN_SENTINEL
    }
}

// --- conversions ---------------------------------------------------------

/// Convert a vault `Connection` into the public DTO that fronts the
/// UI. No secrets — `has_password` replaces the actual reference.
pub(super) fn to_public(name: &str, c: &Connection) -> ConnectionPublic {
    let view = c.as_dyn();
    let common = view.common();
    ConnectionPublic {
        name: name.to_string(),
        driver: view.driver().to_string(),
        host: view.host().map(str::to_owned),
        port: view.port(),
        database_name: view.database_name().map(str::to_owned),
        username: view.username().map(str::to_owned),
        has_password: password_present(c),
        ssl_mode: view.ssl_mode().map(str::to_owned),
        is_readonly: common.read_only,
        description: common.description.clone(),
    }
}

/// Convert a vault `Connection` to the legacy struct that the pool
/// manager still understands. Resolves keychain references via
/// [`secret_resolver::resolve_value`], so the returned struct is
/// usable for an actual DB connection.
///
/// This adapter dies when the legacy `db::connections::Connection`
/// shape is removed (post frontend).
pub(super) fn to_legacy(name: &str, c: &Connection) -> Result<LegacyConnection, String> {
    let view = c.as_dyn();
    let password = match view.password() {
        Some(raw) => resolve_value(raw)?,
        None => None,
    };

    Ok(LegacyConnection {
        id: name.to_string(),
        name: name.to_string(),
        driver: view.driver().to_string(),
        host: view.host().map(str::to_owned),
        port: view.port().map(|p| p as i64),
        database_name: view.database_name().map(str::to_owned),
        username: view.username().map(str::to_owned),
        password,
        ssl_mode: view.ssl_mode().map(str::to_owned),
        timeout_ms: 10000,
        query_timeout_ms: 30000,
        ttl_seconds: 300,
        max_pool_size: 5,
        is_readonly: view.common().read_only,
        last_tested_at: None,
        created_at: String::new(),
        updated_at: String::new(),
    })
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use crate::db::keychain::{delete_secret, KEYCHAIN_TEST_LOCK};
    use crate::vault_config::connections::{
        CommonFields, MysqlConfig, PostgresConfig, ShellConfig, SqliteConfig,
    };
    use crate::vault_config::secret_resolver::ensure_keychain_ref;

    fn pg(password: &str) -> Connection {
        Connection::Postgres(PostgresConfig {
            host: "h".into(),
            port: 5432,
            database: "d".into(),
            user: "u".into(),
            password: password.into(),
            ssl_mode: Some("require".into()),
            common: CommonFields {
                description: Some("pg".into()),
                read_only: true,
            },
        })
    }

    fn mysql(password: &str) -> Connection {
        Connection::Mysql(MysqlConfig {
            host: "mh".into(),
            port: 3306,
            database: "md".into(),
            user: "mu".into(),
            password: password.into(),
            common: CommonFields::default(),
        })
    }

    fn sqlite() -> Connection {
        Connection::Sqlite(SqliteConfig {
            path: "/tmp/x.sqlite".into(),
            common: CommonFields::default(),
        })
    }

    fn shell() -> Connection {
        Connection::Shell(ShellConfig {
            shell: "/bin/sh".into(),
            cwd: None,
            common: CommonFields::default(),
        })
    }

    #[test]
    fn driver_string_covers_every_variant() {
        assert_eq!(driver_string_for(&pg("")), "postgres");
        assert_eq!(driver_string_for(&mysql("")), "mysql");
        assert_eq!(driver_string_for(&sqlite()), "sqlite");
        assert_eq!(driver_string_for(&shell()), "shell");
    }

    #[test]
    fn host_of_returns_none_for_non_db_variants() {
        assert_eq!(host_of(&pg("")).as_deref(), Some("h"));
        assert_eq!(host_of(&mysql("")).as_deref(), Some("mh"));
        assert_eq!(host_of(&sqlite()), None);
        assert_eq!(host_of(&shell()), None);
    }

    #[test]
    fn port_of_returns_none_for_non_db_variants() {
        assert_eq!(port_of(&pg("")), Some(5432));
        assert_eq!(port_of(&mysql("")), Some(3306));
        assert_eq!(port_of(&sqlite()), None);
    }

    #[test]
    fn database_name_of_includes_sqlite_path() {
        assert_eq!(database_name_of(&pg("")).as_deref(), Some("d"));
        assert_eq!(
            database_name_of(&sqlite()).as_deref(),
            Some("/tmp/x.sqlite")
        );
        assert_eq!(database_name_of(&shell()), None);
    }

    #[test]
    fn username_of_returns_none_for_non_db_variants() {
        assert_eq!(username_of(&pg("")).as_deref(), Some("u"));
        assert_eq!(username_of(&mysql("")).as_deref(), Some("mu"));
        assert_eq!(username_of(&sqlite()), None);
    }

    #[test]
    fn ssl_mode_of_only_for_postgres() {
        assert_eq!(ssl_mode_of(&pg("")).as_deref(), Some("require"));
        assert_eq!(ssl_mode_of(&mysql("")), None);
    }

    #[test]
    fn existing_readonly_and_description_pull_from_common() {
        assert!(existing_readonly(&pg("")));
        assert_eq!(description_of(&pg("")).as_deref(), Some("pg"));
        assert!(!existing_readonly(&mysql("")));
        assert_eq!(description_of(&mysql("")), None);
    }

    #[test]
    fn carry_password_ref_only_for_password_variants() {
        assert_eq!(carry_password_ref(&pg("ref")).as_deref(), Some("ref"));
        assert_eq!(carry_password_ref(&mysql("ref")).as_deref(), Some("ref"));
        assert_eq!(carry_password_ref(&sqlite()), None);
    }

    #[test]
    fn password_present_false_for_non_password_variants() {
        assert!(!password_present(&sqlite()));
        assert!(!password_present(&shell()));
    }

    #[test]
    fn password_present_false_for_empty() {
        assert!(!password_present(&pg("")));
    }

    #[test]
    fn password_present_true_for_plaintext_legacy() {
        assert!(password_present(&pg("legacy-plaintext")));
    }

    #[test]
    fn password_present_false_for_keychain_sentinel() {
        assert!(!password_present(&pg(KEYCHAIN_SENTINEL)));
    }

    #[test]
    fn password_present_reflects_keychain_entry_existence() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let key = "conn:test-views:password";
        let _ = delete_secret(key);

        let reference = ensure_keychain_ref(key, "real").unwrap();
        assert!(password_present(&pg(&reference)));

        let _ = delete_secret(key);
        assert!(!password_present(&pg(&reference)));
    }

    #[test]
    fn to_public_includes_all_fields_from_postgres() {
        let pub_ = to_public("pg-conn", &pg("legacy-plaintext"));
        assert_eq!(pub_.name, "pg-conn");
        assert_eq!(pub_.driver, "postgres");
        assert_eq!(pub_.host.as_deref(), Some("h"));
        assert_eq!(pub_.port, Some(5432));
        assert_eq!(pub_.database_name.as_deref(), Some("d"));
        assert_eq!(pub_.username.as_deref(), Some("u"));
        assert!(pub_.has_password);
        assert_eq!(pub_.ssl_mode.as_deref(), Some("require"));
        assert!(pub_.is_readonly);
        assert_eq!(pub_.description.as_deref(), Some("pg"));
    }

    #[test]
    fn to_public_for_sqlite_omits_optional_fields() {
        let pub_ = to_public("local", &sqlite());
        assert_eq!(pub_.driver, "sqlite");
        assert_eq!(pub_.host, None);
        assert_eq!(pub_.port, None);
        assert!(!pub_.has_password);
    }

    #[test]
    fn to_legacy_resolves_password_for_postgres() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let key = "conn:to-legacy:password";
        let _ = delete_secret(key);

        let reference = ensure_keychain_ref(key, "real-pw").unwrap();
        let legacy = to_legacy("pg-conn", &pg(&reference)).unwrap();
        assert_eq!(legacy.id, "pg-conn");
        assert_eq!(legacy.name, "pg-conn");
        assert_eq!(legacy.driver, "postgres");
        assert_eq!(legacy.host.as_deref(), Some("h"));
        assert_eq!(legacy.port, Some(5432));
        assert_eq!(legacy.password.as_deref(), Some("real-pw"));
        assert!(legacy.is_readonly);

        let _ = delete_secret(key);
    }

    #[test]
    fn to_legacy_passes_plaintext_password_through() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let legacy = to_legacy("legacy", &pg("plaintext-old")).unwrap();
        assert_eq!(legacy.password.as_deref(), Some("plaintext-old"));
    }

    #[test]
    fn to_legacy_returns_none_password_for_non_password_variants() {
        let legacy = to_legacy("local", &sqlite()).unwrap();
        assert_eq!(legacy.password, None);
    }

    #[test]
    fn to_legacy_propagates_missing_keychain_entry_error() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let key = "conn:no-such-entry:password";
        let _ = delete_secret(key);

        let reference = format!("{{{{keychain:{key}}}}}");
        let err = to_legacy("pg-conn", &pg(&reference)).expect_err("must error");
        assert!(err.contains("did not resolve"));
    }
}
