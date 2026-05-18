//! `envs/{name}.toml` schema.
//!
//! See ADR 0001. The split between `[vars]` (literals OK) and
//! `[secrets]` (must be `{{...}}` references) is structural. The
//! validator (story-02) enforces the constraint on `[secrets]`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Version;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EnvFile {
    #[serde(default)]
    pub version: Version,

    #[serde(default)]
    pub vars: BTreeMap<String, String>,

    #[serde(default)]
    pub secrets: BTreeMap<String, String>,

    #[serde(default)]
    pub meta: EnvMeta,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvMeta {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub read_only: bool,
    #[serde(default)]
    pub require_confirm: bool,
    #[serde(default)]
    pub color: Option<String>,

    /// Canvas §6 true marks the env as a throw-away (the
    /// UI surfaces a `temporary` chip; user cleanup remains manual).
    #[serde(default)]
    pub temporary: bool,

    /// Canvas §6 allowlist of connection IDs this env
    /// may target. Empty list (default) means "all connections in
    /// the vault are visible while this env is active".
    #[serde(default)]
    pub connections_used: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_env_file() {
        let raw = r#"
version = "1"

[vars]
BASE_URL = "https://api.staging.acme.dev"
TENANT_ID = "tnt_8f2a91"

[secrets]
ADMIN_TOKEN = "{{keychain:env:staging:ADMIN_TOKEN}}"
PG_PASSWORD = "{{keychain:env:staging:PG_PASSWORD}}"

[meta]
description = "Staging — Acme primary"
read_only = false
require_confirm = false
color = "amber"
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        assert_eq!(f.version, Version::V1);
        assert_eq!(
            f.vars.get("BASE_URL").unwrap(),
            "https://api.staging.acme.dev"
        );
        assert_eq!(
            f.secrets.get("ADMIN_TOKEN").unwrap(),
            "{{keychain:env:staging:ADMIN_TOKEN}}"
        );
        assert_eq!(f.meta.color.as_deref(), Some("amber"));
    }

    #[test]
    fn empty_sections_default() {
        let raw = r#"version = "1""#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        assert!(f.vars.is_empty());
        assert!(f.secrets.is_empty());
        assert!(!f.meta.read_only);
    }

    #[test]
    fn temporary_defaults_to_false() {
        let raw = r#"version = "1""#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        assert!(!f.meta.temporary);
    }

    #[test]
    fn temporary_round_trips_through_serde() {
        let raw = r#"
version = "1"
[meta]
temporary = true
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        assert!(f.meta.temporary);
        let back = toml::to_string(&f).unwrap();
        assert!(back.contains("temporary = true"));
    }

    #[test]
    fn connections_used_defaults_to_empty_meaning_all() {
        let raw = r#"version = "1""#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        assert!(f.meta.connections_used.is_empty());
    }

    #[test]
    fn connections_used_round_trips_through_serde() {
        let raw = r#"
version = "1"
[meta]
connections_used = ["pg-staging", "redis-cache"]
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        assert_eq!(
            f.meta.connections_used,
            vec!["pg-staging".to_string(), "redis-cache".to_string()],
        );
        let back = toml::to_string(&f).unwrap();
        assert!(back.contains("connections_used"));
        assert!(back.contains("pg-staging"));
    }

    #[test]
    fn pre_existing_envs_load_without_the_new_meta_fields() {
        // Lazy-migration: an env file written before has no
        // `temporary` or `connections_used` keys. It must still parse
        // and return defaults.
        let raw = r#"
version = "1"
[vars]
BASE_URL = "x"
[meta]
description = "older shape"
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        assert_eq!(f.meta.description.as_deref(), Some("older shape"));
        assert!(!f.meta.temporary);
        assert!(f.meta.connections_used.is_empty());
    }

    #[test]
    fn vars_and_secrets_are_independent() {
        let raw = r#"
version = "1"
[vars]
A = "literal"
[secrets]
B = "{{keychain:ns:k}}"
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        assert_eq!(f.vars.len(), 1);
        assert_eq!(f.secrets.len(), 1);
    }
}
