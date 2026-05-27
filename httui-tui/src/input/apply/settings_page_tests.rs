//! Settings page applier tests.
//!
//! Uses the same async tempdir+sqlite fixture as `envs_page_tests`
//! so the applier runs against a real `App`.

#![cfg(test)]

use crate::app::{App, CaptureState, SettingsPageState, SettingsSection};
use crate::config::Config;
use crate::input::action::Action;
use crate::modal::Modal;
use crate::vault::ResolvedVault;
use crate::vim::mode::Mode;
use httui_core::db::init_db;
use tempfile::TempDir;

use super::settings_page::{apply_settings_page, keymap_row_count};

async fn app_fixture(body: &str) -> (App, TempDir, TempDir) {
    let data = TempDir::new().unwrap();
    let vault = TempDir::new().unwrap();
    std::fs::write(vault.path().join("note.md"), body).unwrap();
    let pool = init_db(data.path()).await.unwrap();
    let resolved = ResolvedVault {
        vault: vault.path().to_path_buf(),
    };
    let app = App::new(Config::default(), resolved, pool);
    (app, data, vault)
}

fn page(app: &App) -> Option<&SettingsPageState> {
    if let Some(Modal::Settings(s)) = app.modal.as_ref() {
        Some(s)
    } else {
        None
    }
}

fn page_mut(app: &mut App) -> Option<&mut SettingsPageState> {
    if let Some(Modal::Settings(s)) = app.modal.as_mut() {
        Some(s)
    } else {
        None
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn open_settings_page_installs_modal_and_enters_modal_mode() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    let s = page(&app).expect("Settings modal open");
    assert_eq!(s.section, SettingsSection::Keymaps);
    assert_eq!(s.keymap_cursor, 0);
    assert!(s.capture.is_none());
    assert_eq!(app.vim.mode, Mode::Modal);
}

#[tokio::test(flavor = "multi_thread")]
async fn close_settings_page_clears_modal_and_restores_normal() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    apply_settings_page(&mut app, Action::CloseSettingsPage);
    assert!(app.modal.is_none());
    assert_eq!(app.vim.mode, Mode::Normal);
}

#[tokio::test(flavor = "multi_thread")]
async fn next_prev_cycle_sections() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    apply_settings_page(&mut app, Action::SettingsNextSection);
    assert_eq!(page(&app).unwrap().section, SettingsSection::Theme);
    apply_settings_page(&mut app, Action::SettingsNextSection);
    assert_eq!(page(&app).unwrap().section, SettingsSection::Editor);
    apply_settings_page(&mut app, Action::SettingsNextSection);
    assert_eq!(page(&app).unwrap().section, SettingsSection::Keymaps);
    apply_settings_page(&mut app, Action::SettingsPrevSection);
    assert_eq!(page(&app).unwrap().section, SettingsSection::Editor);
}

#[tokio::test(flavor = "multi_thread")]
async fn move_cursor_clamps_within_keymap_section() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    apply_settings_page(&mut app, Action::SettingsMoveCursor(-5));
    assert_eq!(page(&app).unwrap().keymap_cursor, 0);
    apply_settings_page(&mut app, Action::SettingsMoveCursor(3));
    assert_eq!(page(&app).unwrap().keymap_cursor, 3);
    // Push past the end — clamps to last row.
    let last = keymap_row_count() - 1;
    apply_settings_page(&mut app, Action::SettingsMoveCursor(9999));
    assert_eq!(page(&app).unwrap().keymap_cursor, last);
}

