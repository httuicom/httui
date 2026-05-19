//! Inspectable keymap data — single source of truth for the
//! per-profile chord → [`Action`] bindings.
//!
//! Standard mode consults [`lookup_standard`] from
//! [`crate::input::standard`]; vim's modal grammar (operator + motion +
//! text-object combinations) stays in the existing parser by design
//! (TD2 / TD4 — no further investment in the vim engine). The vim
//! entries below cover the *flat leaf chords* used by the engine and
//! are **documentary**: they're surfaced by [`dump_profile`] for V9's
//! future keymap UI but the vim path itself doesn't read them at
//! runtime.
//!
//! Cross-profile chords (currently only the hot toggle) live in
//! [`meta_entries`] and are appended to both profiles by
//! [`dump_profile`]. They are intercepted in
//! [`crate::input::route::route`] before any per-profile decoding.
//!
//! Introduced by tui-V01 / fase 6 p2 (replaces the fase 1 p7 stub).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;
use crate::input::types::Motion;

/// Editor input profile. `Standard` is the conventional non-modal
/// model (default); `Vim` opts into the modal engine.
///
/// `dead_code` allow: only consumed via [`dump_profile`] which is
/// also `dead_code`-allowed until V9 wires the inspection UI; the
/// variants themselves are exercised by the unit tests in this
/// module.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Standard,
    Vim,
}

/// Scope where the binding applies. Standard collapses into a single
/// `Editing` scope; vim splits into its modal flavors. `Any` covers
/// cross-mode chords like the hot toggle.
///
/// `dead_code` allow: vim's modal scopes (`Insert`/`Visual`/`Search`)
/// aren't referenced by any current entry — they're reserved for the
/// inspection UI (V9) so the data shape stays stable when vim's
/// per-mode chord vocabulary grows. Exercised by unit tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Standard's only scope — no modal split.
    Editing,
    /// Vim Normal mode.
    Normal,
    /// Vim Insert mode.
    Insert,
    /// Vim Visual / VisualLine.
    Visual,
    /// Vim Command-line (`:wq`, …).
    Cmdline,
    /// Vim Search prompt (`/`, `?`).
    Search,
    /// Cross-mode / cross-profile chord (intercepted regardless of
    /// the active mode). Used by the hot toggle.
    Any,
}

/// One chord-to-action binding.
///
/// - `chord_code = Some(...)` + `chord_modifiers` describes a single
///   keystroke the entry can be decoded from at runtime; that's how
///   Standard's leaf chords land here and how the cross-profile
///   toggle is recognised before the per-profile branch.
/// - `chord_code = None` marks a **documentary** entry — a multi-key
///   chord like vim's `gv` / `<C-w>s` that can't be expressed as a
///   single `KeyEvent`. The label is the source of truth for those;
///   the parser owns runtime decoding.
///
/// `profile` is omitted on purpose — it's implicit in which getter
/// surfaces the entry (`standard_entries`, `vim_entries`, or via
/// `dump_profile` which attaches the profile for the meta chord).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub scope: Scope,
    pub chord_code: Option<KeyCode>,
    pub chord_modifiers: KeyModifiers,
    pub label: &'static str,
    pub action: Action,
    pub doc: &'static str,
}

/// `(Profile, Entry)` pair returned by [`dump_profile`] so the
/// inspecting caller can render rows without re-attaching the
/// profile.
///
/// `dead_code` allow: consumed by inspection callers (V9 UI) that
/// don't exist yet; covered by unit tests.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DumpRow {
    pub profile: Profile,
    pub entry: Entry,
}

// ---------------------------------------------------------------------
// Internal entry constructors.
//
// `KeyEvent` matching is exact on modifiers — terminals that fold
// `Shift+<char>` into uppercase report `KeyCode::Char('M')` with
// `SHIFT | CONTROL` (for example), while others keep lowercase with
// the same modifier set. The table includes BOTH casings of any
// `Ctrl+Shift+<letter>` so the lookup matches without mask gymnastics.
// ---------------------------------------------------------------------

fn editing_bare(code: KeyCode, label: &'static str, action: Action, doc: &'static str) -> Entry {
    Entry {
        scope: Scope::Editing,
        chord_code: Some(code),
        chord_modifiers: KeyModifiers::NONE,
        label,
        action,
        doc,
    }
}

