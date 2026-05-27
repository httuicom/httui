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
    pub mouse_enabled: bool,

    pub ui: UiConfig,
    pub blocks: BlocksConfig,
    pub chat: ChatConfig,
    pub editor: EditorConfig,
    pub keymap: KeymapConfig,
    /// Per-slot palette overrides layered on top of `theme`. Each
    /// entry is `slot_name → "#rrggbb" | "#rgb" | ansi_name`. Unknown
    /// slot names and unparseable values are ignored at boot — see
    /// `crate::ui::theme::Theme::apply_overrides`.
    pub palette: std::collections::BTreeMap<String, String>,
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

/// Default is empty: the toggle lives in Settings → Editor → Mode
/// (Enter activates). Users can still bind a chord via Settings →
/// Keymaps → `[ vim ↔ standard toggle ]`.
fn default_toggle_mode_key() -> String {
    String::new()
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
/// `"alt+r"`, …).
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

    /// Overwrite (or insert) the chord for `action_name`. Used by
    /// the legacy-default migration in `load_or_init` and by the
    /// Settings page's rebind flow.
    pub fn set(&mut self, action_name: &str, chord: String) {
        self.0.insert(action_name.to_string(), chord);
    }

    /// Drop the entry for `action_name` so the resolver falls back
    /// to the built-in default. Used by rename migrations and by
    /// the Settings page's "Reset to default" action.
    pub fn remove(&mut self, action_name: &str) {
        self.0.remove(action_name);
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
            theme: "default-dark".into(),
            sidebar_default_visible: true,
            sidebar_width: 28,
            mouse_enabled: false,
            ui: UiConfig::default(),
            blocks: BlocksConfig::default(),
            chat: ChatConfig::default(),
            editor: EditorConfig::default(),
            keymap: KeymapConfig::default(),
            palette: std::collections::BTreeMap::new(),
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

/// Serialize a `Config` to its canonical TOML form.
fn render_config(cfg: &Config) -> TuiResult<String> {
    toml::to_string_pretty(cfg).map_err(|e| TuiError::Config(format!("serialize config: {e}")))
}

/// Legacy F-key defaults from tui-V03 keymap fase 2 + the initial input
/// fix. When upgrading past fase 4, any user whose `[keymap]` still
/// carries the exact old default gets bumped to the new `Alt+letter`
/// default in place — the canonical rewrite in `load_or_init` then
/// persists the migration. Custom values (anything other than the
/// listed legacy default) are untouched.
const LEGACY_KEYMAP_DEFAULTS: &[(&str, &str, &str)] = &[
    ("run_block", "f5", "alt+r"),
    ("rerun_last_block", "f6", "alt+."),
    ("open_help", "f1", "alt+?"),
    ("open_tab_picker", "f3", "alt+t"),
    ("open_environment_picker", "f4", "alt+e"),
    ("open_block_history", "f7", "alt+h"),
    ("open_export_picker", "f8", "alt+g"),
    // `open_settings` (was `open_block_settings`): the F9 legacy
    // default migrates to `alt+,`, and the rename itself runs below
    // in `migrate_legacy_keymap` so custom chords also move over.
    ("open_settings", "f9", "alt+,"),
    ("open_block_template_picker", "f10", "alt+n"),
];

/// Past defaults for `editor.toggle_mode_key`. A user still on one
/// of these (= never rebound) migrates to the unset default so the
/// chord doesn't shadow newer global bindings. Custom chords are
/// preserved.
const LEGACY_TOGGLE_MODE_KEYS: &[&str] = &["f2", "alt+m"];

/// Keymap-entry renames. Each pair carries the user's customised
/// chord across the rename: someone who set the old name to the old
/// F-key default ends up with the new name + new chord via
/// [`LEGACY_KEYMAP_DEFAULTS`]; someone with a custom chord keeps
/// that value under the new key name.
const LEGACY_KEYMAP_RENAMES: &[(&str, &str)] = &[("open_block_settings", "open_settings")];

/// Migrate legacy F-key defaults to the new `Alt+letter` chords.
/// Idempotent: only rewrites entries whose value matches the OLD
/// default verbatim, so a user-customised chord is preserved.
pub(crate) fn migrate_legacy_keymap(cfg: &mut Config) {
    // Rename pass first — afterwards everything below operates under
    // the new names, so a renamed action's legacy F-key default still
    // migrates to its new Alt+letter chord.
    for (from, to) in LEGACY_KEYMAP_RENAMES {
        if let Some(chord) = cfg.keymap.chord_for(from).map(str::to_string) {
            cfg.keymap.remove(from);
            cfg.keymap.set(to, chord);
        }
    }
    for (name, old, new) in LEGACY_KEYMAP_DEFAULTS {
        if cfg.keymap.chord_for(name) == Some(*old) {
            cfg.keymap.set(name, (*new).to_string());
        }
    }
    if LEGACY_TOGGLE_MODE_KEYS.contains(&cfg.editor.toggle_mode_key.as_str()) {
        cfg.editor.toggle_mode_key = default_toggle_mode_key();
    }
}

pub fn save_config(path: &Path, cfg: &Config) -> TuiResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, render_config(cfg)?)?;
    Ok(())
}

