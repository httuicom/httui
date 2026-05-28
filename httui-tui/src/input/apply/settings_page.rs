//! Settings page action appliers — navigation, keymap rebind/reset,
//! conflict detection, persistence.

use crate::app::{App, CaptureState, SettingsPageState, SettingsSection, StatusKind};
use crate::buffer::{Cursor, Segment};
use crate::config::save_config;
use crate::input::action::Action;
use crate::input::keychord::{chord_string_from_key, parse_key_chord};
use crate::vim::mode::Mode;

/// Pseudo action-name used by the `editor.toggle_mode_key` row in the
/// Keymaps section. It is NOT a `standard_actions()` entry — the
/// applier discriminates on this sentinel to route writes either to
/// `cfg.keymap` or to `cfg.editor.toggle_mode_key`.
pub(crate) const TOGGLE_MODE_ROW_NAME: &str = "editor.toggle_mode";

/// Empty string = unbound. Reset puts the row back here. Kept in
/// sync with `crate::config::default_toggle_mode_key` (asserted by test).
const TOGGLE_MODE_DEFAULT: &str = "";

/// One row in the Keymaps section. The `name` is what the renderer
/// shows on the left and what conflict-detection looks up; the
/// `default_chord` powers "reset to default" and the initial value.
/// `is_toggle_mode` discriminates between standard-keymap rows and the
/// special toggle-mode row.
#[derive(Debug, Clone, Copy)]
pub(crate) struct KeymapRowSpec {
    pub name: &'static str,
    pub default_chord: &'static str,
    pub is_toggle_mode: bool,
}

/// All rebindable rows in the Keymaps section, in display order. The
/// toggle-mode pseudo-row lands at the end — first the well-known
/// editing/navigation actions (config display order), then the
/// special mode toggle.
pub(crate) fn keymap_rows() -> Vec<KeymapRowSpec> {
    let mut rows: Vec<KeymapRowSpec> = crate::input::keymap::standard_actions()
        .into_iter()
        .map(|spec| KeymapRowSpec {
            name: spec.name,
            default_chord: spec.default_chord,
            is_toggle_mode: false,
        })
        .collect();
    rows.push(KeymapRowSpec {
        name: TOGGLE_MODE_ROW_NAME,
        default_chord: TOGGLE_MODE_DEFAULT,
        is_toggle_mode: true,
    });
    rows
}

pub(crate) fn keymap_row_count() -> usize {
    keymap_rows().len()
}

/// Resolved view of a single Keymaps row, prepared by the applier
/// before render. Decouples the renderer from the `App` so it can be
/// unit-tested with a hand-built `Vec<KeymapRowView>`.
#[derive(Debug, Clone)]
pub struct KeymapRowView {
    /// Action / pseudo-row name (`copy`, `editor.toggle_mode`, …).
    pub name: String,
    /// Cosmetic label shown to the user.
    pub display: String,
    /// Chord string currently in effect.
    pub chord: String,
    /// `Some(other_name)` when this chord is also bound to another
    /// action. Detected by [`find_chord_owner`] at prep time.
    pub conflict_with: Option<String>,
}

/// Render-time snapshot of the Keymaps section. Built once per
/// frame so [`current_chord`] / [`find_chord_owner`] don't run from
/// inside the renderer.
pub fn keymap_view(app: &App) -> Vec<KeymapRowView> {
    let rows = keymap_rows();
    rows.iter()
        .map(|row| {
            let chord = current_chord(app, *row);
            let conflict = find_chord_owner(app, &chord, row.name);
            KeymapRowView {
                name: row.name.to_string(),
                display: display_name(row.name).to_string(),
                chord,
                conflict_with: conflict,
            }
        })
        .collect()
}

/// Cosmetic-only rename for display. The internal sentinel
/// `editor.toggle_mode` is shown as `[ vim ↔ standard toggle ]`.
pub fn display_name(name: &str) -> &str {
    if name == TOGGLE_MODE_ROW_NAME {
        "[ vim ↔ standard toggle ]"
    } else {
        name
    }
}

/// Look up the chord currently bound to `row` in `app.config`. For
/// the standard actions, falls back to the built-in default when the
/// user hasn't overridden it (mirrors how `resolve_standard_keymap`
/// resolves). For the toggle row, returns the configured value
/// verbatim — it's always populated (the field is `String`, not
/// `Option<String>`).
pub(crate) fn current_chord(app: &App, row: KeymapRowSpec) -> String {
    if row.is_toggle_mode {
        app.config.editor.toggle_mode_key.clone()
    } else {
        app.config
            .keymap
            .chord_for(row.name)
            .unwrap_or(row.default_chord)
            .to_string()
    }
}