#[tokio::test(flavor = "multi_thread")]
async fn move_cursor_clamps_to_last_row_per_section() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    apply_settings_page(&mut app, Action::SettingsNextSection); // Theme
    apply_settings_page(&mut app, Action::SettingsMoveCursor(99));
    // Theme has 3 presets → last index is 2.
    assert_eq!(page(&app).unwrap().theme_cursor, 2);
    // Editor section has 2 rows → last index is 1.
    apply_settings_page(&mut app, Action::SettingsNextSection); // Editor
    apply_settings_page(&mut app, Action::SettingsMoveCursor(99));
    assert_eq!(page(&app).unwrap().editor_cursor, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn switching_section_clears_capture_and_last_error() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    if let Some(s) = page_mut(&mut app) {
        s.capture = Some(CaptureState {
            action_name: "copy".into(),
            conflict_with: None,
            last_invalid: None,
        });
        s.last_error = Some("disk full".into());
    }
    apply_settings_page(&mut app, Action::SettingsNextSection);
    let s = page(&app).unwrap();
    assert!(s.capture.is_none());
    assert!(s.last_error.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn open_settings_resolves_config_path_to_default() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    let expected = crate::config::default_config_path().unwrap();
    assert_eq!(page(&app).unwrap().config_path, expected);
}

#[tokio::test(flavor = "multi_thread")]
async fn open_settings_falls_through_to_page_for_prose_cursor() {
    // Cursor stays on the prose segment of the fixture doc — no DB
    // block, so the contextual dispatcher opens the page.
    let (mut app, _d, _v) = app_fixture("just prose, no block\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    assert!(page(&app).is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn close_settings_when_no_modal_is_noop() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    // Pre-condition: no modal open.
    assert!(app.modal.is_none());
    apply_settings_page(&mut app, Action::CloseSettingsPage);
    assert!(app.modal.is_none());
    assert_eq!(app.vim.mode, Mode::Normal);
}

#[tokio::test(flavor = "multi_thread")]
async fn close_does_not_disturb_a_different_modal() {
    // If another modal is open and we accidentally fire
    // `CloseSettingsPage`, we must not nuke the unrelated modal.
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    // Swap in a different modal underneath us.
    app.modal = Some(Modal::Help);
    apply_settings_page(&mut app, Action::CloseSettingsPage);
    assert!(matches!(app.modal, Some(Modal::Help)));
}

#[tokio::test(flavor = "multi_thread")]
async fn cursor_actions_noop_when_settings_modal_not_open() {
    // Defensive: actions emitted from a stale subscription must not
    // touch an unrelated state shape.
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::SettingsMoveCursor(1));
    apply_settings_page(&mut app, Action::SettingsNextSection);
    assert!(app.modal.is_none());
}

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

#[tokio::test(flavor = "multi_thread")]
async fn begin_capture_marks_the_selected_row() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    // First row is `move_up`.
    apply_settings_page(&mut app, Action::SettingsActivateRow);
    let s = page(&app).unwrap();
    assert_eq!(s.capture.as_ref().unwrap().action_name, "move_up");
}

