//! Public DTOs returned by `EnvironmentsStore` plus the small
//! conversion helpers that produce them.

use serde::Serialize;

use crate::vault_config::envs::EnvFile;

#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentPublic {
    pub name: String,
    pub description: Option<String>,
    pub read_only: bool,
    pub require_confirm: bool,
    pub color: Option<String>,
    pub var_count: usize,
    pub secret_count: usize,
    /// Canvas §6 `[meta].temporary`. Drives the
    /// `temporary` chip in the Environments page.
    pub temporary: bool,
    /// Canvas §6 `[meta].connections_used` allowlist.
    /// Empty list means "all connections".
    pub connections_used: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnvVariablePublic {
    pub key: String,
    /// Plaintext value when from `[vars]`; empty string when from
    /// `[secrets]` (caller resolves on demand).
    pub value: String,
    pub is_secret: bool,
}

#[derive(Debug, Clone)]
pub struct SetVarInput {
    pub env_name: String,
    pub key: String,
    /// Raw value. For secret vars it's stored in the keychain and the
    /// TOML keeps only a `{{keychain:...}}` reference.
    ///
    /// Empty + `is_secret=true` is a sentinel: "preserve the existing
    /// keychain entry" (same rule as connection passwords). Errors on
    /// create because there's nothing to preserve.
    pub value: String,
    pub is_secret: bool,
}

pub(super) fn env_to_public(name: &str, file: &EnvFile) -> EnvironmentPublic {
    EnvironmentPublic {
        name: name.to_string(),
        description: file.meta.description.clone(),
        read_only: file.meta.read_only,
        require_confirm: file.meta.require_confirm,
        color: file.meta.color.clone(),
        var_count: file.vars.len(),
        secret_count: file.secrets.len(),
        temporary: file.meta.temporary,
        connections_used: file.meta.connections_used.clone(),
    }
}

pub(super) fn is_valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault_config::envs::{EnvFile, EnvMeta};

    #[test]
    fn env_to_public_propagates_meta_and_counts() {
        let mut file = EnvFile::default();
        file.meta = EnvMeta {
            description: Some("d".into()),
            read_only: true,
            require_confirm: true,
            color: Some("red".into()),
            temporary: true,
            connections_used: vec!["c1".into()],
        };
        file.vars.insert("a".into(), "1".into());
        file.vars.insert("b".into(), "2".into());
        file.secrets.insert("s".into(), "{{ref}}".into());
        let p = env_to_public("dev", &file);
        assert_eq!(p.name, "dev");
        assert_eq!(p.var_count, 2);
        assert_eq!(p.secret_count, 1);
        assert!(p.read_only);
        assert!(p.temporary);
        assert_eq!(p.connections_used, vec!["c1".to_string()]);
    }

    #[test]
    fn is_valid_env_name_accepts_alnum_dash_underscore() {
        assert!(is_valid_env_name("dev"));
        assert!(is_valid_env_name("prod-1"));
        assert!(is_valid_env_name("test_env"));
        assert!(is_valid_env_name("ABC123"));
    }

    #[test]
    fn is_valid_env_name_rejects_empty_and_special_chars() {
        assert!(!is_valid_env_name(""));
        assert!(!is_valid_env_name("dev/prod"));
        assert!(!is_valid_env_name("env name"));
        assert!(!is_valid_env_name("env.toml"));
        assert!(!is_valid_env_name("ç"));
    }
}
