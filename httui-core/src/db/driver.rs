//! Typed enum for the SQL drivers we pool — replaces the ad-hoc
//! `driver: String` shape that accepted any literal at the boundary
//! and only blew up at validation time.
//!
//! Surface:
//!
//! - [`DbDriver`] — `Postgres | Mysql | Sqlite`. Serde reads/writes
//!   lowercase strings (`"postgres"` etc.) so existing TOML and the
//!   legacy `db::connections::Connection.driver: String` keep
//!   parsing unchanged.
//! - [`DbDriver::from_str`] — backward-compat parser; accepts the
//!   same lowercase strings as serde.
//! - [`DbDriver::as_str`] — canonical lowercase name, the value the
//!   trait's `DbConnection::driver()` returns and what's written to
//!   `connections.toml` `type = "..."`.
//!
//! Non-DB connection variants (HTTP, WS, gRPC, …) live on the
//! `Connection` enum but don't go through the SQL pool, so they
//! aren't represented here. `DbDriver::from_str` returns an `Err`
//! for them.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DbDriver {
    Postgres,
    Mysql,
    Sqlite,
}

impl DbDriver {
    /// Canonical lowercase name. Matches the serde tag and the
    /// `type = "..."` value written into `connections.toml`.
    pub fn as_str(&self) -> &'static str {
        match self {
            DbDriver::Postgres => "postgres",
            DbDriver::Mysql => "mysql",
            DbDriver::Sqlite => "sqlite",
        }
    }
}

impl fmt::Display for DbDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DbDriver {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "postgres" => Ok(DbDriver::Postgres),
            "mysql" => Ok(DbDriver::Mysql),
            "sqlite" => Ok(DbDriver::Sqlite),
            other => Err(format!("unsupported driver: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_accepts_lowercase_canonical_names() {
        assert_eq!(DbDriver::from_str("postgres").unwrap(), DbDriver::Postgres);
        assert_eq!(DbDriver::from_str("mysql").unwrap(), DbDriver::Mysql);
        assert_eq!(DbDriver::from_str("sqlite").unwrap(), DbDriver::Sqlite);
    }

    #[test]
    fn from_str_rejects_uppercase() {
        // Mixed/upper cases are rejected — TOML serde reads lowercase
        // by contract; users hand-editing should normalize.
        assert!(DbDriver::from_str("Postgres").is_err());
        assert!(DbDriver::from_str("POSTGRES").is_err());
    }

    #[test]
    fn from_str_rejects_non_db_drivers() {
        // HTTP / mongo / gRPC are valid Connection variants but not
        // pool-backed, so DbDriver::from_str must reject them.
        for s in [
            "http", "mongo", "ws", "grpc", "graphql", "bigquery", "shell",
        ] {
            let err = DbDriver::from_str(s).expect_err(s);
            assert!(err.contains("unsupported driver"));
        }
    }

    #[test]
    fn from_str_rejects_empty_and_garbage() {
        assert!(DbDriver::from_str("").is_err());
        assert!(DbDriver::from_str("weirdb").is_err());
    }

    #[test]
    fn as_str_round_trips_through_from_str() {
        for d in [DbDriver::Postgres, DbDriver::Mysql, DbDriver::Sqlite] {
            let s = d.as_str();
            assert_eq!(DbDriver::from_str(s).unwrap(), d);
        }
    }

    #[test]
    fn display_matches_as_str() {
        assert_eq!(format!("{}", DbDriver::Postgres), "postgres");
        assert_eq!(format!("{}", DbDriver::Mysql), "mysql");
        assert_eq!(format!("{}", DbDriver::Sqlite), "sqlite");
    }

    #[test]
    fn serde_round_trips_lowercase() {
        // Pin the wire format — TOML / JSON / etc. all see lowercase
        // strings, never variant casing.
        let json = serde_json::to_string(&DbDriver::Postgres).unwrap();
        assert_eq!(json, "\"postgres\"");
        let parsed: DbDriver = serde_json::from_str("\"sqlite\"").unwrap();
        assert_eq!(parsed, DbDriver::Sqlite);
    }
}
