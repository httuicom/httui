//! Settings page state.
//!
//! Single modal surface with three sections (Keymaps / Theme /
//! Editor). Every mutation persists through `crate::config::save_config`
//! so closing the modal never loses state.

use std::path::PathBuf;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    #[default]
    Keymaps,
    Theme,
    Editor,
}

impl SettingsSection {
    pub const ALL: [Self; 3] = [Self::Keymaps, Self::Theme, Self::Editor];

    pub fn label(self) -> &'static str {
        match self {
            Self::Keymaps => "Keymaps",
            Self::Theme => "Theme",
            Self::Editor => "Editor",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::Keymaps => Self::Theme,
            Self::Theme => Self::Editor,
            Self::Editor => Self::Keymaps,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Keymaps => Self::Editor,
            Self::Theme => Self::Keymaps,
            Self::Editor => Self::Theme,
        }
    }
}

/// Capture mode — present while waiting for the next key event to
/// become the new chord for `action_name`.
#[derive(Debug, Clone)]
pub struct CaptureState {
    pub action_name: String,
    /// When non-empty, the captured chord collides with another action;
    /// the renderer surfaces it so the user can pick a different chord.
    pub conflict_with: Option<String>,
    /// Last KeyEvent the user pressed that didn't yield a valid chord
    /// string. Surfaces as inline hint without forcing the user out of
    /// capture mode.
    pub last_invalid: Option<String>,
}

#[derive(Debug)]
pub struct SettingsPageState {
    pub section: SettingsSection,
    pub keymap_cursor: usize,
    pub theme_cursor: usize,
    pub editor_cursor: usize,
    pub capture: Option<CaptureState>,
    /// Captured at open so the applier can `save_config` without
    /// re-resolving on every mutation.
    pub config_path: PathBuf,
    pub last_error: Option<String>,
}

impl SettingsPageState {
    pub fn new(config_path: PathBuf) -> Self {
        Self {
            section: SettingsSection::default(),
            keymap_cursor: 0,
            theme_cursor: 0,
            editor_cursor: 0,
            capture: None,
            config_path,
            last_error: None,
        }
    }

    pub fn active_cursor(&self) -> usize {
        match self.section {
            SettingsSection::Keymaps => self.keymap_cursor,
            SettingsSection::Theme => self.theme_cursor,
            SettingsSection::Editor => self.editor_cursor,
        }
    }

    pub fn active_cursor_mut(&mut self) -> &mut usize {
        match self.section {
            SettingsSection::Keymaps => &mut self.keymap_cursor,
            SettingsSection::Theme => &mut self.theme_cursor,
            SettingsSection::Editor => &mut self.editor_cursor,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_cycles_forward_through_all_three() {
        let mut s = SettingsSection::default();
        assert_eq!(s, SettingsSection::Keymaps);
        s = s.next();
        assert_eq!(s, SettingsSection::Theme);
        s = s.next();
        assert_eq!(s, SettingsSection::Editor);
        s = s.next();
        assert_eq!(s, SettingsSection::Keymaps);
    }

    #[test]
    fn section_cycles_backward() {
        let s = SettingsSection::Keymaps.prev();
        assert_eq!(s, SettingsSection::Editor);
        assert_eq!(s.prev(), SettingsSection::Theme);
    }

    #[test]
    fn active_cursor_tracks_section() {
        let mut state = SettingsPageState::new(PathBuf::from("/tmp/cfg.toml"));
        state.keymap_cursor = 3;
        state.theme_cursor = 1;
        state.editor_cursor = 2;

        state.section = SettingsSection::Keymaps;
        assert_eq!(state.active_cursor(), 3);
        state.section = SettingsSection::Theme;
        assert_eq!(state.active_cursor(), 1);
        state.section = SettingsSection::Editor;
        assert_eq!(state.active_cursor(), 2);
    }

    #[test]
    fn active_cursor_mut_writes_into_active_section() {
        let mut state = SettingsPageState::new(PathBuf::from("/tmp/cfg.toml"));
        state.section = SettingsSection::Theme;
        *state.active_cursor_mut() = 5;
        assert_eq!(state.theme_cursor, 5);
        assert_eq!(state.keymap_cursor, 0);
        assert_eq!(state.editor_cursor, 0);
    }

    #[test]
    fn section_label_round_trips_through_all() {
        for s in SettingsSection::ALL {
            assert!(!s.label().is_empty());
        }
    }
}
