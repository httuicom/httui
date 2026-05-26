//! Centralized table of *app-level* keybindings — shortcuts that
//! aren't part of the vim engine itself (motions, operators, mode
//! transitions). Putting them here makes them easy to find, swap,
//! and — eventually — promote to a user-config (`vim.toml`-style).
//!
//! The vim primitives (`hjkl`, `wbe`, `gg`/`G`, `f`/`t`, `d`/`c`/`y`,
//! `i`/`a`/`o`, `v`/`V`, `/`, `:`, `u`, `p` …) deliberately stay
//! hardcoded in `parser.rs`: rebinding `j` would break user mental
//! model and surprise plugin/extension authors. Everything in *this*
//! module is fair game for end-user remapping.
//!
//! Each binding is exposed both as a constant (so call sites stay
//! grep-able for "what does Ctrl+P do") and as a `matches_*` helper
//! (so the dispatch parser can ask "is this key the QuickOpen
//! trigger?" without re-typing the modifier match arm).
//!
//! ## Adding a new shortcut
//!
//! 1. Add a `KeyChord` constant near the bottom (single-key) or a
//!    `pending_*` helper (multi-key chord like `gc`).
//! 2. Add a `matches_*` helper that wraps the comparison.
//! 3. Use the helper in `parser.rs::parse_normal`.
//! 4. Promote to user-config when `vim.toml` keymap loading lands.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Single-key combination — modifiers + base key. The constants
/// below name every app-level shortcut; `matches_*` helpers compare
/// an incoming `KeyEvent` against them.
#[derive(Debug, Clone, Copy)]
pub struct KeyChord {
    pub modifiers: KeyModifiers,
    pub code: KeyCode,
}

impl KeyChord {
    pub const fn new(modifiers: KeyModifiers, code: KeyCode) -> Self {
        Self { modifiers, code }
    }

    pub fn matches(&self, key: &KeyEvent) -> bool {
        key.modifiers == self.modifiers && key.code == self.code
    }
}

// ───────────── single-key shortcuts ─────────────

/// `Ctrl+P` — open the quick-open file picker modal.
pub const QUICK_OPEN: KeyChord = KeyChord::new(KeyModifiers::CONTROL, KeyCode::Char('p'));

/// `Ctrl+E` — toggle the file-tree sidebar focus.
pub const TREE_TOGGLE: KeyChord = KeyChord::new(KeyModifiers::CONTROL, KeyCode::Char('e'));

/// `Ctrl+G` — toggle the git side panel (right of editor). Shared
/// chord across vim + standard profiles so the panel feels global,
/// not modal.
pub const GIT_PANEL_TOGGLE: KeyChord = KeyChord::new(KeyModifiers::CONTROL, KeyCode::Char('g'));

/// `Ctrl+F` — open the content-search modal (FTS5 over the vault's
/// `.md` files). Vim binds `<C-f>` to "page down" but only via the
/// `Motion` family — Notes uses `<C-d>` for half-page down which
/// covers most needs, freeing `<C-f>` for "Find content". The
/// modal is full-screen overlay; keys flow into the input until
/// Esc/Ctrl-C closes.
pub const CONTENT_SEARCH: KeyChord = KeyChord::new(KeyModifiers::CONTROL, KeyCode::Char('f'));

/// `Tab` — swap focus between the sidebar and the editor.
/// `matches_focus_swap` accepts any modifier (terminals send
/// `<S-Tab>` with SHIFT for the reverse direction); the constant
/// stays as documentation of the canonical binding.
#[allow(dead_code)]
pub const FOCUS_SWAP: KeyChord = KeyChord::new(KeyModifiers::NONE, KeyCode::Tab);

/// `r` (no modifier) — run the executable block at the cursor.
/// Vim's `r{char}` replace-single-char isn't implemented, so the
/// key is free for our use.
pub const RUN_BLOCK: KeyChord = KeyChord::new(KeyModifiers::NONE, KeyCode::Char('r'));