pub(crate) fn apply_settings_page(app: &mut App, action: Action) {
    match action {
        Action::OpenSettings => dispatch_open_settings(app),
        Action::CloseSettingsPage => close_settings_page(app),
        Action::SettingsNextSection => with_page(app, |s| {
            s.section = s.section.next();
            s.capture = None;
            s.last_error = None;
        }),
        Action::SettingsPrevSection => with_page(app, |s| {
            s.section = s.section.prev();
            s.capture = None;
            s.last_error = None;
        }),
        Action::SettingsMoveCursor(delta) => move_cursor(app, delta),
        Action::SettingsActivateRow => activate_row(app),
        Action::SettingsCancelCapture => with_page(app, |s| s.capture = None),
        Action::SettingsCommitCapture(key) => commit_capture(app, key),
        Action::SettingsResetBinding => reset_binding(app),
        _ => {}
    }
}

fn activate_row(app: &mut App) {
    let section = match app.modal.as_ref() {
        Some(crate::modal::Modal::Settings(s)) => s.section,
        _ => return,
    };
    match section {
        SettingsSection::Keymaps => begin_capture(app),
        SettingsSection::Theme => apply_selected_theme(app),
        SettingsSection::Editor => toggle_selected_editor_row(app),
    }
}

fn toggle_selected_editor_row(app: &mut App) {
    let idx = match app.modal.as_ref() {
        Some(crate::modal::Modal::Settings(s)) => s.editor_cursor,
        _ => return,
    };
    let Some(row) = EDITOR_ROWS.get(idx) else {
        return;
    };
    match row.kind {
        EditorRowKind::Mode => {
            app.config.editor.mode = match app.config.editor.mode {
                crate::config::EditorMode::Standard => crate::config::EditorMode::Vim,
                crate::config::EditorMode::Vim => crate::config::EditorMode::Standard,
            };
            // The editor scope reads `app.config.editor.mode` on every
            // input event so the toggle takes effect on the next key
            // — no extra refresh needed.
        }
        EditorRowKind::Mouse => {
            app.config.mouse_enabled = !app.config.mouse_enabled;
        }
    }
    persist(app);
}

/// `Alt+,` dispatcher. When the cursor sits on a DB or HTTP block
/// this opens the per-block settings modal (limit/timeout); anywhere
/// else it opens the app-wide Settings page.
fn dispatch_open_settings(app: &mut App) {
    if cursor_on_settings_block(app) {
        if let Err(msg) = crate::commands::db::open_db_settings_modal(app) {
            app.set_status(StatusKind::Error, msg);
        }
    } else if let Err(msg) = open_settings_page(app) {
        app.set_status(StatusKind::Error, msg);
    }
}

fn cursor_on_settings_block(app: &App) -> bool {
    let Some(doc) = app.document() else {
        return false;
    };
    let segment_idx = match doc.cursor() {
        Cursor::InBlock { segment_idx, .. } | Cursor::InBlockResult { segment_idx, .. } => {
            segment_idx
        }
        _ => return false,
    };
    matches!(
        doc.segments().get(segment_idx),
        Some(Segment::Block(b)) if b.is_db() || b.is_http()
    )
}

pub(crate) fn open_settings_page(app: &mut App) -> Result<(), String> {
    let path =
        crate::config::default_config_path().map_err(|e| format!("resolve config path: {e}"))?;
    app.modal = Some(crate::modal::Modal::Settings(SettingsPageState::new(path)));
    app.vim.mode = Mode::Modal;
    app.vim.reset_pending();
    Ok(())
}

fn close_settings_page(app: &mut App) {
    if matches!(app.modal, Some(crate::modal::Modal::Settings(_))) {
        app.modal = None;
    }
    app.vim.enter_normal();
}

fn with_page(app: &mut App, f: impl FnOnce(&mut SettingsPageState)) {
    if let Some(crate::modal::Modal::Settings(s)) = app.modal.as_mut() {
        f(s);
    }
}

fn move_cursor(app: &mut App, delta: i32) {
    let row_count = active_row_count(app);
    with_page(app, |s| {
        if row_count == 0 {
            return;
        }
        let last = row_count.saturating_sub(1) as i64;
        let cur = s.active_cursor() as i64;
        *s.active_cursor_mut() = (cur + delta as i64).clamp(0, last) as usize;
    });
}

