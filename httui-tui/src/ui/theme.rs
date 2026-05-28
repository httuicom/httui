//! Runtime-swappable color palette. Four built-in presets; per-slot
//! overrides come from `config.toml` under `[palette]` (any slot you
//! set there wins over the preset).
//!
//! Lifecycle: [`init`] populates the global `RwLock<Theme>` at
//! startup from `cfg.theme` + `cfg.palette`. The Settings page calls
//! [`init`] again on a preset switch — the new colors take effect on
//! the next render.

use ratatui::style::Color;
use std::collections::BTreeMap;
use std::sync::RwLock;

/// One palette. Every slot the renderers consume lives here; adding
/// a new slot is a field on this struct + a default for each preset
/// + a name in [`Theme::apply_overrides`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// Frame-wide background. `Color::Reset` means "inherit from the
    /// terminal" — leave that for dark themes whose chrome already
    /// assumes a dark canvas. Light themes paint an explicit color
    /// so the whole TUI looks light even on a dark terminal.
    pub background: Color,
    /// Frame-wide default foreground. Pairs with [`Self::background`] —
    /// light themes set an explicit dark `foreground` so unstyled
    /// text doesn't disappear against the bright canvas.
    pub foreground: Color,
    pub border: Color,
    pub accent: Color,
    pub muted: Color,
    pub secondary: Color,
    pub selection_bg: Color,
    pub amber: Color,
    pub amber_fg_on_amber_bg: Color,
    /// Header / footer bar bg of every executable block. Built-in
    /// presets keep this equal to [`Self::block_body_bg`] so the
    /// whole card reads as a single surface; a `[palette]` override
    /// can re-introduce a chrome/body contrast if the user wants one.
    pub block_chrome_bg: Color,
    /// Tinted body area of executable blocks (SQL editor, response
    /// pane, etc.). Lifted from the canvas so blocks read as
    /// distinct cards.
    pub block_body_bg: Color,
    /// Highlight bg of the active result sub-tab.
    pub block_active_bg: Color,
    /// Alternating row bg in result tables.
    pub table_zebra_bg: Color,
    /// Bg of every popup/modal panel. Deliberately distinct from
    /// [`Self::background`] so popups "lift" off the canvas — in dark
    /// presets popup is pure black against the terminal-native canvas;
    /// in light presets popup is near-white against the off-white
    /// canvas.
    pub popup_bg: Color,
    /// Border + section-title accent inside popups. Brighter than
    /// [`Self::accent`] because popups absorb attention and the
    /// border has to read against [`Self::popup_bg`].
    pub popup_border_accent: Color,
    /// Inline chord / key glyph color inside popups (e.g. `Ctrl-C`).
    /// Distinct from regular labels so a quick scan picks out which
    /// keys do what.
    pub popup_key_label: Color,
    /// "Positive / connected / ON" indicator color (toggle marks,
    /// connection status dot, success banner).
    pub success: Color,
    /// "Negative / failed / error" indicator color (failed run badge,
    /// error status, dirty/destructive markers). Theme-aware so a
    /// preset can soften the harsh ANSI red into its own palette.
    pub error: Color,
}

impl Theme {
    /// Warm blues + amber on a dark terminal background. Leaves
    /// background/foreground at `Color::Reset` so the user's
    /// terminal chrome shows through.
    pub const DEFAULT_DARK: Theme = Theme {
        background: Color::Reset,
        foreground: Color::Reset,
        border: Color::Rgb(110, 140, 175),
        accent: Color::Rgb(130, 170, 220),
        muted: Color::Rgb(120, 120, 120),
        secondary: Color::Rgb(170, 170, 170),
        selection_bg: Color::Rgb(60, 70, 110),
        amber: Color::Rgb(255, 176, 0),
        amber_fg_on_amber_bg: Color::Rgb(20, 14, 0),
        block_chrome_bg: Color::Reset,
        block_body_bg: Color::Reset,
        block_active_bg: Color::Rgb(50, 60, 90),
        table_zebra_bg: Color::Rgb(18, 20, 26),
        popup_bg: Color::Black,
        popup_border_accent: Color::LightYellow,
        popup_key_label: Color::Cyan,
        success: Color::LightGreen,
        error: Color::Red,
    };

