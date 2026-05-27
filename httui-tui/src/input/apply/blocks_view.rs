use crate::app::{
    App, AppView, BlockDraft, BlockIndex, BlocksUnsavedPromptFocus, BlocksUnsavedPromptState,
    BlocksWorkspace, EditField, EditSubMode, RegionEdit, StatusKind,
};
use crate::config::EditorMode;
use crate::input::action::Action;
use crate::modal::Modal;
use crate::vim::mode::Mode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
        return resolve_edit_key(key, sub_mode, vim);
    }
    // BLOCKS NAV — try the user's configured chords first (so a
    // remapped `blocks_run` reaches Run before the hardcoded letters
    // below capture it).
    if let Some(action) = crate::input::keymap::lookup(&app.standard_keymap, key) {
        if matches!(
            action,
            Action::BlocksRunFocused | Action::BlocksCancelRun
        ) {
            return Some(action);
        }
    }
    let in_headers = focused_region_is_http_headers(app);
    resolve_nav_key(key, vim, in_headers)
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

fn focused_region_is_http_headers(app: &App) -> bool {
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
fn resolve_edit_key(key: KeyEvent, sub_mode: EditSubMode, vim: bool) -> Option<Action> {
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
        _ => None,
    }
}

fn resolve_nav_key(key: KeyEvent, vim: bool, in_headers: bool) -> Option<Action> {
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
        (KeyModifiers::NONE, KeyCode::Up)
        | (KeyModifiers::NONE, KeyCode::Char('k')) => Some(Action::BlocksPaneRowUp),
        (KeyModifiers::NONE, KeyCode::Down)
        | (KeyModifiers::NONE, KeyCode::Char('j')) => Some(Action::BlocksPaneRowDown),
        (KeyModifiers::NONE, KeyCode::Left)
        | (KeyModifiers::NONE, KeyCode::Char('h')) => Some(Action::BlocksPaneColLeft),
        (KeyModifiers::NONE, KeyCode::Right)
        | (KeyModifiers::NONE, KeyCode::Char('l')) => Some(Action::BlocksPaneColRight),
        (KeyModifiers::NONE, KeyCode::Enter) => Some(Action::BlocksRegionEnterEdit),
        // Vim-only NAV chords: `i`/`a`/`o` skip past NORMAL straight
        // into INSERT (`Enter` in vim lands in NORMAL).
        (KeyModifiers::NONE, KeyCode::Char('i')) if vim => {
            Some(Action::BlocksRegionEnterEditInsert)
        }
        (KeyModifiers::NONE, KeyCode::Char('a')) if vim => {
            Some(Action::BlocksRegionEnterEditInsert)
        }
        (KeyModifiers::NONE, KeyCode::Char('o')) if vim => {
            Some(Action::BlocksRegionEnterEditInsert)
        }
        // `]`/`[` step between blocks in the workspace (vim only —
        // standard uses PageDown/PageUp below).
        (KeyModifiers::NONE, KeyCode::Char(']')) if vim => {
            Some(Action::BlocksNextBlockMotion)
        }
        (KeyModifiers::NONE, KeyCode::Char('[')) if vim => {
            Some(Action::BlocksPrevBlockMotion)
        }
        (KeyModifiers::NONE, KeyCode::PageDown) => Some(Action::BlocksNextBlockMotion),
        (KeyModifiers::NONE, KeyCode::PageUp) => Some(Action::BlocksPrevBlockMotion),
        // Table CRUD in `[2] Headers`. `o` mirrors vim's "open below",
        // `d` deletes the focused row. Scoped to the Headers region —
        // outside it, `o` keeps its vim NAV semantics (EnterEditInsert),
        // and `d` falls through to None.
        (KeyModifiers::NONE, KeyCode::Char('o')) if in_headers => {
            Some(Action::BlocksHeaderInsertRow)
        }
        (KeyModifiers::NONE, KeyCode::Insert) if in_headers => {
            Some(Action::BlocksHeaderInsertRow)
        }
        (KeyModifiers::NONE, KeyCode::Char('d')) if in_headers => {
            Some(Action::BlocksHeaderDeleteRow)
        }
        (KeyModifiers::NONE, KeyCode::Delete) if in_headers => {
            Some(Action::BlocksHeaderDeleteRow)
        }
        (m, KeyCode::Char('s')) if m == KeyModifiers::CONTROL => Some(Action::BlocksSaveDraft),
        (m, KeyCode::Char('t')) if m == KeyModifiers::ALT => Some(Action::BlocksResponseNextTab),
        (m, KeyCode::Char('T')) if m == (KeyModifiers::ALT | KeyModifiers::SHIFT) => {
            Some(Action::BlocksResponsePrevTab)
        }
        _ => None,
    }
}

pub(crate) fn apply_blocks_view(app: &mut App, action: Action) {
    match action {
        Action::ToggleAppView => request_toggle_view(app),
        Action::BlocksPaneNextRegion => shift_region(app, 1),
        Action::BlocksPanePrevRegion => shift_region(app, -1),
        Action::BlocksPaneJumpRegion(n) => set_region(app, n.saturating_sub(1)),
        Action::BlocksPanePickerChoose(n) => choose_picker(app, n.saturating_sub(1)),
        Action::BlocksPanePickerCancel => cancel_picker(app),
        Action::BlocksPaneRowUp => shift_row(app, -1),
        Action::BlocksPaneRowDown => shift_row(app, 1),
        Action::BlocksPaneColLeft => shift_col(app, -1),
        Action::BlocksPaneColRight => shift_col(app, 1),
        Action::BlocksRegionEnterEdit => enter_edit(app, EnterMode::Auto),
        Action::BlocksRegionEnterEditInsert => enter_edit(app, EnterMode::Insert),
        Action::BlocksRegionCommitEdit => commit_edit(app),
        Action::BlocksRegionCancelEdit => cancel_edit(app),
        Action::BlocksSaveDraft => save_draft(app),
        Action::BlocksNextBlockMotion => shift_block(app, 1),
        Action::BlocksPrevBlockMotion => shift_block(app, -1),
        Action::BlocksRunFocused => run_focused_block(app),
        Action::BlocksCancelRun => {
            crate::commands::db::cancel_running_query(app);
        }
        Action::BlocksHeaderInsertRow => insert_header_row(app),
        Action::BlocksHeaderDeleteRow => delete_header_row(app),
        Action::BlocksResponseNextTab => shift_response_subtab(app, 1),
        Action::BlocksResponsePrevTab => shift_response_subtab(app, -1),
        Action::BlocksTreeNewBlock => tree_new_block(app),
        Action::BlocksTreeReorderUp => tree_reorder_block(app, -1),
        Action::BlocksTreeReorderDown => tree_reorder_block(app, 1),
        Action::BlocksTreeDeleteBlock => tree_delete_block(app),
        Action::BlocksUnsavedPromptSave => {
            close_unsaved_prompt(app);
            save_draft(app);
            toggle_view(app);
        }
        Action::BlocksUnsavedPromptDiscard => {
            close_unsaved_prompt(app);
            discard_all_drafts(app);
            toggle_view(app);
        }
        Action::BlocksUnsavedPromptCancel => {
            close_unsaved_prompt(app);
        }
        _ => {}
    }
}

