use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{TuiError, TuiResult};

/// Persisted user configuration. Mirrors `docs/tui-design.md` §11.2.
///
/// Every field is `#[serde(default)]`, so partial files (or future
/// additions) load gracefully — missing keys take the default.
///
/// Vault state (registered vaults + active selection) lives in the
/// shared SQLite app_config (see `httui_core::vaults`), not here —
/// that's how the desktop and the TUI converge on the same workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub sidebar_default_visible: bool,
    pub sidebar_width: u16,
    pub auto_save_debounce_ms: u64,
    pub mouse_enabled: bool,

    pub ui: UiConfig,
    pub blocks: BlocksConfig,
    pub chat: ChatConfig,
    pub editor: EditorConfig,
    pub keymap: KeymapConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub show_line_numbers: bool,
    pub show_relative_numbers: bool,
    pub font_features: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BlocksConfig {
    pub default_display_mode: String,
    pub auto_run_on_cached_miss: bool,
    /// How navigation flows between a block's SQL body and its result
    /// table. Accepted values:
    /// - `"flow"` (default) — `j`/`k` cross from the last SQL line
    ///   into the first result row and back, like prose.
    /// - `"tab"` — `Tab` key alternates focus; `j`/`k` stay in the
    ///   active section (planned, currently behaves like `flow`).
    /// - `"scroll-key"` — `Ctrl+J`/`Ctrl+K` scroll the result while
    ///   `j`/`k` skip past the result entirely (planned, currently
    ///   behaves like `flow`).
    pub result_nav: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatConfig {
    pub enabled: bool,
}

/// Editor input profile.
///
/// `Standard` (the default) is the conventional editor model — arrow
/// keys, `Ctrl+Z`/`Ctrl+Y` undo/redo, `Ctrl+C`/`X`/`V` clipboard,
/// `Shift+arrow` selection. `Vim` opts into the modal engine. Vim is
/// opt-in by design: the default UX must not assume vim knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorMode {
    #[default]
    Standard,
    Vim,
}

/// Default chord for the vim↔standard toggle. An F-key so it works in
/// every terminal regardless of the kitty keyboard protocol —
/// `Ctrl+Shift+<letter>` collapses to a single modifier on terminals
/// without it. Parsed by `crate::input::keychord::parse_key_chord`.
fn default_toggle_mode_key() -> String {
    "f2".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EditorConfig {
    pub mode: EditorMode,
    /// Keychord that toggles between vim and standard editing, e.g.
    /// `"f2"`, `"f12"`, `"ctrl+e"`, `"alt+m"`. See `parse_key_chord`
    /// for the accepted grammar.
    #[serde(default = "default_toggle_mode_key")]
    pub toggle_mode_key: String,
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            mode: EditorMode::default(),
            toggle_mode_key: default_toggle_mode_key(),
        }
    }
}

/// `[keymap]` — per-action chord overrides for the Standard editing
/// profile. Keys are action names (the `name` field of
/// `crate::input::keymap::standard_actions`); values are chord strings
/// in the `crate::input::keychord` grammar (`"ctrl+c"`, `"shift+up"`,
/// `"f5"`, …).
///
/// An action absent from the map uses its built-in default, so a
/// hand-edited partial `[keymap]` never disables the rest. The
/// first-run config is written fully populated (`Default`) so users
/// can see and edit every binding without consulting docs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeymapConfig(std::collections::BTreeMap<String, String>);

impl KeymapConfig {
    /// The configured chord string for `action_name`, if the user set
    /// one. `None` means "use the built-in default".
    pub fn chord_for(&self, action_name: &str) -> Option<&str> {
        self.0.get(action_name).map(String::as_str)
    }
}