    /// Light-background variant. Paints a near-white canvas + dark
    /// ink so the whole TUI reads as light even when launched from a
    /// dark terminal.
    pub const DEFAULT_LIGHT: Theme = Theme {
        background: Color::Rgb(248, 248, 245),
        foreground: Color::Rgb(30, 30, 30),
        border: Color::Rgb(160, 160, 175),
        accent: Color::Rgb(40, 90, 170),
        muted: Color::Rgb(120, 120, 120),
        secondary: Color::Rgb(60, 60, 60),
        selection_bg: Color::Rgb(210, 225, 250),
        amber: Color::Rgb(170, 110, 0),
        amber_fg_on_amber_bg: Color::Rgb(255, 245, 220),
        block_chrome_bg: Color::Rgb(248, 248, 245),
        block_body_bg: Color::Rgb(248, 248, 245),
        block_active_bg: Color::Rgb(200, 215, 240),
        table_zebra_bg: Color::Rgb(238, 240, 245),
        popup_bg: Color::Rgb(252, 252, 250),
        popup_border_accent: Color::Rgb(170, 110, 0),
        popup_key_label: Color::Rgb(40, 90, 170),
        success: Color::Rgb(0, 130, 60),
        error: Color::Rgb(176, 0, 32),
    };

    /// Every slot is an ANSI named color (or `Color::Reset`) so the
    /// user's terminal theme drives the actual hue.
    pub const TERMINAL_NATIVE: Theme = Theme {
        background: Color::Reset,
        foreground: Color::Reset,
        border: Color::Gray,
        accent: Color::Cyan,
        muted: Color::DarkGray,
        secondary: Color::Gray,
        selection_bg: Color::Reset,
        amber: Color::Yellow,
        amber_fg_on_amber_bg: Color::Black,
        block_chrome_bg: Color::Reset,
        block_body_bg: Color::Reset,
        block_active_bg: Color::Reset,
        table_zebra_bg: Color::Reset,
        popup_bg: Color::Reset,
        popup_border_accent: Color::Yellow,
        popup_key_label: Color::Cyan,
        success: Color::Green,
        error: Color::Red,
    };

    /// Tokyo Night — deep navy canvas with a lifted panel surface and
    /// soft blue/green/red accents. Paints explicit backgrounds (canvas
    /// + panel) so the IDE-style region elevation reads on any terminal.
    pub const TOKYO_NIGHT: Theme = Theme {
        background: Color::Rgb(0x1a, 0x1b, 0x26),
        foreground: Color::Rgb(0xc0, 0xca, 0xf5),
        border: Color::Rgb(0x41, 0x48, 0x68),
        accent: Color::Rgb(0x7a, 0xa2, 0xf7),
        muted: Color::Rgb(0x56, 0x5f, 0x89),
        secondary: Color::Rgb(0x9a, 0xa5, 0xce),
        selection_bg: Color::Rgb(0x28, 0x34, 0x57),
        amber: Color::Rgb(0xe0, 0xaf, 0x68),
        amber_fg_on_amber_bg: Color::Rgb(0x1a, 0x1b, 0x26),
        block_chrome_bg: Color::Rgb(0x24, 0x28, 0x3b),
        block_body_bg: Color::Rgb(0x24, 0x28, 0x3b),
        block_active_bg: Color::Rgb(0x2f, 0x33, 0x4d),
        table_zebra_bg: Color::Rgb(0x1e, 0x20, 0x30),
        popup_bg: Color::Rgb(0x1f, 0x23, 0x35),
        popup_border_accent: Color::Rgb(0x7d, 0xcf, 0xff),
        popup_key_label: Color::Rgb(0x7d, 0xcf, 0xff),
        success: Color::Rgb(0x9e, 0xce, 0x6a),
        error: Color::Rgb(0xf7, 0x76, 0x8e),
    };

    /// Map a user-facing preset name to a [`Theme`]. `"auto"` is an
    /// alias for `terminal-native` so configs that carry the old
    /// shipped default (`theme = "auto"`) keep working.
    pub fn from_preset(name: &str) -> Option<Theme> {
        match name {
            "default-dark" | "dark" => Some(Self::DEFAULT_DARK),
            "default-light" | "light" => Some(Self::DEFAULT_LIGHT),
            "terminal-native" | "auto" => Some(Self::TERMINAL_NATIVE),
            "tokyo-night" | "tokyonight" => Some(Self::TOKYO_NIGHT),
            _ => None,
        }
    }

