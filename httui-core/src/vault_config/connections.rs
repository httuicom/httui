//! `connections.toml` schema.
//!
//! See ADR 0001 for the full contract. Any string field MAY be a
//! `{{...}}` reference (ADR 0002); the validator warns when
//! sensitive-named fields hold literal values.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Version;

/// Top-level shape of `connections.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionsFile {
    #[serde(default)]
    pub version: Version,

    #[serde(default)]
    pub connections: BTreeMap<String, Connection>,
}

/// One connection. Tagged by `type`; per-type fields live in the variants.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Connection {
    Postgres(PostgresConfig),
    Mysql(MysqlConfig),
    Sqlite(SqliteConfig),
    Mongo(MongoConfig),
    Http(HttpConfig),
    Ws(WsConfig),
    Grpc(GrpcConfig),
    Graphql(GraphqlConfig),
    Bigquery(BigqueryConfig),
    Shell(ShellConfig),
}

/// Fields shared by every connection variant. Flattened into each one.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CommonFields {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    #[serde(default)]
    pub ssl_mode: Option<String>,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MysqlConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub password: String,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    /// Filesystem path to the SQLite database. Supports `~` expansion
    /// at the resolver layer; raw value is stored verbatim.
    pub path: String,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoConfig {
    pub uri: String,
    #[serde(default)]
    pub auth: Option<String>,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    pub base_url: String,
    #[serde(default)]
    pub default_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsConfig {
    pub url: String,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcConfig {
    pub endpoint: String,
    #[serde(default)]
    pub tls: bool,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphqlConfig {
    pub endpoint: String,
    #[serde(default)]
    pub default_headers: BTreeMap<String, String>,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BigqueryConfig {
    pub project_id: String,
    pub credentials_path: String,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConfig {
    pub shell: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(flatten)]
    pub common: CommonFields,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_postgres_with_secret_refs() {
        let raw = r#"
version = "1"

[connections.payments-staging]
type = "postgres"
host = "pg-staging.acme.local"
port = 5432
database = "payments"
user = "{{keychain:payments-staging:user}}"
password = "{{keychain:payments-staging:password}}"
ssl_mode = "require"
"#;
        let f: ConnectionsFile = toml::from_str(raw).expect("parse");
        assert_eq!(f.version, Version::V1);
        let conn = f.connections.get("payments-staging").unwrap();
        match conn {
            Connection::Postgres(pg) => {
                assert_eq!(pg.host, "pg-staging.acme.local");
                assert_eq!(pg.user, "{{keychain:payments-staging:user}}");
                assert_eq!(pg.ssl_mode.as_deref(), Some("require"));
            }
            _ => panic!("expected postgres variant"),
        }
    }

    #[test]
    fn parses_http_with_default_headers() {
        let raw = r#"
version = "1"

[connections.payments-api]
type = "http"
base_url = "https://api.example.com"
default_headers = { "X-Tenant" = "{{TENANT_ID}}" }
timeout_ms = 30000
"#;
        let f: ConnectionsFile = toml::from_str(raw).expect("parse");
        let conn = f.connections.get("payments-api").unwrap();
        match conn {
            Connection::Http(h) => {
                assert_eq!(h.base_url, "https://api.example.com");
                assert_eq!(h.default_headers.get("X-Tenant").unwrap(), "{{TENANT_ID}}");
                assert_eq!(h.timeout_ms, Some(30000));
            }
            _ => panic!("expected http variant"),
        }
    }

    #[test]
    fn parses_sqlite() {
        let raw = r#"
version = "1"
[connections.local-cache]
type = "sqlite"
path = "/Users/me/cache.db"
"#;
        let f: ConnectionsFile = toml::from_str(raw).expect("parse");
        match f.connections.get("local-cache").unwrap() {
            Connection::Sqlite(s) => assert_eq!(s.path, "/Users/me/cache.db"),
            _ => panic!("expected sqlite variant"),
        }
    }

    #[test]
    fn parses_all_ten_variants() {
        let raw = r#"
version = "1"

[connections.pg]
type = "postgres"
host = "h"
port = 5432
database = "d"
user = "u"
password = "p"

[connections.my]
type = "mysql"
host = "h"
port = 3306
database = "d"
user = "u"
password = "p"

[connections.sq]
type = "sqlite"
path = "/tmp/db.sqlite"

[connections.mo]
type = "mongo"
uri = "mongodb://localhost"

[connections.ht]
type = "http"
base_url = "https://x"

[connections.ws]
type = "ws"
url = "wss://x"

[connections.gr]
type = "grpc"
endpoint = "x:443"

[connections.gq]
type = "graphql"
endpoint = "https://x/graphql"

[connections.bq]
type = "bigquery"
project_id = "p"
credentials_path = "{{keychain:bq:creds}}"

[connections.sh]
type = "shell"
shell = "bash"
"#;
        let f: ConnectionsFile = toml::from_str(raw).expect("parse");
        assert_eq!(f.connections.len(), 10);
    }

    #[test]
    fn rejects_unknown_connection_type() {
        let raw = r#"
version = "1"
[connections.bogus]
type = "weirdb"
host = "h"
"#;
        let err = toml::from_str::<ConnectionsFile>(raw).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("weirdb") || msg.contains("unknown variant"),
            "expected unknown-variant error, got: {msg}"
        );
    }

    #[test]
    fn round_trip_preserves_struct_data() {
        let raw = r#"
version = "1"
[connections.pg]
type = "postgres"
host = "h"
port = 5432
database = "d"
user = "u"
password = "p"
"#;
        let parsed: ConnectionsFile = toml::from_str(raw).unwrap();
        let serialized = toml::to_string(&parsed).unwrap();
        let reparsed: ConnectionsFile = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed.connections.len(), reparsed.connections.len());
    }
}
