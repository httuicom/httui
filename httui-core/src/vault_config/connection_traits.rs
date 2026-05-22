//! `trait DbConnection` — variant-uniform access to a `Connection`.
//!
//! Replaces the chain of free `match`-based accessors that used to live
//! across `connection_views.rs`, `validate.rs`, etc. Each `Connection`
//! variant now ships its own `impl DbConnection` block. Adding a new
//! variant requires:
//!
//! 1. A new variant on the [`super::connections::Connection`] enum.
//! 2. A new `Config` struct in `super::connections`.
//! 3. One new `impl DbConnection for FooConfig` block in this file.
//! 4. One new arm in [`Connection::as_dyn`] dispatching to the impl.
//!
//! Existing `match` arms across the codebase don't need touching —
//! they all collapse to `connection.as_dyn().method()` calls.

use super::connections::{
    BigqueryConfig, CommonFields, Connection, GraphqlConfig, GrpcConfig, HttpConfig, MongoConfig,
    MysqlConfig, PostgresConfig, ShellConfig, SqliteConfig, WsConfig,
};
use super::validate::{check_field, Report};

/// Variant-uniform interface for `Connection` values.
///
/// Optional accessors return `None` by default — variants only override
/// the methods that make sense for them. `driver` and `common` are
/// required because every variant has both.
pub trait DbConnection {
    /// Tagged-union discriminator written into TOML as `type = "..."`.
    fn driver(&self) -> &'static str;

    /// Shared metadata (description, read_only).
    fn common(&self) -> &CommonFields;

    /// Hostname for variants that connect over TCP. `None` for
    /// path-based or URI-based variants.
    fn host(&self) -> Option<&str> {
        None
    }

    /// TCP port for variants that connect over TCP.
    fn port(&self) -> Option<u16> {
        None
    }

    /// Logical "database" identifier — schema for SQL variants, file
    /// path for SQLite, `None` for variants without a database concept.
    fn database_name(&self) -> Option<&str> {
        None
    }

    /// Username for variants with explicit authentication.
    fn username(&self) -> Option<&str> {
        None
    }

    /// TLS / SSL preference; only Postgres carries it today.
    fn ssl_mode(&self) -> Option<&str> {
        None
    }

    /// Password slot — `Some("")` when the variant has the slot but it's
    /// empty, `None` when the variant has no password concept.
    /// The caller resolves keychain references via `secret_resolver`.
    fn password(&self) -> Option<&str> {
        None
    }

    /// Per-variant schema validation: append findings to `report`.
    /// Default: no checks. Override on variants that carry
    /// sensitive-named fields (host/database/user/password/uri/etc).
    /// `connection_name` is the TOML map key — used for `[connections.NAME]`
    /// path prefixes in error messages.
    fn validate_fields(&self, _connection_name: &str, _report: &mut Report) {}
}

impl DbConnection for PostgresConfig {
    fn driver(&self) -> &'static str {
        "postgres"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
    fn host(&self) -> Option<&str> {
        Some(&self.host)
    }
    fn port(&self) -> Option<u16> {
        Some(self.port)
    }
    fn database_name(&self) -> Option<&str> {
        Some(&self.database)
    }
    fn username(&self) -> Option<&str> {
        Some(&self.user)
    }
    fn ssl_mode(&self) -> Option<&str> {
        self.ssl_mode.as_deref()
    }
    fn password(&self) -> Option<&str> {
        Some(&self.password)
    }
    fn validate_fields(&self, name: &str, report: &mut Report) {
        let base = format!("[connections.{name}]");
        check_field(report, &format!("{base}.host"), "host", &self.host);
        check_field(
            report,
            &format!("{base}.database"),
            "database",
            &self.database,
        );
        check_field(report, &format!("{base}.user"), "user", &self.user);
        check_field(
            report,
            &format!("{base}.password"),
            "password",
            &self.password,
        );
    }
}

impl DbConnection for MysqlConfig {
    fn driver(&self) -> &'static str {
        "mysql"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
    fn host(&self) -> Option<&str> {
        Some(&self.host)
    }
    fn port(&self) -> Option<u16> {
        Some(self.port)
    }
    fn database_name(&self) -> Option<&str> {
        Some(&self.database)
    }
    fn username(&self) -> Option<&str> {
        Some(&self.user)
    }
    fn password(&self) -> Option<&str> {
        Some(&self.password)
    }
    fn validate_fields(&self, name: &str, report: &mut Report) {
        let base = format!("[connections.{name}]");
        check_field(report, &format!("{base}.host"), "host", &self.host);
        check_field(
            report,
            &format!("{base}.database"),
            "database",
            &self.database,
        );
        check_field(report, &format!("{base}.user"), "user", &self.user);
        check_field(
            report,
            &format!("{base}.password"),
            "password",
            &self.password,
        );
    }
}