/// `<CR>` (Enter) in normal mode — open the DB row-detail modal
/// when the cursor is parked on a result row. Dispatch checks the
/// cursor; on any other position it's a no-op. `<CR>` in normal
/// is `+` in stock vim, which we don't bind.
pub const OPEN_DB_ROW_DETAIL: KeyChord = KeyChord::new(KeyModifiers::NONE, KeyCode::Enter);

// ───────────── multi-key chords ─────────────

/// `Ctrl+L` — open the connection picker for the DB block at the
/// cursor. Vim binds `Ctrl+L` to "redraw screen" by default; we
/// don't implement that, so the slot is free. Mnemonic: "L" =
/// **list** of connections.
pub const OPEN_CONNECTION_PICKER: KeyChord =
    KeyChord::new(KeyModifiers::CONTROL, KeyCode::Char('l'));

/// `Ctrl+X` — wrap the focused DB block's query in the dialect's
/// EXPLAIN keyword and run it. Vim binds `Ctrl+X` to "decrement
/// number under cursor" (the counterpart to `Ctrl+A`); we don't
/// implement either, so the slot is free. Mnemonic: "X" = E**X**plain.
pub const EXPLAIN_BLOCK: KeyChord = KeyChord::new(KeyModifiers::CONTROL, KeyCode::Char('x'));

// `Ctrl+Shift+C` — copy the focused HTTP block as a cURL command,
// with `{{refs}}` resolved and clipboard write inline (no picker).
// Not stored as a single `KeyChord` const because `KeyModifiers`'
// `|` isn't const in the version we depend on; the `matches_*`
// helper below recognises both the SHIFT-folded encoding (terminals
// that send `CONTROL|SHIFT + 'C'`) and the bare `CONTROL + 'C'`
// fallback. Spec'd as `Mod-Shift-c`.

// `ga` (alias edit) lives in the `pending_g` chord branch in
// `vim/parser.rs::parse_normal`, not as a standalone `KeyChord` —
// using a g-prefix keeps the action discoverable next to `gd`
// (display-mode cycle) and avoids collisions with tmux prefixes
// like `<C-a>` / `<C-b>` that some users bind.

// ───────────── helpers ─────────────

pub fn matches_quick_open(key: &KeyEvent) -> bool {
    QUICK_OPEN.matches(key)
}

pub fn matches_tree_toggle(key: &KeyEvent) -> bool {
    TREE_TOGGLE.matches(key)
}

pub fn matches_git_panel_toggle(key: &KeyEvent) -> bool {
    GIT_PANEL_TOGGLE.matches(key)
}

pub fn matches_content_search(key: &KeyEvent) -> bool {
    CONTENT_SEARCH.matches(key)
}

pub fn matches_focus_swap(key: &KeyEvent) -> bool {
    // `Tab` in some terminals carries SHIFT for `<S-Tab>`; we accept
    // any modifier set since the focus swap is symmetric.
    matches!(key.code, KeyCode::Tab)
}

pub fn matches_run_block(key: &KeyEvent) -> bool {
    RUN_BLOCK.matches(key)
}

pub fn matches_open_db_row_detail(key: &KeyEvent) -> bool {
    OPEN_DB_ROW_DETAIL.matches(key)
}

pub fn matches_open_connection_picker(key: &KeyEvent) -> bool {
    OPEN_CONNECTION_PICKER.matches(key)
}

pub fn matches_explain_block(key: &KeyEvent) -> bool {
    EXPLAIN_BLOCK.matches(key)
}

/// `Ctrl+Shift+C` matches both encodings terminals use for the
/// chord: `CONTROL|SHIFT + 'C'` and the bare `CONTROL + 'C'`
/// fallback some terminals send for shifted control chords.
///
/// We deliberately gate on the keycode being upper-case `C` — a
/// plain `<C-c>` (lower-case) keeps its cancel semantics for
/// in-flight queries (see `dispatch::dispatch` top-level Ctrl-C
/// intercept).
pub fn matches_copy_as_curl(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('C'))
}