    /// Apply hex/ANSI per-slot overrides on top of `self`. Unknown
    /// slot names are ignored (silently — typo in config.toml
    /// shouldn't break the boot); unparseable values fall back to
    /// the preset color.
    pub fn apply_overrides(mut self, overrides: &BTreeMap<String, String>) -> Self {
        for (slot, raw) in overrides {
            let Some(color) = parse_color(raw) else {
                continue;
            };
            match slot.as_str() {
                "background" => self.background = color,
                "foreground" => self.foreground = color,
                "border" => self.border = color,
                "accent" => self.accent = color,
                "muted" => self.muted = color,
                "secondary" => self.secondary = color,
                "selection_bg" => self.selection_bg = color,
                "amber" => self.amber = color,
                "amber_fg_on_amber_bg" => self.amber_fg_on_amber_bg = color,
                "block_chrome_bg" => self.block_chrome_bg = color,
                "block_body_bg" => self.block_body_bg = color,
                "block_active_bg" => self.block_active_bg = color,
                "table_zebra_bg" => self.table_zebra_bg = color,
                "popup_bg" => self.popup_bg = color,
                "popup_border_accent" => self.popup_border_accent = color,
                "popup_key_label" => self.popup_key_label = color,
                "success" => self.success = color,
                "error" => self.error = color,
                _ => {}
            }
        }
        self
    }
}

static THEME: RwLock<Theme> = RwLock::new(Theme::DEFAULT_DARK);

/// Install `preset` (with `overrides` layered on top) as the active
/// palette. Subsequent calls to the [`crate::ui::palette`] accessors
/// see the new colors. Called at startup AND on every preset change
/// from the Settings page.
pub fn init(preset: &str, overrides: &BTreeMap<String, String>) {
    let base = Theme::from_preset(preset).unwrap_or(Theme::DEFAULT_DARK);
    let resolved = base.apply_overrides(overrides);
    if let Ok(mut guard) = THEME.write() {
        *guard = resolved;
    }
}

/// Snapshot of the active theme. Returns a `Copy` value so callers
/// don't hold the lock for the duration of a render.
pub fn current() -> Theme {
    *THEME.read().expect("theme lock poisoned")
}

/// Parse a single color value from the config file. Accepted forms:
/// - `"#rrggbb"` (case-insensitive 6-char hex)
/// - `"#rgb"`    (3-char hex, expanded as `#rrggbb`)
/// - `"reset"` / `"black"` / `"red"` / `"green"` / `"yellow"` /
///   `"blue"` / `"magenta"` / `"cyan"` / `"white"` / `"gray"` /
///   `"darkgray"` (canonical ANSI names; both spellings of dark-gray
///   accepted)
fn parse_color(raw: &str) -> Option<Color> {
    let s = raw.trim().to_ascii_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    Some(match s.as_str() {
        "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" | "dark-gray" | "dark-grey" => Color::DarkGray,
        _ => return None,
    })
}

fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim();
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(Color::Rgb(r, g, b))
        }
        3 => {
            // Expand #abc → #aabbcc, matching CSS shorthand.
            let r = u8::from_str_radix(&s[0..1], 16).ok()? * 0x11;
            let g = u8::from_str_radix(&s[1..2], 16).ok()? * 0x11;
            let b = u8::from_str_radix(&s[2..3], 16).ok()? * 0x11;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_resolve_by_canonical_name() {
        assert_eq!(
            Theme::from_preset("default-dark"),
            Some(Theme::DEFAULT_DARK)
        );
        assert_eq!(
            Theme::from_preset("default-light"),
            Some(Theme::DEFAULT_LIGHT),
        );
        assert_eq!(
            Theme::from_preset("terminal-native"),
            Some(Theme::TERMINAL_NATIVE),
        );
    }

    #[test]
    fn presets_accept_short_aliases_and_auto() {
        assert_eq!(Theme::from_preset("dark"), Some(Theme::DEFAULT_DARK));
        assert_eq!(Theme::from_preset("light"), Some(Theme::DEFAULT_LIGHT));
        assert_eq!(Theme::from_preset("auto"), Some(Theme::TERMINAL_NATIVE));
    }

    #[test]
    fn tokyo_night_resolves_and_carries_distinct_panel_and_error() {
        let t = Theme::from_preset("tokyo-night").expect("tokyo-night preset");
        assert_eq!(t, Theme::TOKYO_NIGHT);
        assert_eq!(Theme::from_preset("tokyonight"), Some(Theme::TOKYO_NIGHT));
        // Panel surface must lift off the canvas for the IDE-style look.
        assert_ne!(t.block_body_bg, t.background);
        assert_eq!(t.error, Color::Rgb(0xf7, 0x76, 0x8e));
        assert_eq!(t.accent, Color::Rgb(0x7a, 0xa2, 0xf7));
    }

    #[test]
    fn apply_overrides_sets_error_slot() {
        let mut over = BTreeMap::new();
        over.insert("error".into(), "#abcdef".into());
        let t = Theme::DEFAULT_DARK.apply_overrides(&over);
        assert_eq!(t.error, Color::Rgb(0xab, 0xcd, 0xef));
    }

    #[test]
    fn presets_reject_unknown_names() {
        assert!(Theme::from_preset("nord").is_none());
        assert!(Theme::from_preset("").is_none());
    }

    #[test]
    fn parse_color_hex_6_char() {
        assert_eq!(parse_color("#ff8800"), Some(Color::Rgb(0xff, 0x88, 0x00)));
        assert_eq!(parse_color("#FFAABB"), Some(Color::Rgb(0xff, 0xaa, 0xbb)));
    }

    #[test]
    fn parse_color_hex_3_char_expands() {
        assert_eq!(parse_color("#f80"), Some(Color::Rgb(0xff, 0x88, 0x00)));
        assert_eq!(parse_color("#000"), Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn parse_color_named_ansi() {
        assert_eq!(parse_color("reset"), Some(Color::Reset));
        assert_eq!(parse_color("red"), Some(Color::Red));
        assert_eq!(parse_color("gray"), Some(Color::Gray));
        assert_eq!(parse_color("grey"), Some(Color::Gray));
        assert_eq!(parse_color("darkgray"), Some(Color::DarkGray));
        assert_eq!(parse_color("dark-grey"), Some(Color::DarkGray));
    }

    #[test]
    fn parse_color_rejects_invalid() {
        assert!(parse_color("not a color").is_none());
        assert!(parse_color("#xyz").is_none());
        assert!(parse_color("#abcd").is_none());
        assert!(parse_color("").is_none());
    }

    #[test]
    fn apply_overrides_replaces_only_recognised_slots() {
        let mut over = BTreeMap::new();
        over.insert("border".into(), "#abcdef".into());
        over.insert("typo_slot_name".into(), "#000000".into());
        let theme = Theme::DEFAULT_DARK.apply_overrides(&over);
        assert_eq!(theme.border, Color::Rgb(0xab, 0xcd, 0xef));
        // Other slots untouched.
        assert_eq!(theme.accent, Theme::DEFAULT_DARK.accent);
    }

    #[test]
    fn apply_overrides_skips_unparseable_values() {
        let mut over = BTreeMap::new();
        over.insert("border".into(), "bogus".into());
        let theme = Theme::DEFAULT_DARK.apply_overrides(&over);
        assert_eq!(theme.border, Theme::DEFAULT_DARK.border);
    }

    // Init tests share the process-wide `RwLock<Theme>` — a stray
    // parallel test would race with `current()`. They live behind a
    // single sequential test that exercises both code paths so the
    // suite stays deterministic without needing `serial_test`.
    #[test]
    fn init_writes_and_falls_back_sequentially() {
        let mut over = BTreeMap::new();
        over.insert("accent".into(), "#112233".into());
        init("default-dark", &over);
        assert_eq!(current().accent, Color::Rgb(0x11, 0x22, 0x33));

        // Unknown preset → default-dark; the previous accent override
        // doesn't carry over because we pass an empty map.
        init("not-a-real-preset", &BTreeMap::new());
        assert_eq!(current().accent, Theme::DEFAULT_DARK.accent);

        // Restore the suite-wide initial state so any concurrent test
        // reading `current()` sees the same default as cold-boot.
        init("default-dark", &BTreeMap::new());
    }
}
