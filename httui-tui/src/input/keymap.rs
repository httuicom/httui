//! Config-driven keymap for the Standard editing profile.
//!
//! The Standard profile's chord→[`Action`] bindings are not hard-coded:
//! they come from `KeymapConfig` (config.toml `[keymap]`). This module
//! owns three things:
//!
//! 1. [`standard_actions`] — the table of bindable Standard actions,
//!    each with a stable config name, a default chord, and its
//!    `Action`. Single source of truth for both the defaults and the
//!    set of names accepted in `[keymap]`.
//! 2. [`resolve_standard_keymap`] — turns a `KeymapConfig` into a
//!    runtime lookup list.
//! 3. [`lookup`] — decodes a key event against a resolved keymap.
//!
//! Vim's modal grammar is deliberately NOT covered (TD2 — no further
//! investment in the vim engine); only the Standard profile is
//! config-driven.

use crossterm::event::KeyEvent;

use crate::config::KeymapConfig;
use crate::input::action::Action;
use crate::input::keychord::{parse_key_chord, KeyChord};
use crate::input::types::Motion;

/// One bindable Standard-mode action.
///
/// - `name` — the stable key used in config.toml `[keymap]`.
/// - `default_chord` — the built-in chord, parsed by
///   [`crate::input::keychord::parse_key_chord`].
/// - `action` — the [`Action`] emitted when the chord is pressed.
pub struct ActionSpec {
    pub name: &'static str,
    pub default_chord: &'static str,
    pub action: Action,
}

/// All bindable Standard-mode actions, in config-file display order
/// (movement → selection → edit → clipboard → undo → save → block).
///
/// `InsertChar(c)` is intentionally absent — it is parametric in the
/// character and stays a fallback in `standard::resolve`, not a
/// table binding.
pub fn standard_actions() -> Vec<ActionSpec> {
    fn spec(name: &'static str, default_chord: &'static str, action: Action) -> ActionSpec {
        ActionSpec {
            name,
            default_chord,
            action,
        }
    }
    vec![
        // Movement.
        spec("move_up", "up", Action::Motion(Motion::Up, 1)),
        spec("move_down", "down", Action::Motion(Motion::Down, 1)),
        spec("move_left", "left", Action::Motion(Motion::Left, 1)),
        spec("move_right", "right", Action::Motion(Motion::Right, 1)),
        spec(
            "move_line_start",
            "home",
            Action::Motion(Motion::LineStart, 1),
        ),
        spec("move_line_end", "end", Action::Motion(Motion::LineEnd, 1)),
        spec(
            "move_page_up",
            "pageup",
            Action::Motion(Motion::HalfPageUp, 1),
        ),
        spec(
            "move_page_down",
            "pagedown",
            Action::Motion(Motion::HalfPageDown, 1),
        ),
        // Selection — Shift + motion.
        spec("select_up", "shift+up", Action::SelectExtend(Motion::Up)),
        spec(
            "select_down",
            "shift+down",
            Action::SelectExtend(Motion::Down),
        ),
        spec(
            "select_left",
            "shift+left",
            Action::SelectExtend(Motion::Left),
        ),
        spec(
            "select_right",
            "shift+right",
            Action::SelectExtend(Motion::Right),
        ),
        spec(
            "select_line_start",
            "shift+home",
            Action::SelectExtend(Motion::LineStart),
        ),
        spec(
            "select_line_end",
            "shift+end",
            Action::SelectExtend(Motion::LineEnd),
        ),
        // Edit primitives.
        spec("insert_newline", "enter", Action::InsertNewline),
        spec("delete_back", "backspace", Action::DeleteBackwardStandard),
        spec("delete_forward", "delete", Action::DeleteForward),
        // Clipboard.
        spec("copy", "ctrl+c", Action::Copy),
        spec("cut", "ctrl+x", Action::Cut),
        spec("paste", "ctrl+v", Action::PasteSystem),
        // Undo / redo.
        spec("undo", "ctrl+z", Action::Undo),
        spec("redo", "ctrl+y", Action::Redo),
        // Save.
        spec("save", "ctrl+s", Action::WriteFile),
        // Block execution. `explain_block` defaults to `alt+x` rather
        // than `ctrl+shift+x`: a chord matcher ignores Shift on letter
        // keys (Shift on a letter is unreliable across terminals), so
        // `ctrl+shift+x` would collide with `cut` (`ctrl+x`).
        //
        // Business actions below are vim-only in the modal engine
        // (`g`-prefixed chords); `g` is a literal letter in Standard,
        // so they get fresh `Alt+letter` defaults here. F-keys were
        // tried first (tui-V03 keymap fase 2) but rejected on UX
        // grounds — distant from the home row. Alt+letter slots avoid
        // the edit-op collisions of Ctrl+letter and remain reachable
        // in terminals without the kitty keyboard protocol.
        //
        // `rerun_last_block` lands on `alt+.` rather than the obvious
        // `alt+shift+r`: the chord matcher ignores `Shift` on `Char`
        // codes (Shift on a letter is unreliable across terminals), so
        // `alt+shift+r` would collide with `alt+r` (run).
        spec("explain_block", "alt+x", Action::ExplainBlock),
        spec("run_block", "alt+r", Action::RunBlock),
        spec("rerun_last_block", "alt+.", Action::RerunLastBlock),
        spec("jump_next_block", "alt+down", Action::JumpNextBlock),
        spec("jump_prev_block", "alt+up", Action::JumpPrevBlock),
        // Panels & pickers.
        spec("open_help", "alt+?", Action::OpenHelp),
        spec("open_tab_picker", "alt+t", Action::OpenTabPicker),
        spec(
            "open_environment_picker",
            "alt+e",
            Action::OpenEnvironmentPicker,
        ),
        spec("open_block_history", "alt+h", Action::OpenBlockHistory),
        spec("open_export_picker", "alt+g", Action::OpenDbExportPicker),
        spec("open_block_settings", "alt+,", Action::OpenDbSettingsModal),
        spec(
            "open_block_template_picker",
            "alt+n",
            Action::OpenBlockTemplatePicker,
        ),
        spec(
            "open_connections_page",
            "alt+p",
            Action::OpenConnectionsPage,
        ),
        spec("open_envs_page", "alt+i", Action::OpenEnvsPage),
        spec("open_vault_picker", "alt+;", Action::OpenVaultPicker),
        spec("quick_open", "ctrl+p", Action::EnterQuickOpen),
        spec("content_search", "ctrl+f", Action::OpenContentSearch),
        // Workspace.
        spec("tree_toggle", "ctrl+b", Action::TreeToggle),
        spec("git_panel_toggle", "ctrl+g", Action::GitPanelToggle),
        spec("tab_next", "ctrl+pagedown", Action::TabNext),
        spec("tab_prev", "ctrl+pageup", Action::TabPrev),
        // `Tab` / `Shift+Tab` reuse the same `TabNext`/`TabPrev`
        // semantics as `gt`/`gT` and `Ctrl+PageDown`/`Up`: cycle the
        // focused block's result-panel tab when the cursor is on the
        // block, fall through to next/prev editor tab when in prose.
        spec("tab_next_key", "tab", Action::TabNext),
        spec("tab_prev_key", "shift+tab", Action::TabPrev),
        spec("save_all", "ctrl+alt+s", Action::WriteAll),
    ]
}