/// `Alt+M` entry point. If any pane carries a draft, open the
/// Save/Discard/Cancel modal instead of toggling immediately. The
/// modal's emit re-enters this applier with the resolved action.
fn request_toggle_view(app: &mut App) {
    let dirty = collect_dirty_panes(app);
    if dirty.is_empty() {
        toggle_view(app);
        return;
    }
    let files: Vec<std::path::PathBuf> = dirty.iter().map(|(p, _)| p.clone()).collect();
    app.modal = Some(Modal::BlocksUnsavedPrompt(BlocksUnsavedPromptState {
        dirty: files,
        focus: BlocksUnsavedPromptFocus::default(),
    }));
}

fn close_unsaved_prompt(app: &mut App) {
    if let Some(Modal::BlocksUnsavedPrompt(_)) = app.modal {
        app.modal = None;
    }
}

fn discard_all_drafts(app: &mut App) {
    let Some(tab) = app.active_tab_mut() else {
        return;
    };
    walk_panes_mut(&mut tab.root, &mut |pane| {
        pane.block_draft = None;
        pane.block_edit = None;
    });
}

fn choose_picker(app: &mut App, leaf_idx: usize) {
    let Some(target) = app
        .blocks_workspace
        .as_ref()
        .and_then(|w| w.pane_picker)
    else {
        return;
    };
    let Some(tab) = app.active_tab_mut() else {
        cancel_picker(app);
        return;
    };
    let leaves = tab.leaf_count();
    if leaves == 0 {
        cancel_picker(app);
        return;
    }
    let idx = leaf_idx.min(leaves - 1);
    let mut visited = 0usize;
    apply_to_nth_leaf(&mut tab.root, idx, &mut visited, &mut |pane| {
        pane.block_selected = Some(target);
        pane.block_region = 0;
    });
    cancel_picker(app);
}

fn cancel_picker(app: &mut App) {
    if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.pane_picker = None;
    }
}

fn apply_to_nth_leaf(
    node: &mut crate::pane::PaneNode,
    target: usize,
    counter: &mut usize,
    f: &mut impl FnMut(&mut crate::pane::Pane),
) -> bool {
    match node {
        crate::pane::PaneNode::Leaf(pane) => {
            if *counter == target {
                f(pane);
                return true;
            }
            *counter += 1;
            false
        }
        crate::pane::PaneNode::Split { first, second, .. } => {
            if apply_to_nth_leaf(first, target, counter, f) {
                return true;
            }
            apply_to_nth_leaf(second, target, counter, f)
        }
    }
}

/// Cycle the response sub-tab on the focused pane. Only applies
/// when the focused block is HTTP and the focused region is the
/// Response region (region == 3). 5 tabs: Body / Headers / Cookies
/// / Timing / History.
fn shift_response_subtab(app: &mut App, delta: isize) {
    let target = app
        .blocks_workspace
        .as_ref()
        .zip(app.active_pane())
        .and_then(|(ws, pane)| {
            let sel = pane.block_selected?;
            let file = ws.index.files.get(sel.file_idx)?;
            let block = file.blocks.get(sel.block_idx)?;
            if block.block_type != "http" || pane.block_region != 3 {
                return None;
            }
            Some((file.display.clone(), block.alias.clone()))
        });
    let Some((file, alias)) = target else {
        return;
    };
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let count: isize = 5;
    pane.response_subtab =
        (pane.response_subtab as isize + delta).rem_euclid(count) as usize;
    if pane.response_subtab == 4 {
        if let Some(alias) = alias {
            refresh_pane_history(app, file, alias);
        }
    }
}

/// Pull recent `block_run_history` rows for (file, alias) and stash
/// them on the focused pane. Block-on-pool is OK here because the
/// list is capped by `history_retention` (default 10).
fn refresh_pane_history(app: &mut App, file: String, alias: String) {
    let pool = app.pool_manager.app_pool().clone();
    let entries = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            httui_core::block_history::list_history(&pool, &file, &alias)
                .await
                .unwrap_or_default()
        })
    });
    if let Some(pane) = app.active_pane_mut() {
        pane.response_history = Some(Box::new(crate::pane::ResponseHistory {
            file,
            alias,
            cursor: 0,
            entries,
        }));
    }
}