#[tokio::test(flavor = "multi_thread")]
async fn cancel_capture_clears_state() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    apply_settings_page(&mut app, Action::SettingsActivateRow);
    apply_settings_page(&mut app, Action::SettingsCancelCapture);
    assert!(page(&app).unwrap().capture.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn commit_capture_writes_chord_and_persists() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    // Move cursor to `copy` (chord is `ctrl+c`).
    let rows = super::settings_page::keymap_rows();
    let copy_idx = rows.iter().position(|r| r.name == "copy").unwrap();
    apply_settings_page(&mut app, Action::SettingsMoveCursor(copy_idx as i32));
    apply_settings_page(&mut app, Action::SettingsActivateRow);
    apply_settings_page(
        &mut app,
        Action::SettingsCommitCapture(key(KeyCode::Char('y'), KeyModifiers::CONTROL)),
    );
    assert!(page(&app).unwrap().capture.is_none());
    // In-memory config updated.
    assert_eq!(app.config.keymap.chord_for("copy"), Some("ctrl+y"));
    // On-disk too — `persist` round-tripped through `save_config`.
    let on_disk = std::fs::read_to_string(&page(&app).unwrap().config_path).unwrap();
    assert!(on_disk.contains("copy = \"ctrl+y\""));
    // No persistence error.
    assert!(page(&app).unwrap().last_error.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn commit_capture_with_invalid_key_keeps_capture_open() {
    // Esc → modal handler emits CancelCapture; if some other path
    // funnels a bad event in (Null code), commit must NOT clear the
    // capture and must NOT mutate config.
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    apply_settings_page(&mut app, Action::SettingsActivateRow);
    let before = app.config.keymap.chord_for("move_up").map(str::to_string);
    apply_settings_page(
        &mut app,
        Action::SettingsCommitCapture(key(KeyCode::Null, KeyModifiers::NONE)),
    );
    let after = app.config.keymap.chord_for("move_up").map(str::to_string);
    assert_eq!(before, after, "config must not change on invalid capture");
    let cap = page(&app).unwrap().capture.as_ref().unwrap();
    assert!(cap.last_invalid.is_some(), "renderer hint must be set");
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_binding_drops_user_override() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    // Pre-seed a custom binding so reset has something to undo.
    app.config.keymap.set("copy", "alt+c".into());
    apply_settings_page(&mut app, Action::OpenSettings);
    let rows = super::settings_page::keymap_rows();
    let copy_idx = rows.iter().position(|r| r.name == "copy").unwrap();
    apply_settings_page(&mut app, Action::SettingsMoveCursor(copy_idx as i32));
    apply_settings_page(&mut app, Action::SettingsResetBinding);
    // The override is gone — fallback to the built-in default.
    assert!(app.config.keymap.chord_for("copy").is_none());
    // Disk reflects it.
    let on_disk = std::fs::read_to_string(&page(&app).unwrap().config_path).unwrap();
    assert!(!on_disk.contains("copy = \"alt+c\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_toggle_mode_row_writes_default() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    app.config.editor.toggle_mode_key = "ctrl+e".into();
    apply_settings_page(&mut app, Action::OpenSettings);
    let rows = super::settings_page::keymap_rows();
    let toggle_idx = rows.iter().position(|r| r.is_toggle_mode).unwrap();
    apply_settings_page(&mut app, Action::SettingsMoveCursor(toggle_idx as i32));
    apply_settings_page(&mut app, Action::SettingsResetBinding);
    assert_eq!(app.config.editor.toggle_mode_key, "alt+m");
}

#[tokio::test(flavor = "multi_thread")]
async fn commit_capture_toggle_mode_row_targets_editor_field() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    let rows = super::settings_page::keymap_rows();
    let toggle_idx = rows.iter().position(|r| r.is_toggle_mode).unwrap();
    apply_settings_page(&mut app, Action::SettingsMoveCursor(toggle_idx as i32));
    apply_settings_page(&mut app, Action::SettingsActivateRow);
    apply_settings_page(
        &mut app,
        Action::SettingsCommitCapture(key(KeyCode::F(7), KeyModifiers::NONE)),
    );
    assert_eq!(app.config.editor.toggle_mode_key, "f7");
    // Keymap config untouched (no entry for the pseudo-row).
    assert!(app
        .config
        .keymap
        .chord_for(super::settings_page::TOGGLE_MODE_ROW_NAME)
        .is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn conflict_detection_finds_other_owner() {
    // Bind `copy` to the same chord as `cut`. `find_chord_owner`
    // must return "cut" when looking up that chord while excluding
    // "copy".
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    app.config.keymap.set("copy", "ctrl+x".into()); // cut's default
    let owner = super::settings_page::find_chord_owner(&app, "ctrl+x", "copy");
    assert_eq!(owner.as_deref(), Some("cut"));
}

#[tokio::test(flavor = "multi_thread")]
async fn conflict_detection_ignores_self() {
    let (app, _d, _v) = app_fixture("stub\n").await;
    let owner = super::settings_page::find_chord_owner(&app, "ctrl+c", "copy");
    assert_eq!(owner, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn current_chord_falls_back_to_default_when_unset() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    // Drop the override (the default-populated config carries it).
    app.config.keymap.remove("copy");
    let rows = super::settings_page::keymap_rows();
    let copy = rows.iter().find(|r| r.name == "copy").unwrap();
    assert_eq!(super::settings_page::current_chord(&app, *copy), "ctrl+c");
}

#[tokio::test(flavor = "multi_thread")]
async fn current_chord_reads_toggle_mode_field() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    app.config.editor.toggle_mode_key = "ctrl+m".into();
    let rows = super::settings_page::keymap_rows();
    let toggle = rows.iter().find(|r| r.is_toggle_mode).unwrap();
    assert_eq!(super::settings_page::current_chord(&app, *toggle), "ctrl+m");
}

#[tokio::test(flavor = "multi_thread")]
async fn activate_row_on_theme_section_applies_preset_live() {
    use crate::ui::theme;
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    apply_settings_page(&mut app, Action::SettingsNextSection); // Theme
                                                                // Move cursor to "default-light" (idx 1).
    apply_settings_page(&mut app, Action::SettingsMoveCursor(1));
    apply_settings_page(&mut app, Action::SettingsActivateRow);

    assert_eq!(app.config.theme, "default-light");
    assert_eq!(theme::current().accent, theme::Theme::DEFAULT_LIGHT.accent);
    // Disk reflects it.
    let on_disk = std::fs::read_to_string(&page(&app).unwrap().config_path).unwrap();
    assert!(on_disk.contains("theme = \"default-light\""));

    // Restore process-wide default so other parallel tests aren't
    // affected by the global theme switch.
    theme::init("default-dark", &std::collections::BTreeMap::new());
}

#[tokio::test(flavor = "multi_thread")]
async fn editor_toggle_mode_flips_standard_and_vim() {
    use crate::config::EditorMode;
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    apply_settings_page(&mut app, Action::SettingsNextSection); // Theme
    apply_settings_page(&mut app, Action::SettingsNextSection); // Editor
                                                                // Row 0 = Editor mode.
    assert_eq!(app.config.editor.mode, EditorMode::Standard);
    apply_settings_page(&mut app, Action::SettingsActivateRow);
    assert_eq!(app.config.editor.mode, EditorMode::Vim);
    apply_settings_page(&mut app, Action::SettingsActivateRow);
    assert_eq!(app.config.editor.mode, EditorMode::Standard);
    // Persisted to disk too.
    let on_disk = std::fs::read_to_string(&page(&app).unwrap().config_path).unwrap();
    assert!(on_disk.contains("mode = \"standard\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn editor_toggle_mouse_flips_bool() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    apply_settings_page(&mut app, Action::SettingsNextSection);
    apply_settings_page(&mut app, Action::SettingsNextSection);
    apply_settings_page(&mut app, Action::SettingsMoveCursor(1)); // Mouse row
    let before = app.config.mouse_enabled;
    apply_settings_page(&mut app, Action::SettingsActivateRow);
    assert_eq!(app.config.mouse_enabled, !before);
}

#[tokio::test(flavor = "multi_thread")]
async fn editor_view_reports_current_values() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    app.config.editor.mode = crate::config::EditorMode::Vim;
    app.config.mouse_enabled = true;
    let view = super::settings_page::editor_view(&app);
    let by_label = |needle: &str| view.iter().find(|r| r.label.contains(needle)).unwrap();
    assert_eq!(by_label("mode").value, "Vim");
    assert_eq!(by_label("Mouse").value, "ON");
}

#[tokio::test(flavor = "multi_thread")]
async fn theme_view_marks_the_currently_active_preset() {
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    app.config.theme = "terminal-native".into();
    let view = super::settings_page::theme_view(&app);
    assert!(view
        .iter()
        .any(|r| r.name == "terminal-native" && r.is_active));
    assert!(view.iter().filter(|r| r.is_active).count() == 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn commit_capture_refreshes_app_resolved_keymap() {
    // Regression: `app.standard_keymap` is cached at App::new and
    // consumed by the input router on every keystroke. A rebind that
    // only mutates `app.config.keymap` (and persists) leaves the
    // running router on the stale resolved list — the user has to
    // restart the TUI to see the new chord. The applier MUST rebuild
    // the resolved list after each commit so the new chord fires
    // immediately.
    use crate::input::keychord::parse_key_chord;
    use crate::input::keymap::lookup;

    let (mut app, _d, _v) = app_fixture("stub\n").await;
    apply_settings_page(&mut app, Action::OpenSettings);
    let rows = super::settings_page::keymap_rows();
    let copy_idx = rows.iter().position(|r| r.name == "copy").unwrap();
    apply_settings_page(&mut app, Action::SettingsMoveCursor(copy_idx as i32));
    apply_settings_page(&mut app, Action::SettingsActivateRow);
    apply_settings_page(
        &mut app,
        Action::SettingsCommitCapture(key(KeyCode::Char('y'), KeyModifiers::CONTROL)),
    );

    // The router-facing list must lookup `ctrl+y` to `Action::Copy`
    // now, not the stale `ctrl+c`.
    let new_chord = parse_key_chord("ctrl+y").unwrap();
    let resolved_action = app
        .standard_keymap
        .iter()
        .find(|(chord, _)| chord == &new_chord)
        .map(|(_, a)| *a);
    assert_eq!(resolved_action, Some(crate::input::action::Action::Copy));
    // And the input dispatcher's `lookup` agrees.
    assert_eq!(
        lookup(
            &app.standard_keymap,
            key(KeyCode::Char('y'), KeyModifiers::CONTROL)
        ),
        Some(crate::input::action::Action::Copy),
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn reset_binding_refreshes_app_resolved_keymap() {
    use crate::input::keychord::parse_key_chord;
    let (mut app, _d, _v) = app_fixture("stub\n").await;
    // Seed a custom binding both in config AND in the cached resolved
    // list, mirroring "user opened TUI, then rebinds via Settings".
    app.config.keymap.set("copy", "alt+c".into());
    app.standard_keymap = crate::input::keymap::resolve_standard_keymap(&app.config.keymap);
    apply_settings_page(&mut app, Action::OpenSettings);
    let rows = super::settings_page::keymap_rows();
    let copy_idx = rows.iter().position(|r| r.name == "copy").unwrap();
    apply_settings_page(&mut app, Action::SettingsMoveCursor(copy_idx as i32));
    apply_settings_page(&mut app, Action::SettingsResetBinding);

    // After reset, the override is gone — resolved list must reflect
    // the built-in default `ctrl+c`, not the stale `alt+c`.
    let default_chord = parse_key_chord("ctrl+c").unwrap();
    let stale_chord = parse_key_chord("alt+c").unwrap();
    assert!(app
        .standard_keymap
        .iter()
        .any(|(chord, a)| chord == &default_chord && *a == crate::input::action::Action::Copy));
    assert!(!app
        .standard_keymap
        .iter()
        .any(|(chord, _)| chord == &stale_chord));
}

#[test]
fn toggle_mode_default_matches_config_module() {
    // Single-source-of-truth check: the Settings page's hard-coded
    // toggle default must equal the config module's. If
    // `default_toggle_mode_key` ever changes, this fails fast.
    assert_eq!(
        super::settings_page::keymap_rows()
            .iter()
            .find(|r| r.is_toggle_mode)
            .unwrap()
            .default_chord,
        crate::config::EditorConfig::default().toggle_mode_key,
    );
}