fn editing_with(
    code: KeyCode,
    modifiers: KeyModifiers,
    label: &'static str,
    action: Action,
    doc: &'static str,
) -> Entry {
    Entry {
        scope: Scope::Editing,
        chord_code: Some(code),
        chord_modifiers: modifiers,
        label,
        action,
        doc,
    }
}

#[allow(dead_code)] // exercised by tests; production consumer (V9 UI) pending
fn vim_doc(scope: Scope, label: &'static str, action: Action, doc: &'static str) -> Entry {
    Entry {
        scope,
        chord_code: None,
        chord_modifiers: KeyModifiers::NONE,
        label,
        action,
        doc,
    }
}

/// All Standard-mode leaf chords, in roughly the order users meet
/// them: motions → selection → edit → clipboard → save → undo →
/// special chords. Every leaf chord previously hardcoded in
/// `input::standard::resolve`'s match arms appears here exactly once
/// (mod the dual-casing for `Ctrl+Shift+<letter>`).
pub fn standard_entries() -> Vec<Entry> {
    let ctrl = KeyModifiers::CONTROL;
    let shift = KeyModifiers::SHIFT;
    let ctrl_shift = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
    vec![
        // Motions — bare arrows + Home/End + half-page.
        editing_bare(
            KeyCode::Up,
            "Up",
            Action::Motion(Motion::Up, 1),
            "Move cursor up",
        ),
        editing_bare(
            KeyCode::Down,
            "Down",
            Action::Motion(Motion::Down, 1),
            "Move cursor down",
        ),
        editing_bare(
            KeyCode::Left,
            "Left",
            Action::Motion(Motion::Left, 1),
            "Move cursor left",
        ),
        editing_bare(
            KeyCode::Right,
            "Right",
            Action::Motion(Motion::Right, 1),
            "Move cursor right",
        ),
        editing_bare(
            KeyCode::Home,
            "Home",
            Action::Motion(Motion::LineStart, 1),
            "Cursor to line start",
        ),
        editing_bare(
            KeyCode::End,
            "End",
            Action::Motion(Motion::LineEnd, 1),
            "Cursor to line end",
        ),
        editing_bare(
            KeyCode::PageUp,
            "PageUp",
            Action::Motion(Motion::HalfPageUp, 1),
            "Scroll up half page",
        ),
        editing_bare(
            KeyCode::PageDown,
            "PageDown",
            Action::Motion(Motion::HalfPageDown, 1),
            "Scroll down half page",
        ),
        // Page keys keep their plain motion even with SHIFT — V1
        // scope decision (no SelectExtend variant). The old match
        // arm used `_` to ignore modifiers; the data-driven shape
        // expresses that as explicit SHIFT entries. Other modifier
        // combos (CONTROL+PageDown, ALT+PageDown) fall through to
        // None — those are terminal-owned in practice.
        editing_with(
            KeyCode::PageUp,
            shift,
            "Shift+PageUp",
            Action::Motion(Motion::HalfPageUp, 1),
            "Scroll up half page (selection unchanged)",
        ),
        editing_with(
            KeyCode::PageDown,
            shift,
            "Shift+PageDown",
            Action::Motion(Motion::HalfPageDown, 1),
            "Scroll down half page (selection unchanged)",
        ),
        // Selection-extend — Shift+<motion>.
        editing_with(
            KeyCode::Up,
            shift,
            "Shift+Up",
            Action::SelectExtend(Motion::Up),
            "Extend selection up",
        ),
        editing_with(
            KeyCode::Down,
            shift,
            "Shift+Down",
            Action::SelectExtend(Motion::Down),
            "Extend selection down",
        ),
        editing_with(
            KeyCode::Left,
            shift,
            "Shift+Left",
            Action::SelectExtend(Motion::Left),
            "Extend selection left",
        ),
        editing_with(
            KeyCode::Right,
            shift,
            "Shift+Right",
            Action::SelectExtend(Motion::Right),
            "Extend selection right",
        ),
        editing_with(
            KeyCode::Home,
            shift,
            "Shift+Home",
            Action::SelectExtend(Motion::LineStart),
            "Extend selection to line start",
        ),
        editing_with(
            KeyCode::End,
            shift,
            "Shift+End",
            Action::SelectExtend(Motion::LineEnd),
            "Extend selection to line end",
        ),
        // Edit primitives — leaf bindings; `InsertChar(c)` stays in
        // `standard::resolve` because the action is parametric in `c`.
        editing_bare(
            KeyCode::Enter,
            "Enter",
            Action::InsertNewline,
            "Insert newline",
        ),
        editing_bare(
            KeyCode::Backspace,
            "Backspace",
            Action::DeleteBackward,
            "Delete char before cursor",
        ),
        editing_bare(
            KeyCode::Delete,
            "Delete",
            Action::DeleteForward,
            "Delete char at cursor",
        ),
        // Clipboard / save.
        editing_with(
            KeyCode::Char('c'),
            ctrl,
            "Ctrl+C",
            Action::Copy,
            "Copy selection",
        ),
        editing_with(
            KeyCode::Char('x'),
            ctrl,
            "Ctrl+X",
            Action::Cut,
            "Cut selection",
        ),
        editing_with(
            KeyCode::Char('v'),
            ctrl,
            "Ctrl+V",
            Action::PasteSystem,
            "Paste from clipboard",
        ),
        editing_with(
            KeyCode::Char('s'),
            ctrl,
            "Ctrl+S",
            Action::WriteFile,
            "Save file",
        ),
        // Undo / redo. `Ctrl+Y` is the Windows alias; `Ctrl+Shift+Z`
        // is the cross-platform alias. Both casings of the letter
        // because some terminals fold Shift into uppercase.
        editing_with(KeyCode::Char('z'), ctrl, "Ctrl+Z", Action::Undo, "Undo"),
        editing_with(KeyCode::Char('y'), ctrl, "Ctrl+Y", Action::Redo, "Redo"),
        editing_with(
            KeyCode::Char('z'),
            ctrl_shift,
            "Ctrl+Shift+Z",
            Action::Redo,
            "Redo (alt)",
        ),
        editing_with(
            KeyCode::Char('Z'),
            ctrl_shift,
            "Ctrl+Shift+Z",
            Action::Redo,
            "Redo (alt, uppercase fold)",
        ),
        // EXPLAIN on focused DB block — vim binds plain `Ctrl+X`, but
        // in Standard `Ctrl+X` is Cut, so EXPLAIN moves to the
        // shifted variant.
        editing_with(
            KeyCode::Char('x'),
            ctrl_shift,
            "Ctrl+Shift+X",
            Action::ExplainBlock,
            "Run EXPLAIN on focused block",
        ),
        editing_with(
            KeyCode::Char('X'),
            ctrl_shift,
            "Ctrl+Shift+X",
            Action::ExplainBlock,
            "Run EXPLAIN on focused block (uppercase fold)",
        ),
    ]
}