/// Resolve a [`KeymapConfig`] into a runtime lookup list. For each
/// known action, parse its configured chord — falling back to the
/// built-in default when the action is unset OR the configured string
/// is unparseable — and pair it with the action.
pub fn resolve_standard_keymap(cfg: &KeymapConfig) -> Vec<(KeyChord, Action)> {
    standard_actions()
        .into_iter()
        .filter_map(|spec| {
            let configured = cfg.chord_for(spec.name).and_then(parse_key_chord);
            let chord = configured.or_else(|| parse_key_chord(spec.default_chord))?;
            Some((chord, spec.action))
        })
        .collect()
}

/// Decode `key` against a resolved keymap. First match wins —
/// [`resolve_standard_keymap`] preserves [`standard_actions`] order.
///
/// macOS Option-key compat: when `Use Option as Meta key` is OFF
/// (Terminal/iTerm2 default), `Option+letter` emits the Unicode
/// composed glyph instead of `Alt+letter`. We unmap a few of those
/// glyphs back to the equivalent Alt chord so the user doesn't have
/// to learn a different binding per terminal.
pub fn lookup(keymap: &[(KeyChord, Action)], key: KeyEvent) -> Option<Action> {
    let key = unmap_macos_option(key);
    keymap
        .iter()
        .find(|(chord, _)| chord.matches(key))
        .map(|(_, action)| *action)
}

fn unmap_macos_option(key: KeyEvent) -> KeyEvent {
    use crossterm::event::KeyCode;
    let KeyEvent {
        code, modifiers, ..
    } = key;
    if modifiers.contains(crossterm::event::KeyModifiers::ALT) {
        // Already saw Alt set — terminal IS sending Meta. Pass through.
        return key;
    }
    let mapped_char = match code {
        KeyCode::Char(c) => macos_option_to_ascii(c),
        _ => None,
    };
    if let Some(c) = mapped_char {
        KeyEvent::new(
            KeyCode::Char(c),
            modifiers | crossterm::event::KeyModifiers::ALT,
        )
    } else {
        key
    }
}

/// Maps the Unicode glyph that macOS emits for Option+letter (default
/// keyboard, no "Use Option as Meta key") back to the underlying
/// letter. Only covers letters bound to actions in `standard_actions`
/// — adding more is a one-liner.
fn macos_option_to_ascii(c: char) -> Option<char> {
    Some(match c {
        'ˆ' => 'i', // Option+i (dead key)
        '√' => 'v', // Option+v
        '®' => 'r', // Option+r
        '†' => 't', // Option+t
        '˙' => 'h', // Option+h
        '©' => 'g', // Option+g
        '≤' => ',', // Option+,
        '˜' => 'n', // Option+n
        'π' => 'p', // Option+p
        '´' => 'e', // Option+e (dead-key fallback)
        '¿' => '?', // Option+? — often Shift-1 variant
        '≈' => 'x', // Option+x
        '…' => '.', // Option+.
        _ => return None,
    })
}

