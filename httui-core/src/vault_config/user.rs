//! `~/.config/httui/user.toml` schema.
//!
//! See ADR 0001. Per-machine, never synced. Holds visual prefs,
//! shortcuts, secrets backend choice.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Version;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserFile {
    #[serde(default)]
    pub version: Version,

    #[serde(default)]
    pub ui: UiPrefs,

    #[serde(default)]
    pub shortcuts: BTreeMap<String, String>,

    #[serde(default)]
    pub secrets: SecretsBackend,

    #[serde(default)]
    pub mcp: McpConfig,

    /// Active environment per vault, keyed by absolute vault path.
    /// Per-machine state — never committed to git. Read by
    /// `EnvironmentsStore::active_env(vault_path)`.
    #[serde(default)]
    pub active_envs: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiPrefs {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default = "default_font_size")]
    pub font_size: u16,
    #[serde(default = "default_density")]
    pub density: String,
    /// Editor auto-save debounce window in milliseconds.
    /// MVP `app_config` key: `auto_save_ms`.
    #[serde(default = "default_auto_save_ms")]
    pub auto_save_ms: u32,
    /// DB block default `LIMIT` when the user hasn't explicitly
    /// pinned one. MVP `app_config` key: `default_fetch_size`.
    #[serde(default = "default_fetch_size")]
    pub default_fetch_size: u32,
    /// Per-block history retention cap. MVP `app_config` key:
    /// `history_retention`.
    #[serde(default = "default_history_retention")]
    pub history_retention: u32,
    /// Editor vim-mode toggle. MVP `app_config` key: `vim_enabled`.
    #[serde(default)]
    pub vim_enabled: bool,
    /// Sidebar open/closed. MVP `app_config` key: `sidebar_open`.
    #[serde(default = "default_sidebar_open")]
    pub sidebar_open: bool,
    /// Git side-panel (VS-Code-style SCM column) open/closed.
    /// V10.1 cenário 1 — persists across launches. Default closed.
    #[serde(default)]
    pub git_side_panel_open: bool,
    /// Commit-message template for the git side panel. V10.1
    /// cenário 8. Empty = use the built-in conditional default
    /// ("Update <stem>" / "Update N notes"). Placeholders:
    /// `{{notes}}`, `{{count}}`, `{{date}}`.
    #[serde(default)]
    pub git_commit_template: String,
    /// Color mode: `"system" | "light" | "dark"`. Frontend wires it
    /// to Chakra/next-themes via `<ColorModeSync>`. Separate from
    /// `theme` (legacy customisation JSON pending Epic 19 sweep).
    #[serde(default = "default_color_mode")]
    pub color_mode: String,
    /// True when the user has dismissed the MVP-to-v1 migration
    /// banner. Surfaced in the Empty state when a legacy `notes.db`
    /// is detected without a `.httui/` v1 layout. Once dismissed,
    /// the banner stays hidden across launches (Epic 41 Story 07
    /// carry).
    #[serde(default)]
    pub mvp_migration_dismissed: bool,
    /// True when quick-open should hide files whose frontmatter
    /// `status:` is `archived`. Default `false` (archived files
    /// still surface). Epic 52 Story 06 task 3 — the toggle the
    /// settings UI flips. The actual filter applies inside the
    /// quick-open ranker; this flag only persists the user's
    /// choice.
    #[serde(default)]
    pub hide_archived_in_quick_open: bool,
    /// Keyboard-shortcut profile selected in the Settings UI. One of
    /// `"default" | "vim" | "vscode" | "jetbrains"`. V3 ships
    /// `default` and `vim` functional; `vscode` / `jetbrains` are
    /// surfaced disabled in the UI ("coming soon") and resolve to
    /// `default` behavior at runtime if persisted somehow.
    #[serde(default = "default_shortcut_profile")]
    pub shortcut_profile: String,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            font_family: default_font_family(),
            font_size: default_font_size(),
            density: default_density(),
            auto_save_ms: default_auto_save_ms(),
            default_fetch_size: default_fetch_size(),
            history_retention: default_history_retention(),
            vim_enabled: false,
            sidebar_open: default_sidebar_open(),
            git_side_panel_open: false,
            git_commit_template: String::new(),
            color_mode: default_color_mode(),
            mvp_migration_dismissed: false,
            hide_archived_in_quick_open: false,
            shortcut_profile: default_shortcut_profile(),
        }
    }
}