impl DbConnection for SqliteConfig {
    fn driver(&self) -> &'static str {
        "sqlite"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
    fn database_name(&self) -> Option<&str> {
        Some(&self.path)
    }
}

impl DbConnection for MongoConfig {
    fn driver(&self) -> &'static str {
        "mongo"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
    fn validate_fields(&self, name: &str, report: &mut Report) {
        let base = format!("[connections.{name}]");
        check_field(report, &format!("{base}.uri"), "uri", &self.uri);
        if let Some(auth) = &self.auth {
            check_field(report, &format!("{base}.auth"), "auth", auth);
        }
    }
}

impl DbConnection for HttpConfig {
    fn driver(&self) -> &'static str {
        "http"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
    fn validate_fields(&self, name: &str, report: &mut Report) {
        let base = format!("[connections.{name}]");
        for (header_name, header_value) in &self.default_headers {
            check_field(
                report,
                &format!("{base}.default_headers.{header_name}"),
                header_name,
                header_value,
            );
        }
    }
}

impl DbConnection for WsConfig {
    fn driver(&self) -> &'static str {
        "ws"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
}

impl DbConnection for GrpcConfig {
    fn driver(&self) -> &'static str {
        "grpc"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
}

impl DbConnection for GraphqlConfig {
    fn driver(&self) -> &'static str {
        "graphql"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
}

impl DbConnection for BigqueryConfig {
    fn driver(&self) -> &'static str {
        "bigquery"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
    fn validate_fields(&self, name: &str, report: &mut Report) {
        let base = format!("[connections.{name}]");
        check_field(
            report,
            &format!("{base}.credentials_path"),
            "credentials_path",
            &self.credentials_path,
        );
    }
}

impl DbConnection for ShellConfig {
    fn driver(&self) -> &'static str {
        "shell"
    }
    fn common(&self) -> &CommonFields {
        &self.common
    }
}