/// Actions that fire regardless of the active editor profile (vim vs
/// standard). They are bound only in `standard_keymap` — vim's mode
/// parsers don't know them — so the Editor scope checks this list
/// before delegating to the vim engine. Because the check lives in
/// the Editor scope, modal/popup scopes naturally block these
/// shortcuts when open (no leak by construction).
pub fn is_editor_global_shortcut(action: Action) -> bool {
    matches!(
        action,
        Action::RunBlock
            | Action::RerunLastBlock
            | Action::ExplainBlock
            | Action::JumpNextBlock
            | Action::JumpPrevBlock
            | Action::OpenHelp
            | Action::OpenTabPicker
            | Action::OpenEnvironmentPicker
            | Action::OpenBlockHistory
            | Action::OpenDbExportPicker
            | Action::OpenDbSettingsModal
            | Action::OpenBlockTemplatePicker
            | Action::OpenConnectionsPage
            | Action::OpenEnvsPage
            | Action::OpenVaultPicker
            | Action::EnterQuickOpen
            | Action::OpenContentSearch
            | Action::TreeToggle
            | Action::GitPanelToggle
            | Action::TabNext
            | Action::TabPrev
            | Action::WriteAll
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn action_names_are_unique() {
        let mut names: Vec<&str> = standard_actions().iter().map(|s| s.name).collect();
        let total = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), total, "duplicate action name in the table");
    }

    #[test]
    fn every_default_chord_parses() {
        for spec in standard_actions() {
            assert!(
                parse_key_chord(spec.default_chord).is_some(),
                "default chord `{}` for `{}` does not parse",
                spec.default_chord,
                spec.name,
            );
        }
    }

    #[test]
    fn default_config_resolves_one_entry_per_action() {
        let keymap = resolve_standard_keymap(&KeymapConfig::default());
        assert_eq!(keymap.len(), standard_actions().len());
    }

    #[test]
    fn lookup_finds_default_bindings() {
        let keymap = resolve_standard_keymap(&KeymapConfig::default());
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::Copy),
        );
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Up, KeyModifiers::NONE)),
            Some(Action::Motion(Motion::Up, 1)),
        );
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Up, KeyModifiers::SHIFT)),
            Some(Action::SelectExtend(Motion::Up)),
        );
    }

    #[test]
    fn lookup_finds_business_action_bindings() {
        // The vim-only business actions get fresh Standard chords.
        let keymap = resolve_standard_keymap(&KeymapConfig::default());
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Char('r'), KeyModifiers::ALT)),
            Some(Action::RunBlock),
        );
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Char('p'), KeyModifiers::CONTROL)),
            Some(Action::EnterQuickOpen),
        );
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Char('b'), KeyModifiers::CONTROL)),
            Some(Action::TreeToggle),
        );
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Down, KeyModifiers::ALT)),
            Some(Action::JumpNextBlock),
        );
    }

    #[test]
    fn lookup_finds_vault_picker_on_alt_semicolon() {
        //  hotfix 3: tested chord migration — Alt+W and
        // Alt+K both got intercepted by the user's terminal host
        // (WezTerm). Alt+; passes through; lockdown the binding
        // here so future keymap refactors don't silently drop it.
        let keymap = resolve_standard_keymap(&KeymapConfig::default());
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Char(';'), KeyModifiers::ALT)),
            Some(Action::OpenVaultPicker),
        );
    }

    #[test]
    fn lookup_returns_none_for_unbound_key() {
        let keymap = resolve_standard_keymap(&KeymapConfig::default());
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Char('a'), KeyModifiers::NONE)),
            None,
        );
    }

    #[test]
    fn configured_override_replaces_the_default_chord() {
        // A partial `[keymap]` (only `save` set) — every other action
        // still resolves via its built-in default.
        let cfg: KeymapConfig = toml::from_str("save = \"f9\"\n").unwrap();
        let keymap = resolve_standard_keymap(&cfg);
        assert_eq!(
            lookup(&keymap, ev(KeyCode::F(9), KeyModifiers::NONE)),
            Some(Action::WriteFile),
        );
        // The old default chord no longer triggers save.
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            None,
        );
        // An untouched action keeps its default.
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::Copy),
        );
    }

    #[test]
    fn unparseable_override_falls_back_to_default() {
        let cfg: KeymapConfig = toml::from_str("copy = \"not a chord\"\n").unwrap();
        let keymap = resolve_standard_keymap(&cfg);
        // Falls back to the built-in `ctrl+c`.
        assert_eq!(
            lookup(&keymap, ev(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::Copy),
        );
    }
}