/// Documentary vim-engine leaf chords. None of these are decoded by
/// looking up this table — the existing parser owns them — but the
/// inspector needs a single place to list them so V9 can render the
/// "Normal-mode chord vocabulary" without scraping the parser.
/// Operator+motion+text-object combinations are deliberately NOT
/// enumerated here: they're a grammar, not a binding table, and
/// rewriting them would mean redoing the vim engine.
#[allow(dead_code)] // exercised by tests; production consumer (V9 UI) pending
pub fn vim_entries() -> Vec<Entry> {
    vec![
        vim_doc(Scope::Cmdline, ":q", Action::Quit, "Quit"),
        vim_doc(Scope::Normal, "u", Action::Undo, "Undo"),
        vim_doc(Scope::Normal, "<C-r>", Action::Redo, "Redo"),
        vim_doc(Scope::Normal, "<C-s>", Action::WriteFile, "Save file"),
        vim_doc(
            Scope::Normal,
            "<C-e>",
            Action::TreeToggle,
            "Toggle file tree",
        ),
        vim_doc(
            Scope::Normal,
            "<C-p>",
            Action::EnterQuickOpen,
            "Quick-open file picker",
        ),
        vim_doc(
            Scope::Normal,
            "<C-f>",
            Action::OpenContentSearch,
            "Full-text content search",
        ),
        vim_doc(Scope::Normal, "gW", Action::WriteAll, "Save all dirty tabs"),
        vim_doc(
            Scope::Normal,
            "gv",
            Action::ReselectVisual,
            "Re-enter last visual selection",
        ),
        vim_doc(
            Scope::Normal,
            "g]",
            Action::JumpNextBlock,
            "Jump to next executable block",
        ),
        vim_doc(
            Scope::Normal,
            "g[",
            Action::JumpPrevBlock,
            "Jump to previous executable block",
        ),
        vim_doc(
            Scope::Normal,
            "gr",
            Action::RerunLastBlock,
            "Rerun last block",
        ),
        vim_doc(
            Scope::Normal,
            "gs",
            Action::OpenDbSettingsModal,
            "Open block settings modal (limit / timeout)",
        ),
        vim_doc(
            Scope::Normal,
            "gh",
            Action::OpenBlockHistory,
            "Open block run history",
        ),
        vim_doc(
            Scope::Normal,
            "gx",
            Action::OpenDbExportPicker,
            "Open DB export-format picker",
        ),
        vim_doc(
            Scope::Normal,
            "gE",
            Action::OpenEnvironmentPicker,
            "Open environment picker",
        ),
        vim_doc(
            Scope::Normal,
            "gN",
            Action::OpenBlockTemplatePicker,
            "Open block-template picker",
        ),
        vim_doc(
            Scope::Normal,
            "gb",
            Action::OpenTabPicker,
            "Open tab picker",
        ),
        vim_doc(Scope::Normal, "g?", Action::OpenHelp, "Open keymap help"),
        vim_doc(Scope::Normal, "gt", Action::TabNext, "Next tab"),
        vim_doc(Scope::Normal, "gT", Action::TabPrev, "Previous tab"),
    ]
}

