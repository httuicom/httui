//! `.httui/workspace.toml` schema.
//!
//! See ADR 0001. Strictly limited to collaboration-relevant defaults.
//! Visual settings live in `user.toml`, not here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Version;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceFile {
    #[serde(default)]
    pub version: Version,

    #[serde(default)]
    pub defaults: WorkspaceDefaults,

    /// Per-file collaboration-relevant settings keyed by vault-relative
    /// path. Empty / unset entries serialize away — only files with
    /// non-default settings show up on disk.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub files: BTreeMap<String, FileSettings>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceDefaults {
    #[serde(default)]
    pub environment: Option<String>,
    #[serde(default)]
    pub git_remote: Option<String>,
    #[serde(default)]
    pub git_branch: Option<String>,
    /// Human-friendly vault label shown in the workspace switcher.
    /// Falls back to the directory name when unset. Per-vault committed
    /// preference — every collaborator sees the same name.
    #[serde(default)]
    pub display_name: Option<String>,
}

/// Per-field origin marker returned alongside [`WorkspaceDefaults`] by
/// [`super::workspace_store::WorkspaceStore::defaults_with_sources`].
/// Powers the "overridden locally" badge in the Settings UI (V3
/// cenário 3): a `Local` source means the value lives in
/// `workspace.local.toml`, not the committed base file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    #[default]
    Workspace,
    Local,
}

/// Origin of every field in [`WorkspaceDefaults`]. One entry per field
/// — exhaustive on purpose so the UI can't accidentally forget a
/// badge when new fields land.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceSources {
    #[serde(default)]
    pub environment: Source,
    #[serde(default)]
    pub git_remote: Source,
    #[serde(default)]
    pub git_branch: Source,
    #[serde(default)]
    pub display_name: Source,
}

/// Bundle returned by `defaults_with_sources` so the frontend gets the
/// values + their origin in a single round-trip.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkspaceDefaultsWithSources {
    pub defaults: WorkspaceDefaults,
    pub sources: WorkspaceSources,
}

/// Per-file settings persisted in `[files."path/to/note.md"]` blocks
/// in workspace.toml. Each field carries `#[serde(default)]` so the
/// table only needs to spell out the values that diverge from the
/// defaults — keeping the file human-reviewable in PRs.
///
/// Default-valued instances are pruned from `WorkspaceFile.files` on
/// write so empty `[files."x"]` headers don't accumulate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSettings {
    /// Whether the editor toolbar's auto-capture toggle is on for this
    /// note. Off by default — captures are an opt-in surface.
    #[serde(default, skip_serializing_if = "is_default_bool")]
    pub auto_capture: bool,
    /// Whether the DocHeader card is in compact mode (only H1 + meta
    /// strip visible). Off by default. Click-on-title in
    /// `<DocHeaderCard>` toggles this; persistence keeps the
    /// preference per-file across reopen.
    #[serde(default, skip_serializing_if = "is_default_bool")]
    pub docheader_compact: bool,
}

fn is_default_bool(b: &bool) -> bool {
    !*b
}

