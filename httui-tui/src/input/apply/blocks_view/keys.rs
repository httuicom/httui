use super::*;

/// Decode keystrokes that target the BLOCKS-view pane.
///
/// Two surfaces:
/// - **NAV** (no field open): region/row/col motion + Enter / `i`/`a`/`o`
///   to open a field + Tab/digits/PageUp/PageDown for navigation.
/// - **EDIT** (a field's sub-`Document` is open): we only claim the
///   lifecycle chords (`Esc`, `Ctrl+C`, `Ctrl+S`). Everything else
///   falls through to the editor scope so the vim/standard engine
///   operates directly on the sub-doc (via the `App::document_mut`
///   redirect — same pattern as `DbRowDetail`).
pub(crate) fn resolve_pane_key(app: &App, key: KeyEvent) -> Option<Action> {
    // Ctrl+W <hjkl> window chord: both engines own the suffix
    // dispatch via their `pending_window*` flag. Letting NAV claim
    // `h/j/k/l` here would shadow it.
    if app.standard.pending_window_chord || app.vim.pending_window {
        return None;
    }
    // Any open modal owns the keyboard. Keys it Forwards must reach
    // the modal's own pipeline (doc redirect / dispatch via
    // vim.mode), NOT this NAV resolver. Otherwise `h/j/k/l/Tab` etc.
    // shadow the modal's cursor and the BLOCKS pane silently
    // re-uses the keystroke.
    if app.modal.is_some() {
        return None;
    }
    let in_edit = app
        .active_pane()
        .map(|p| p.block_edit.is_some())
        .unwrap_or(false);
    let vim = app.config.editor.mode == EditorMode::Vim;
    if in_edit {
        // Effective sub-mode comes from the engine, not the stored
        // hint on `RegionEdit` — vim's `i`/`a`/`o` flip vim.mode for
        // us, so reading `edit.sub_mode` would lag behind. Standard
        // profile is always Insert.
        let sub_mode = effective_sub_mode(app);
        // Single-line HTTP cells (header key/value, URL) reject any chord
        // that would inject a newline: INSERT Enter/Tab advance to the next
        // field; vim NORMAL `o`/`O` (which would "open below") map to the
        // same advance. Multi-line fields fall through unchanged.
        if let Some(action) = resolve_single_line_advance(app, sub_mode, key) {
            return Some(action);
        }
        return resolve_edit_key(key, sub_mode, vim);
    }
    // BLOCKS NAV — try the user's configured chords first (so a
    // remapped `blocks_run` reaches Run before the hardcoded letters
    // below capture it).
    if let Some(action) = crate::input::keymap::lookup(&app.standard_keymap, key) {
        if matches!(action, Action::BlocksRunFocused | Action::BlocksCancelRun) {
            return Some(action);
        }
    }
    let in_headers = focused_region_is_http_headers(app);
    resolve_nav_key(key, vim, in_headers)
}

/// Single-line HTTP cells (header key/value, URL) forbid newlines, so any
/// chord that would inject `\n` redirects to "advance to next field":
/// - INSERT: `Enter`/`Tab` (form input style).
/// - vim NORMAL: `o`/`O` (vim's "open below/above" — meaningless on a
///   single-line cell, so we treat it as moving to the next row).
/// Multi-line fields (HTTP body / DB query) and non-HTTP cells fall through.
fn resolve_single_line_advance(
    app: &App,
    sub_mode: EditSubMode,
    key: KeyEvent,
) -> Option<Action> {
    let edit = app.active_pane().and_then(|p| p.block_edit.as_ref())?;
    if !matches!(
        edit.field,
        EditField::HttpHeaderKey(_) | EditField::HttpHeaderValue(_) | EditField::HttpUrl
    ) {
        return None;
    }
    match (sub_mode, key.modifiers, key.code) {
        (EditSubMode::Insert, KeyModifiers::NONE, KeyCode::Enter)
        | (EditSubMode::Insert, KeyModifiers::NONE, KeyCode::Tab) => {
            Some(Action::BlocksFieldAdvanceNext)
        }
        (EditSubMode::Normal, KeyModifiers::NONE, KeyCode::Char('o')) => {
            Some(Action::BlocksFieldOpenBelow)
        }
        (EditSubMode::Normal, KeyModifiers::NONE, KeyCode::Char('O')) => {
            Some(Action::BlocksFieldOpenAbove)
        }
        _ => None,
    }
}

