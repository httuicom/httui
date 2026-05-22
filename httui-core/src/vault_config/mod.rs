//! Vault configuration files (TOML).
//!
//! Implements the schemas defined in ADR 0001:
//!
//! - `connections.toml` — connection definitions
//! - `envs/{name}.toml` — per-environment vars and secrets
//! - `.httui/workspace.toml` — workspace defaults
//! - `~/.config/httui/user.toml` — per-machine user prefs
//!
//! Plus `*.local.toml` overrides, handled by the merge layer (ADR 0004).

pub mod atomic;
pub mod connection_traits;
pub mod connection_views;
pub mod connections;
pub mod connections_store;
pub mod create;
pub mod environments_store;
pub mod envs;
pub mod error;
pub mod gitignore;
pub mod layout;
pub mod merge;
pub mod migration;
pub mod missing_secrets;
pub mod scaffold;
pub mod secret_resolver;
pub mod user;
pub mod user_store;
pub mod validate;
pub mod watch_paths;
pub mod workspace;
pub mod workspace_store;

pub use connection_views::ConnectionPublic;
pub use connections::{Connection, ConnectionsFile};
pub use connections_store::{ConnectionsStore, CreateConnectionInput, UpdateConnectionInput};
pub use environments_store::{
    EnvVariablePublic, EnvironmentPublic, EnvironmentsStore, SetVarInput,
};
pub use envs::{EnvFile, EnvMeta};
pub use error::{ConnectionsError, VaultConfigError};
pub use user::UserFile;
pub use user_store::UserStore;
pub use workspace::{
    Source, WorkspaceDefaults, WorkspaceDefaultsWithSources, WorkspaceFile, WorkspaceSources,
};
pub use workspace_store::WorkspaceStore;

use serde::{Deserialize, Serialize};

/// Schema version stamped at the top of every vault TOML file.
///
/// Bump only on breaking schema changes. Files without an explicit
/// `version` field default to `V1` (grandfathered for v1 itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Version {
    #[default]
    #[serde(rename = "1")]
    V1,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_default_is_v1() {
        assert_eq!(Version::default(), Version::V1);
    }

    #[derive(Serialize, Deserialize)]
    struct VersionedDoc {
        version: Version,
    }

    #[test]
    fn version_serializes_as_string_one() {
        // Whole-vault TOML files stamp `version = "1"`. Bumping the enum
        // discriminator without updating the rename would silently break
        // every existing vault, so this test pins the wire format.
        let toml_str = toml::to_string(&VersionedDoc {
            version: Version::V1,
        })
        .unwrap();
        assert!(toml_str.contains("version = \"1\""), "got: {toml_str}");
    }

    #[test]
    fn version_round_trips_through_toml() {
        let doc: VersionedDoc = toml::from_str("version = \"1\"").unwrap();
        assert_eq!(doc.version, Version::V1);
    }
}