fn shift_region(app: &mut App, delta: isize) {
    let count = active_block_region_count(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    if count == 0 {
        pane.block_region = 0;
        return;
    }
    let current = pane.block_region as isize;
    let next = (current + delta).rem_euclid(count as isize);
    pane.block_region = next as usize;
    pane.block_row = 0;
    pane.block_col = 1;
}

#[derive(Debug, Clone, Copy)]
enum EnterMode {
    /// Profile picks: standard lands in INSERT, vim in NORMAL. Used by
    /// the `Enter` chord from NAV.
    Auto,
    /// Force INSERT (used by vim `i`/`a`/`o`).
    Insert,
}

fn shift_row(app: &mut App, delta: isize) {
    let count = active_region_row_count(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    if count == 0 {
        pane.block_row = 0;
        return;
    }
    let last = (count - 1) as isize;
    pane.block_row = (pane.block_row as isize + delta).clamp(0, last) as usize;
}

fn shift_col(app: &mut App, delta: isize) {
    let cols = active_region_col_count(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    if cols == 0 {
        pane.block_col = 0;
        return;
    }
    let last = (cols - 1) as isize;
    pane.block_col = (pane.block_col as isize + delta).clamp(0, last) as usize;
}

/// Row count of the focused region in the focused pane. Single-line
/// regions return `1` so vertical motion is a no-op clamp rather than a
/// division-by-zero / panic.
fn active_region_row_count(app: &App) -> usize {
    let Some(pane) = app.active_pane() else {
        return 0;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return 0;
    };
    let Some(target) = pane.block_selected else {
        return 0;
    };
    let Some(file) = ws.index.files.get(target.file_idx) else {
        return 0;
    };
    let Some(block) = file.blocks.get(target.block_idx) else {
        return 0;
    };
    if block.block_type.starts_with("db") && pane.block_region == 2 {
        return db_result_row_count(pane).max(1);
    }
    if block.block_type != "http" {
        return 1;
    }
    match pane.block_region {
        1 => {
            // Headers row count comes from the draft if any, otherwise
            // the on-disk parse — the renderer reads the same source so
            // the cursor never points past a non-existent row.
            if let Some(draft) = pane.block_draft.as_ref() {
                draft.header_count().max(1)
            } else {
                read_header_count(&app.vault_path, &file.path, block.line_start).max(1)
            }
        }
        _ => 1,
    }
}

/// `Some((segment_idx, row))` when the focused pane is on `[3]
/// Result` of a DB block whose run is cached. Used to park the
/// cursor before delegating to the DOC view's row-detail handler.
fn focused_db_result_row(app: &App) -> Option<(usize, usize)> {
    let pane = app.active_pane()?;
    if pane.block_region != 2 {
        return None;
    }
    let ws = app.blocks_workspace.as_ref()?;
    let sel = pane.block_selected?;
    let file = ws.index.files.get(sel.file_idx)?;
    let block = file.blocks.get(sel.block_idx)?;
    if !block.block_type.starts_with("db") {
        return None;
    }
    let doc = pane.document.as_ref()?;
    for (idx, seg) in doc.segments().iter().enumerate() {
        if let crate::buffer::Segment::Block(b) = seg {
            if b.block_type == block.block_type && b.alias == block.alias {
                let total = b
                    .cached_result
                    .as_ref()
                    .and_then(|v| v.get("results"))
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|r| r.get("rows"))
                    .and_then(|r| r.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                if total == 0 {
                    return None;
                }
                let row = pane.block_row.min(total.saturating_sub(1));
                return Some((idx, row));
            }
        }
    }
    None
}

/// Number of result rows accessible from the focused pane's loaded
/// document. `0` when the block hasn't been run yet so up/down stays
/// pinned.
fn db_result_row_count(pane: &crate::pane::Pane) -> usize {
    let doc = match pane.document.as_ref() {
        Some(d) => d,
        None => return 0,
    };
    for seg in doc.segments() {
        if let crate::buffer::Segment::Block(b) = seg {
            if b.block_type.starts_with("db") {
                return b
                    .cached_result
                    .as_ref()
                    .and_then(|v| v.get("results"))
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|r| r.get("rows"))
                    .and_then(|r| r.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
            }
        }
    }
    0
}

/// Column count of the focused region. Headers have `2` (key + value).
/// Every other region is single-column.
fn active_region_col_count(app: &App) -> usize {
    let Some(pane) = app.active_pane() else {
        return 0;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return 0;
    };
    let Some(target) = pane.block_selected else {
        return 0;
    };
    let Some(file) = ws.index.files.get(target.file_idx) else {
        return 0;
    };
    let Some(block) = file.blocks.get(target.block_idx) else {
        return 0;
    };
    if block.block_type == "http" && pane.block_region == 1 {
        2
    } else {
        1
    }
}

fn read_header_count(
    vault: &std::path::Path,
    file: &std::path::Path,
    line_start: usize,
) -> usize {
    let Ok(text) = httui_core::fs::read_note(&vault.to_string_lossy(), &file.to_string_lossy())
    else {
        return 0;
    };
    let parsed = httui_core::blocks::parse_blocks(&text);
    let Some(p) = parsed.iter().find(|p| p.line_start == line_start) else {
        return 0;
    };
    p.params
        .get("headers")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0)
}

/// Open the field at `(region, row, col)` for inline editing. The
/// sub-`Document` is seeded from the draft's current value so vim
/// motions / search / undo land on something real from frame zero.
/// `EnterMode::Auto` defers the sub-mode to the active profile:
/// standard → INSERT, vim → NORMAL. `EnterMode::Insert` forces INSERT
/// (vim `i`/`a`/`o`).
fn enter_edit(app: &mut App, mode: EnterMode) {
    // Result region of a DB block: Enter opens the row-detail modal
    // (reuses the DOC view's `apply_open_db_row_detail` after parking
    // the cursor on the focused row).
    if let Some((segment_idx, row)) = focused_db_result_row(app) {
        if let Some(doc) = app.document_mut() {
            doc.set_cursor(crate::buffer::Cursor::InBlockResult { segment_idx, row });
        }
        crate::input::apply::modal_detail::apply_open_db_row_detail(app);
        return;
    }
    let Some(field) = field_for_focus(app) else {
        return;
    };
    let needs_hydrate = app
        .active_pane()
        .map(|p| p.block_draft.is_none())
        .unwrap_or(true);
    if needs_hydrate && !hydrate_draft(app) {
        app.set_status(StatusKind::Error, "block missing on disk");
        return;
    }
    let initial = current_field_value(app, &field).unwrap_or_default();
    let vim = app.config.editor.mode == EditorMode::Vim;
    let edit = match mode {
        EnterMode::Insert => RegionEdit::insert(field, initial),
        EnterMode::Auto if vim => RegionEdit::normal(field, initial),
        EnterMode::Auto => RegionEdit::insert(field, initial),
    };
    if let Some(pane) = app.active_pane_mut() {
        pane.block_edit = Some(Box::new(edit));
    }
    // Pin the vim engine to the mode matching the sub-doc so chords
    // arrive at the right parser (`parse_normal` for NORMAL,
    // `parse_insert` for INSERT). Standard profile ignores vim.mode.
    let mode = if matches!(
        app.active_pane().and_then(|p| p.block_edit.as_ref()).map(|e| e.sub_mode),
        Some(EditSubMode::Insert)
    ) {
        Mode::Insert
    } else {
        Mode::Normal
    };
    app.vim.mode = mode;
    app.vim.reset_pending();
}

/// Esc on an active buffer: serialize the sub-doc and write it into
/// the draft's matching field, then clear `block_edit`. The trailing
/// newline that `Document::to_markdown` appends is stripped — it's a
/// rendering artefact, not part of the user's value.
fn commit_edit(app: &mut App) {
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let Some(edit) = pane.block_edit.take() else {
        return;
    };
    let Some(draft) = pane.block_draft.as_mut() else {
        return;
    };
    let value = edit.current_text();
    match edit.field {
        EditField::HttpUrl => draft.set_url(value),
        EditField::HttpHeaderKey(row) => {
            draft.set_header(row, 0, value);
        }
        EditField::HttpHeaderValue(row) => {
            draft.set_header(row, 1, value);
        }
        EditField::HttpBody => draft.set_body(value),
        EditField::DbQuery => draft.set_query(value),
    }
    // Restore vim NORMAL on the (now hidden) field so the next Enter
    // doesn't land in stale Insert state.
    app.vim.enter_normal();
}

/// `r` in BLOCKS NAV: ensure the focused pane has the block's file
/// loaded into `pane.document`, park the cursor on that block's
/// segment, then delegate to the existing run pipeline. The pipeline
/// reads `app.document()` → our redirect returns `pane.document` (NAV
/// has no `block_edit`), so the executor / cache / refs machinery
/// works without forking.
fn run_focused_block(app: &mut App) {
    if !matches!(app.view, AppView::Blocks) {
        return;
    }
    let Some(pane) = app.active_pane() else { return };
    let Some(sel) = pane.block_selected else {
        app.set_status(StatusKind::Error, "no block selected");
        return;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return;
    };
    let Some(file) = ws.index.files.get(sel.file_idx) else {
        return;
    };
    let Some(block) = file.blocks.get(sel.block_idx) else {
        return;
    };
    let file_path_rel = file.path.clone();
    let target_alias = block.alias.clone();
    let target_type = block.block_type.clone();
    let abs_path = app.vault_path.join(&file_path_rel);

    // Load (or reload) the file into the pane's document if it
    // doesn't already point at the bloc's file. Stale results from a
    // previous file would carry over otherwise.
    let needs_load = app
        .active_pane()
        .map(|p| p.document_path.as_deref() != Some(abs_path.as_path()))
        .unwrap_or(true);
    if needs_load {
        let text = match httui_core::fs::read_note(
            &app.vault_path.to_string_lossy(),
            &file_path_rel.to_string_lossy(),
        ) {
            Ok(t) => t,
            Err(e) => {
                app.set_status(StatusKind::Error, format!("read failed: {e}"));
                return;
            }
        };
        let doc = match crate::buffer::Document::from_markdown(&text) {
            Ok(d) => d,
            Err(e) => {
                app.set_status(StatusKind::Error, format!("parse failed: {e}"));
                return;
            }
        };
        if let Some(p) = app.active_pane_mut() {
            p.document = Some(doc);
            p.document_path = Some(abs_path);
        }
    }

    // Map the selected block to a segment_idx in the doc. Match by
    // `(block_type, alias)` — the sidebar guarantees these identify
    // a unique block within the file at this point in the workspace
    // index.
    let segment_idx = {
        let Some(doc) = app.document() else { return };
        let mut found = None;
        for (idx, seg) in doc.segments().iter().enumerate() {
            if let crate::buffer::Segment::Block(b) = seg {
                if b.block_type == target_type && b.alias == target_alias {
                    found = Some(idx);
                    break;
                }
            }
        }
        match found {
            Some(s) => s,
            None => {
                app.set_status(StatusKind::Error, "block not found in file");
                return;
            }
        }
    };

    // Park the cursor on the block so `apply_run_block` picks it up.
    if let Some(doc) = app.document_mut() {
        doc.set_cursor(crate::buffer::Cursor::InBlock {
            segment_idx,
            offset: 0,
        });
    }

    crate::commands::refs::apply_run_block(app);
}

fn cancel_edit(app: &mut App) {
    if let Some(pane) = app.active_pane_mut() {
        pane.block_edit = None;
    }
    app.vim.enter_normal();
}

/// `]b`/`[b` motion — flatten every block in the workspace into a
/// single list and step `delta` positions, wrapping at both ends.
fn shift_block(app: &mut App, delta: isize) {
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return;
    };
    let flat: Vec<crate::app::BlockRef> = ws
        .index
        .files
        .iter()
        .enumerate()
        .flat_map(|(fi, f)| {
            (0..f.blocks.len()).map(move |bi| crate::app::BlockRef {
                file_idx: fi,
                block_idx: bi,
            })
        })
        .collect();
    if flat.is_empty() {
        return;
    }
    let current = app
        .active_pane()
        .and_then(|p| p.block_selected)
        .or(ws.selected);
    let pos = current
        .and_then(|sel| flat.iter().position(|r| *r == sel))
        .unwrap_or(0) as isize;
    let len = flat.len() as isize;
    let next = ((pos + delta) % len + len) % len;
    let target = flat[next as usize];
    if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.select(target);
        if !ws.expanded.contains(&target.file_idx) {
            ws.expanded.insert(target.file_idx);
        }
        if let Some(row) = ws
            .rows()
            .iter()
            .position(|r| r.file_idx == target.file_idx && r.block_idx == Some(target.block_idx))
        {
            ws.cursor = row;
        }
    }
    if let Some(pane) = app.active_pane_mut() {
        pane.block_selected = Some(target);
        pane.block_region = 0;
        pane.block_row = 0;
        pane.block_col = 1;
    }
}