/// Tree-mode chords specific to BLOCKS view. `n`/`N` on a `.md` file
/// appends a new HTTP block; `Ctrl+Shift+↑/↓` (or `Shift+↑/↓` as
/// fallback for terminals that don't deliver the Ctrl bit) on a
/// block reorders it. Returns `None` for everything else so the
/// generic tree handler still processes navigation chords.
pub(crate) fn resolve_tree_key(app: &App, key: KeyEvent) -> Option<Action> {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    let current = app.tree.current()?;
    if current.block.is_some() {
        // Block row focused: reorder chords. Accept Ctrl+Shift+arrow
        // (the documented chord) AND bare Shift+arrow as a fallback —
        // most terminals collapse Ctrl+Shift+arrow into Shift+arrow,
        // and Shift+arrow has no other meaning on a block row.
        let is_reorder = modifiers.contains(KeyModifiers::SHIFT);
        if is_reorder {
            return match code {
                KeyCode::Up => Some(Action::BlocksTreeReorderUp),
                KeyCode::Down => Some(Action::BlocksTreeReorderDown),
                _ => None,
            };
        }
        // `n`/`N` on a block creates a sibling block in the same
        // file. Append-only for now — picker for type + alias lands
        // in a follow-up.
        if matches!(code, KeyCode::Char('n') | KeyCode::Char('N'))
            && !modifiers.contains(KeyModifiers::CONTROL)
        {
            return Some(Action::BlocksTreeNewBlock);
        }
        // `d`/`D`/`Delete` on a block removes the block (NOT the
        // parent file). Intercepts before the generic tree-delete
        // so we can't accidentally lose the whole `.md`.
        if matches!(
            code,
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete
        ) && !modifiers.contains(KeyModifiers::CONTROL)
        {
            return Some(Action::BlocksTreeDeleteBlock);
        }
        return None;
    }
    // File row focused: `n`/`N` create a new block in the file.
    if matches!(code, KeyCode::Char('n') | KeyCode::Char('N'))
        && !modifiers.contains(KeyModifiers::CONTROL)
    {
        return Some(Action::BlocksTreeNewBlock);
    }
    None
}

pub(crate) fn focused_region_is_http_headers(app: &App) -> bool {
    let Some(pane) = app.active_pane() else {
        return false;
    };
    if pane.block_region != 1 {
        return false;
    }
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return false;
    };
    let Some(sel) = pane.block_selected else {
        return false;
    };
    ws.index
        .files
        .get(sel.file_idx)
        .and_then(|f| f.blocks.get(sel.block_idx))
        .map(|b| b.block_type == "http")
        .unwrap_or(false)
}

/// Source of truth for "is the EDIT buffer currently in Normal or
/// Insert?". Vim profile reads `app.vim.mode`; standard is always
/// Insert. Used by both the keystroke resolver and the status bar so
/// the displayed chip can't disagree with the engine's behaviour.
pub fn effective_sub_mode(app: &App) -> EditSubMode {
    if app.config.editor.mode == EditorMode::Standard {
        return EditSubMode::Insert;
    }
    match app.vim.mode {
        Mode::Normal => EditSubMode::Normal,
        _ => EditSubMode::Insert,
    }
}

/// Lifecycle chords for EDIT — everything else is `None` so the
/// keystroke flows to the editor engine via the document redirect.
///
/// The Esc resolution depends on whether the engine is currently in
/// Insert (e.g. user just pressed `i`) or Normal: in vim Insert we
/// drop to Normal without committing (first Esc); in vim Normal /
/// standard we commit + return to NAV. We swallow only the `Esc`
/// keystroke itself — the vim Insert→Normal transition is handled
/// by the engine in the OTHER side of this branch (when we return
/// `None`, the engine sees Esc and flips mode for free).
pub(crate) fn resolve_edit_key(key: KeyEvent, sub_mode: EditSubMode, vim: bool) -> Option<Action> {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (KeyModifiers::NONE, KeyCode::Esc) => match (vim, sub_mode) {
            // Vim INSERT → let the engine see Esc and flip to Normal
            // (no commit). We don't claim — `None` falls through.
            (true, EditSubMode::Insert) => None,
            // Vim NORMAL or standard profile → Esc commits and exits.
            _ => Some(Action::BlocksRegionCommitEdit),
        },
        (m, KeyCode::Char('c')) if m == KeyModifiers::CONTROL => {
            Some(Action::BlocksRegionCancelEdit)
        }
        (m, KeyCode::Char('s')) if m == KeyModifiers::CONTROL => Some(Action::BlocksSaveDraft),
        // Run / cancel from inside EDIT.
        // Alt+R / Alt+. always work (terminal-friendly chord).
        (m, KeyCode::Char('r')) if m == KeyModifiers::ALT => Some(Action::BlocksRunFocused),
        (m, KeyCode::Char('.')) if m == KeyModifiers::ALT => Some(Action::BlocksCancelRun),
        // Bare `r` / `.` in vim NORMAL sub-mode (no typing happening
        // here) — same chord NAV uses, lifted into EDIT-NORMAL so
        // the run/cancel cycle works without leaving the buffer.
        (KeyModifiers::NONE, KeyCode::Char('r'))
            if vim && matches!(sub_mode, EditSubMode::Normal) =>
        {
            Some(Action::BlocksRunFocused)
        }
        (KeyModifiers::NONE, KeyCode::Char('.'))
            if vim && matches!(sub_mode, EditSubMode::Normal) =>
        {
            Some(Action::BlocksCancelRun)
        }
        _ => None,
    }
}