/// Cross-profile chords intercepted by [`crate::input::route::route`]
/// before any per-profile decoding. Surfaced under BOTH profiles by
/// [`dump_profile`] so V9's UI lists the toggle in either view.
pub fn meta_entries() -> Vec<Entry> {
    let ctrl_shift = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
    vec![
        Entry {
            scope: Scope::Any,
            chord_code: Some(KeyCode::Char('m')),
            chord_modifiers: ctrl_shift,
            label: "Ctrl+Shift+M",
            action: Action::ToggleEditorMode,
            doc: "Toggle editor profile (Standard ↔ Vim)",
        },
        // Some terminals fold SHIFT into uppercase. Listing both
        // casings keeps the inspection rows truthful — `route::route`
        // accepts either at runtime.
        Entry {
            scope: Scope::Any,
            chord_code: Some(KeyCode::Char('M')),
            chord_modifiers: ctrl_shift,
            label: "Ctrl+Shift+M",
            action: Action::ToggleEditorMode,
            doc: "Toggle editor profile (Standard ↔ Vim, uppercase fold)",
        },
    ]
}

/// All bindings exposed under `profile`, including the cross-profile
/// meta chords. The order is `profile_entries() ++ meta_entries()`.
/// Pure: returns a fresh `Vec` each call (the table is small —
/// inspection is the only consumer).
#[allow(dead_code)] // exercised by tests; V9 keymap UI is the production consumer
pub fn dump_profile(profile: Profile) -> Vec<DumpRow> {
    let entries = match profile {
        Profile::Standard => standard_entries(),
        Profile::Vim => vim_entries(),
    };
    let mut out: Vec<DumpRow> = entries
        .into_iter()
        .map(|entry| DumpRow { profile, entry })
        .collect();
    for entry in meta_entries() {
        out.push(DumpRow { profile, entry });
    }
    out
}

/// Decode a keystroke against the Standard chord table + the
/// cross-profile meta chords. Returns `None` for keys the table
/// doesn't bind — the caller (`standard::resolve`) falls back to its
/// parametric arms (`InsertChar(c)` for printable chars). Pure: no
/// `App`, no side effects.
pub fn lookup_standard(key: KeyEvent) -> Option<Action> {
    standard_entries()
        .into_iter()
        .chain(meta_entries())
        .find(|entry| matches_chord(entry, &key))
        .map(|entry| entry.action)
}