/// Load config from `path`, creating it with defaults on first run.
///
/// On every load the file is re-written in canonical form: fields
/// added since it was last saved (e.g. new `[keymap]` actions) appear
/// with their defaults, so the user never has to consult docs to learn
/// a binding name. The rewrite is best-effort — a write failure does
/// not block startup, and it is skipped when the file is already
/// canonical.
///
/// `migrate_legacy_keymap` runs between parse and rewrite so legacy
/// F-key defaults inherited from earlier tui-V03 builds are bumped to
/// their new `Alt+letter` equivalents in place.
pub fn load_or_init(path: &Path) -> TuiResult<Config> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let cfg = Config::default();
        std::fs::write(path, render_config(&cfg)?)?;
        return Ok(cfg);
    }

    let raw = std::fs::read_to_string(path)?;
    let mut cfg: Config =
        toml::from_str(&raw).map_err(|e| TuiError::Config(format!("parse {path:?}: {e}")))?;
    migrate_legacy_keymap(&mut cfg);
    if let Ok(canonical) = render_config(&cfg) {
        if canonical != raw {
            let _ = std::fs::write(path, canonical);
        }
    }
    Ok(cfg)
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
        assert!(cfg.ui.show_line_numbers); // nested default preserved
    }

    #[test]
    fn load_or_init_creates_file_on_first_run() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested").join("config.toml");
        let cfg = load_or_init(&path).unwrap();
        assert_eq!(cfg.theme, "default-dark");
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
    fn load_or_init_backfills_missing_keymap() {
        // A legacy config with no [keymap]: after load the file is
        // rewritten with the full keymap, so the user sees and can
        // edit every binding without consulting docs.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, "theme = \"dark\"\n").unwrap();
        let cfg = load_or_init(&path).unwrap();
        assert_eq!(cfg.theme, "dark", "user value preserved");
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(
            on_disk.contains("[keymap]") && on_disk.contains("copy = "),
            "load must rewrite the file with the full [keymap]:\n{on_disk}"
        );
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

    #[test]
    fn migrate_rewrites_legacy_fkey_defaults() {
        // A user upgrading from tui-V03 fase 2 has `run_block = "f5"`
        // (and friends) in their config. After migration each entry
        // points at its new `Alt+letter` chord.
        let mut cfg = Config::default();
        for (name, old, _) in LEGACY_KEYMAP_DEFAULTS {
            cfg.keymap.set(name, (*old).to_string());
        }
        cfg.editor.toggle_mode_key = "f2".to_string();
        migrate_legacy_keymap(&mut cfg);
        for (name, _, new) in LEGACY_KEYMAP_DEFAULTS {
            assert_eq!(
                cfg.keymap.chord_for(name),
                Some(*new),
                "{name} should migrate to {new}"
            );
        }
        assert_eq!(cfg.editor.toggle_mode_key, default_toggle_mode_key());
    }

    #[test]
    fn migrate_wipes_alt_m_toggle_default() {
        let mut cfg = Config::default();
        cfg.editor.toggle_mode_key = "alt+m".to_string();
        migrate_legacy_keymap(&mut cfg);
        assert_eq!(cfg.editor.toggle_mode_key, "");
    }

    #[test]
    fn migrate_preserves_user_customizations() {
        // Anything not equal to the OLD default is the user's choice;
        // migration must leave it alone — including a chord the user
        // happened to set to a different F-key.
        let mut cfg = Config::default();
        cfg.keymap.set("run_block", "ctrl+r".to_string());
        cfg.keymap.set("open_help", "f11".to_string()); // not the old default (f1)
        cfg.editor.toggle_mode_key = "ctrl+e".to_string();
        migrate_legacy_keymap(&mut cfg);
        assert_eq!(cfg.keymap.chord_for("run_block"), Some("ctrl+r"));
        assert_eq!(cfg.keymap.chord_for("open_help"), Some("f11"));
        assert_eq!(cfg.editor.toggle_mode_key, "ctrl+e");
    }

    #[test]
    fn migrate_is_idempotent() {
        // Running the migration twice produces the same Config as
        // running it once — important because `load_or_init` runs it on
        // every load.
        let mut cfg = Config::default();
        for (name, old, _) in LEGACY_KEYMAP_DEFAULTS {
            cfg.keymap.set(name, (*old).to_string());
        }
        cfg.editor.toggle_mode_key = "f2".to_string();
        migrate_legacy_keymap(&mut cfg);
        let after_first = cfg.clone();
        migrate_legacy_keymap(&mut cfg);
        for (name, _, _) in LEGACY_KEYMAP_DEFAULTS {
            assert_eq!(
                cfg.keymap.chord_for(name),
                after_first.keymap.chord_for(name)
            );
        }
        assert_eq!(
            cfg.editor.toggle_mode_key,
            after_first.editor.toggle_mode_key
        );
    }

    #[test]
    fn migrate_renames_open_block_settings_to_open_settings() {
        // A user-customised chord under the old name moves to the
        // new name verbatim.
        let mut cfg = Config::default();
        cfg.keymap.remove("open_settings");
        cfg.keymap.set("open_block_settings", "ctrl+,".to_string());
        migrate_legacy_keymap(&mut cfg);
        assert_eq!(cfg.keymap.chord_for("open_settings"), Some("ctrl+,"));
        assert_eq!(cfg.keymap.chord_for("open_block_settings"), None);
    }

    #[test]
    fn migrate_renames_then_promotes_legacy_fkey() {
        // A user with the old name + the old F-key default ends up
        // with the new name + the new chord after migration.
        let mut cfg = Config::default();
        cfg.keymap.remove("open_settings");
        cfg.keymap.set("open_block_settings", "f9".to_string());
        migrate_legacy_keymap(&mut cfg);
        assert_eq!(cfg.keymap.chord_for("open_settings"), Some("alt+,"));
        assert_eq!(cfg.keymap.chord_for("open_block_settings"), None);
    }

    #[test]
    fn keymap_remove_drops_entry() {
        let mut km = KeymapConfig::default();
        assert!(km.chord_for("copy").is_some());
        km.remove("copy");
        assert!(km.chord_for("copy").is_none());
    }

    #[test]
    fn load_or_init_migrates_legacy_fkey_on_disk() {
        // End-to-end: a config.toml on disk with `toggle_mode_key = "f2"`
        // and `run_block = "f5"` is rewritten to the Alt+letter forms
        // after load. The user never has to edit the file by hand.
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(
            &path,
            "[editor]\n\
             mode = \"standard\"\n\
             toggle_mode_key = \"f2\"\n\
             \n\
             [keymap]\n\
             run_block = \"f5\"\n",
        )
        .unwrap();
        let cfg = load_or_init(&path).unwrap();
        assert_eq!(cfg.editor.toggle_mode_key, "");
        assert_eq!(cfg.keymap.chord_for("run_block"), Some("alt+r"));
        let on_disk = std::fs::read_to_string(&path).unwrap();
        assert!(
            on_disk.contains("toggle_mode_key = \"\"")
                && on_disk.contains("run_block = \"alt+r\""),
            "load must persist the migrated chords:\n{on_disk}"
        );
    }
}
