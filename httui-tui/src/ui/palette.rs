use ratatui::style::Color;

pub const SELECTION_BG: Color = Color::Rgb(60, 70, 110);

/// Default border for modals/pages. Subtle blue-gray, safer across
/// warm-light terminal themes than `LightMagenta`.
pub const BORDER: Color = Color::Rgb(110, 140, 175);

/// Cool accent for titles, section headers, numeric shortcuts.
pub const ACCENT: Color = Color::Rgb(130, 170, 220);

/// Warm amber for ephemeral state (TEMP badge, session override
/// values). Distinct from `Color::Red` (reserved for errors).
pub const AMBER: Color = Color::Rgb(255, 176, 0);
pub const AMBER_FG_ON_AMBER_BG: Color = Color::Rgb(20, 14, 0);

/// Muted gray for secondary text (labels, inactive shortcuts,
/// timestamps). Fixed RGB so it reads as dim cross-platform —
/// ratatui's `DarkGray` can render almost white on warm themes.
pub const MUTED: Color = Color::Rgb(120, 120, 120);
/// Mid-emphasis gray for still-readable secondary content (unfocused
/// commit subjects). Lighter than `MUTED`.
pub const SECONDARY: Color = Color::Rgb(170, 170, 170);