impl Default for KeymapConfig {
    /// Fully populated from `standard_actions` so the generated
    /// config.toml lists every binding with its default.
    fn default() -> Self {
        Self(
            crate::input::keymap::standard_actions()
                .into_iter()
                .map(|spec| (spec.name.to_string(), spec.default_chord.to_string()))
                .collect(),
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "auto".into(),
            sidebar_default_visible: true,
            sidebar_width: 28,
            auto_save_debounce_ms: 1000,
            mouse_enabled: false,
            ui: UiConfig::default(),
            blocks: BlocksConfig::default(),
            chat: ChatConfig::default(),
            editor: EditorConfig::default(),
            keymap: KeymapConfig::default(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            show_line_numbers: true,
            show_relative_numbers: false,
            font_features: true,
        }
    }
}

impl Default for BlocksConfig {
    fn default() -> Self {
        Self {
            default_display_mode: "split".into(),
            auto_run_on_cached_miss: false,
            result_nav: "flow".into(),
        }
    }
}

/// Path to the config file: `$HOME/.config/httui/config.toml`. Uses
/// the unified data directory shared by every httui binary
/// (`httui_core::paths::default_data_dir`) — NOT a Tauri/`ProjectDirs`
/// namespace, so the TUI, desktop and MCP all converge on one dir.
pub fn default_config_path() -> TuiResult<PathBuf> {
    let dir = httui_core::paths::default_data_dir()
        .map_err(|e| TuiError::Config(format!("resolve config dir: {e}")))?;
    Ok(dir.join("config.toml"))
}

/// Directory for the TUI's rolling log files:
/// `$HOME/.config/httui/logs` — under the same unified data dir as
/// [`default_config_path`].
pub fn log_dir() -> TuiResult<PathBuf> {
    let dir = httui_core::paths::default_data_dir()
        .map_err(|e| TuiError::Config(format!("resolve log dir: {e}")))?;
    Ok(dir.join("logs"))
}

/// Load config from `path`, creating it with defaults on first run.
pub fn load_or_init(path: &Path) -> TuiResult<Config> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let cfg = Config::default();
        let body = toml::to_string_pretty(&cfg)
            .map_err(|e| TuiError::Config(format!("serialize defaults: {e}")))?;
        std::fs::write(path, body)?;
        return Ok(cfg);
    }

    let raw = std::fs::read_to_string(path)?;
    toml::from_str(&raw).map_err(|e| TuiError::Config(format!("parse {path:?}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_roundtrip_through_toml() {
        let cfg = Config::default();
        let s = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.theme, cfg.theme);
        assert_eq!(back.sidebar_width, cfg.sidebar_width);
        assert_eq!(
            back.blocks.default_display_mode,
            cfg.blocks.default_display_mode
        );
    }

    #[test]
    fn partial_toml_falls_back_to_defaults() {
        let raw = "theme = \"dark\"\nsidebar_width = 40\n";
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.theme, "dark");
        assert_eq!(cfg.sidebar_width, 40);
        assert_eq!(cfg.auto_save_debounce_ms, 1000); // default preserved
        assert!(cfg.ui.show_line_numbers); // nested default preserved
    }

    #[test]
    fn load_or_init_creates_file_on_first_run() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested").join("config.toml");
        let cfg = load_or_init(&path).unwrap();
        assert_eq!(cfg.theme, "auto");
        assert!(path.exists());

        // Second call reads what was written.
        let cfg2 = load_or_init(&path).unwrap();
        assert_eq!(cfg2.theme, cfg.theme);
    }

    #[test]
    fn load_or_init_preserves_user_edits() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "theme = \"dark\"\nsidebar_width = 50\n").unwrap();
        let cfg = load_or_init(&path).unwrap();
        assert_eq!(cfg.theme, "dark");
        assert_eq!(cfg.sidebar_width, 50);
    }

    #[test]
    fn editor_mode_defaults_to_standard() {
        // Vim is opt-in: a fresh config must NOT be vim (TD2).
        assert_eq!(EditorMode::default(), EditorMode::Standard);
        assert_eq!(Config::default().editor.mode, EditorMode::Standard);
    }

    #[test]
    fn editor_mode_roundtrips_through_toml() {
        let mut cfg = Config::default();
        cfg.editor.mode = EditorMode::Vim;
        let s = toml::to_string_pretty(&cfg).unwrap();
        let back: Config = toml::from_str(&s).unwrap();
        assert_eq!(back.editor.mode, EditorMode::Vim);
        // Serialized form is lowercase.
        assert!(s.contains("mode = \"vim\""));
    }

    #[test]
    fn config_without_editor_section_falls_back_to_standard() {
        let raw = "theme = \"dark\"\n";
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.editor.mode, EditorMode::Standard);
    }

    #[test]
    fn explicit_editor_mode_vim_parses() {
        let raw = "[editor]\nmode = \"vim\"\n";
        let cfg: Config = toml::from_str(raw).unwrap();
        assert_eq!(cfg.editor.mode, EditorMode::Vim);
    }

    #[test]
    fn log_dir_is_absolute_and_ends_in_logs() {
        // `default_data_dir` resolves against $HOME; assert only
        // platform-stable invariants (no hardcoded prefix).
        let dir = log_dir().expect("log_dir resolves in a normal env");
        assert!(dir.is_absolute(), "log dir must be absolute: {dir:?}");
        assert!(dir.ends_with("logs"), "log dir must end in `logs`: {dir:?}");
    }

    #[test]
    fn log_dir_shares_root_with_default_config_path() {
        // Both derive from `httui_core::paths::default_data_dir()`
        // (`$HOME/.config/httui`): config.toml sits directly in it and
        // logs/ is a child — so the config file's parent equals the
        // log dir's parent (the shared data dir).
        let log = log_dir().unwrap();
        let cfg = default_config_path().unwrap();
        assert_eq!(cfg.parent(), log.parent());
    }
}