/// Append a new HTTP block to the file the sidebar cursor is on.
/// Reads the file, parses, builds a canonical empty HTTP block,
/// writes back, refreshes the index. Auto-aliases as `untitled1`,
/// `untitled2`, … to avoid colliding with existing aliases.
fn tree_new_block(app: &mut App) {
    let Some(node) = app.tree.current().cloned() else {
        return;
    };
    // Cursor on a block row → resolve the parent `.md` via the
    // workspace index. Cursor on a `.md` row → use it directly.
    let rel_path = if let Some(meta) = node.block.as_ref() {
        let ws = app.blocks_workspace.as_ref();
        let file = ws.and_then(|w| w.index.files.get(meta.file_idx));
        match file {
            Some(f) => f.path.to_string_lossy().to_string(),
            None => return,
        }
    } else if node.is_dir || !node.path.ends_with(".md") {
        return;
    } else {
        node.path.clone()
    };
    let vault = app.vault_path.to_string_lossy().to_string();
    let Ok(text) = httui_core::fs::read_note(&vault, &rel_path) else {
        app.set_status(StatusKind::Error, "could not read file");
        return;
    };
    let parsed = httui_core::blocks::parse_blocks(&text);
    let used_aliases: std::collections::HashSet<String> = parsed
        .iter()
        .filter_map(|p| p.alias.clone())
        .collect();
    let mut idx = parsed.len() + 1;
    let alias = loop {
        let candidate = format!("untitled{idx}");
        if !used_aliases.contains(&candidate) {
            break candidate;
        }
        idx += 1;
    };
    let appended = if text.ends_with('\n') {
        format!(
            "{text}\n```http alias={alias}\nGET https://example.com\n```\n"
        )
    } else {
        format!(
            "{text}\n\n```http alias={alias}\nGET https://example.com\n```\n"
        )
    };
    if let Err(e) = httui_core::fs::write_note(&vault, &rel_path, &appended) {
        app.set_status(StatusKind::Error, format!("write failed: {e}"));
        return;
    }
    refresh_blocks_index_and_tree(app);
    app.set_status(StatusKind::Info, format!("new block: {alias}"));
}

/// Open the destructive-confirm prompt for the focused block. Same
/// shape as the file-delete prompt — Enter on `y`/`Y` runs the
/// actual removal via [`tree_delete_block_confirmed`].
fn tree_delete_block(app: &mut App) {
    let Some(node) = app.tree.current().cloned() else {
        return;
    };
    let Some(meta) = node.block.as_ref() else {
        return;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return;
    };
    let Some(file) = ws.index.files.get(meta.file_idx) else {
        return;
    };
    let Some(block) = file.blocks.get(meta.block_idx) else {
        return;
    };
    let label = block.label();
    let rel_path = file.path.to_string_lossy().to_string();
    app.tree.prompt = Some(crate::tree::TreePrompt::new(
        crate::tree::TreePromptKind::DeleteBlock {
            rel_path,
            block_idx: meta.block_idx,
            label,
        },
        String::new(),
    ));
    app.vim.mode = crate::vim::mode::Mode::TreePrompt;
}