pub(crate) fn resolve_nav_key(key: KeyEvent, vim: bool, in_headers: bool) -> Option<Action> {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (KeyModifiers::NONE, KeyCode::Tab) => Some(Action::BlocksPaneNextRegion),
        (KeyModifiers::SHIFT, KeyCode::BackTab) | (_, KeyCode::BackTab) => {
            Some(Action::BlocksPanePrevRegion)
        }
        (KeyModifiers::NONE, KeyCode::Char(c)) if c.is_ascii_digit() && c != '0' => {
            let n = (c as u8 - b'0') as usize;
            Some(Action::BlocksPaneJumpRegion(n))
        }
        // hjkl mirrors arrows in NAV for both profiles. The pane isn't
        // editing, so claiming the letters here can't shadow text input.
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            Some(Action::BlocksPaneRowUp)
        }
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            Some(Action::BlocksPaneRowDown)
        }
        (KeyModifiers::NONE, KeyCode::Left) | (KeyModifiers::NONE, KeyCode::Char('h')) => {
            Some(Action::BlocksPaneColLeft)
        }
        (KeyModifiers::NONE, KeyCode::Right) | (KeyModifiers::NONE, KeyCode::Char('l')) => {
            Some(Action::BlocksPaneColRight)
        }
        (KeyModifiers::NONE, KeyCode::Enter) => Some(Action::BlocksRegionEnterEdit),
        // Vim-only NAV chords: `i`/`a`/`o` skip past NORMAL straight
        // into INSERT (`Enter` in vim lands in NORMAL).
        (KeyModifiers::NONE, KeyCode::Char('i')) if vim => {
            Some(Action::BlocksRegionEnterEditInsert)
        }
        (KeyModifiers::NONE, KeyCode::Char('a')) if vim => {
            Some(Action::BlocksRegionEnterEditInsert)
        }
        // Headers table claims `o` for "insert row" before vim's "open below"
        // gets a chance — otherwise the vim arm would shadow the table chord
        // and try to enter EDIT on a possibly nonexistent row.
        (KeyModifiers::NONE, KeyCode::Char('o')) if vim && !in_headers => {
            Some(Action::BlocksRegionEnterEditInsert)
        }
        // `]`/`[` step between blocks in the workspace (vim only —
        // standard uses PageDown/PageUp below).
        (KeyModifiers::NONE, KeyCode::Char(']')) if vim => Some(Action::BlocksNextBlockMotion),
        (KeyModifiers::NONE, KeyCode::Char('[')) if vim => Some(Action::BlocksPrevBlockMotion),
        (KeyModifiers::NONE, KeyCode::PageDown) => Some(Action::BlocksNextBlockMotion),
        (KeyModifiers::NONE, KeyCode::PageUp) => Some(Action::BlocksPrevBlockMotion),
        // Table CRUD in `[2] Headers`. `o` mirrors vim's "open below",
        // `d` deletes the focused row. Scoped to the Headers region —
        // outside it, `o` keeps its vim NAV semantics (EnterEditInsert),
        // and `d` falls through to None.
        (KeyModifiers::NONE, KeyCode::Char('o')) if in_headers => {
            Some(Action::BlocksHeaderInsertRow)
        }
        (KeyModifiers::NONE, KeyCode::Insert) if in_headers => Some(Action::BlocksHeaderInsertRow),
        (KeyModifiers::NONE, KeyCode::Char('d')) if in_headers => {
            Some(Action::BlocksHeaderDeleteRow)
        }
        (KeyModifiers::NONE, KeyCode::Delete) if in_headers => Some(Action::BlocksHeaderDeleteRow),
        (KeyModifiers::NONE, KeyCode::Char(' ')) if in_headers => {
            Some(Action::BlocksHeaderToggleEnabled)
        }
        (m, KeyCode::Char('s')) if m == KeyModifiers::CONTROL => Some(Action::BlocksSaveDraft),
        (m, KeyCode::Char('t')) if m == KeyModifiers::ALT => Some(Action::BlocksResponseNextTab),
        (m, KeyCode::Char('T')) if m == (KeyModifiers::ALT | KeyModifiers::SHIFT) => {
            Some(Action::BlocksResponsePrevTab)
        }
        _ => None,
    }
}
