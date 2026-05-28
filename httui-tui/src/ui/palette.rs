//! Thin accessor over [`crate::ui::theme`]. Renderers go through
//! these functions so a theme switch (from the Settings page or
//! `config.toml`) takes effect on the next paint without touching
//! every call site.

use ratatui::style::Color;

use crate::ui::theme;

/// Frame-wide canvas color. `Color::Reset` (most presets) means
/// "inherit the terminal background"; explicit RGB paints a fixed
/// background so the TUI canvas matches the preset regardless of
/// the host terminal.
#[inline]
pub fn background() -> Color {
    theme::current().background
}

/// Frame-wide default text color. Paired with [`background`].
#[inline]
pub fn foreground() -> Color {
    theme::current().foreground
}

#[inline]
pub fn selection_bg() -> Color {
    theme::current().selection_bg
}

/// Default border for modals/pages.
#[inline]
pub fn border() -> Color {
    theme::current().border
}

/// Cool accent for titles, section headers, numeric shortcuts.
#[inline]
pub fn accent() -> Color {
    theme::current().accent
}

/// Warm amber for ephemeral state (TEMP badge, session override
/// values). Distinct from `Color::Red` (reserved for errors).
#[inline]
pub fn amber() -> Color {
    theme::current().amber
}

#[inline]
pub fn amber_fg_on_amber_bg() -> Color {
    theme::current().amber_fg_on_amber_bg
}

/// Muted gray for secondary text (labels, inactive shortcuts,
/// timestamps).
#[inline]
pub fn muted() -> Color {
    theme::current().muted
}

/// Mid-emphasis gray for still-readable secondary content (unfocused
/// commit subjects). Lighter than [`muted`].
#[inline]
pub fn secondary() -> Color {
    theme::current().secondary
}

/// Header / footer bar bg of executable blocks.
#[inline]
pub fn block_chrome_bg() -> Color {
    theme::current().block_chrome_bg
}

/// Tinted body bg of executable blocks.
#[inline]
pub fn block_body_bg() -> Color {
    theme::current().block_body_bg
}

/// Active sub-tab highlight bg inside a block's result panel.
#[inline]
pub fn block_active_bg() -> Color {
    theme::current().block_active_bg
}

/// Alternating row bg for result tables.
#[inline]
pub fn table_zebra_bg() -> Color {
    theme::current().table_zebra_bg
}

/// Bg of popup / modal panels.
#[inline]
pub fn popup_bg() -> Color {
    theme::current().popup_bg
}

/// Border + title accent inside popups.
#[inline]
pub fn popup_border_accent() -> Color {
    theme::current().popup_border_accent
}

/// Inline chord / key glyph color inside popups.
#[inline]
pub fn popup_key_label() -> Color {
    theme::current().popup_key_label
}

/// "Positive / connected / ON" indicator color.
#[inline]
pub fn success() -> Color {
    theme::current().success
}

/// "Negative / failed / error" indicator color. Theme-aware
/// replacement for a hardcoded `Color::Red`.
#[inline]
pub fn error() -> Color {
    theme::current().error
}