/// Execute the block delete after the user typed `y` / `Y` in the
/// confirm prompt. Reads the file, drops the block fence (and its
/// trailing blank line), writes back, refreshes the index.
pub(crate) fn tree_delete_block_confirmed(
    app: &mut App,
    rel_path: &str,
    block_idx: usize,
) {
    let vault = app.vault_path.to_string_lossy().to_string();
    let Ok(text) = httui_core::fs::read_note(&vault, &rel_path) else {
        return;
    };
    let parsed = httui_core::blocks::parse_blocks(&text);
    let Some(target) = parsed.get(block_idx) else {
        return;
    };
    let lines: Vec<&str> = text.lines().collect();
    let start = target.line_start;
    let end = target.line_end.min(lines.len().saturating_sub(1));
    // Drop the block AND one trailing blank line (if any) so the
    // resulting markdown doesn't grow a double-blank gap.
    let drop_until = if end + 1 < lines.len() && lines[end + 1].trim().is_empty() {
        end + 1
    } else {
        end
    };
    let mut out = String::new();
    for l in &lines[..start] {
        out.push_str(l);
        out.push('\n');
    }
    if drop_until + 1 < lines.len() {
        for l in &lines[drop_until + 1..] {
            out.push_str(l);
            out.push('\n');
        }
    }
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    if let Err(e) = httui_core::fs::write_note(&vault, &rel_path, &out) {
        app.set_status(StatusKind::Error, format!("write failed: {e}"));
        return;
    }
    refresh_blocks_index_and_tree(app);
    // Clamp cursor so it doesn't dangle on a now-missing row.
    if app.tree.selected > 0 {
        let count = app.tree.entries.len();
        if app.tree.selected >= count {
            app.tree.selected = count.saturating_sub(1);
        }
    }
    app.set_status(StatusKind::Info, "block removed");
}

/// Rebuild both the workspace `BlockIndex` and the file tree's
/// `block_index` copy, then refresh the visible tree entries. Tree
/// shows stale data otherwise — the two indices live separately so
/// updating only one leaves the sidebar out of sync.
fn refresh_blocks_index_and_tree(app: &mut App) {
    let vault_path = app.vault_path.clone();
    let fresh = crate::app::BlockIndex::build(&vault_path);
    if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.index = fresh.clone();
    }
    app.tree.block_index = Some(fresh);
    app.tree.refresh(&vault_path);
}

/// Swap the currently-focused block in the sidebar with its
/// neighbour by `delta` (+1 down, -1 up) in the same file. Updates
/// the `.md` on disk and refreshes the index. No-op when the block
/// is at the edge.
fn tree_reorder_block(app: &mut App, delta: isize) {
    let Some(node) = app.tree.current().cloned() else {
        return;
    };
    let Some(meta) = node.block.as_ref() else {
        return;
    };
    let block_idx = meta.block_idx;
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return;
    };
    let Some(file) = ws.index.files.get(meta.file_idx) else {
        return;
    };
    let target_idx = block_idx as isize + delta;
    if target_idx < 0 || target_idx as usize >= file.blocks.len() {
        return;
    }
    let rel_path = file.path.to_string_lossy().to_string();
    let vault = app.vault_path.to_string_lossy().to_string();
    let Ok(text) = httui_core::fs::read_note(&vault, &rel_path) else {
        return;
    };
    let mut parsed = httui_core::blocks::parse_blocks(&text);
    if block_idx >= parsed.len() || target_idx as usize >= parsed.len() {
        return;
    }
    // Build the rewritten markdown by extracting the two block ranges
    // and swapping them. Prose between blocks stays put.
    let a = block_idx.min(target_idx as usize);
    let b = block_idx.max(target_idx as usize);
    let lines: Vec<&str> = text.lines().collect();
    let block_a = &parsed[a];
    let block_b = &parsed[b];
    let a_start = block_a.line_start;
    let a_end = block_a.line_end.min(lines.len().saturating_sub(1));
    let b_start = block_b.line_start;
    let b_end = block_b.line_end.min(lines.len().saturating_sub(1));
    let mut out = String::new();
    // Lines before a
    for l in &lines[..a_start] {
        out.push_str(l);
        out.push('\n');
    }
    // Block b in a's place
    out.push_str(&lines[b_start..=b_end].join("\n"));
    out.push('\n');
    // Lines between a and b (a_end+1 .. b_start)
    if a_end + 1 < b_start {
        for l in &lines[a_end + 1..b_start] {
            out.push_str(l);
            out.push('\n');
        }
    }
    // Block a in b's place
    out.push_str(&lines[a_start..=a_end].join("\n"));
    out.push('\n');
    // Lines after b
    if b_end + 1 < lines.len() {
        for l in &lines[b_end + 1..] {
            out.push_str(l);
            out.push('\n');
        }
    }
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    if let Err(e) = httui_core::fs::write_note(&vault, &rel_path, &out) {
        app.set_status(StatusKind::Error, format!("write failed: {e}"));
        return;
    }
    app.set_status(
        StatusKind::Info,
        format!("reordered block {block_idx} ↔ {}", target_idx),
    );
    let _ = parsed; // dropped before mut borrow below
    refresh_blocks_index_and_tree(app);
    // Move the sidebar cursor along with the block so the user can
    // keep stepping in the same direction. Tree entries are flat
    // (file row + expanded block rows), and reorder only swaps two
    // adjacent block rows under the same file — so a `delta`-step
    // in entry index lands on the moved block's new row.
    let new_selected = (app.tree.selected as isize + delta).max(0) as usize;
    app.tree.selected = new_selected;
}

/// `o`/`Insert` in HTTP `[2] Headers`: append an empty row right
/// after the focused one, advance the cursor to the new row's key
/// cell. Hydrates the draft on first use so subsequent edits land in
/// the same in-memory copy.
fn insert_header_row(app: &mut App) {
    if !focused_region_is_http_headers(app) {
        return;
    }
    if app
        .active_pane()
        .map(|p| p.block_draft.is_none())
        .unwrap_or(true)
        && !hydrate_draft(app)
    {
        app.set_status(StatusKind::Error, "block missing on disk");
        return;
    }
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let Some(draft) = pane.block_draft.as_mut() else {
        return;
    };
    let insert_at = pane.block_row.saturating_add(1).min(draft.header_count());
    let arr = draft
        .block
        .params
        .as_object_mut()
        .and_then(|o| o.get_mut("headers"))
        .and_then(|v| v.as_array_mut());
    let arr = match arr {
        Some(a) => a,
        None => {
            // No `headers` key yet — synth one.
            let params = draft
                .block
                .params
                .as_object_mut()
                .expect("ParsedBlock.params is always an object");
            params.insert(
                "headers".to_string(),
                serde_json::Value::Array(Vec::new()),
            );
            params
                .get_mut("headers")
                .and_then(|v| v.as_array_mut())
                .expect("just inserted")
        }
    };
    arr.insert(insert_at, serde_json::json!({"key": "", "value": ""}));
    pane.block_row = insert_at;
    pane.block_col = 0;
}