/// Chord predicate — exact match on both `code` and `modifiers`.
/// Documentary entries (`chord_code = None`) never match.
fn matches_chord(entry: &Entry, key: &KeyEvent) -> bool {
    entry.chord_code == Some(key.code) && entry.chord_modifiers == key.modifiers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    // -----------------------------------------------------------------
    // lookup_standard
    // -----------------------------------------------------------------

    #[test]
    fn lookup_standard_resolves_bare_arrow_to_motion() {
        assert_eq!(
            lookup_standard(ev(KeyCode::Up, KeyModifiers::NONE)),
            Some(Action::Motion(Motion::Up, 1)),
        );
    }

    #[test]
    fn lookup_standard_resolves_shift_arrow_to_select_extend() {
        assert_eq!(
            lookup_standard(ev(KeyCode::Right, KeyModifiers::SHIFT)),
            Some(Action::SelectExtend(Motion::Right)),
        );
    }

    #[test]
    fn lookup_standard_resolves_ctrl_clipboard_chords() {
        // Triple covered in one shot — all three chord-to-Action
        // bindings sit in the table, so a quick spot check is enough.
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::Copy),
        );
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('x'), KeyModifiers::CONTROL)),
            Some(Action::Cut),
        );
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('v'), KeyModifiers::CONTROL)),
            Some(Action::PasteSystem),
        );
    }

    #[test]
    fn lookup_standard_resolves_undo_redo_aliases() {
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('z'), KeyModifiers::CONTROL)),
            Some(Action::Undo),
        );
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('y'), KeyModifiers::CONTROL)),
            Some(Action::Redo),
        );
        // Cross-platform redo via SHIFT, lowercase casing.
        let ctrl_shift = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('z'), ctrl_shift)),
            Some(Action::Redo),
        );
        // Same chord, uppercase fold — must also resolve.
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('Z'), ctrl_shift)),
            Some(Action::Redo),
        );
    }

    #[test]
    fn lookup_standard_resolves_explain_block_both_casings() {
        let ctrl_shift = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('x'), ctrl_shift)),
            Some(Action::ExplainBlock),
        );
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('X'), ctrl_shift)),
            Some(Action::ExplainBlock),
        );
    }

    #[test]
    fn lookup_standard_resolves_save_and_edit_primitives() {
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('s'), KeyModifiers::CONTROL)),
            Some(Action::WriteFile),
        );
        assert_eq!(
            lookup_standard(ev(KeyCode::Enter, KeyModifiers::NONE)),
            Some(Action::InsertNewline),
        );
        assert_eq!(
            lookup_standard(ev(KeyCode::Backspace, KeyModifiers::NONE)),
            Some(Action::DeleteBackward),
        );
        assert_eq!(
            lookup_standard(ev(KeyCode::Delete, KeyModifiers::NONE)),
            Some(Action::DeleteForward),
        );
    }

    #[test]
    fn lookup_standard_resolves_meta_toggle_chord() {
        // Meta chords are exposed via the same lookup helper so the
        // route layer can ask "is this key in the standard surface?"
        // and find the toggle alongside the leaf chords.
        let ctrl_shift = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('m'), ctrl_shift)),
            Some(Action::ToggleEditorMode),
        );
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('M'), ctrl_shift)),
            Some(Action::ToggleEditorMode),
        );
    }

    #[test]
    fn lookup_standard_returns_none_for_unbound_chord() {
        // Plain printable chars without CONTROL go through the
        // parametric `InsertChar` arm in `standard::resolve`, NOT
        // through this table.
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('a'), KeyModifiers::NONE)),
            None,
        );
        // Esc isn't bound in the table — `route_standard` handles it
        // specially (cancel running query).
        assert_eq!(lookup_standard(ev(KeyCode::Esc, KeyModifiers::NONE)), None,);
        // Function keys: unbound.
        assert_eq!(lookup_standard(ev(KeyCode::F(5), KeyModifiers::NONE)), None,);
    }

    #[test]
    fn lookup_standard_rejects_wrong_modifier_set() {
        // `Ctrl+C` is Copy; bare `c` is parametric InsertChar (None).
        // `Alt+C` is unbound — modifiers must match EXACTLY.
        assert_eq!(
            lookup_standard(ev(KeyCode::Char('c'), KeyModifiers::ALT)),
            None,
        );
    }

    // -----------------------------------------------------------------
    // matches_chord helper
    // -----------------------------------------------------------------

    #[test]
    fn matches_chord_rejects_documentary_entries() {
        // Documentary vim entries (`chord_code = None`) can never
        // match a runtime keystroke — that's the whole point of the
        // Optional code field.
        let any_vim = vim_entries().into_iter().next().expect("non-empty");
        assert!(any_vim.chord_code.is_none());
        let pressed = ev(KeyCode::Char('u'), KeyModifiers::NONE);
        assert!(!matches_chord(&any_vim, &pressed));
    }

    // -----------------------------------------------------------------
    // dump_profile
    // -----------------------------------------------------------------

    #[test]
    fn dump_profile_standard_includes_clipboard_and_meta_toggle() {
        let rows = dump_profile(Profile::Standard);
        assert!(
            rows.iter()
                .all(|row| matches!(row.profile, Profile::Standard)),
            "every row in a Standard dump must carry Profile::Standard",
        );
        let labels: Vec<&'static str> = rows.iter().map(|r| r.entry.label).collect();
        for must in [
            "Ctrl+C",
            "Ctrl+X",
            "Ctrl+V",
            "Ctrl+S",
            "Ctrl+Z",
            "Ctrl+Y",
            "Ctrl+Shift+M",
            "Ctrl+Shift+X",
            "Up",
            "Down",
            "Enter",
        ] {
            assert!(
                labels.contains(&must),
                "Standard dump should list `{must}` (got: {labels:?})",
            );
        }
    }

    #[test]
    fn dump_profile_vim_includes_flat_chords_and_meta_toggle() {
        let rows = dump_profile(Profile::Vim);
        assert!(
            rows.iter().all(|row| matches!(row.profile, Profile::Vim)),
            "every row in a Vim dump must carry Profile::Vim",
        );
        let labels: Vec<&'static str> = rows.iter().map(|r| r.entry.label).collect();
        for must in [
            "u",
            "<C-r>",
            "<C-s>",
            "<C-e>",
            "<C-p>",
            "<C-f>",
            "gW",
            "gv",
            "g?",
            "Ctrl+Shift+M",
        ] {
            assert!(
                labels.contains(&must),
                "Vim dump should list `{must}` (got: {labels:?})",
            );
        }
    }

    #[test]
    fn dump_profile_has_no_empty_docs_or_labels() {
        // Inspection UI relies on labels + docs being non-empty.
        for profile in [Profile::Standard, Profile::Vim] {
            for row in dump_profile(profile) {
                assert!(!row.entry.label.is_empty(), "label was empty: {row:?}");
                assert!(!row.entry.doc.is_empty(), "doc was empty: {row:?}");
            }
        }
    }

    #[test]
    fn dump_profile_meta_entries_appear_under_both_profiles() {
        // The hot toggle is cross-profile — both `dump_profile`
        // results must list it, with the requested profile attached
        // so the inspecting UI doesn't have to guess.
        let std_meta = dump_profile(Profile::Standard)
            .into_iter()
            .filter(|r| r.entry.action == Action::ToggleEditorMode)
            .count();
        let vim_meta = dump_profile(Profile::Vim)
            .into_iter()
            .filter(|r| r.entry.action == Action::ToggleEditorMode)
            .count();
        assert_eq!(std_meta, meta_entries().len());
        assert_eq!(vim_meta, meta_entries().len());
    }

    // -----------------------------------------------------------------
    // Single-source guarantee — chord-to-Action mappings are NOT
    // duplicated between `standard_entries` and `meta_entries` (the
    // toggle lives only in meta; everything else lives only in
    // standard).
    // -----------------------------------------------------------------

    #[test]
    fn standard_table_does_not_include_meta_chords() {
        let meta_chords: Vec<(KeyCode, KeyModifiers)> = meta_entries()
            .into_iter()
            .filter_map(|e| e.chord_code.map(|c| (c, e.chord_modifiers)))
            .collect();
        let standard_chords: Vec<(KeyCode, KeyModifiers)> = standard_entries()
            .into_iter()
            .filter_map(|e| e.chord_code.map(|c| (c, e.chord_modifiers)))
            .collect();
        for chord in &meta_chords {
            assert!(
                !standard_chords.contains(chord),
                "chord {chord:?} appears in both meta_entries and standard_entries",
            );
        }
    }

    #[test]
    fn vim_entries_are_all_documentary() {
        // Vim entries can't be looked up by runtime KeyEvent — by
        // construction every vim entry has `chord_code = None`.
        for entry in vim_entries() {
            assert!(
                entry.chord_code.is_none(),
                "vim entry `{}` has a runtime chord_code; documentary entries must use None",
                entry.label,
            );
        }
    }
}