fn active_row_count(app: &App) -> usize {
    let Some(crate::modal::Modal::Settings(s)) = app.modal.as_ref() else {
        return 0;
    };
    match s.section {
        SettingsSection::Keymaps => keymap_row_count(),
        SettingsSection::Theme => THEME_PRESETS.len(),
        SettingsSection::Editor => EDITOR_ROWS.len(),
    }
}

/// One row in the Editor section. The applier dispatches `Enter` to
/// the matching `editor_*` mutator by [`EditorRowKind`].
///
/// Rows are limited to settings that actually drive behaviour in the
/// running TUI — `Mode` swaps the input engine live; `Mouse` flips
/// the persisted flag (`terminal::setup` reads it on launch). Dead
/// config fields like `show_line_numbers` aren't surfaced here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorRowKind {
    Mode,
    Mouse,
}

#[derive(Debug, Clone, Copy)]
struct EditorRowSpec {
    kind: EditorRowKind,
    label: &'static str,
}

const EDITOR_ROWS: &[EditorRowSpec] = &[
    EditorRowSpec {
        kind: EditorRowKind::Mode,
        label: "Editor mode",
    },
    EditorRowSpec {
        kind: EditorRowKind::Mouse,
        label: "Mouse enabled",
    },
];

/// Resolved view of an Editor row — label + current value formatted
/// the way the renderer should display it.
#[derive(Debug, Clone)]
pub struct EditorRowView {
    pub label: String,
    pub value: String,
    /// Right-column hint (contextual note about what the row means
    /// or when it's effective).
    pub hint: String,
}

pub fn editor_view(app: &App) -> Vec<EditorRowView> {
    EDITOR_ROWS
        .iter()
        .map(|row| {
            let value = match row.kind {
                EditorRowKind::Mode => match app.config.editor.mode {
                    crate::config::EditorMode::Standard => "Standard".into(),
                    crate::config::EditorMode::Vim => "Vim".into(),
                },
                EditorRowKind::Mouse => bool_label(app.config.mouse_enabled),
            };
            let hint = match row.kind {
                EditorRowKind::Mode => {
                    "Standard: arrows + Ctrl-Z/Y/C/V/S. Vim: full modal engine.".into()
                }
                EditorRowKind::Mouse => "Takes effect on next launch.".into(),
            };
            EditorRowView {
                label: row.label.to_string(),
                value,
                hint,
            }
        })
        .collect()
}

fn bool_label(on: bool) -> String {
    if on {
        "ON".into()
    } else {
        "OFF".into()
    }
}

/// Built-in theme presets, in display order — also the cursor index
/// space for the Theme section.
pub const THEME_PRESETS: &[&str] =
    &["default-dark", "default-light", "terminal-native", "tokyo-night"];

/// Resolved view of a single Theme row.
#[derive(Debug, Clone)]
pub struct ThemeRowView {
    pub name: String,
    pub is_active: bool,
}

/// Snapshot of the Theme section: every preset + which one is live.
pub fn theme_view(app: &App) -> Vec<ThemeRowView> {
    THEME_PRESETS
        .iter()
        .map(|name| ThemeRowView {
            name: (*name).to_string(),
            is_active: app.config.theme == *name,
        })
        .collect()
}

fn apply_selected_theme(app: &mut App) {
    let idx = match app.modal.as_ref() {
        Some(crate::modal::Modal::Settings(s)) => s.theme_cursor,
        _ => return,
    };
    let Some(preset) = THEME_PRESETS.get(idx) else {
        return;
    };
    app.config.theme = (*preset).to_string();
    crate::ui::theme::init(preset, &app.config.palette);
    persist(app);
}

fn selected_keymap_row(app: &App) -> Option<KeymapRowSpec> {
    let s = match app.modal.as_ref()? {
        crate::modal::Modal::Settings(s) => s,
        _ => return None,
    };
    if s.section != SettingsSection::Keymaps {
        return None;
    }
    keymap_rows().get(s.keymap_cursor).copied()
}

fn begin_capture(app: &mut App) {
    let Some(row) = selected_keymap_row(app) else {
        return;
    };
    with_page(app, |s| {
        s.capture = Some(CaptureState {
            action_name: row.name.to_string(),
            conflict_with: None,
            last_invalid: None,
        });
        s.last_error = None;
    });
}