/// `d`/`Delete` in HTTP `[2] Headers`: remove the focused row. Cursor
/// clamps to the row above (or 0 when the list went empty). No-op when
/// the headers array is empty.
fn delete_header_row(app: &mut App) {
    if !focused_region_is_http_headers(app) {
        return;
    }
    if app
        .active_pane()
        .map(|p| p.block_draft.is_none())
        .unwrap_or(true)
        && !hydrate_draft(app)
    {
        return;
    }
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let Some(draft) = pane.block_draft.as_mut() else {
        return;
    };
    let arr = draft
        .block
        .params
        .as_object_mut()
        .and_then(|o| o.get_mut("headers"))
        .and_then(|v| v.as_array_mut());
    let Some(arr) = arr else {
        return;
    };
    if arr.is_empty() || pane.block_row >= arr.len() {
        return;
    }
    arr.remove(pane.block_row);
    if pane.block_row > 0 && pane.block_row >= arr.len() {
        pane.block_row -= 1;
    }
}

/// Map `(region, row, col)` for the focused pane to an `EditField`.
/// Response is read-only (no editable field); DB Connection (`[1]`) is
/// picker-driven (Story 9) so it has no inline buffer.
fn field_for_focus(app: &App) -> Option<EditField> {
    let pane = app.active_pane()?;
    let ws = app.blocks_workspace.as_ref()?;
    let target = pane.block_selected?;
    let file = ws.index.files.get(target.file_idx)?;
    let block = file.blocks.get(target.block_idx)?;
    if block.block_type == "http" {
        return match pane.block_region {
            0 => Some(EditField::HttpUrl),
            1 => Some(if pane.block_col == 0 {
                EditField::HttpHeaderKey(pane.block_row)
            } else {
                EditField::HttpHeaderValue(pane.block_row)
            }),
            2 => Some(EditField::HttpBody),
            _ => None,
        };
    }
    if block.block_type.starts_with("db") {
        return match pane.block_region {
            1 => Some(EditField::DbQuery),
            _ => None,
        };
    }
    None
}

/// Read the current value of `field` from the focused pane's draft.
/// Falls back to the empty string when the draft hasn't been hydrated
/// yet (every caller hydrates first, but the empty default keeps the
/// function total).
fn current_field_value(app: &App, field: &EditField) -> Option<String> {
    let pane = app.active_pane()?;
    let draft = pane.block_draft.as_ref()?;
    let out = match field {
        EditField::HttpUrl => draft.url().to_string(),
        EditField::HttpHeaderKey(row) => draft.header_at(*row, 0).to_string(),
        EditField::HttpHeaderValue(row) => draft.header_at(*row, 1).to_string(),
        EditField::HttpBody => draft.body().to_string(),
        EditField::DbQuery => draft.query().to_string(),
    };
    Some(out)
}

/// First-edit lazy: build a `BlockDraft` from the on-disk parse of
/// the focused block. Returns `false` when the block can't be located
/// (file deleted, line offset stale).
fn hydrate_draft(app: &mut App) -> bool {
    let Some(pane) = app.active_pane() else {
        return false;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return false;
    };
    let Some(target) = pane.block_selected else {
        return false;
    };
    let Some(file) = ws.index.files.get(target.file_idx) else {
        return false;
    };
    let Some(block) = file.blocks.get(target.block_idx) else {
        return false;
    };
    let Ok(text) = httui_core::fs::read_note(
        &app.vault_path.to_string_lossy(),
        &file.path.to_string_lossy(),
    ) else {
        return false;
    };
    let parsed = httui_core::blocks::parse_blocks(&text);
    let Some(found) = parsed
        .into_iter()
        .find(|p| p.line_start == block.line_start && p.block_type == block.block_type)
    else {
        return false;
    };
    let draft = BlockDraft {
        file_path: file.path.clone(),
        block_line_start: block.line_start,
        block: found,
    };
    if let Some(pane) = app.active_pane_mut() {
        pane.block_draft = Some(Box::new(draft));
        return true;
    }
    false
}

/// Ctrl+S: serialize every dirty pane in the focused tab back into its
/// `.md` via `write_note`, then clear the draft. Saving is per-pane
/// (not per-tab) so two panes editing different files both flush.
fn save_draft(app: &mut App) {
    let dirty = collect_dirty_panes(app);
    if dirty.is_empty() {
        return;
    }
    let vault = app.vault_path.clone();
    let mut saved = 0usize;
    let mut failed: Vec<String> = Vec::new();
    for (path, line_start) in dirty {
        // Re-borrow per pane to satisfy the borrow checker — each pane
        // mutation is independent and the draft contents are cloned
        // before the write so the file IO doesn't overlap the borrow.
        let Some((draft_block, draft_path)) = take_draft_for(app, &path, line_start) else {
            continue;
        };
        match save_block_to_disk(&vault, &draft_path, line_start, &draft_block) {
            Ok(_) => {
                saved += 1;
            }
            Err(e) => {
                tracing::warn!(error = %e, "blocks-view save failed");
                failed.push(format!("{}: {e}", draft_path.display()));
                // Re-install the draft so the user doesn't silently
                // lose the unsaved edits on a failed write.
                restore_draft(app, &draft_path, line_start, draft_block);
            }
        }
    }
    if !failed.is_empty() {
        app.set_status(StatusKind::Error, format!("save failed: {}", failed.join("; ")));
    } else if saved > 0 {
        // Rebuild the index so the sidebar reflects fresh aliases /
        // header counts after the save. Keep selection by ref.
        if let Some(ws) = app.blocks_workspace.as_mut() {
            ws.index = BlockIndex::build(&vault);
        }
        app.set_status(
            StatusKind::Info,
            if saved == 1 {
                "saved".to_string()
            } else {
                format!("saved {saved} blocks")
            },
        );
    }
}

/// Walk every pane in the active tab and collect the `(file_path,
/// line_start)` pair of each one that has a draft. Returns an empty
/// vec when nothing is dirty.
fn collect_dirty_panes(app: &App) -> Vec<(std::path::PathBuf, usize)> {
    let Some(tab) = app.active_tab() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk_panes(&tab.root, &mut |pane| {
        if let Some(draft) = pane.block_draft.as_ref() {
            out.push((draft.file_path.clone(), draft.block_line_start));
        }
    });
    out
}

fn walk_panes(node: &crate::pane::PaneNode, f: &mut impl FnMut(&crate::pane::Pane)) {
    match node {
        crate::pane::PaneNode::Leaf(p) => f(p),
        crate::pane::PaneNode::Split { first, second, .. } => {
            walk_panes(first, f);
            walk_panes(second, f);
        }
    }
}

fn walk_panes_mut(
    node: &mut crate::pane::PaneNode,
    f: &mut impl FnMut(&mut crate::pane::Pane),
) {
    match node {
        crate::pane::PaneNode::Leaf(p) => f(p),
        crate::pane::PaneNode::Split { first, second, .. } => {
            walk_panes_mut(first, f);
            walk_panes_mut(second, f);
        }
    }
}