impl FileSettings {
    /// True when the struct holds nothing distinguishable from the
    /// `Default`-derived value. Used by the store to prune unset
    /// entries from disk on write.
    pub fn is_default(&self) -> bool {
        self == &FileSettings::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_workspace() {
        let raw = r#"
version = "1"
[defaults]
environment = "staging"
git_remote = "origin"
git_branch = "main"
display_name = "Payments"
"#;
        let f: WorkspaceFile = toml::from_str(raw).unwrap();
        assert_eq!(f.defaults.environment.as_deref(), Some("staging"));
        assert_eq!(f.defaults.git_remote.as_deref(), Some("origin"));
        assert_eq!(f.defaults.git_branch.as_deref(), Some("main"));
        assert_eq!(f.defaults.display_name.as_deref(), Some("Payments"));
    }

    #[test]
    fn display_name_round_trips() {
        let mut f = WorkspaceFile::default();
        f.defaults.display_name = Some("Payments".into());
        let raw = toml::to_string(&f).unwrap();
        assert!(raw.contains("display_name = \"Payments\""));
        let back: WorkspaceFile = toml::from_str(&raw).unwrap();
        assert_eq!(back.defaults.display_name.as_deref(), Some("Payments"));
    }

    #[test]
    fn display_name_omitted_when_none() {
        let f = WorkspaceFile::default();
        let raw = toml::to_string(&f).unwrap();
        assert!(!raw.contains("display_name"), "got: {raw}");
    }

    #[test]
    fn source_default_is_workspace() {
        assert_eq!(Source::default(), Source::Workspace);
    }

    #[test]
    fn source_serializes_lowercase() {
        let raw = toml::to_string(&WorkspaceSources {
            environment: Source::Local,
            git_remote: Source::Workspace,
            git_branch: Source::Workspace,
            display_name: Source::Local,
        })
        .unwrap();
        assert!(raw.contains("environment = \"local\""), "got: {raw}");
        assert!(raw.contains("git_remote = \"workspace\""), "got: {raw}");
    }

    #[test]
    fn source_round_trips_via_serde() {
        let s = WorkspaceSources {
            environment: Source::Local,
            git_remote: Source::Workspace,
            git_branch: Source::Local,
            display_name: Source::Workspace,
        };
        let serialized = serde_json::to_string(&s).unwrap();
        let back: WorkspaceSources = serde_json::from_str(&serialized).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn empty_workspace_defaults_to_v1() {
        let f: WorkspaceFile = toml::from_str(r#"version = "1""#).unwrap();
        assert_eq!(f.version, Version::V1);
        assert!(f.defaults.environment.is_none());
    }

    #[test]
    fn parses_per_file_settings() {
        let raw = r#"
version = "1"
[files."rollout-v2.3.md"]
auto_capture = true

[files."health-check.md"]
auto_capture = false
"#;
        let f: WorkspaceFile = toml::from_str(raw).unwrap();
        assert!(f.files.get("rollout-v2.3.md").unwrap().auto_capture);
        assert!(!f.files.get("health-check.md").unwrap().auto_capture);
    }

    #[test]
    fn missing_files_table_defaults_to_empty() {
        let f: WorkspaceFile = toml::from_str(r#"version = "1""#).unwrap();
        assert!(f.files.is_empty());
    }

    #[test]
    fn empty_files_table_skipped_on_serialize() {
        let f = WorkspaceFile::default();
        let raw = toml::to_string(&f).unwrap();
        assert!(!raw.contains("[files"), "got: {raw}");
    }

    #[test]
    fn file_settings_is_default_recognises_default_value() {
        assert!(FileSettings::default().is_default());
        assert!(
            !FileSettings {
                auto_capture: true,
                docheader_compact: false,
            }
            .is_default()
        );
        assert!(
            !FileSettings {
                auto_capture: false,
                docheader_compact: true,
            }
            .is_default()
        );
    }

    #[test]
    fn parses_docheader_compact_per_file() {
        let raw = r#"
version = "1"
[files."notes/db.md"]
docheader_compact = true
"#;
        let f: WorkspaceFile = toml::from_str(raw).unwrap();
        assert!(f.files.get("notes/db.md").unwrap().docheader_compact);
    }

    #[test]
    fn docheader_compact_false_omitted_from_serialize() {
        // A file with auto_capture=true + docheader_compact=false (the
        // default) should not write the compact key at all — keeps the
        // TOML clean.
        let mut f = WorkspaceFile::default();
        f.files.insert(
            "x.md".into(),
            FileSettings {
                auto_capture: true,
                docheader_compact: false,
            },
        );
        let raw = toml::to_string(&f).unwrap();
        assert!(raw.contains("auto_capture"));
        assert!(!raw.contains("docheader_compact"), "got: {raw}");
    }

    #[test]
    fn docheader_compact_true_round_trips() {
        let mut f = WorkspaceFile::default();
        f.files.insert(
            "x.md".into(),
            FileSettings {
                auto_capture: false,
                docheader_compact: true,
            },
        );
        let raw = toml::to_string(&f).unwrap();
        let back: WorkspaceFile = toml::from_str(&raw).unwrap();
        assert!(back.files.get("x.md").unwrap().docheader_compact);
        assert!(!back.files.get("x.md").unwrap().auto_capture);
    }
}