/// Convert a captured `KeyEvent` to a chord string, validate it
/// parses, write it back to `Config`, persist, and clear capture.
/// Invalid events (Esc / `KeyCode::Null` / unsupported codes) keep
/// the modal in capture mode so the user can press a different key.
fn commit_capture(app: &mut App, key: crossterm::event::KeyEvent) {
    let Some(row_name) = capture_target_name(app) else {
        return;
    };
    let Some(row) = keymap_rows().into_iter().find(|r| r.name == row_name) else {
        return;
    };
    let Some(chord_str) = chord_string_from_key(key) else {
        with_page(app, |s| {
            if let Some(c) = s.capture.as_mut() {
                c.last_invalid = Some(format!("{key:?}"));
            }
        });
        return;
    };
    if parse_key_chord(&chord_str).is_none() {
        with_page(app, |s| {
            if let Some(c) = s.capture.as_mut() {
                c.last_invalid = Some(chord_str.clone());
            }
        });
        return;
    }
    write_chord(app, row, &chord_str);
    persist(app);
    refresh_resolved_keymap(app);
    with_page(app, |s| s.capture = None);
    refresh_conflict_for_selected(app);
}

fn capture_target_name(app: &App) -> Option<String> {
    let s = match app.modal.as_ref()? {
        crate::modal::Modal::Settings(s) => s,
        _ => return None,
    };
    s.capture.as_ref().map(|c| c.action_name.clone())
}

fn write_chord(app: &mut App, row: KeymapRowSpec, chord: &str) {
    if row.is_toggle_mode {
        app.config.editor.toggle_mode_key = chord.to_string();
    } else {
        app.config.keymap.set(row.name, chord.to_string());
    }
}

fn reset_binding(app: &mut App) {
    let Some(row) = selected_keymap_row(app) else {
        return;
    };
    if row.is_toggle_mode {
        app.config.editor.toggle_mode_key = row.default_chord.to_string();
    } else {
        // Drop the override so the resolver falls back to the
        // built-in default — future default changes then propagate
        // automatically to anyone who reset.
        app.config.keymap.remove(row.name);
    }
    persist(app);
    refresh_resolved_keymap(app);
    refresh_conflict_for_selected(app);
}

/// Rebuild `app.standard_keymap` from `app.config.keymap` so the
/// input router sees the rebind on the very next keystroke. Without
/// this the keymap only refreshes on TUI restart, since the resolved
/// chord→action list is cached at construction time.
fn refresh_resolved_keymap(app: &mut App) {
    app.standard_keymap = crate::input::keymap::resolve_standard_keymap(&app.config.keymap);
}

/// Recompute the `conflict_with` annotation on the selected row's
/// `CaptureState` if one exists, otherwise no-op. Called after every
/// write so the renderer surfaces the new state without the user
/// having to move the cursor.
fn refresh_conflict_for_selected(app: &mut App) {
    let Some(row) = selected_keymap_row(app) else {
        return;
    };
    let chord_here = current_chord(app, row);
    let other = find_chord_owner(app, &chord_here, row.name);
    with_page(app, |s| {
        if let Some(c) = s.capture.as_mut() {
            c.conflict_with = other.clone();
        } else {
            // Even when capture is closed we remember the latest
            // conflict so the row's secondary line shows it. We
            // stash it via a fresh CaptureState? No — keep state
            // minimal: conflict is recomputed on render via
            // `find_chord_owner`. Nothing to write here.
        }
    });
    let _ = other; // silence the unused-var lint in the second branch
}

/// Returns the name of the OTHER row whose current chord equals
/// `chord` (the freshly-bound one). `excluding` is the row we just
/// edited — never returns it. Comparison is via the parser so two
/// chord strings that resolve to the same chord (e.g. case
/// differences) still surface as a conflict.
pub(crate) fn find_chord_owner(app: &App, chord: &str, excluding: &str) -> Option<String> {
    let target = parse_key_chord(chord)?;
    for row in keymap_rows() {
        if row.name == excluding {
            continue;
        }
        let other_chord = current_chord(app, row);
        if let Some(other) = parse_key_chord(&other_chord) {
            if chords_equal(target, other) {
                return Some(row.name.to_string());
            }
        }
    }
    None
}

fn chords_equal(a: crate::input::keychord::KeyChord, b: crate::input::keychord::KeyChord) -> bool {
    a.code == b.code && a.ctrl == b.ctrl && a.alt == b.alt && a.shift == b.shift
}

/// Write the current `app.config` to its file. On error, the message
/// is surfaced in the page footer so the user sees the failure
/// without losing the in-memory change.
fn persist(app: &mut App) {
    let path = match app.modal.as_ref() {
        Some(crate::modal::Modal::Settings(s)) => s.config_path.clone(),
        _ => return,
    };
    let result = save_config(&path, &app.config);
    with_page(app, |s| {
        s.last_error = match result {
            Ok(()) => None,
            Err(e) => Some(format!("{e}")),
        };
    });
}