fn take_draft_for(
    app: &mut App,
    file_path: &std::path::Path,
    line_start: usize,
) -> Option<(httui_core::blocks::parser::ParsedBlock, std::path::PathBuf)> {
    let tab = app.active_tab_mut()?;
    let mut out = None;
    walk_panes_mut(&mut tab.root, &mut |pane| {
        if out.is_some() {
            return;
        }
        if let Some(draft) = pane.block_draft.as_ref() {
            if draft.file_path == file_path && draft.block_line_start == line_start {
                let taken = pane.block_draft.take().expect("just matched");
                out = Some((taken.block, taken.file_path));
            }
        }
    });
    out
}

fn restore_draft(
    app: &mut App,
    file_path: &std::path::Path,
    line_start: usize,
    block: httui_core::blocks::parser::ParsedBlock,
) {
    let Some(tab) = app.active_tab_mut() else {
        return;
    };
    let mut installed = false;
    walk_panes_mut(&mut tab.root, &mut |pane| {
        if installed {
            return;
        }
        let matches = pane
            .block_selected
            .map(|sel| {
                pane.block_draft.is_none()
                    && pane
                        .document_path
                        .as_ref()
                        .map(|_| sel)
                        .is_some()
            })
            .unwrap_or(false);
        if matches {
            pane.block_draft = Some(Box::new(BlockDraft {
                file_path: file_path.to_path_buf(),
                block_line_start: line_start,
                block: block.clone(),
            }));
            installed = true;
        }
    });
}

/// Serialize `draft` and replace the original block region in the file
/// on disk. The serializer is the same one the desktop uses, so the
/// resulting fence parses byte-identical to the in-memory ParsedBlock.
fn save_block_to_disk(
    vault: &std::path::Path,
    file_path: &std::path::Path,
    line_start: usize,
    draft: &httui_core::blocks::parser::ParsedBlock,
) -> std::io::Result<()> {
    let vault_str = vault.to_string_lossy().to_string();
    let file_str = file_path.to_string_lossy().to_string();
    let current = httui_core::fs::read_note(&vault_str, &file_str)
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let lines: Vec<&str> = current.lines().collect();
    // Find the block in the current text using the parser — the
    // user might have edited the file between hydrate and save, so we
    // can't trust the original `line_end` blindly.
    let parsed = httui_core::blocks::parse_blocks(&current);
    let Some(target) = parsed
        .iter()
        .find(|p| p.line_start == line_start && p.block_type == draft.block_type)
    else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "block no longer present at the recorded offset",
        ));
    };
    let start = target.line_start;
    let end = target.line_end.min(lines.len().saturating_sub(1));
    let serialized = httui_core::blocks::serialize_block(draft);
    let trailing_newline = current.ends_with('\n');
    let mut out = String::new();
    for line in &lines[..start] {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&serialized);
    if end + 1 < lines.len() {
        out.push('\n');
        for line in &lines[end + 1..] {
            out.push_str(line);
            out.push('\n');
        }
    } else if trailing_newline {
        out.push('\n');
    }
    httui_core::fs::write_note(&vault_str, &file_str, &out)
        .map_err(|e| std::io::Error::other(e.to_string()))
}

fn set_region(app: &mut App, index: usize) {
    let count = active_block_region_count(app);
    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    if count == 0 {
        pane.block_region = 0;
        return;
    }
    pane.block_region = index.min(count - 1);
}

fn active_block_region_count(app: &App) -> usize {
    let Some(pane) = app.active_pane() else {
        return 0;
    };
    let Some(ws) = app.blocks_workspace.as_ref() else {
        return 0;
    };
    let Some(target) = pane.block_selected else {
        return 0;
    };
    ws.index
        .files
        .get(target.file_idx)
        .and_then(|f| f.blocks.get(target.block_idx))
        .map(|b| crate::app::region_count_for(&b.block_type))
        .unwrap_or(0)
}

fn toggle_view(app: &mut App) {
    match app.view {
        AppView::Doc => enter_blocks(app),
        AppView::Blocks => exit_blocks(app),
    }
}

fn enter_blocks(app: &mut App) {
    let index = BlockIndex::build(&app.vault_path);
    if app.blocks_workspace.is_none() {
        app.blocks_workspace = Some(BlocksWorkspace::new(index.clone()));
    } else if let Some(ws) = app.blocks_workspace.as_mut() {
        ws.index = index.clone();
        if let Some(sel) = ws.selected {
            let still_valid = ws
                .index
                .files
                .get(sel.file_idx)
                .map(|f| sel.block_idx < f.blocks.len())
                .unwrap_or(false);
            if !still_valid {
                ws.selected = None;
            }
        }
    }
    app.view = AppView::Blocks;
    app.tree.block_index = Some(index);
    app.tree.visible = true;
    let vault = app.vault_path.clone();
    app.tree.refresh(&vault);
    app.vim.mode = Mode::Tree;
    app.vim.reset_pending();
}