fn default_theme() -> String {
    "system".to_string()
}
fn default_font_family() -> String {
    "JetBrains Mono".to_string()
}
fn default_font_size() -> u16 {
    14
}
fn default_density() -> String {
    "comfortable".to_string()
}
fn default_auto_save_ms() -> u32 {
    1000
}
fn default_fetch_size() -> u32 {
    100
}
fn default_history_retention() -> u32 {
    10
}
fn default_sidebar_open() -> bool {
    true
}
fn default_color_mode() -> String {
    "system".to_string()
}
fn default_shortcut_profile() -> String {
    "default".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsBackend {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_biometric")]
    pub biometric: bool,
    #[serde(default = "default_prompt_timeout")]
    pub prompt_timeout_s: u32,
}

impl Default for SecretsBackend {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            biometric: default_biometric(),
            prompt_timeout_s: default_prompt_timeout(),
        }
    }
}

fn default_backend() -> String {
    "auto".to_string()
}
fn default_biometric() -> bool {
    true
}
fn default_prompt_timeout() -> u32 {
    60
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub servers: BTreeMap<String, toml::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_user_file() {
        let raw = r#"
version = "1"

[ui]
theme = "dark"
font_family = "Fira Code"
font_size = 13
density = "compact"

[shortcuts]
"toggle.sidebar" = "Cmd+B"

[secrets]
backend = "1password"
biometric = true
prompt_timeout_s = 30
"#;
        let f: UserFile = toml::from_str(raw).unwrap();
        assert_eq!(f.ui.theme, "dark");
        assert_eq!(f.ui.font_family, "Fira Code");
        assert_eq!(f.ui.font_size, 13);
        assert_eq!(f.shortcuts.get("toggle.sidebar").unwrap(), "Cmd+B");
        assert_eq!(f.secrets.backend, "1password");
    }

    #[test]
    fn empty_user_file_yields_defaults() {
        let f: UserFile = toml::from_str("").unwrap();
        assert_eq!(f.ui.theme, "system");
        assert_eq!(f.ui.font_size, 14);
        assert_eq!(f.ui.color_mode, "system");
        assert_eq!(f.secrets.backend, "auto");
        assert!(f.secrets.biometric);
        assert!(f.active_envs.is_empty());
    }

    #[test]
    fn color_mode_round_trips() {
        let raw = "version = \"1\"\n[ui]\ncolor_mode = \"dark\"\n";
        let f: UserFile = toml::from_str(raw).unwrap();
        assert_eq!(f.ui.color_mode, "dark");

        let serialized = toml::to_string(&f).unwrap();
        assert!(serialized.contains("color_mode = \"dark\""));

        let back: UserFile = toml::from_str(&serialized).unwrap();
        assert_eq!(back.ui.color_mode, "dark");
    }

    #[test]
    fn ui_prefs_default_populates_every_field() {
        let p = UiPrefs::default();
        assert_eq!(p.theme, "system");
        assert_eq!(p.font_family, "JetBrains Mono");
        assert_eq!(p.font_size, 14);
        assert_eq!(p.density, "comfortable");
        assert_eq!(p.auto_save_ms, 1000);
        assert_eq!(p.default_fetch_size, 100);
        assert_eq!(p.history_retention, 10);
        assert!(!p.vim_enabled);
        assert!(p.sidebar_open);
        assert!(!p.git_side_panel_open);
        assert_eq!(p.git_commit_template, "");
        assert_eq!(p.color_mode, "system");
        assert!(!p.mvp_migration_dismissed);
        assert!(!p.hide_archived_in_quick_open);
        assert_eq!(p.shortcut_profile, "default");
    }

    #[test]
    fn shortcut_profile_round_trips() {
        let raw = "version = \"1\"\n[ui]\nshortcut_profile = \"vim\"\n";
        let f: UserFile = toml::from_str(raw).unwrap();
        assert_eq!(f.ui.shortcut_profile, "vim");

        let serialized = toml::to_string(&f).unwrap();
        assert!(serialized.contains("shortcut_profile = \"vim\""));

        let back: UserFile = toml::from_str(&serialized).unwrap();
        assert_eq!(back.ui.shortcut_profile, "vim");
    }

    #[test]
    fn shortcut_profile_defaults_when_omitted() {
        let raw = "version = \"1\"\n[ui]\ntheme = \"dark\"\n";
        let f: UserFile = toml::from_str(raw).unwrap();
        assert_eq!(f.ui.shortcut_profile, "default");
    }

    #[test]
    fn mvp_migration_dismissed_round_trips() {
        let raw = "version = \"1\"\n[ui]\nmvp_migration_dismissed = true\n";
        let f: UserFile = toml::from_str(raw).unwrap();
        assert!(f.ui.mvp_migration_dismissed);

        let serialized = toml::to_string(&f).unwrap();
        assert!(serialized.contains("mvp_migration_dismissed = true"));

        let back: UserFile = toml::from_str(&serialized).unwrap();
        assert!(back.ui.mvp_migration_dismissed);
    }

    #[test]
    fn mvp_migration_dismissed_defaults_to_false_when_omitted() {
        let raw = "version = \"1\"\n[ui]\ntheme = \"dark\"\n";
        let f: UserFile = toml::from_str(raw).unwrap();
        assert!(!f.ui.mvp_migration_dismissed);
    }

    #[test]
    fn hide_archived_in_quick_open_round_trips() {
        let raw =
            "version = \"1\"\n[ui]\nhide_archived_in_quick_open = true\n";
        let f: UserFile = toml::from_str(raw).unwrap();
        assert!(f.ui.hide_archived_in_quick_open);

        let serialized = toml::to_string(&f).unwrap();
        assert!(serialized.contains("hide_archived_in_quick_open = true"));

        let back: UserFile = toml::from_str(&serialized).unwrap();
        assert!(back.ui.hide_archived_in_quick_open);
    }

    #[test]
    fn hide_archived_in_quick_open_defaults_to_false_when_omitted() {
        let raw = "version = \"1\"\n[ui]\ntheme = \"dark\"\n";
        let f: UserFile = toml::from_str(raw).unwrap();
        assert!(!f.ui.hide_archived_in_quick_open);
    }

    #[test]
    fn secrets_backend_default_matches_documented_values() {
        let b = SecretsBackend::default();
        assert_eq!(b.backend, "auto");
        assert!(b.biometric);
        assert_eq!(b.prompt_timeout_s, 60);
    }

    #[test]
    fn empty_user_file_serialises_back_to_default_round_trip() {
        let original: UserFile = toml::from_str("").unwrap();
        let serialised = toml::to_string(&original).unwrap();
        let reparsed: UserFile = toml::from_str(&serialised).unwrap();
        assert_eq!(reparsed.ui.theme, "system");
        assert_eq!(reparsed.ui.color_mode, "system");
        assert_eq!(reparsed.secrets.backend, "auto");
    }

    #[test]
    fn each_default_fn_returns_documented_value() {
        // serde's `#[serde(default = "fn_name")]` calls each fn via
        // the function-pointer path; coverage tools sometimes miss
        // those hits, so call them directly. Doubles as a contract
        // check that the documented defaults haven't drifted.
        assert_eq!(default_theme(), "system");
        assert_eq!(default_font_family(), "JetBrains Mono");
        assert_eq!(default_font_size(), 14);
        assert_eq!(default_density(), "comfortable");
        assert_eq!(default_auto_save_ms(), 1000);
        assert_eq!(default_fetch_size(), 100);
        assert_eq!(default_history_retention(), 10);
        assert!(default_sidebar_open());
        assert_eq!(default_color_mode(), "system");
        assert_eq!(default_shortcut_profile(), "default");
        assert_eq!(default_backend(), "auto");
        assert!(default_biometric());
        assert_eq!(default_prompt_timeout(), 60);
    }

    #[test]
    fn derived_traits_compile_and_run() {
        // Exercises the macro-generated Debug + Clone impls so the
        // `#[derive(...)]` lines are hit by the coverage tool. Cheap
        // smoke that catches accidental drift in the derive set.
        let p = UiPrefs::default();
        let cloned = p.clone();
        assert_eq!(p.theme, cloned.theme);
        let debug = format!("{p:?}");
        assert!(debug.contains("UiPrefs"));

        let s = SecretsBackend::default();
        let s_clone = s.clone();
        assert_eq!(s.backend, s_clone.backend);
        let s_debug = format!("{s:?}");
        assert!(s_debug.contains("SecretsBackend"));

        let m = McpConfig::default();
        let m_clone = m.clone();
        assert_eq!(m.servers.len(), m_clone.servers.len());
        let m_debug = format!("{m:?}");
        assert!(m_debug.contains("McpConfig"));

        let f = UserFile::default();
        let f_clone = f.clone();
        assert_eq!(f.ui.theme, f_clone.ui.theme);
        let f_debug = format!("{f:?}");
        assert!(f_debug.contains("UserFile"));
    }

    #[test]
    fn mcp_config_round_trips_servers_table() {
        let raw = r#"
version = "1"
[mcp.servers."notes-mcp"]
command = "httui-mcp"
"#;
        let f: UserFile = toml::from_str(raw).unwrap();
        assert!(f.mcp.servers.contains_key("notes-mcp"));

        let mcp_default = McpConfig::default();
        assert!(mcp_default.servers.is_empty());
    }

    #[test]
    fn active_envs_round_trip() {
        let raw = r#"
version = "1"
[active_envs]
"/Users/me/work" = "staging"
"/Users/me/personal" = "local"
"#;
        let f: UserFile = toml::from_str(raw).unwrap();
        assert_eq!(f.active_envs.get("/Users/me/work").unwrap(), "staging");
        assert_eq!(f.active_envs.get("/Users/me/personal").unwrap(), "local");

        let serialized = toml::to_string(&f).unwrap();
        let reparsed: UserFile = toml::from_str(&serialized).unwrap();
        assert_eq!(reparsed.active_envs.len(), 2);
    }
}