impl Connection {
    /// Single-match dispatch from the enum into the trait. Replaces N
    /// scattered match arms — adding a new variant edits this one
    /// match plus the new `impl DbConnection` block, nothing else.
    pub fn as_dyn(&self) -> &dyn DbConnection {
        match self {
            Connection::Postgres(c) => c,
            Connection::Mysql(c) => c,
            Connection::Sqlite(c) => c,
            Connection::Mongo(c) => c,
            Connection::Http(c) => c,
            Connection::Ws(c) => c,
            Connection::Grpc(c) => c,
            Connection::Graphql(c) => c,
            Connection::Bigquery(c) => c,
            Connection::Shell(c) => c,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn pg() -> Connection {
        Connection::Postgres(PostgresConfig {
            host: "h".into(),
            port: 5432,
            database: "d".into(),
            user: "u".into(),
            password: "p".into(),
            ssl_mode: Some("require".into()),
            common: CommonFields {
                description: Some("pg".into()),
                read_only: true,
            },
        })
    }

    fn mysql() -> Connection {
        Connection::Mysql(MysqlConfig {
            host: "mh".into(),
            port: 3306,
            database: "md".into(),
            user: "mu".into(),
            password: "mp".into(),
            common: CommonFields::default(),
        })
    }

    fn sqlite() -> Connection {
        Connection::Sqlite(SqliteConfig {
            path: "/tmp/x.sqlite".into(),
            common: CommonFields::default(),
        })
    }

    fn mongo() -> Connection {
        Connection::Mongo(MongoConfig {
            uri: "mongodb://x".into(),
            auth: None,
            common: CommonFields::default(),
        })
    }

    fn http() -> Connection {
        Connection::Http(HttpConfig {
            base_url: "https://x".into(),
            default_headers: BTreeMap::new(),
            timeout_ms: None,
            common: CommonFields::default(),
        })
    }

    fn ws() -> Connection {
        Connection::Ws(WsConfig {
            url: "wss://x".into(),
            common: CommonFields::default(),
        })
    }

    fn grpc() -> Connection {
        Connection::Grpc(GrpcConfig {
            endpoint: "x:443".into(),
            tls: false,
            common: CommonFields::default(),
        })
    }

    fn graphql() -> Connection {
        Connection::Graphql(GraphqlConfig {
            endpoint: "https://x/graphql".into(),
            default_headers: BTreeMap::new(),
            common: CommonFields::default(),
        })
    }

    fn bigquery() -> Connection {
        Connection::Bigquery(BigqueryConfig {
            project_id: "p".into(),
            credentials_path: "{{keychain:bq:creds}}".into(),
            common: CommonFields::default(),
        })
    }

    fn shell() -> Connection {
        Connection::Shell(ShellConfig {
            shell: "bash".into(),
            cwd: None,
            common: CommonFields::default(),
        })
    }

    #[test]
    fn driver_string_matches_serde_tag_for_every_variant() {
        // The trait's `driver()` MUST equal the serde rename_all
        // "lowercase" tag — `connections.toml` writes type = "..."
        // and downstream code keys off that string.
        assert_eq!(pg().as_dyn().driver(), "postgres");
        assert_eq!(mysql().as_dyn().driver(), "mysql");
        assert_eq!(sqlite().as_dyn().driver(), "sqlite");
        assert_eq!(mongo().as_dyn().driver(), "mongo");
        assert_eq!(http().as_dyn().driver(), "http");
        assert_eq!(ws().as_dyn().driver(), "ws");
        assert_eq!(grpc().as_dyn().driver(), "grpc");
        assert_eq!(graphql().as_dyn().driver(), "graphql");
        assert_eq!(bigquery().as_dyn().driver(), "bigquery");
        assert_eq!(shell().as_dyn().driver(), "shell");
    }

    #[test]
    fn common_fields_accessible_from_every_variant() {
        // Sanity check that `common()` works for the full enum surface.
        assert!(pg().as_dyn().common().read_only);
        assert!(!mysql().as_dyn().common().read_only);
        assert_eq!(pg().as_dyn().common().description.as_deref(), Some("pg"));
        // `_` to make sure each variant compiles without panicking.
        let _ = sqlite().as_dyn().common();
        let _ = mongo().as_dyn().common();
        let _ = http().as_dyn().common();
        let _ = ws().as_dyn().common();
        let _ = grpc().as_dyn().common();
        let _ = graphql().as_dyn().common();
        let _ = bigquery().as_dyn().common();
        let _ = shell().as_dyn().common();
    }

    #[test]
    fn host_only_for_tcp_db_variants() {
        assert_eq!(pg().as_dyn().host(), Some("h"));
        assert_eq!(mysql().as_dyn().host(), Some("mh"));
        // Variants without an explicit host concept return None.
        assert_eq!(sqlite().as_dyn().host(), None);
        assert_eq!(mongo().as_dyn().host(), None);
        assert_eq!(http().as_dyn().host(), None);
        assert_eq!(shell().as_dyn().host(), None);
    }

    #[test]
    fn port_only_for_tcp_db_variants() {
        assert_eq!(pg().as_dyn().port(), Some(5432));
        assert_eq!(mysql().as_dyn().port(), Some(3306));
        assert_eq!(sqlite().as_dyn().port(), None);
        assert_eq!(grpc().as_dyn().port(), None);
    }

    #[test]
    fn database_name_includes_sqlite_path() {
        assert_eq!(pg().as_dyn().database_name(), Some("d"));
        assert_eq!(mysql().as_dyn().database_name(), Some("md"));
        // SQLite's "database" is the on-disk path — same slot as PG/MySQL's database name.
        assert_eq!(sqlite().as_dyn().database_name(), Some("/tmp/x.sqlite"));
        assert_eq!(mongo().as_dyn().database_name(), None);
    }

    #[test]
    fn username_only_for_authenticated_variants() {
        assert_eq!(pg().as_dyn().username(), Some("u"));
        assert_eq!(mysql().as_dyn().username(), Some("mu"));
        assert_eq!(sqlite().as_dyn().username(), None);
        assert_eq!(http().as_dyn().username(), None);
    }

    #[test]
    fn ssl_mode_only_for_postgres_today() {
        assert_eq!(pg().as_dyn().ssl_mode(), Some("require"));
        assert_eq!(mysql().as_dyn().ssl_mode(), None);
        assert_eq!(sqlite().as_dyn().ssl_mode(), None);
    }

    #[test]
    fn password_distinguishes_unsupported_from_empty() {
        // PG / MySQL always return Some — even when empty — because
        // they have a password slot.
        assert_eq!(pg().as_dyn().password(), Some("p"));
        assert_eq!(mysql().as_dyn().password(), Some("mp"));
        // Variants without a password concept return None entirely
        // (caller treats as "no password applicable").
        assert_eq!(sqlite().as_dyn().password(), None);
        assert_eq!(mongo().as_dyn().password(), None);
        assert_eq!(shell().as_dyn().password(), None);
    }

    #[test]
    fn as_dyn_returns_the_right_impl_per_variant() {
        // Drives the as_dyn match for every variant; if a future
        // variant gets added but its arm is missing, this test still
        // compiles but exhaustiveness in `as_dyn` is enforced by the
        // compiler via the match. Keep this here as a sentinel.
        for c in [
            pg(),
            mysql(),
            sqlite(),
            mongo(),
            http(),
            ws(),
            grpc(),
            graphql(),
            bigquery(),
            shell(),
        ] {
            let _ = c.as_dyn().driver();
        }
    }
}