fn exit_blocks(app: &mut App) {
    app.view = AppView::Doc;
    app.tree.block_index = None;
    app.tree.expanded.clear();
    let vault = app.vault_path.clone();
    app.tree.refresh(&vault);
    app.tree.selected = 0;
    if matches!(app.vim.mode, Mode::Tree | Mode::TreePrompt) {
        app.vim.enter_normal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppView;
    use crate::config::Config;
    use crate::vault::ResolvedVault;
    use httui_core::db::init_db;
    use std::path::Path;
    use tempfile::TempDir;

    fn write(dir: &Path, rel: &str, body: &str) {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, body).unwrap();
    }

    async fn app_with_blocks() -> (App, TempDir, TempDir) {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        write(
            vault.path(),
            "api.md",
            "# api\n\n```http alias=login\nGET https://x.com\n```\n",
        );
        write(
            vault.path(),
            "users.md",
            "# users\n\n```http alias=list\nGET https://x.com/users\n```\n",
        );
        let pool = init_db(data.path()).await.unwrap();
        let resolved = ResolvedVault {
            vault: vault.path().to_path_buf(),
        };
        let app = App::new(Config::default(), resolved, pool);
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_enters_blocks_with_index_loaded() {
        let (mut app, _d, _v) = app_with_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        assert!(matches!(app.view, AppView::Blocks));
        assert!(app.blocks_workspace.is_some());
        assert!(app.tree.block_index.is_some());
        assert!(app.tree.visible);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_back_restores_doc_view() {
        let (mut app, _d, _v) = app_with_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        apply_blocks_view(&mut app, Action::ToggleAppView);
        assert!(matches!(app.view, AppView::Doc));
        assert!(app.tree.block_index.is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_preserves_workspace_state_across_round_trips() {
        let (mut app, _d, _v) = app_with_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        if let Some(ws) = app.blocks_workspace.as_mut() {
            ws.selected = Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
        }
        apply_blocks_view(&mut app, Action::ToggleAppView);
        apply_blocks_view(&mut app, Action::ToggleAppView);
        let ws = app.blocks_workspace.as_ref().unwrap();
        assert_eq!(
            ws.selected,
            Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 0
            })
        );
    }

    /// Drive the focused pane into BLOCKS view with the first HTTP
    /// block selected — every Story 5 test starts here. Returns the
    /// vault dir so the test can read what got written.
    async fn enter_blocks_on_first_http() -> (App, TempDir, TempDir) {
        let (mut app, data, vault) = app_with_blocks().await;
        apply_blocks_view(&mut app, Action::ToggleAppView);
        if let Some(ws) = app.blocks_workspace.as_mut() {
            ws.selected = Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
        }
        if let Some(pane) = app.active_pane_mut() {
            pane.block_selected = Some(crate::app::BlockRef {
                file_idx: 0,
                block_idx: 0,
            });
            pane.block_region = 0;
            pane.block_row = 0;
            pane.block_col = 1;
        }
        (app, data, vault)
    }

    /// Stand-in for the engine wiring: simulate the user typing into
    /// the sub-Document by writing directly via the redirect, the
    /// same path the vim/standard route uses in production.
    fn type_into_active_edit(app: &mut App, text: &str) {
        let doc = app.document_mut().expect("EDIT must be active");
        for c in text.chars() {
            if c == '\n' {
                doc.insert_newline_at_cursor();
            } else {
                doc.insert_char_at_cursor(c);
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn enter_edit_url_hydrates_draft_and_seeds_subdoc() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        let pane = app.active_pane().unwrap();
        assert!(pane.block_draft.is_some(), "draft hydrated on first edit");
        let edit = pane.block_edit.as_ref().expect("edit state allocated");
        assert!(matches!(edit.field, EditField::HttpUrl));
        assert_eq!(edit.current_text(), "https://x.com");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn commit_writes_subdoc_text_into_draft() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, " /test");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        let pane = app.active_pane().unwrap();
        assert!(pane.block_edit.is_none(), "edit cleared after commit");
        assert_eq!(pane.block_draft.as_ref().unwrap().url(), "https://x.com /test");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_discards_subdoc_without_touching_draft() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "xxxx");
        apply_blocks_view(&mut app, Action::BlocksRegionCancelEdit);
        let pane = app.active_pane().unwrap();
        assert!(pane.block_edit.is_none(), "edit cleared after cancel");
        // Draft was hydrated by enter_edit (so it's Some), but the
        // URL is unchanged because the sub-doc was discarded.
        assert_eq!(pane.block_draft.as_ref().unwrap().url(), "https://x.com");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn save_writes_canonical_fence_to_disk() {
        let (mut app, _d, vault) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "/edited");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        apply_blocks_view(&mut app, Action::BlocksSaveDraft);
        let on_disk = std::fs::read_to_string(vault.path().join("api.md")).unwrap();
        assert!(
            on_disk.contains("https://x.com/edited"),
            "saved file should contain edited URL, got: {on_disk:?}"
        );
        let pane = app.active_pane().unwrap();
        assert!(pane.block_draft.is_none(), "draft cleared after save");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn toggle_view_with_dirty_opens_unsaved_prompt() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "z");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        apply_blocks_view(&mut app, Action::ToggleAppView);
        assert!(matches!(
            app.modal,
            Some(crate::modal::Modal::BlocksUnsavedPrompt(_))
        ));
        assert!(matches!(app.view, AppView::Blocks));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unsaved_prompt_discard_drops_drafts_and_toggles() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "z");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        apply_blocks_view(&mut app, Action::ToggleAppView);
        apply_blocks_view(&mut app, Action::BlocksUnsavedPromptDiscard);
        assert!(app.modal.is_none());
        assert!(matches!(app.view, AppView::Doc));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multiline_body_inserts_newline_then_commits_via_esc() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(3));
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        type_into_active_edit(&mut app, "line1\nline2");
        apply_blocks_view(&mut app, Action::BlocksRegionCommitEdit);
        let pane = app.active_pane().unwrap();
        assert_eq!(pane.block_draft.as_ref().unwrap().body(), "line1\nline2");
    }

    // ---- Story 6: vim opt-in (via Document) ----

    async fn enter_blocks_vim() -> (App, TempDir, TempDir) {
        let (mut app, data, vault) = enter_blocks_on_first_http().await;
        app.config.editor.mode = crate::config::EditorMode::Vim;
        (app, data, vault)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vim_enter_lands_with_engine_in_normal() {
        let (mut app, _d, _v) = enter_blocks_vim().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        assert!(app.active_pane().unwrap().block_edit.is_some());
        // Engine pinned to Normal so vim chords parse correctly.
        assert_eq!(app.vim.mode, Mode::Normal);
        assert_eq!(effective_sub_mode(&app), EditSubMode::Normal);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn vim_i_action_lands_engine_in_insert() {
        let (mut app, _d, _v) = enter_blocks_vim().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEditInsert);
        assert_eq!(app.vim.mode, Mode::Insert);
        assert_eq!(effective_sub_mode(&app), EditSubMode::Insert);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn standard_enter_lands_engine_in_insert() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksRegionEnterEdit);
        // Standard profile always reports Insert; vim.mode is the
        // same so the engine inserts directly.
        assert_eq!(effective_sub_mode(&app), EditSubMode::Insert);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn insert_header_row_grows_array_and_moves_cursor() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        let before = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .map(|d| d.header_count())
            .unwrap_or(0);
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        let pane = app.active_pane().unwrap();
        let after = pane.block_draft.as_ref().unwrap().header_count();
        assert_eq!(after, before + 1);
        assert_eq!(pane.block_col, 0, "cursor moved to new key cell");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn delete_header_row_shrinks_array() {
        let (mut app, _d, _v) = enter_blocks_on_first_http().await;
        apply_blocks_view(&mut app, Action::BlocksPaneJumpRegion(2));
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        apply_blocks_view(&mut app, Action::BlocksHeaderInsertRow);
        let after_insert = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .unwrap()
            .header_count();
        apply_blocks_view(&mut app, Action::BlocksHeaderDeleteRow);
        let after_delete = app
            .active_pane()
            .and_then(|p| p.block_draft.as_ref())
            .unwrap()
            .header_count();
        assert_eq!(after_delete, after_insert - 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn next_block_motion_wraps_workspace() {
        let (mut app, _d, _v) = enter_blocks_vim().await;
        let total: usize = app
            .blocks_workspace
            .as_ref()
            .unwrap()
            .index
            .files
            .iter()
            .map(|f| f.blocks.len())
            .sum();
        assert!(total >= 2, "fixture has >= 2 blocks");
        apply_blocks_view(&mut app, Action::BlocksNextBlockMotion);
        let after_one = app.active_pane().unwrap().block_selected;
        for _ in 0..total {
            apply_blocks_view(&mut app, Action::BlocksNextBlockMotion);
        }
        assert_eq!(app.active_pane().unwrap().block_selected, after_one);
    }
}
