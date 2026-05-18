use crossterm::event::KeyEvent;
use ropey::Rope;

use crate::app::{App, StatusKind};
use crate::buffer::block::BlockNode;
use crate::buffer::{Cursor, Segment};
use crate::commands::db::{load_active_env_vars, resolve_connection_id_sync};
use crate::pane::{FocusDir, SplitDir};
use crate::tree::{TreePrompt, TreePromptKind};
use crate::vim::change::{ChangeOrigin, ChangeRecord};
use crate::vim::ex::{self, ExResult};
use crate::vim::insert::{position_for_insert, recoil_after_exit};
use crate::vim::mode::Mode;
use crate::vim::motions;
use crate::vim::operator;
use crate::vim::parser::{
    parse_block_history, parse_block_template_picker, parse_cmdline, parse_connection_picker,
    parse_content_search, parse_db_confirm_run, parse_db_export_picker, parse_db_row_detail,
    parse_db_settings_modal, parse_environment_picker, parse_fence_edit, parse_help,
    parse_http_response_detail, parse_insert, parse_normal, parse_quickopen, parse_search,
    parse_tab_picker, parse_tree, parse_tree_prompt, parse_visual, Action, InsertPos, Motion,
    Operator, PastePos, TextObject, WindowCmd,
};
use crate::vim::search;

/// Top-level vim key dispatcher. The app's `handle_key` delegates here.
pub fn dispatch(app: &mut App, key: KeyEvent) {
    // Any keystroke clears the previous transient status message,
    // matching vim's "press a key to dismiss" feel.
    app.clear_status();

    // `Ctrl-C` while a query is running cancels it — runs before
    // mode parsing so it works from anywhere (Normal, Modal, the
    // middle of a chord). Other modes that bind Ctrl-C (modal close
    // etc.) lose to it; the next key after the cancel completes
    // returns control.
    use crossterm::event::{KeyCode, KeyModifiers};
    if app.running_query.is_some()
        && key.modifiers == KeyModifiers::CONTROL
        && key.code == KeyCode::Char('c')
    {
        crate::commands::db::cancel_running_query(app);
        return;
    }

    // `Ctrl+Space` in insert mode — manual trigger for the SQL
    // completion popup. Lets the user browse the full dialect
    // listing right after a space (where the auto-trigger has no
    // prefix to chew on) or force-reopen a popup they just dismissed.
    //
    // Terminal quirk: most terminals report Ctrl+Space as KeyCode::
    // Char(' ') with the CONTROL modifier set, but some emit the
    // legacy NUL byte form (`Char('\0')`). Accept both.
    if app.vim.mode == Mode::Insert
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(' ') | KeyCode::Char('\0'))
    {
        force_open_completion_popup(app);
        return;
    }

    // Completion popup keys are intercepted before mode parsing so a
    // user mid-typing (Mode::Insert) can navigate / accept / dismiss
    // without leaving insert. Any unmatched key falls through to the
    // mode parser; that re-filter happens in the trigger below.
    if app.completion_popup.is_some() {
        if let Some(action) = parse_completion_popup_key(key) {
            apply_action(app, action, false);
            return;
        }
    }

    let action = match app.vim.mode {
        Mode::Normal => parse_normal(&mut app.vim, key),
        Mode::Insert => parse_insert(key),
        Mode::CommandLine => parse_cmdline(key),
        Mode::Search => parse_search(key),
        Mode::QuickOpen => parse_quickopen(key),
        Mode::Tree => parse_tree(key),
        Mode::TreePrompt => parse_tree_prompt(key),
        Mode::Visual | Mode::VisualLine => parse_visual(&mut app.vim, key),
        Mode::DbRowDetail => parse_db_row_detail(&mut app.vim, key),
        Mode::HttpResponseDetail => parse_http_response_detail(&mut app.vim, key),
        Mode::ConnectionPicker => parse_connection_picker(key),
        Mode::DbConfirmRun => parse_db_confirm_run(key),
        Mode::DbExportPicker => parse_db_export_picker(key),
        Mode::FenceEdit => parse_fence_edit(key),
        Mode::DbSettings => parse_db_settings_modal(key),
        Mode::BlockHistory => parse_block_history(key),
        Mode::ContentSearch => parse_content_search(key),
        Mode::EnvironmentPicker => parse_environment_picker(key),
        Mode::Help => parse_help(key),
        Mode::BlockTemplatePicker => parse_block_template_picker(key),
        Mode::TabPicker => parse_tab_picker(key),
    };

    // Snapshot the pre-swap cursor so the post-action "reparse on
    // ExitInsert" hook can tell whether the user was genuinely in
    // prose (closing-fence-completion case) or inside a block via
    // the swap (then `swap.exit` already rebuilds the block, and
    // reparse would corrupt the cursor).
    let cursor_before_swap = app.document().map(|d| d.cursor());
    let was_exit_insert = matches!(action, Action::ExitInsert);

    // When the cursor is parked inside a block's editable body, swap
    // the block segment for a synthetic prose segment so the entire
    // motion/operator engine — built around `Cursor::InProse` — can
    // run unchanged. The reverse swap happens after the action so the
    // file on disk still serializes back to a fence.
    let swap = if action_needs_block_swap(&action) {
        InBlockSwap::maybe_enter(app)
    } else {
        None
    };
    apply_action(app, action, /* recording = */ true);
    if let Some(s) = swap {
        s.exit(app);
    }

    // ExitInsert from genuine prose: the user might have just typed a
    // closing fence, so re-scan the prose for newly complete blocks
    // and splice them in. This *must* run after the swap is fully
    // unwound — running it while the swap is active would splice the
    // synthetic prose (= the original block's raw) back into the doc
    // and teleport the cursor out of the block.
    let was_in_prose = matches!(cursor_before_swap, Some(Cursor::InProse { .. }));
    if was_exit_insert && was_in_prose {
        if let Some(Cursor::InProse { segment_idx, .. }) = app.document().map(|d| d.cursor()) {
            if let Some(doc) = app.document_mut() {
                if doc.reparse_prose_at(segment_idx) {
                    app.set_status(StatusKind::Info, "block parsed");
                }
            }
        }
    }
    // After a typing-relevant action lands in a DB block, refresh
    // the completion popup against the new prefix. `InsertChar` and
    // `DeleteBackward` are the two paths that shift the prefix at
    // the cursor; everything else is a no-op for the popup.
    if matches!(action, Action::InsertChar(_) | Action::DeleteBackward) {
        refresh_completion_popup(app);
    }
}

/// Map a key event to a `CompletionPopup*` action, or `None` if the
/// key isn't one the popup wants to claim. Caller (the dispatcher)
/// only calls this when a popup is open; an unrecognized key falls
/// through to mode parsing and the post-action trigger reopens the
/// popup with the new prefix.
fn parse_completion_popup_key(key: crossterm::event::KeyEvent) -> Option<Action> {
    use crossterm::event::{KeyCode, KeyModifiers};
    let plain = key.modifiers == KeyModifiers::NONE;
    let ctrl = key.modifiers == KeyModifiers::CONTROL;
    match key.code {
        KeyCode::Esc => Some(Action::CompletionDismiss),
        KeyCode::Char('c') if ctrl => Some(Action::CompletionDismiss),
        KeyCode::Tab if plain => Some(Action::CompletionAccept),
        KeyCode::Enter if plain => Some(Action::CompletionAccept),
        KeyCode::Down if plain => Some(Action::CompletionNext),
        KeyCode::Up if plain => Some(Action::CompletionPrev),
        KeyCode::Char('n') if ctrl => Some(Action::CompletionNext),
        KeyCode::Char('p') if ctrl => Some(Action::CompletionPrev),
        _ => None,
    }
}

/// Recompute the completion popup against the cursor's current
/// position. Called after every keystroke that may have shifted the
/// prefix word (insert, backspace). Closes the popup when:
/// - cursor isn't in a DB block body, or
/// - prefix is empty, or
/// - no candidates match (avoids painting an empty popup).
fn refresh_completion_popup(app: &mut App) {
    rebuild_completion_popup(app, /* allow_empty_prefix = */ false);
}

/// `Ctrl+Space` — manual trigger. Opens the popup even when the
/// cursor sits right after a non-word char (where there's no prefix
/// yet) so the user can browse the full dialect listing without
/// having to type the first letter. Inside a partial word it
/// behaves like the auto-trigger and shows the filtered list.
fn force_open_completion_popup(app: &mut App) {
    rebuild_completion_popup(app, /* allow_empty_prefix = */ true);
}

/// Shared body for both the auto trigger and the manual one. The
/// only knob is whether an empty prefix is acceptable — auto closes
/// the popup, manual opens it with the full dialect listing.
fn rebuild_completion_popup(app: &mut App, allow_empty_prefix: bool) {
    let Some(doc) = app.document() else {
        app.completion_popup = None;
        return;
    };
    let Cursor::InBlock {
        segment_idx,
        offset: raw_offset,
    } = doc.cursor()
    else {
        app.completion_popup = None;
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        app.completion_popup = None;
        return;
    };
    let Segment::Block(block) = seg else {
        app.completion_popup = None;
        return;
    };
    if !block.is_db() {
        app.completion_popup = None;
        return;
    }
    // The completion engine speaks body `(line, col)` — convert the
    // cursor's raw-rope offset to body coords and bail out on
    // header / closer rows (no completion in fence text).
    let (line, offset) = match crate::buffer::block::raw_section_at(&block.raw, raw_offset) {
        crate::buffer::block::RawSection::Body { line, col } => (line, col),
        _ => {
            app.completion_popup = None;
            return;
        }
    };
    let body = match block.params.get("query").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => {
            app.completion_popup = None;
            return;
        }
    };
    // Refs win over SQL completion: when the cursor sits inside an
    // open `{{...}}` ref, the popup switches to alias / env-var /
    // ref-path mode entirely. The prefix the popup tracks is what's
    // typed since the last `{{` or `.` — same accept semantics as
    // the SQL path (backspace prefix, splice label).
    if let Some(ref_detect) = crate::sql_completion::detect_ref_context(&body, line, offset) {
        // Need env vars to populate top-level ref candidates.
        let env_vars: std::collections::HashMap<String, String> =
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(load_active_env_vars(app.pool_manager.app_pool()))
            })
            .unwrap_or_default();
        let segments_snapshot: Vec<crate::buffer::Segment> = app
            .document()
            .map(|d| d.segments().to_vec())
            .unwrap_or_default();
        let items = crate::sql_completion::complete_refs(
            &ref_detect,
            &segments_snapshot,
            segment_idx,
            &env_vars,
        );
        if items.is_empty() {
            app.completion_popup = None;
            return;
        }
        let prior_label = app
            .completion_popup
            .as_ref()
            .and_then(|p| p.items.get(p.selected))
            .map(|i| i.label.clone());
        let selected = prior_label
            .and_then(|lbl| items.iter().position(|i| i.label == lbl))
            .unwrap_or(0);
        app.completion_popup = Some(crate::app::CompletionPopupState {
            segment_idx,
            items,
            selected,
            anchor_line: line,
            anchor_offset: ref_detect.anchor_offset,
            prefix: ref_detect.prefix,
        });
        return;
    }

    // Detect the prefix word at the cursor. When we got there via a
    // manual trigger and the cursor isn't on a word char, fall back
    // to "no prefix, anchor at cursor" so the popup still opens.
    let (anchor_offset, prefix) = match crate::sql_completion::prefix_at_cursor(&body, line, offset)
    {
        Some(p) => p,
        None if allow_empty_prefix => (offset, String::new()),
        None => {
            app.completion_popup = None;
            return;
        }
    };
    let dialect = crate::sql_completion::Dialect::from_block(block);
    let context = crate::sql_completion::detect_context(&body, line, anchor_offset);
    // The fence may carry either a UUID (canonical, written by the
    // picker) or a human slug (legacy / hand-typed). The schema
    // cache is always keyed by UUID, so resolve here via the
    // `connection_names` map (id → name) — a reverse scan finds the
    // id when the fence has a slug.
    let conn_raw = block
        .params
        .get("connection")
        .or_else(|| block.params.get("connection_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let conn_id = conn_raw
        .as_deref()
        .map(|raw| resolve_connection_id_sync(raw, &app.connection_names));
    // Borrow of `block` ends here — the snapshot below clones the
    // schema tables so we can mutate `app` after.
    let schema_tables: Option<Vec<crate::schema::SchemaTable>> = conn_id
        .as_deref()
        .and_then(|id| app.schema_cache.get(id))
        .map(|e| e.tables.clone());
    // First-time popup on this connection? Kick off the background
    // introspection now. The fetch is idempotent + dedup'd, so
    // calling it again on every keystroke is free; the result lands
    // via `AppEvent::SchemaLoaded` and the next popup refresh sees
    // it cached.
    if let Some(id) = conn_id.as_deref() {
        if schema_tables.is_none() {
            app.ensure_schema_loaded(id);
        }
    }
    let items =
        crate::sql_completion::complete(dialect, &prefix, context, schema_tables.as_deref());
    if items.is_empty() {
        app.completion_popup = None;
        return;
    }
    // Preserve the previous selection's label so re-filtering on a
    // longer prefix doesn't reset the highlight to the top — useful
    // when the user is typing toward a known target.
    let prior_label = app
        .completion_popup
        .as_ref()
        .and_then(|p| p.items.get(p.selected))
        .map(|i| i.label.clone());
    let selected = prior_label
        .and_then(|lbl| items.iter().position(|i| i.label == lbl))
        .unwrap_or(0);
    app.completion_popup = Some(crate::app::CompletionPopupState {
        segment_idx,
        items,
        selected,
        anchor_line: line,
        anchor_offset,
        prefix,
    });
}

// All DB-domain helpers (`build_db_executor_params`,
// `compute_db_cache_hash`, the SQL classifiers, ref/bind resolvers,
// `summarize_db_response`, `load_active_env_vars`, `resolve_connection_id`,
// `resolve_connection_id_sync`, `save_db_cache_async`,
// `db_summary_from_value`) moved to `crate::commands::db`. The imports
// at the top bring them into local scope so the existing
// `apply_run_block` / `handle_db_block_result` / `rebuild_completion_popup`
// call sites keep working until those flows migrate too (next session).

/// `y` (or `Enter`) inside the run-confirm modal — close the
/// modal, drop back to normal mode, then re-run the original
/// segment with the unscoped-destructive gate bypassed. The
/// segment_idx comes from the modal state because the cursor may
/// have moved while the modal was up.
fn apply_confirm_db_run(app: &mut App) {
    let Some(state) = app.db_confirm_run.take() else {
        app.vim.enter_normal();
        return;
    };
    app.vim.enter_normal();
    crate::commands::db::run_db_block_inner(
        app,
        state.segment_idx,
        /* force_unscoped = */ true,
        None,
        /* as_explain = */ false,
    );
}

/// `n` / `Esc` / `Ctrl-C` — close the modal without running.
fn apply_cancel_db_run(app: &mut App) {
    if app.db_confirm_run.take().is_some() {
        app.set_status(StatusKind::Info, "run cancelled");
    }
    app.vim.enter_normal();
}

fn apply_completion_next(app: &mut App) {
    let Some(state) = app.completion_popup.as_mut() else {
        return;
    };
    if state.items.is_empty() {
        return;
    }
    state.selected = (state.selected + 1) % state.items.len();
}

fn apply_completion_prev(app: &mut App) {
    let Some(state) = app.completion_popup.as_mut() else {
        return;
    };
    if state.items.is_empty() {
        return;
    }
    state.selected = if state.selected == 0 {
        state.items.len() - 1
    } else {
        state.selected - 1
    };
}

fn apply_completion_dismiss(app: &mut App) {
    app.completion_popup = None;
}

/// Splice the selected item's label in place of the prefix word at
/// the cursor. Implementation: backspace `prefix.len()` characters
/// (which clears the partial word in the body), then insert each
/// char of the label. Cursor lands at the end of the inserted text.
fn apply_completion_accept(app: &mut App) {
    let Some(state) = app.completion_popup.take() else {
        return;
    };
    let Some(item) = state.items.get(state.selected).cloned() else {
        return;
    };
    let prefix_chars = state.prefix.chars().count();
    let Some(doc) = app.tabs.active_document_mut() else {
        return;
    };
    doc.snapshot();
    for _ in 0..prefix_chars {
        doc.delete_char_before_cursor();
    }
    for ch in item.label.chars() {
        doc.insert_char_at_cursor(ch);
    }
}

/// Decide whether an action should run with the block-as-prose swap
/// active. Buffer-touching actions (motions, operators, edits, paste,
/// undo) need the swap so they see the SQL as a normal rope; mode
/// transitions and tab/window plumbing don't care.
///
/// Vertical motions (`j`/`k`) are deliberately excluded: they need to
/// see `Cursor::InBlock` so they can hop into the result table at the
/// SQL boundary. Inside the SQL, the same branches in `motions::apply_*`
/// already handle line-by-line navigation — no swap required.
fn action_needs_block_swap(action: &Action) -> bool {
    if let Action::Motion(motion, _) = action {
        if matches!(motion, Motion::Down | Motion::Up) {
            return false;
        }
    }
    matches!(
        action,
        Action::Motion(..)
            | Action::OperatorMotion(..)
            | Action::OperatorLinewise(..)
            | Action::OperatorTextObject(..)
            | Action::VisualOperator(_)
            | Action::VisualSwap
            | Action::Paste(..)
            | Action::Undo
            | Action::Redo
            | Action::RepeatChange(_)
            | Action::InsertChar(_)
            | Action::InsertNewline
            | Action::DeleteBackward
            | Action::DeleteForward
            | Action::EnterInsert(_)
            | Action::ExitInsert
            | Action::EnterVisual
            | Action::EnterVisualLine
            | Action::SearchExecute
            | Action::SearchRepeat { .. }
    )
}

/// While alive, the active document's `segment_idx`-th block is
/// pretending to be a prose run with the SQL as its content.
/// `exit` puts the block back together with whatever the action ended
/// up writing into the prose, and converts the cursor back to
/// `InBlock` if it's still pointing into the swapped slot.
struct InBlockSwap {
    segment_idx: usize,
    original_block: BlockNode,
    original_query: String,
}

impl InBlockSwap {
    fn maybe_enter(app: &mut App) -> Option<Self> {
        let cursor = app.document()?.cursor();
        let Cursor::InBlock {
            segment_idx,
            offset: raw_offset,
        } = cursor
        else {
            return None;
        };
        let doc = app.tabs.active_document_mut()?;
        let block = match doc.segments().get(segment_idx)? {
            Segment::Block(b) => b.clone(),
            _ => return None,
        };
        // Promote the entire raw rope (header + body + closer) to a
        // Prose segment so the operator engine treats every row as
        // editable text. After the operator runs, `exit` parses the
        // mutated rope back into a block (or keeps last-good fields
        // when the fence is broken mid-edit). This is what unlocks
        // d / y / c on the fence header — `alias=foo` deletions, etc.
        let raw_text = block.raw.to_string();
        doc.replace_segment(segment_idx, Segment::Prose(Rope::from_str(&raw_text)));
        let abs = raw_offset.min(raw_text.chars().count());
        doc.set_cursor(Cursor::InProse {
            segment_idx,
            offset: abs,
        });
        Some(Self {
            segment_idx,
            original_block: block,
            original_query: raw_text,
        })
    }

    fn exit(self, app: &mut App) {
        let Some(doc) = app.tabs.active_document_mut() else {
            return;
        };
        let new_raw = match doc.segments().get(self.segment_idx) {
            Some(Segment::Prose(rope)) => rope.to_string(),
            _ => self.original_query.clone(),
        };
        let cursor_after = doc.cursor();
        // Rebuild the block from the mutated raw rope. We try to
        // reparse — clean parses update derived fields; broken
        // parses keep last-good fields so the user can keep editing.
        // Either way, id / state / cached_result survive (Phase 3
        // contract).
        let mut new_block = self.original_block.clone();
        new_block.raw = Rope::from_str(&new_raw);
        new_block.reparse_from_raw();
        doc.replace_segment(self.segment_idx, Segment::Block(new_block));
        // If the cursor still points at the swapped segment, convert
        // back to InBlock at the equivalent raw offset. The prose-mode
        // operator may have moved it past the new rope's length —
        // clamp.
        if let Cursor::InProse {
            segment_idx,
            offset: abs,
        } = cursor_after
        {
            if segment_idx == self.segment_idx {
                let new_len = new_raw.chars().count();
                let clamped = abs.min(new_len);
                doc.set_cursor(Cursor::InBlock {
                    segment_idx,
                    offset: clamped,
                });
            }
        }
    }
}

/// Run an action against the app. `recording` toggles whether the
/// resulting change updates `last_change` — `.` replay sets it to
/// `false` so a `.` after a `.` doesn't trample its own record.
fn apply_action(app: &mut App, action: Action, recording: bool) {
    match action {
        Action::Noop => {}
        Action::Quit => {
            app.should_quit = true;
        }
        Action::Motion(m, count) => {
            // When the row-detail modal is open `app.document_mut()`
            // redirects to its body doc, so the motion engine drives
            // the modal's cursor automatically. Skip the editor-only
            // book-keeping (paginated-result prefetch, viewport
            // refresh) when the modal owns the focus.
            let in_modal = app.vim.mode == Mode::DbRowDetail;
            if !in_modal && matches!(m, Motion::Down) {
                maybe_prefetch_db_more_rows(app);
            }
            let viewport = app.viewport_height();
            if let Some(doc) = app.document_mut() {
                motions::apply(m, doc, count, viewport);
            }
            if m.is_find() {
                app.vim.last_find = Some(m);
            }
            if !in_modal {
                app.refresh_viewport_for_cursor();
            }
        }
        Action::EnterInsert(pos) => {
            if let Some(doc) = app.document_mut() {
                doc.snapshot();
                position_for_insert(doc, pos);
            }
            app.vim.enter_insert();
            app.vim.insert_session.start_plain(pos);
            app.refresh_viewport_for_cursor();
        }
        Action::EnterVisual => {
            if let Some(doc) = app.document() {
                let cur = doc.cursor();
                app.vim.enter_visual(cur);
            }
        }
        Action::EnterVisualLine => {
            if let Some(doc) = app.document() {
                let cur = doc.cursor();
                app.vim.enter_visual_line(cur);
            }
        }
        Action::ExitVisual => {
            return_from_visual(app);
        }
        Action::VisualSwap => {
            if let (Some(anchor), Some(doc)) = (app.vim.visual_anchor, app.document_mut()) {
                let cur = doc.cursor();
                doc.set_cursor(anchor);
                app.vim.visual_anchor = Some(cur);
                app.refresh_viewport_for_cursor();
            }
        }
        Action::VisualOperator(op) => apply_visual_operator(app, op, recording),
        Action::VisualSelectTextObject(textobj) => {
            apply_visual_select_textobject(app, textobj);
        }
        Action::RunBlock => crate::commands::db::apply_run_block(app),
        Action::OpenDbRowDetail => apply_open_result_detail(app),
        Action::CloseDbRowDetail => apply_close_db_row_detail(app),
        Action::CopyDbRowDetailJson => apply_copy_db_row_detail_json(app),
        Action::CloseHttpResponseDetail => apply_close_http_response_detail(app),
        Action::CopyHttpResponseBody => apply_copy_http_response_body(app),
        Action::OpenConnectionPicker => {
            if let Err(msg) = open_connection_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::ExplainBlock => crate::commands::db::run_explain(app),
        Action::CopyAsCurl => crate::commands::http::copy_as_curl(app),
        Action::CycleDisplayMode => crate::commands::db::cycle_display_mode(app),
        Action::CloseConnectionPicker => apply_close_connection_picker(app),
        Action::MoveConnectionPickerCursor(delta) => {
            apply_move_connection_picker_cursor(app, delta)
        }
        Action::ConfirmConnectionPicker => apply_confirm_connection_picker(app),
        Action::DeleteConnectionInPicker => apply_delete_connection_in_picker(app),
        Action::OpenDbExportPicker => {
            if let Err(msg) = crate::commands::db::open_export_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseDbExportPicker => crate::commands::db::close_export_picker(app),
        Action::MoveDbExportPickerCursor(delta) => {
            crate::commands::db::move_export_picker_cursor(app, delta)
        }
        Action::ConfirmDbExportPicker => crate::commands::db::confirm_export_picker(app),
        Action::OpenDbSettingsModal => {
            if let Err(msg) = crate::commands::db::open_db_settings_modal(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseDbSettingsModal => crate::commands::db::close_db_settings_modal(app),
        Action::ConfirmDbSettingsModal => crate::commands::db::confirm_db_settings_modal(app),
        Action::DbSettingsFocusNext => crate::commands::db::db_settings_focus_step(app, 1),
        Action::DbSettingsFocusPrev => crate::commands::db::db_settings_focus_step(app, -1),
        Action::DbSettingsChar(c) => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().insert_char(c);
            }
        }
        Action::DbSettingsBackspace => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().delete_before();
            }
        }
        Action::DbSettingsDelete => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().delete_after();
            }
        }
        Action::DbSettingsCursorLeft => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().move_left();
            }
        }
        Action::DbSettingsCursorRight => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().move_right();
            }
        }
        Action::DbSettingsCursorHome => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().move_home();
            }
        }
        Action::DbSettingsCursorEnd => {
            if let Some(s) = app.db_settings.as_mut() {
                s.focused_input_mut().move_end();
            }
        }
        Action::OpenBlockHistory => {
            if let Err(msg) = crate::commands::http::open_block_history(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseBlockHistory => crate::commands::http::close_block_history(app),
        Action::MoveBlockHistoryCursor(delta) => {
            crate::commands::http::move_block_history_cursor(app, delta)
        }
        Action::OpenContentSearch => {
            if let Err(msg) = crate::commands::search::open_content_search(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseContentSearch => crate::commands::search::close_content_search(app),
        Action::ConfirmContentSearch => crate::commands::search::confirm_content_search(app),
        Action::MoveContentSearchCursor(delta) => {
            crate::commands::search::move_content_search_cursor(app, delta)
        }
        Action::ContentSearchChar(c) => crate::commands::search::content_search_char(app, c),
        Action::ContentSearchBackspace => crate::commands::search::content_search_backspace(app),
        Action::ContentSearchDelete => crate::commands::search::content_search_delete(app),
        Action::ContentSearchCursorLeft => {
            if let Some(s) = app.content_search.as_mut() {
                s.query.move_left();
            }
        }
        Action::ContentSearchCursorRight => {
            if let Some(s) = app.content_search.as_mut() {
                s.query.move_right();
            }
        }
        Action::ContentSearchCursorHome => {
            if let Some(s) = app.content_search.as_mut() {
                s.query.move_home();
            }
        }
        Action::ContentSearchCursorEnd => {
            if let Some(s) = app.content_search.as_mut() {
                s.query.move_end();
            }
        }
        Action::OpenEnvironmentPicker => {
            if let Err(msg) = open_environment_picker(app) {
                app.set_status(StatusKind::Error, msg);
            }
        }
        Action::CloseEnvironmentPicker => apply_close_environment_picker(app),
        Action::MoveEnvironmentPickerCursor(delta) => {
            apply_move_environment_picker_cursor(app, delta)
        }
        Action::ConfirmEnvironmentPicker => apply_confirm_environment_picker(app),
        Action::OpenHelp => {
            app.help_visible = true;
            app.vim.mode = Mode::Help;
            app.vim.reset_pending();
        }
        Action::CloseHelp => {
            app.help_visible = false;
            app.vim.enter_normal();
        }
        Action::JumpNextBlock => apply_jump_block(app, JumpDir::Next),
        Action::JumpPrevBlock => apply_jump_block(app, JumpDir::Prev),
        Action::RerunLastBlock => apply_rerun_last_block(app),
        Action::WriteFile => {
            // `<C-s>` — same code path as `:w`, status reporting and
            // all. Routed through `ex::execute` (rather than the
            // string-based `ex::run`) to skip a redundant parse.
            match ex::execute(app, ex::ExCmd::Write) {
                ex::ExResult::Ok(msg) => app.set_status(StatusKind::Info, msg),
                ex::ExResult::Err(msg) => app.set_status(StatusKind::Error, msg),
                _ => {}
            }
        }
        Action::WriteAll => apply_write_all(app),
        Action::ReselectVisual => apply_reselect_visual(app),
        Action::ScrollCursorTo(pos) => apply_scroll_cursor_to(app, pos),
        Action::OpenBlockTemplatePicker => {
            app.block_template_picker = Some(crate::app::BlockTemplatePickerState::new());
            app.vim.mode = Mode::BlockTemplatePicker;
            app.vim.reset_pending();
        }
        Action::CloseBlockTemplatePicker => {
            app.block_template_picker = None;
            app.vim.enter_normal();
        }
        Action::MoveBlockTemplatePickerCursor(delta) => {
            apply_move_block_template_picker_cursor(app, delta)
        }
        Action::ConfirmBlockTemplatePicker => apply_confirm_block_template_picker(app),
        Action::OpenTabPicker => apply_open_tab_picker(app),
        Action::CloseTabPicker => {
            app.tab_picker = None;
            app.vim.enter_normal();
        }
        Action::MoveTabPickerCursor(delta) => apply_move_tab_picker_cursor(app, delta),
        Action::ConfirmTabPicker => apply_confirm_tab_picker(app),
        Action::CompletionNext => apply_completion_next(app),
        Action::CompletionPrev => apply_completion_prev(app),
        Action::CompletionAccept => apply_completion_accept(app),
        Action::CompletionDismiss => apply_completion_dismiss(app),
        Action::ConfirmDbRun => apply_confirm_db_run(app),
        Action::CancelDbRun => apply_cancel_db_run(app),
        Action::ExitInsert => {
            // Recoil the cursor one column (vim's `<Esc>` semantics)
            // and flip the mode. The "did the user just finish a
            // fence?" reparse is handled at the dispatch top level —
            // running it here would fire while the in-block swap is
            // still pretending the block is a Prose segment, which
            // splices the synthetic prose back into the doc and
            // jumps the cursor out of the block.
            if let Some(doc) = app.document_mut() {
                recoil_after_exit(doc);
            }
            app.vim.enter_normal();
            if recording {
                if let Some(record) = app.vim.insert_session.finish() {
                    app.vim.last_change = Some(record);
                }
            } else {
                // Discard the in-flight session without overwriting the
                // existing `last_change` — replay path.
                let _ = app.vim.insert_session.finish();
            }
        }
        Action::InsertChar(c) => {
            if let Some(doc) = app.document_mut() {
                doc.insert_char_at_cursor(c);
            }
            app.vim.insert_session.push_char(c);
        }
        Action::InsertNewline => {
            if let Some(doc) = app.document_mut() {
                doc.insert_newline_at_cursor();
            }
            app.vim.insert_session.push_newline();
            app.refresh_viewport_for_cursor();
        }
        Action::DeleteBackward => {
            if let Some(doc) = app.document_mut() {
                doc.delete_char_before_cursor();
            }
            app.vim.insert_session.pop_char();
        }
        Action::DeleteForward => {
            if let Some(doc) = app.document_mut() {
                doc.delete_char_at_cursor();
            }
        }
        Action::EnterCmdline => {
            app.vim.enter_cmdline();
        }
        Action::CmdlineChar(c) => {
            app.vim.cmdline_push(c);
        }
        Action::CmdlineBackspace => {
            // Empty buffer + backspace exits the prompt — same as `<Esc>`.
            if !app.vim.cmdline_pop() {
                app.vim.enter_normal();
            }
        }
        Action::CmdlineDelete => {
            app.vim.cmdline.delete_after();
        }
        Action::CmdlineCursorLeft => app.vim.cmdline.move_left(),
        Action::CmdlineCursorRight => app.vim.cmdline.move_right(),
        Action::CmdlineCursorHome => app.vim.cmdline.move_home(),
        Action::CmdlineCursorEnd => app.vim.cmdline.move_end(),
        Action::CmdlineCancel => {
            app.vim.enter_normal();
        }
        Action::CmdlineExecute => {
            let buf = app.vim.cmdline.take();
            app.vim.enter_normal();
            match ex::run(app, &buf) {
                ExResult::Ok(msg) => app.set_status(StatusKind::Info, msg),
                ExResult::Err(msg) => app.set_status(StatusKind::Error, msg),
                ExResult::Unknown(s) => app.set_status(
                    StatusKind::Error,
                    format!("E492: not an editor command: {s}"),
                ),
                ExResult::Empty | ExResult::Quit => {}
            }
        }
        Action::OperatorMotion(op, motion, count) => {
            apply_op_motion(app, op, motion, count, recording);
        }
        Action::OperatorLinewise(op, count) => {
            apply_op_linewise(app, op, count, recording);
        }
        Action::OperatorTextObject(op, textobj, count) => {
            apply_op_textobject(app, op, textobj, count, recording);
        }
        Action::Paste(pos, count) => {
            apply_paste(app, pos, count, recording);
        }
        Action::Undo => {
            if let Some(doc) = app.document_mut() {
                if !doc.undo() {
                    app.set_status(StatusKind::Info, "already at oldest change");
                }
            }
            app.refresh_viewport_for_cursor();
        }
        Action::Redo => {
            if let Some(doc) = app.document_mut() {
                if !doc.redo() {
                    app.set_status(StatusKind::Info, "already at newest change");
                }
            }
            app.refresh_viewport_for_cursor();
        }
        Action::RepeatChange(count) => {
            replay_last_change(app, count.max(1));
        }
        Action::EnterSearch(forward) => {
            app.vim.enter_search(forward);
        }
        Action::SearchChar(c) => {
            app.vim.search_push(c);
        }
        Action::SearchBackspace => {
            if !app.vim.search_pop() {
                app.vim.enter_normal();
            }
        }
        Action::SearchDelete => {
            app.vim.search_buf.delete_after();
        }
        Action::SearchCursorLeft => app.vim.search_buf.move_left(),
        Action::SearchCursorRight => app.vim.search_buf.move_right(),
        Action::SearchCursorHome => app.vim.search_buf.move_home(),
        Action::SearchCursorEnd => app.vim.search_buf.move_end(),
        Action::SearchCancel => {
            app.vim.enter_normal();
        }
        Action::SearchExecute => {
            let pattern = app.vim.search_buf.take();
            let forward = app.vim.search_forward;
            app.vim.enter_normal();
            execute_search(app, &pattern, forward, /* save = */ true);
        }
        Action::SearchRepeat { reverse } => {
            let Some(pattern) = app.vim.last_search.clone() else {
                app.set_status(StatusKind::Error, "no previous search");
                return;
            };
            let forward = if reverse {
                !app.vim.last_search_forward
            } else {
                app.vim.last_search_forward
            };
            execute_search(app, &pattern, forward, /* save = */ false);
        }
        Action::EnterQuickOpen => {
            let files = list_vault_md_files(&app.vault_path.to_string_lossy());
            app.vim.enter_quickopen(files);
        }
        Action::QuickOpenChar(c) => {
            app.vim.quickopen.push_char(c);
        }
        Action::QuickOpenBackspace => {
            // Empty buffer + backspace closes the modal — same as `<Esc>`.
            if app.vim.quickopen.query.is_empty() {
                app.vim.enter_normal();
            } else {
                app.vim.quickopen.pop_char();
            }
        }
        Action::QuickOpenDelete => app.vim.quickopen.delete_after(),
        Action::QuickOpenCursorLeft => app.vim.quickopen.move_left(),
        Action::QuickOpenCursorRight => app.vim.quickopen.move_right(),
        Action::QuickOpenCursorHome => app.vim.quickopen.move_home(),
        Action::QuickOpenCursorEnd => app.vim.quickopen.move_end(),
        Action::QuickOpenSelectNext => {
            app.vim.quickopen.select_next();
        }
        Action::QuickOpenSelectPrev => {
            app.vim.quickopen.select_prev();
        }
        Action::QuickOpenCancel => {
            app.vim.enter_normal();
        }
        Action::QuickOpenExecute => {
            // Quick Open is the picker — always opens in a new tab (or
            // switches to the existing tab if already open). The vim
            // ex command `:e <path>` is the explicit "replace current"
            // path for users who want that.
            let chosen = app.vim.quickopen.chosen_path();
            app.vim.enter_normal();
            if let Some(path) = chosen {
                match app.open_in_new_tab(path) {
                    Ok(msg) => app.set_status(StatusKind::Info, msg),
                    Err(msg) => app.set_status(StatusKind::Error, msg),
                }
            }
        }
        Action::Window(cmd) => apply_window_cmd(app, cmd),
        Action::TreeToggle => {
            if app.tree.visible {
                app.tree.visible = false;
                if app.vim.mode == Mode::Tree {
                    app.vim.enter_normal();
                }
            } else {
                app.tree.visible = true;
                app.tree.refresh(&app.vault_path);
                app.vim.mode = Mode::Tree;
            }
        }
        Action::FocusSwap => {
            if !app.tree.visible {
                return;
            }
            if app.vim.mode == Mode::Tree {
                app.vim.enter_normal();
            } else if app.vim.mode == Mode::Normal {
                app.vim.mode = Mode::Tree;
            }
        }
        Action::TreeSelectNext => app.tree.select_next(),
        Action::TreeSelectPrev => app.tree.select_prev(),
        Action::TreeSelectFirst => app.tree.select_first(),
        Action::TreeSelectLast => app.tree.select_last(),
        Action::TreeRefresh => {
            let vault = app.vault_path.clone();
            app.tree.refresh(&vault);
        }
        Action::TreeCollapse => {
            if app.tree.collapse_parent() {
                let vault = app.vault_path.clone();
                app.tree.refresh(&vault);
            }
        }
        Action::TreeActivate => {
            let Some(node) = app.tree.current().cloned() else {
                return;
            };
            if node.is_dir {
                if app.tree.toggle_expand() {
                    let vault = app.vault_path.clone();
                    app.tree.refresh(&vault);
                }
            } else {
                // Tree-driven open mirrors the modal: every Enter opens
                // a new tab (or switches to an existing one). Use `:e
                // <path>` if you want the vim-style replace behavior.
                let path = std::path::PathBuf::from(&node.path);
                match app.open_in_new_tab(path) {
                    Ok(msg) => {
                        app.set_status(StatusKind::Info, msg);
                        // Hand focus back to the editor on successful open —
                        // matches how netrw exits the tree after Enter.
                        app.vim.enter_normal();
                    }
                    Err(msg) => app.set_status(StatusKind::Error, msg),
                }
            }
        }
        Action::TabNext => {
            // When the cursor sits on a result row, `gt` cycles
            // the result-panel tab (Result → Messages → Plan →
            // Stats → Result) instead of switching editor tabs —
            // the editor-tab swap wouldn't be useful from inside a
            // table, and the result-panel needs *some* keyboard
            // affordance.
            if matches!(
                app.document().map(|d| d.cursor()),
                Some(Cursor::InBlockResult { .. })
            ) {
                app.db_result_tab = app.db_result_tab.next();
            } else {
                app.next_tab();
                app.refresh_viewport_for_cursor();
            }
        }
        Action::TabPrev => {
            if matches!(
                app.document().map(|d| d.cursor()),
                Some(Cursor::InBlockResult { .. })
            ) {
                app.db_result_tab = app.db_result_tab.prev();
            } else {
                app.prev_tab();
                app.refresh_viewport_for_cursor();
            }
        }
        Action::TabGoto(n) => {
            app.goto_tab(n);
            app.refresh_viewport_for_cursor();
        }
        Action::TreeCreate => {
            // Open the in-tree prompt anchored to the selected folder
            // (or the parent of the selected file). The user types
            // either a filename (e.g. `notes.md`) or a name with
            // trailing `/` (e.g. `subdir/`) to make a folder.
            let dir = match app.tree.current() {
                Some(node) if node.is_dir => node.path.clone(),
                Some(node) => match std::path::Path::new(&node.path).parent() {
                    Some(p) if !p.as_os_str().is_empty() => p.display().to_string(),
                    _ => String::new(),
                },
                None => String::new(),
            };
            app.tree.prompt = Some(TreePrompt::new(
                TreePromptKind::Create { dir },
                String::new(),
            ));
            app.vim.mode = Mode::TreePrompt;
        }
        Action::TreeRename => {
            let Some(node) = app.tree.current() else {
                return;
            };
            // Pre-fill the buffer with the source path so the user
            // edits the destination in place. Allowed for files and
            // folders alike — `rename_path` handles both.
            let path = node.path.clone();
            app.tree.prompt = Some(TreePrompt::new(
                TreePromptKind::Rename { from: path.clone() },
                path,
            ));
            app.vim.mode = Mode::TreePrompt;
        }
        Action::TreeDelete => {
            let Some(node) = app.tree.current() else {
                return;
            };
            app.tree.prompt = Some(TreePrompt::new(
                TreePromptKind::Delete {
                    target: node.path.clone(),
                },
                String::new(),
            ));
            app.vim.mode = Mode::TreePrompt;
        }
        Action::TreePromptChar(c) => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.insert_char(c);
            }
        }
        Action::TreePromptBackspace => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                if !prompt.input.delete_before() {
                    // Empty buffer + backspace acts like cancel.
                    app.tree.prompt = None;
                    app.vim.mode = Mode::Tree;
                }
            } else {
                app.vim.mode = Mode::Tree;
            }
        }
        Action::TreePromptDelete => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.delete_after();
            }
        }
        Action::TreePromptCursorLeft => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_left();
            }
        }
        Action::TreePromptCursorRight => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_right();
            }
        }
        Action::TreePromptCursorHome => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_home();
            }
        }
        Action::TreePromptCursorEnd => {
            if let Some(prompt) = app.tree.prompt.as_mut() {
                prompt.input.move_end();
            }
        }
        Action::TreePromptCancel => {
            app.tree.prompt = None;
            app.vim.mode = Mode::Tree;
        }
        Action::TreePromptExecute => {
            let Some(prompt) = app.tree.prompt.take() else {
                app.vim.mode = Mode::Tree;
                return;
            };
            app.vim.mode = Mode::Tree;
            run_tree_prompt(app, prompt);
        }
        Action::OpenFenceEditAlias => crate::commands::db::open_fence_edit_alias(app),
        Action::FenceEditChar(c) => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.insert_char(c);
            }
        }
        Action::FenceEditBackspace => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                // Backspace on an empty buffer cancels — same affordance
                // as the tree prompt; users can hold backspace to bail.
                if !prompt.input.delete_before() {
                    app.fence_edit = None;
                    app.vim.enter_normal();
                }
            } else {
                app.vim.enter_normal();
            }
        }
        Action::FenceEditDelete => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.delete_after();
            }
        }
        Action::FenceEditCursorLeft => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.move_left();
            }
        }
        Action::FenceEditCursorRight => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.move_right();
            }
        }
        Action::FenceEditCursorHome => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.move_home();
            }
        }
        Action::FenceEditCursorEnd => {
            if let Some(prompt) = app.fence_edit.as_mut() {
                prompt.input.move_end();
            }
        }
        Action::FenceEditCancel => {
            app.fence_edit = None;
            app.vim.enter_normal();
            app.set_status(StatusKind::Info, "edit cancelled");
        }
        Action::FenceEditConfirm => crate::commands::db::confirm_fence_edit(app),
    }
}

/// Execute the pending tree prompt against `app`. Refreshes the tree on
/// success so the sidebar shows the new state without a manual `R`.
fn run_tree_prompt(app: &mut App, prompt: TreePrompt) {
    let buffer = prompt.input.buffer;
    let outcome = match prompt.kind {
        TreePromptKind::Create { dir } => {
            let raw = buffer.trim();
            if raw.is_empty() {
                Err("create: name required".to_string())
            } else {
                // Trailing slash → folder; otherwise file.
                let is_folder = raw.ends_with('/') || raw.ends_with(std::path::MAIN_SEPARATOR);
                let name = raw.trim_end_matches(['/', std::path::MAIN_SEPARATOR]);
                if name.is_empty() {
                    Err("create: name required".into())
                } else {
                    let path = if dir.is_empty() {
                        std::path::PathBuf::from(name)
                    } else {
                        std::path::Path::new(&dir).join(name)
                    };
                    if is_folder {
                        app.create_folder(path)
                    } else {
                        app.create_document(path, false)
                    }
                }
            }
        }
        TreePromptKind::Rename { from } => {
            let dst = buffer.trim();
            if dst.is_empty() || dst == from {
                Err("rename: destination unchanged".to_string())
            } else {
                app.rename_path(
                    Some(std::path::PathBuf::from(&from)),
                    std::path::PathBuf::from(dst),
                )
            }
        }
        TreePromptKind::Delete { target } => {
            let answer = buffer.trim().to_lowercase();
            if answer == "y" || answer == "yes" {
                app.delete_path(Some(std::path::PathBuf::from(&target)), true)
            } else {
                Err("delete: cancelled".to_string())
            }
        }
    };
    match outcome {
        Ok(msg) => {
            let vault = app.vault_path.clone();
            app.tree.refresh(&vault);
            app.set_status(StatusKind::Info, msg);
        }
        Err(msg) => app.set_status(StatusKind::Error, msg),
    }
}

/// Recursively list every `.md` file in the vault, returning paths
/// relative to the vault root. Hidden directories and the usual
/// build-artifact dirs are filtered by `httui_core::fs::list_workspace`,
/// so we just walk what it gives us.
fn list_vault_md_files(vault: &str) -> Vec<String> {
    let Ok(entries) = httui_core::fs::list_workspace(vault) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_md(&entries, &mut out);
    // Stable order: alphabetic by full path. The fuzzy filter sort takes
    // over once the user types something.
    out.sort();
    out
}

fn collect_md(entries: &[httui_core::fs::FileEntry], out: &mut Vec<String>) {
    for e in entries {
        if e.is_dir {
            if let Some(children) = e.children.as_deref() {
                collect_md(children, out);
            }
        } else if e.name.ends_with(".md") {
            out.push(e.path.clone());
        }
    }
}

fn execute_search(app: &mut App, pattern: &str, forward: bool, save: bool) {
    if pattern.is_empty() {
        return;
    }
    // Any new search re-arms the highlight that `:noh` may have hidden.
    app.vim.search_highlight = true;
    let result = app
        .document()
        .and_then(|doc| search::search(doc, pattern, forward));
    match result {
        Some(cursor) => {
            if let Some(doc) = app.document_mut() {
                doc.set_cursor(cursor);
            }
            if save {
                app.vim.last_search = Some(pattern.to_string());
                app.vim.last_search_forward = forward;
            }
            app.refresh_viewport_for_cursor();
        }
        None => {
            // Still save the pattern — `n` after a missed search should
            // try the same query again rather than re-prompting.
            if save {
                app.vim.last_search = Some(pattern.to_string());
                app.vim.last_search_forward = forward;
            }
            app.set_status(
                StatusKind::Error,
                format!("E486: Pattern not found: {pattern}"),
            );
        }
    }
}

// ───────────── operator wrappers (snapshot + record) ─────────────

fn apply_op_motion(app: &mut App, op: Operator, motion: Motion, count: usize, recording: bool) {
    let viewport = app.viewport_height();
    let mut outcome = operator::OpOutcome::default();
    // Borrow the unnamed register out so we can use `app.document_mut()`
    // (which holds a mut borrow on the whole app) at the same time.
    // Restore at the end so yanks that landed in this call survive.
    let mut unnamed = std::mem::take(&mut app.vim.unnamed);
    if let Some(doc) = app.document_mut() {
        if op_mutates(op) {
            doc.snapshot();
        }
        outcome = operator::apply_motion(op, motion, count, doc, &mut unnamed, viewport);
    }
    app.vim.unnamed = unnamed;
    sync_yank_to_clipboard(app, op);
    if motion.is_find() {
        app.vim.last_find = Some(motion);
    }
    if outcome.enter_insert {
        app.vim.enter_insert();
        app.vim.insert_session.start_change(ChangeOrigin::Motion {
            motion,
            op_count: count,
        });
    } else if recording && op_mutates(op) {
        app.vim.last_change = Some(ChangeRecord::OperatorMotion(op, motion, count));
    }
    app.refresh_viewport_for_cursor();
}

fn apply_op_linewise(app: &mut App, op: Operator, count: usize, recording: bool) {
    // Block-on-cursor short-circuit: `dd`/`yy`/`cc` on a Block (or
    // its result panel) treats the whole segment as one logical
    // line. The yanked text is the canonical fence markdown — paste
    // anywhere else + re-parse rebuilds the block. CM6-equivalent
    // cut/paste without needing visible fence delimiters.
    let block_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => Some(segment_idx),
        _ => None,
    };
    if let Some(idx) = block_idx {
        let mut yanked: Option<String> = None;
        if let Some(doc) = app.document_mut() {
            if op_mutates(op) {
                doc.snapshot();
            }
            yanked = match op {
                Operator::Yank => doc.yank_block_at(idx),
                Operator::Delete | Operator::Change => doc.delete_block_at(idx),
            };
        }
        if let Some(text) = yanked {
            app.vim.unnamed.set_linewise(text);
        }
        sync_yank_to_clipboard(app, op);
        if matches!(op, Operator::Change) {
            app.vim.enter_insert();
            app.vim
                .insert_session
                .start_change(ChangeOrigin::Linewise { op_count: count });
        } else if recording && op_mutates(op) {
            app.vim.last_change = Some(ChangeRecord::OperatorLinewise(op, count));
        }
        app.refresh_viewport_for_cursor();
        return;
    }

    let mut outcome = operator::OpOutcome::default();
    let mut unnamed = std::mem::take(&mut app.vim.unnamed);
    if let Some(doc) = app.document_mut() {
        if op_mutates(op) {
            doc.snapshot();
        }
        outcome = operator::apply_linewise(op, count, doc, &mut unnamed);
    }
    app.vim.unnamed = unnamed;
    sync_yank_to_clipboard(app, op);
    if outcome.enter_insert {
        app.vim.enter_insert();
        app.vim
            .insert_session
            .start_change(ChangeOrigin::Linewise { op_count: count });
    } else if recording && op_mutates(op) {
        app.vim.last_change = Some(ChangeRecord::OperatorLinewise(op, count));
    }
    app.refresh_viewport_for_cursor();
}

fn apply_op_textobject(
    app: &mut App,
    op: Operator,
    textobj: TextObject,
    count: usize,
    recording: bool,
) {
    let mut outcome = operator::OpOutcome::default();
    let mut unnamed = std::mem::take(&mut app.vim.unnamed);
    if let Some(doc) = app.document_mut() {
        if op_mutates(op) {
            doc.snapshot();
        }
        outcome = operator::apply_text_object(op, textobj, count, doc, &mut unnamed);
    }
    app.vim.unnamed = unnamed;
    sync_yank_to_clipboard(app, op);
    if outcome.enter_insert {
        app.vim.enter_insert();
        app.vim
            .insert_session
            .start_change(ChangeOrigin::TextObject {
                textobj,
                op_count: count,
            });
    } else if recording && op_mutates(op) {
        app.vim.last_change = Some(ChangeRecord::OperatorTextObject(op, textobj, count));
    }
    app.refresh_viewport_for_cursor();
}

fn apply_paste(app: &mut App, pos: PastePos, count: usize, recording: bool) {
    if let Some(doc) = app.document_mut() {
        doc.snapshot();
    }
    let reg = app.vim.unnamed.clone();
    if let Some(doc) = app.document_mut() {
        operator::paste(pos, count, doc, &reg);
    }
    if recording {
        app.vim.last_change = Some(ChangeRecord::Paste(pos, count));
    }
    // Paste lands in prose. If the register held fence text (the
    // common case after `dd` on a block), the just-inserted prose
    // now contains a complete fence — re-parse so the block is
    // reinstated at the destination. Cheap when there's no fence
    // (parse_blocks returns empty and the helper bails).
    if let Some(Cursor::InProse { segment_idx, .. }) = app.document().map(|d| d.cursor()) {
        if let Some(doc) = app.document_mut() {
            doc.reparse_prose_at(segment_idx);
        }
    }
    app.refresh_viewport_for_cursor();
}

fn op_mutates(op: Operator) -> bool {
    !matches!(op, Operator::Yank)
}

/// After a yank lands in `app.vim.unnamed`, push its text to the
/// system clipboard so paste outside the TUI works. Failures (no X
/// forwarder, sandbox, etc.) bubble up to a non-fatal status hint —
/// the unnamed register still holds the text for in-TUI paste.
fn sync_yank_to_clipboard(app: &mut App, op: Operator) {
    if !matches!(op, Operator::Yank) {
        return;
    }
    if app.vim.unnamed.text.is_empty() {
        return;
    }
    if let Err(msg) = crate::clipboard::set_text(&app.vim.unnamed.text) {
        app.set_status(StatusKind::Error, msg);
    }
}

// ───────────── visual mode operators ─────────────

fn apply_visual_operator(app: &mut App, op: Operator, _recording: bool) {
    let Some(anchor) = app.vim.visual_anchor else {
        return;
    };
    let linewise = matches!(app.vim.mode, Mode::VisualLine);
    let mut outcome = operator::OpOutcome::default();
    let mut unnamed = std::mem::take(&mut app.vim.unnamed);

    let cursor_now = app.document().map(|d| d.cursor());
    let cross_segment = matches!(
        (anchor, cursor_now),
        (
            Cursor::InProse { segment_idx: a, .. } | Cursor::InBlock { segment_idx: a, .. },
            Some(Cursor::InProse { segment_idx: c, .. } | Cursor::InBlock { segment_idx: c, .. })
        ) if a != c
    );

    if cross_segment {
        // Selection spans multiple segments (prose ↔ block, block ↔
        // prose, etc.). The single-segment operator engine doesn't
        // know about segment seams, so we round-trip the doc through
        // markdown: serialize → splice the selected range → re-parse.
        // Cached block state survives by alias matching (see
        // `Document::replace_with_text`).
        let cursor_after = apply_cross_segment_visual(
            app,
            op,
            anchor,
            cursor_now.unwrap_or(anchor),
            linewise,
            &mut unnamed,
        );
        app.vim.unnamed = unnamed;
        sync_yank_to_clipboard(app, op);
        if let Some(c) = cursor_after {
            if let Some(doc) = app.document_mut() {
                doc.set_cursor(c);
            }
        }
        if matches!(op, Operator::Change) {
            app.vim.enter_insert();
            app.vim.insert_session.start_plain(InsertPos::Current);
        } else {
            return_from_visual(app);
        }
        app.refresh_viewport_for_cursor();
        return;
    }

    // Same-segment branch. When the selection lives entirely inside
    // a single block, promote `b.raw` to a Prose segment via
    // InBlockSwap so the existing prose-only `apply_visual` engine
    // handles it.
    let in_block_swap = matches!(
        (anchor, cursor_now),
        (Cursor::InBlock { segment_idx: a, .. }, Some(Cursor::InBlock { segment_idx: c, .. })) if a == c
    );
    let swap = if in_block_swap {
        InBlockSwap::maybe_enter(app)
    } else {
        None
    };
    let translated_anchor = if let Some(swap) = swap.as_ref() {
        match anchor {
            Cursor::InBlock { offset, .. } => Cursor::InProse {
                segment_idx: swap.segment_idx,
                offset,
            },
            other => other,
        }
    } else {
        anchor
    };

    if let Some(doc) = app.document_mut() {
        if !matches!(op, Operator::Yank) {
            doc.snapshot();
        }
        let cursor = doc.cursor();
        outcome =
            operator::apply_visual(op, translated_anchor, cursor, linewise, doc, &mut unnamed);
    }
    if let Some(swap) = swap {
        swap.exit(app);
    }
    app.vim.unnamed = unnamed;
    sync_yank_to_clipboard(app, op);
    if outcome.enter_insert {
        app.vim.enter_insert();
        app.vim.insert_session.start_plain(InsertPos::Current);
    } else {
        return_from_visual(app);
    }
    app.refresh_viewport_for_cursor();
}

/// Visual operator (d / y / c) on a selection that crosses segment
/// boundaries — heading + block + paragraph, etc. Round-trips the
/// doc through markdown so the operator doesn't have to teach every
/// segment kind about every cut. Cached block state (state /
/// cached_result) survives via alias matching in `replace_with_text`.
///
/// Returns the cursor's new position (so the caller can apply it
/// after dropping its mutable doc borrow), or `None` when nothing
/// changed.
fn apply_cross_segment_visual(
    app: &mut App,
    op: Operator,
    anchor: Cursor,
    cursor: Cursor,
    linewise: bool,
    reg: &mut crate::vim::register::Register,
) -> Option<Cursor> {
    let (a_seg, a_off) = endpoint_of(anchor)?;
    let (c_seg, c_off) = endpoint_of(cursor)?;

    let doc = app.tabs.active_document_mut()?;
    if !matches!(op, Operator::Yank) {
        doc.snapshot();
    }
    let full_text = doc.to_markdown();
    let chars: Vec<char> = full_text.chars().collect();
    let total = chars.len();
    let lo_global =
        doc.global_offset_for(a_seg.min(c_seg), if a_seg <= c_seg { a_off } else { c_off });
    let hi_global =
        doc.global_offset_for(a_seg.max(c_seg), if a_seg <= c_seg { c_off } else { a_off });
    let (lo, hi) = (lo_global.min(total), hi_global.min(total));

    // Resolve the inclusive char range to delete / yank. Linewise
    // expands to whole lines; charwise is inclusive at hi.
    let (start, end) = if linewise {
        let lo_line_start = line_start_in_chars(&chars, lo);
        let hi_line_end = line_end_inclusive_with_newline(&chars, hi);
        (lo_line_start, hi_line_end)
    } else {
        (lo, (hi + 1).min(total))
    };
    if end <= start {
        return None;
    }

    let yanked: String = chars[start..end].iter().collect();
    reg.text = yanked.clone();
    reg.linewise = linewise;

    if matches!(op, Operator::Yank) {
        // Yank doesn't mutate the doc — restore the original cursor
        // (visual mode collapses to anchor end on yank, vim convention).
        return Some(anchor);
    }

    // Splice the range out and rebuild the doc. The cursor lands
    // at `start` (vim's convention for d / c).
    let new_text: String = chars[..start].iter().chain(chars[end..].iter()).collect();
    // We need a Cursor for the new doc; pre-compute as InProse
    // segment 0 offset 0; replace_with_text clamps it sanely.
    let target = Cursor::InProse {
        segment_idx: 0,
        offset: 0,
    };
    if doc.replace_with_text(&new_text, target).is_err() {
        return None;
    }
    // Now find the segment + offset where `start` falls in the
    // *new* doc by walking segments. global_offset_for is the
    // forward map; we need the inverse.
    let new_cursor = cursor_at_global_offset(doc, start);
    doc.set_cursor(new_cursor);
    Some(new_cursor)
}

fn endpoint_of(c: Cursor) -> Option<(usize, usize)> {
    match c {
        Cursor::InProse {
            segment_idx,
            offset,
        } => Some((segment_idx, offset)),
        Cursor::InBlock {
            segment_idx,
            offset,
        } => Some((segment_idx, offset)),
        Cursor::InBlockResult { .. } => None,
    }
}

fn line_start_in_chars(chars: &[char], offset: usize) -> usize {
    let off = offset.min(chars.len());
    let mut i = off;
    while i > 0 && chars[i - 1] != '\n' {
        i -= 1;
    }
    i
}

fn line_end_inclusive_with_newline(chars: &[char], offset: usize) -> usize {
    let mut i = offset.min(chars.len());
    while i < chars.len() && chars[i] != '\n' {
        i += 1;
    }
    if i < chars.len() {
        i + 1
    } else {
        i
    }
}

/// Find the (segment_idx, offset) cursor that maps to `target_global`
/// in `doc.to_markdown()`. Inverse of `Document::global_offset_for`.
fn cursor_at_global_offset(doc: &crate::buffer::Document, target_global: usize) -> Cursor {
    let mut global = 0usize;
    let last_visible_idx = doc
        .segments()
        .iter()
        .enumerate()
        .filter(|(_, s)| !is_empty_prose(s))
        .map(|(i, _)| i)
        .next_back()
        .unwrap_or(0);
    let mut emitted_so_far = String::new();
    for (i, seg) in doc.segments().iter().enumerate() {
        if is_empty_prose(seg) {
            continue;
        }
        let seg_text = match seg {
            Segment::Prose(r) => r.to_string(),
            Segment::Block(b) => {
                let adapter = httui_core::blocks::parser::ParsedBlock {
                    block_type: b.block_type.clone(),
                    alias: b.alias.clone(),
                    display_mode: b.display_mode.clone(),
                    params: b.params.clone(),
                    line_start: 0,
                    line_end: 0,
                };
                httui_core::blocks::serialize_block(&adapter)
            }
        };
        let seg_len = seg_text.chars().count();
        if target_global >= global && target_global <= global + seg_len {
            let local = target_global - global;
            return match seg {
                Segment::Prose(_) => Cursor::InProse {
                    segment_idx: i,
                    offset: local,
                },
                Segment::Block(_) => Cursor::InBlock {
                    segment_idx: i,
                    offset: local,
                },
            };
        }
        global += seg_len;
        emitted_so_far.push_str(&seg_text);
        if i < last_visible_idx && !emitted_so_far.ends_with('\n') {
            global += 1;
            emitted_so_far.push('\n');
        }
    }
    // Fell off the end — park on the last segment.
    Cursor::InProse {
        segment_idx: doc.segment_count().saturating_sub(1),
        offset: 0,
    }
}

fn is_empty_prose(s: &Segment) -> bool {
    matches!(s, Segment::Prose(r) if r.len_chars() == 0)
}

/// `va{` / `vi{` / `vaw` / `vi"` etc. — extend the current visual
/// selection to cover the resolved text object. Reuses the same
/// `textobject::compute_range` the operator engine uses, so the
/// notion of what's "inside" / "around" stays consistent. The
/// returned range is `[start, end)` (end exclusive); we snap the
/// anchor to `start` and the moving cursor to `end - 1` so the
/// selection paints inclusively at both ends. Mode stays Visual /
/// VisualLine — user can layer more motions on top.
fn apply_visual_select_textobject(app: &mut App, textobj: TextObject) {
    let Some(doc) = app.document_mut() else {
        return;
    };
    let Some((segment_idx, start, end)) = crate::vim::textobject::compute_range(textobj, doc)
    else {
        return;
    };
    if end == 0 || end <= start {
        return;
    }
    app.vim.visual_anchor = Some(Cursor::InProse {
        segment_idx,
        offset: start,
    });
    if let Some(doc) = app.document_mut() {
        doc.set_cursor(Cursor::InProse {
            segment_idx,
            offset: end - 1,
        });
    }
    app.refresh_viewport_for_cursor();
}

/// Leave Visual / VisualLine and pick the right "back" mode. When
/// the row-detail modal is the active surface (it owns its own
/// `Document` via `App::document_mut`'s redirect), we restore
/// `Mode::DbRowDetail` so the modal keeps rendering and key input
/// keeps flowing through `parse_db_row_detail`. Otherwise the
/// editor's normal mode is the natural exit.
fn return_from_visual(app: &mut App) {
    if app.db_row_detail.is_some() {
        app.vim.mode = Mode::DbRowDetail;
        app.vim.visual_anchor = None;
        app.vim.reset_pending();
    } else {
        app.vim.enter_normal();
    }
}

// ───────────── block execution (`r` in normal) ─────────────
//
// `apply_run_block`, `run_db_block_inner`, `spawn_db_query`,
// `handle_db_block_result`, `cancel_running_query`, and
// `load_more_db_block` all moved to `crate::commands::db`. The
// vim-side action handlers (`apply_confirm_db_run`,
// `maybe_prefetch_db_more_rows`, the top-level dispatcher) call
// directly into that module.

/// Distance from the bottom of the loaded result that triggers an
/// eager fetch of the next page. Half the on-screen viewport feels
/// natural — by the time the user is looking at the last visible
/// row, the next batch is usually already there.
const DB_PREFETCH_THRESHOLD: usize = 5;

/// Pure decision function for the infinite-scroll prefetch. Returns
/// `true` when the cursor is close enough to the bottom of the
/// currently loaded rows that we should fetch the next page.
///
/// `cursor_row` is 0-indexed, `total` is the number of rows currently
/// in the cache, and `has_more` is the backend's signal that more
/// pages are still available.
fn should_prefetch(cursor_row: usize, total: usize, has_more: bool, threshold: usize) -> bool {
    has_more && cursor_row + threshold >= total
}

/// Hook called from the motion dispatcher: when the cursor is parked
/// inside a DB result whose backend reports `has_more`, fetch the
/// next page once we're within `DB_PREFETCH_THRESHOLD` rows of the
/// loaded bottom. Mirrors the desktop's near-bottom load-more pattern
/// (`DbFencedPanel.tsx` → `ResultTable.handleScroll`).
fn maybe_prefetch_db_more_rows(app: &mut App) {
    let Some(doc) = app.document() else { return };
    let Cursor::InBlockResult { segment_idx, row } = doc.cursor() else {
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        return;
    };
    let Segment::Block(block) = seg else { return };
    if !block.is_db() {
        return;
    }
    let Some(cached) = block.cached_result.as_ref() else {
        return;
    };
    let Some(first) = cached
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
    else {
        return;
    };
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return;
    }
    let has_more = first
        .get("has_more")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let total = first
        .get("rows")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if !should_prefetch(row, total, has_more, DB_PREFETCH_THRESHOLD) {
        return;
    }
    // While a query is already in flight, the prefetch silently
    // backs off — the user is moving the cursor around naturally
    // and we don't want to spam the status bar with "another query
    // is already running" on every motion.
    if app.running_query.is_some() {
        return;
    }
    if let Err(msg) = crate::commands::db::load_more_db_block(app, segment_idx) {
        app.set_status(StatusKind::Error, format!("load more: {msg}"));
    }
}

// ───────────── connection picker popup ─────────────

/// `gc` — open the connection picker popup anchored to the DB
/// block at the cursor. Loads connections from `httui-core`
/// synchronously (small SQLite read, runs on the dispatch thread)
/// and seeds the picker state. Returns `Err(msg)` on validation
/// failures (no DB block at cursor, no connections registered) so
/// the caller can surface a status.
fn open_connection_picker(app: &mut App) -> Result<(), String> {
    let segment_idx = match app.document().map(|d| d.cursor()) {
        Some(Cursor::InBlock { segment_idx, .. })
        | Some(Cursor::InBlockResult { segment_idx, .. }) => segment_idx,
        _ => return Err("no DB block at cursor".into()),
    };
    let block = match app
        .document()
        .and_then(|d| d.segments().get(segment_idx).cloned())
    {
        Some(Segment::Block(b)) => b,
        _ => return Err("no DB block at cursor".into()),
    };
    if !block.is_db() {
        return Err(format!(
            "`{}` blocks don't have a connection",
            block.block_type
        ));
    }

    let pool_mgr = app.pool_manager.clone();
    let raw = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(httui_core::db::connections::list_connections(
            pool_mgr.app_pool(),
        ))
    });
    let connections: Vec<crate::app::ConnectionEntry> = match raw {
        Ok(list) => list
            .into_iter()
            .map(|c| crate::app::ConnectionEntry {
                id: c.id,
                name: c.name,
                kind: c.driver,
            })
            .collect(),
        Err(e) => return Err(format!("connection list failed: {e}")),
    };
    if connections.is_empty() {
        return Err("no connections registered yet".into());
    }

    // Pre-select the block's current connection so the user can hit
    // Enter to keep it (or arrow to switch). Falls back to the first
    // entry when the current value matches nothing.
    let current = block
        .params
        .get("connection_id")
        .or_else(|| block.params.get("connection"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let selected = connections
        .iter()
        .position(|c| c.id == current || c.name == current)
        .unwrap_or(0);

    app.connection_picker = Some(crate::app::ConnectionPickerState {
        segment_idx,
        connections,
        selected,
    });
    app.vim.mode = Mode::ConnectionPicker;
    app.vim.reset_pending();
    Ok(())
}

fn apply_close_connection_picker(app: &mut App) {
    app.connection_picker = None;
    app.vim.enter_normal();
}

fn apply_move_connection_picker_cursor(app: &mut App, delta: i32) {
    let Some(state) = app.connection_picker.as_mut() else {
        return;
    };
    if state.connections.is_empty() {
        return;
    }
    let last = state.connections.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

/// `Enter` in the picker — write the selected connection's id to
/// the anchored block's params (`connection` field) and close. The
/// document is marked dirty via `snapshot()` so undo can restore
/// the previous value.
fn apply_confirm_connection_picker(app: &mut App) {
    let Some(state) = app.connection_picker.take() else {
        app.vim.enter_normal();
        return;
    };
    app.vim.enter_normal();
    let Some(picked) = state.connections.get(state.selected).cloned() else {
        return;
    };
    let segment_idx = state.segment_idx;
    let picked_id = picked.id.clone();
    let picked_name = picked.name.clone();
    if let Some(doc) = app.tabs.active_document_mut() {
        doc.snapshot();
        if let Some(block) = doc.block_at_mut(segment_idx) {
            if let Some(obj) = block.params.as_object_mut() {
                obj.insert(
                    "connection".into(),
                    serde_json::Value::String(picked_id.clone()),
                );
                // Drop the legacy alias so the next save serializes
                // the canonical `connection=<id>` form only — the
                // `connection_id` field was a JSON-body holdover from
                // pre-redesign blocks and gets resolved the same way
                // at run time.
                obj.remove("connection_id");
            }
        }
    }
    // Kick off schema introspection in the background. By the time
    // the user starts typing inside the SQL field, the
    // completion engine has tables/columns ready to suggest. Cheap to
    // call repeatedly — `ensure_schema_loaded` dedups on `pending`.
    app.ensure_schema_loaded(&picked_id);
    app.set_status(StatusKind::Info, format!("connection set to {picked_name}"));
}

/// `D` in the connection picker — drop the highlighted connection
/// from the registry. The picker stays open with the list reloaded;
/// blocks that referenced the deleted id will surface a missing-
/// connection error on next run, which is the right level of
/// visibility (silent breakage would be worse).
fn apply_delete_connection_in_picker(app: &mut App) {
    let Some(state) = app.connection_picker.as_ref() else {
        return;
    };
    let Some(picked) = state.connections.get(state.selected).cloned() else {
        return;
    };
    let pool_mgr = app.pool_manager.clone();
    let id = picked.id.clone();
    let name = picked.name.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(httui_core::db::connections::delete_connection(
            pool_mgr.app_pool(),
            &id,
        ))
    });
    if let Err(e) = result {
        app.set_status(StatusKind::Error, format!("delete connection failed: {e}"));
        return;
    }

    // Reload the list so the picker reflects the deletion. If we
    // emptied the list, close the picker — there's nothing left to
    // pick.
    let raw = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(httui_core::db::connections::list_connections(
            pool_mgr.app_pool(),
        ))
    });
    match raw {
        Ok(list) => {
            let entries: Vec<crate::app::ConnectionEntry> = list
                .into_iter()
                .map(|c| crate::app::ConnectionEntry {
                    id: c.id,
                    name: c.name,
                    kind: c.driver,
                })
                .collect();
            if entries.is_empty() {
                apply_close_connection_picker(app);
                app.set_status(
                    StatusKind::Info,
                    format!("deleted \"{name}\" — no connections left"),
                );
                return;
            }
            if let Some(state) = app.connection_picker.as_mut() {
                state.selected = state.selected.min(entries.len().saturating_sub(1));
                state.connections = entries;
            }
            // Refresh the global name lookup so block headers stop
            // showing the deleted connection's label.
            app.refresh_connection_names();
            app.set_status(StatusKind::Info, format!("deleted \"{name}\""));
        }
        Err(e) => {
            app.set_status(
                StatusKind::Error,
                format!("connection list reload failed: {e}"),
            );
        }
    }
}

// ───────────── block-jump motions (g] / g[) ─────────────

#[derive(Debug, Clone, Copy)]
enum JumpDir {
    Next,
    Prev,
}

/// Move the cursor to the first body offset of the next or previous
/// block segment relative to the current position. No-wrap: when
/// the cursor is already past the last block (or before the first),
/// the call is a silent no-op — matches vim's `]m` / `[m` feel.
///
/// "Current position" resolves through the cursor's segment_idx.
/// Sitting *inside* a block, `g]` jumps to the next block, not the
/// current one; `g[` jumps to the previous one. Sitting in prose,
/// the same rule applies relative to the surrounding segment index.
fn apply_jump_block(app: &mut App, dir: JumpDir) {
    let Some(doc) = app.document() else { return };
    let cur_idx = match doc.cursor() {
        Cursor::InProse { segment_idx, .. }
        | Cursor::InBlock { segment_idx, .. }
        | Cursor::InBlockResult { segment_idx, .. } => segment_idx,
    };
    let target_idx = match dir {
        JumpDir::Next => doc
            .segments()
            .iter()
            .enumerate()
            .skip(cur_idx + 1)
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i)),
        JumpDir::Prev => doc
            .segments()
            .iter()
            .enumerate()
            .take(cur_idx)
            .rev()
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i)),
    };
    let Some(target) = target_idx else { return };
    if let Some(doc) = app.document_mut() {
        // Park the cursor on offset 0 of the block's raw rope. The
        // first character of the fence header lives there, so the
        // user lands on the block "from above" (predictable spot
        // they can scroll down or directly type into).
        doc.set_cursor(Cursor::InBlock {
            segment_idx: target,
            offset: 0,
        });
    }
    app.refresh_viewport_for_cursor();
}

// ───────────── rerun last block (gr) ─────────────

/// `gr` — re-execute the block recorded in `App.last_run_anchor`,
/// without requiring the cursor to be on it. Resolution rules:
///
/// 1. If `last_run_anchor` is `None` → status hint "no block has
///    been run yet this session".
/// 2. If the active document's path doesn't match the anchor's
///    file → status hint "last run was in <path>" so the user
///    knows where to switch.
/// 3. Look up the target block by alias (preferred — survives
///    edits above the block) and fall back to `segment_idx`. If
///    neither resolves to a block, status hint "anchor lost".
/// 4. Park the cursor on the resolved segment (offset 0) and
///    delegate to `apply_run_block`. The dispatch chain there
///    handles HTTP vs DB, schedules the async task, and updates
///    `last_run_anchor` again with the freshly-resolved index.
fn apply_rerun_last_block(app: &mut App) {
    let Some(anchor) = app.last_run_anchor.clone() else {
        app.set_status(StatusKind::Info, "no block has been run yet");
        return;
    };
    let Some(active_path) = app.active_pane().and_then(|p| p.document_path.clone()) else {
        app.set_status(StatusKind::Info, "no document open");
        return;
    };
    if active_path != anchor.file_path {
        app.set_status(
            StatusKind::Info,
            format!("last run was in {}", anchor.file_path.display()),
        );
        return;
    }
    let target_idx = {
        let Some(doc) = app.document() else { return };
        // Alias-first lookup so edits that shifted segment_idx don't
        // fire the wrong block.
        let by_alias = anchor.alias.as_deref().and_then(|a| {
            doc.segments()
                .iter()
                .enumerate()
                .find_map(|(i, s)| match s {
                    Segment::Block(b) if b.alias.as_deref() == Some(a) => Some(i),
                    _ => None,
                })
        });
        by_alias.or_else(|| {
            // Fall back to the recorded index, but only if it still
            // points at a block — otherwise the anchor is stale.
            match doc.segments().get(anchor.segment_idx) {
                Some(Segment::Block(_)) => Some(anchor.segment_idx),
                _ => None,
            }
        })
    };
    let Some(idx) = target_idx else {
        app.set_status(
            StatusKind::Info,
            "previous block no longer exists in this file",
        );
        return;
    };
    if let Some(doc) = app.document_mut() {
        doc.set_cursor(Cursor::InBlock {
            segment_idx: idx,
            offset: 0,
        });
    }
    app.refresh_viewport_for_cursor();
    crate::commands::db::apply_run_block(app);
}

// ───────────── scroll-positioning chords (zz / zt / zb) ─────────────

/// Re-anchor the active pane's viewport so the cursor's row lands
/// at `pos` within the visible window. Mirrors vim's `zz` / `zt` /
/// `zb`. Reuses the `cursor_y` + `layout_document` plumbing from
/// `App::refresh_viewport_for_cursor` (private there) by computing
/// the target offset directly.
fn apply_scroll_cursor_to(app: &mut App, pos: crate::vim::parser::ScrollPos) {
    use crate::buffer::layout::layout_document;
    use crate::vim::parser::ScrollPos;

    let Some(pane) = app.active_pane_mut() else {
        return;
    };
    let Some(doc) = pane.document.as_ref() else {
        return;
    };
    // `width = 80` matches `App::refresh_viewport_for_cursor`'s
    // sentinel — block-aware layout doesn't actually use width
    // for vertical positioning.
    let layouts = layout_document(doc, 80);
    let cursor_y = compute_cursor_y(doc, &layouts);
    let height = pane.viewport_height.max(1);
    let new_top = match pos {
        ScrollPos::Top => cursor_y,
        ScrollPos::Center => cursor_y.saturating_sub(height / 2),
        ScrollPos::Bottom => cursor_y.saturating_sub(height.saturating_sub(1)),
    };
    pane.viewport_top = new_top;
}

/// Local mirror of `app::cursor_y` (private there). Resolves the
/// document-absolute Y row of the cursor by walking the segment
/// layout. Block cursors land at the body row; result-row cursors
/// land at the result table's offset.
fn compute_cursor_y(
    doc: &crate::buffer::Document,
    layouts: &[crate::buffer::layout::SegmentLayout],
) -> u16 {
    use crate::buffer::block::raw_section_at;
    use crate::buffer::block::RawSection;
    match doc.cursor() {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            let layout = layouts
                .iter()
                .find(|l| l.segment_idx == segment_idx)
                .copied();
            let line_offset = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(rope)) => {
                    rope.char_to_line(offset.min(rope.len_chars())) as u16
                }
                _ => 0,
            };
            layout
                .map(|l| l.y_start)
                .unwrap_or(0)
                .saturating_add(line_offset)
        }
        Cursor::InBlock {
            segment_idx,
            offset,
        } => {
            let Some(layout) = layouts.iter().find(|l| l.segment_idx == segment_idx) else {
                return 0;
            };
            let raw = match doc.segments().get(segment_idx) {
                Some(Segment::Block(b)) => &b.raw,
                _ => return layout.y_start,
            };
            // y_start is the top border row; +2 lands on the fence
            // header, +3 onward is body. Mirror `render_segment`'s
            // mapping in `ui::mod`.
            match raw_section_at(raw, offset) {
                RawSection::Header => layout.y_start.saturating_add(2),
                RawSection::Closer => layout
                    .y_start
                    .saturating_add(layout.height.saturating_sub(3)),
                RawSection::Body { line, .. } => {
                    layout.y_start.saturating_add(3).saturating_add(line as u16)
                }
            }
        }
        Cursor::InBlockResult { segment_idx, .. } => layouts
            .iter()
            .find(|l| l.segment_idx == segment_idx)
            .map(|l| l.y_start)
            .unwrap_or(0),
    }
}

// ───────────── reselect visual (gv) ─────────────

/// `gv` — re-enter visual mode at the last-saved anchor. V1 lands
/// the cursor on the anchor itself (rather than the previous moving
/// end) so the selection collapses to a single position; the user
/// then re-extends with motions. Silent decline when there's no
/// saved selection (`last_visual` is `None`).
fn apply_reselect_visual(app: &mut App) {
    let Some(last) = app.vim.last_visual else {
        return;
    };
    if let Some(doc) = app.document_mut() {
        doc.set_cursor(last.anchor);
    }
    if last.linewise {
        app.vim.enter_visual_line(last.anchor);
    } else {
        app.vim.enter_visual(last.anchor);
    }
    app.refresh_viewport_for_cursor();
}

// ───────────── write-all (gW) ─────────────

/// `gW` — walk every tab, save the active leaf when it has unsaved
/// edits. Hits `:w` semantics for each one (path-required, error
/// on missing file name) but rolls them up into a single status
/// line: "N files written" / "N files written, M errored".
///
/// Strategy: capture the currently-active tab idx, then loop by
/// flipping `tabs.active` to each dirty tab and calling
/// `ex::execute(ExCmd::Write)`. The flip is cheap (just an index)
/// and lets us reuse the existing single-doc save path without
/// duplicating its keychain / dirty-bit / `mark_clean` logic.
/// Restores the original active tab at the end.
fn apply_write_all(app: &mut App) {
    let original_active = app.tabs.active;
    let total = app.tabs.len();
    let mut written = 0usize;
    let mut errored = 0usize;
    let mut last_err: Option<String> = None;

    for idx in 0..total {
        let dirty = app
            .tabs
            .tabs
            .get(idx)
            .and_then(|t| t.active_leaf().document.as_ref())
            .is_some_and(|d| d.is_dirty());
        if !dirty {
            continue;
        }
        // Path-less buffers (`(no name)`) can't be written. Skip
        // silently rather than counting them as errors — `gW` is a
        // bulk action; we want the user to see the successes.
        let has_path = app
            .tabs
            .tabs
            .get(idx)
            .and_then(|t| t.active_leaf().document_path.as_ref())
            .is_some();
        if !has_path {
            continue;
        }
        app.tabs.active = idx;
        match ex::execute(app, ex::ExCmd::Write) {
            ex::ExResult::Ok(_) => written += 1,
            ex::ExResult::Err(msg) => {
                errored += 1;
                last_err = Some(msg);
            }
            _ => {}
        }
    }
    app.tabs.active = original_active;

    let status_kind = if errored > 0 {
        StatusKind::Error
    } else {
        StatusKind::Info
    };
    let msg = match (written, errored) {
        (0, 0) => "no dirty buffers".to_string(),
        (n, 0) => format!("{n} files written"),
        (0, e) => format!(
            "{e} errored: {}",
            last_err.unwrap_or_else(|| "unknown".into())
        ),
        (n, e) => format!(
            "{n} files written, {e} errored: {}",
            last_err.unwrap_or_else(|| "unknown".into())
        ),
    };
    app.set_status(status_kind, msg);
}

// ───────────── tab picker (gb) ─────────────

/// `gb` — snapshot every tab's focused-leaf path + dirty flag and
/// open the picker. Pre-selects the currently-active tab so Enter
/// is a no-op confirm. Silent decline when there's only one tab
/// (the picker would just display that single row, no real choice).
fn apply_open_tab_picker(app: &mut App) {
    if app.tabs.len() <= 1 {
        app.set_status(StatusKind::Info, "only one tab open");
        return;
    }
    let active = app.tabs.active;
    let entries: Vec<crate::app::TabPickerEntry> = app
        .tabs
        .tabs
        .iter()
        .enumerate()
        .map(|(idx, tab)| {
            let leaf = tab.active_leaf();
            let label = leaf
                .document_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "(no file)".into());
            let dirty = leaf.document.as_ref().is_some_and(|d| d.is_dirty());
            crate::app::TabPickerEntry { idx, label, dirty }
        })
        .collect();
    app.tab_picker = Some(crate::app::TabPickerState {
        entries,
        selected: active,
    });
    app.vim.mode = Mode::TabPicker;
    app.vim.reset_pending();
}

fn apply_move_tab_picker_cursor(app: &mut App, delta: i32) {
    let Some(state) = app.tab_picker.as_mut() else {
        return;
    };
    if state.entries.is_empty() {
        return;
    }
    let last = state.entries.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

/// `Enter` in the tab picker — flip `tabs.active` to the picked
/// index and dismiss. The `sync_file_watcher` call after every
/// keystroke (in the main loop) catches the new file in lockstep.
fn apply_confirm_tab_picker(app: &mut App) {
    let Some(state) = app.tab_picker.take() else {
        app.vim.enter_normal();
        return;
    };
    app.vim.enter_normal();
    let Some(picked) = state.entries.get(state.selected) else {
        return;
    };
    if picked.idx < app.tabs.tabs.len() {
        app.tabs.active = picked.idx;
    }
}

// ───────────── block-template picker (gN) ─────────────

fn apply_move_block_template_picker_cursor(app: &mut App, delta: i32) {
    let Some(state) = app.block_template_picker.as_mut() else {
        return;
    };
    let len = crate::app::BlockTemplate::ALL.len();
    if len == 0 {
        return;
    }
    let last = len as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

/// `Enter` in the block-template picker — splice the picked
/// template's text into the active document at the cursor's
/// segment + line and re-parse so the typed fence promotes to a
/// `Segment::Block`. Three placement rules:
///
/// 1. Cursor in prose → insert at the start of the line *after*
///    the cursor's line (so we don't break a half-typed sentence
///    by injecting a fence mid-line).
/// 2. Cursor in a block → status error "exit block first" (the
///    template fence would corrupt the host block's `raw` rope).
/// 3. Cursor in a block result → same as (2).
///
/// Snapshot is taken before the splice so undo restores both the
/// inserted text and any cursor jump that follows. Cursor lands on
/// the freshly-promoted block's first body offset so the user can
/// immediately edit the URL / SQL.
fn apply_confirm_block_template_picker(app: &mut App) {
    let Some(state) = app.block_template_picker.take() else {
        app.vim.enter_normal();
        return;
    };
    app.vim.enter_normal();
    let Some(tpl) = crate::app::BlockTemplate::ALL.get(state.selected).copied() else {
        return;
    };
    let cursor = match app.document().map(|d| d.cursor()) {
        Some(c) => c,
        None => return,
    };
    let (segment_idx, line_offset_for_insert) = match cursor {
        Cursor::InProse {
            segment_idx,
            offset,
        } => {
            // Compute the char offset at the start of the line *after*
            // the cursor's line so the splice goes onto a fresh line.
            let Some(doc) = app.document() else { return };
            let rope = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => r,
                _ => return,
            };
            let total = rope.len_chars();
            let off = offset.min(total);
            let line = rope.char_to_line(off);
            // `line_to_char(line + 1)` is the start of the next line;
            // when the cursor is on the last line this hits `total`,
            // which is also "end of doc" — the splice appends.
            let next_line_start = if line + 1 < rope.len_lines() {
                rope.line_to_char(line + 1)
            } else {
                total
            };
            (segment_idx, next_line_start)
        }
        Cursor::InBlock { .. } | Cursor::InBlockResult { .. } => {
            app.set_status(
                StatusKind::Info,
                "exit block (Esc) before inserting a new block template",
            );
            return;
        }
    };

    if let Some(doc) = app.document_mut() {
        doc.snapshot();
        // Templates already include a trailing newline; if the cursor
        // is on a non-empty last line we prepend a `\n` so the fence
        // doesn't graft onto existing prose. `insert_text_in_segment`
        // is a raw rope insert — `reparse_prose_at` does the magic.
        let needs_leading_newline = {
            let rope = match doc.segments().get(segment_idx) {
                Some(Segment::Prose(r)) => r,
                _ => return,
            };
            line_offset_for_insert == rope.len_chars()
                && rope.len_chars() > 0
                && rope.char(rope.len_chars() - 1) != '\n'
        };
        let to_insert: String = if needs_leading_newline {
            format!("\n{}", tpl.text)
        } else {
            tpl.text.to_string()
        };
        doc.insert_text_in_segment(segment_idx, line_offset_for_insert, &to_insert);
        // Promote the typed fence into a `Segment::Block`. After this
        // call the prose segment may have been split (or replaced),
        // and a new block segment exists in its slot.
        doc.reparse_prose_at(segment_idx);
        // Park the cursor at the start of the first new block, if
        // any — search forward from the original splice point.
        if let Some((new_idx, _)) = doc
            .segments()
            .iter()
            .enumerate()
            .skip(segment_idx)
            .find(|(_, s)| matches!(s, Segment::Block(_)))
        {
            doc.set_cursor(Cursor::InBlock {
                segment_idx: new_idx,
                offset: 0,
            });
        }
    }
    app.refresh_viewport_for_cursor();
    app.set_status(StatusKind::Info, format!("inserted {}", tpl.label));
}

// ───────────── environment picker (gE) ─────────────

/// `gE` — pull every row from the `environments` table, snapshot the
/// active id (so the renderer can flag it), pre-select the active
/// row, and flip mode. Errors bubble up as a status hint.
fn open_environment_picker(app: &mut App) -> Result<(), String> {
    let pool = app.pool_manager.app_pool().clone();
    let (entries, active_id) = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let envs = httui_core::db::environments::list_environments(&pool)
                .await
                .map_err(|e| format!("env list failed: {e}"))?;
            // `get_active_environment_id` returns `Option<String>`
            // (no error path — it swallows DB failures and just
            // reports "no active env"). That's fine here: the
            // picker still opens, just without a pre-selected row.
            let active = httui_core::db::environments::get_active_environment_id(&pool).await;
            Ok::<_, String>((envs, active))
        })
    })?;
    if entries.is_empty() {
        return Err("no environments registered yet".into());
    }
    let entries: Vec<crate::app::EnvironmentEntry> = entries
        .into_iter()
        .map(|e| crate::app::EnvironmentEntry {
            id: e.id,
            name: e.name,
        })
        .collect();
    // Pre-select the active env so Enter is a no-op confirm. Falls
    // back to the first entry when nothing is active.
    let selected = active_id
        .as_deref()
        .and_then(|id| entries.iter().position(|e| e.id == id))
        .unwrap_or(0);

    app.environment_picker = Some(crate::app::EnvironmentPickerState {
        entries,
        selected,
        active_id,
    });
    app.vim.mode = Mode::EnvironmentPicker;
    app.vim.reset_pending();
    Ok(())
}

fn apply_close_environment_picker(app: &mut App) {
    app.environment_picker = None;
    app.vim.enter_normal();
}

fn apply_move_environment_picker_cursor(app: &mut App, delta: i32) {
    let Some(state) = app.environment_picker.as_mut() else {
        return;
    };
    if state.entries.is_empty() {
        return;
    }
    let last = state.entries.len() as i64 - 1;
    let next = (state.selected as i64)
        .saturating_add(delta as i64)
        .clamp(0, last);
    state.selected = next as usize;
}

/// `Enter` in the env picker — flip the active flag in SQLite, refresh
/// the cached display name (so the status-bar chip updates), and
/// dismiss. A no-op when the highlighted entry is already active.
fn apply_confirm_environment_picker(app: &mut App) {
    let Some(state) = app.environment_picker.take() else {
        app.vim.enter_normal();
        return;
    };
    app.vim.enter_normal();
    let Some(picked) = state.entries.get(state.selected).cloned() else {
        return;
    };
    if state.active_id.as_deref() == Some(picked.id.as_str()) {
        // Already active — silent no-op rather than a redundant
        // SQLite write. The display name is current.
        return;
    }
    let pool = app.pool_manager.app_pool().clone();
    let id = picked.id.clone();
    let result = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(
            httui_core::db::environments::set_active_environment(&pool, Some(&id)),
        )
    });
    if let Err(e) = result {
        app.set_status(StatusKind::Error, format!("set active env failed: {e}"));
        return;
    }
    app.refresh_active_env_name();
    app.set_status(StatusKind::Info, format!("env: {}", picked.name));
}

// ───────────── result-detail modals (DB row + HTTP response) ─────────────

/// `<CR>` dispatcher: routes to the right modal based on the block
/// type the cursor is parked on. DB blocks open the row-detail modal
/// (column → value pairs of the focused row); HTTP blocks open the
/// response-detail modal (status line + headers + full body). For any
/// other position the action is a no-op.
fn apply_open_result_detail(app: &mut App) {
    let Some(doc) = app.document() else { return };
    let Cursor::InBlockResult { segment_idx, .. } = doc.cursor() else {
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        return;
    };
    let Segment::Block(block) = seg else { return };
    if block.is_http() {
        apply_open_http_response_detail(app);
    } else if block.is_db() {
        apply_open_db_row_detail(app);
    }
}

/// `<CR>` in normal mode → open the row-detail modal. Validates the
/// cursor is parked on a real result row of a `select`, snapshots
/// the row's columns into a freshly-built `Document` (body text as
/// a single prose run), and flips the mode. The pending vim state
/// is reset so a stale count from the editor doesn't leak into the
/// modal's first keystroke.
fn apply_open_db_row_detail(app: &mut App) {
    let Some(doc) = app.document() else { return };
    let Cursor::InBlockResult { segment_idx, row } = doc.cursor() else {
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        return;
    };
    let Segment::Block(block) = seg else { return };
    if !block.is_db() {
        return;
    }
    let title = build_db_row_modal_title(block, row);
    let body_text = match build_db_row_body_text(block, row) {
        Some(t) => t,
        None => return,
    };
    // Build a Document from the body text. `from_markdown` of plain
    // text yields a single Prose segment, which is exactly what we
    // want — the motion engine treats it as one editable run. We
    // sanitize triple-backticks first so a row carrying ``` doesn't
    // accidentally open a fence and split the body in two.
    let safe_body = body_text.replace("```", "ʼʼʼ");
    let modal_doc = match crate::buffer::Document::from_markdown(&safe_body) {
        Ok(d) => d,
        Err(_) => return,
    };
    app.db_row_detail = Some(crate::app::DbRowDetailState {
        segment_idx,
        row,
        title,
        doc: modal_doc,
        // Updated by the renderer on the first paint; 1 is just a
        // safe lower bound so the first half-page motion (rare, but
        // possible if the user types `Ctrl-d` immediately) doesn't
        // divide by zero anywhere.
        viewport_height: 1,
        viewport_top: 0,
    });
    app.vim.mode = Mode::DbRowDetail;
    app.vim.reset_pending();
}

/// `Esc`/`q`/`Ctrl-c` inside the modal → drop the state and return
/// to normal mode. The editor cursor stays on the result row that
/// was being inspected, which feels right when the modal closes.
fn apply_close_db_row_detail(app: &mut App) {
    app.db_row_detail = None;
    app.vim.enter_normal();
}

/// Build the modal's title line. Uses the block's alias when set so
/// `Row 7 · 4 fields · q1` reads naturally; falls back to
/// `Row N · M fields` when no alias is present.
fn build_db_row_modal_title(block: &BlockNode, row: usize) -> String {
    let columns = block
        .cached_result
        .as_ref()
        .and_then(|v| v.get("results"))
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|r| r.get("columns"))
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let suffix = if columns == 1 { "field" } else { "fields" };
    match block.alias.as_deref() {
        Some(alias) => format!(" Row {} · {} {} · {} ", row + 1, columns, suffix, alias),
        None => format!(" Row {} · {} {} ", row + 1, columns, suffix),
    }
}

/// Render one row as the body text the modal will navigate. Mirrors
/// `ui::db_row_detail::build_body_lines` (column header line + 2-
/// space-indented value lines + blank separator) but emits a `String`
/// so it can be parsed into a `Document` for the motion engine.
fn build_db_row_body_text(block: &BlockNode, row: usize) -> Option<String> {
    let cached = block.cached_result.as_ref()?;
    let first = cached
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return None;
    }
    let columns: Vec<(String, String)> = first
        .get("columns")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|c| {
                    let name = c
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("?")
                        .to_string();
                    let ty = c
                        .get("type")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    (name, ty)
                })
                .collect()
        })
        .unwrap_or_default();
    if columns.is_empty() {
        return None;
    }
    let row_obj = first.get("rows").and_then(|v| v.as_array())?.get(row)?;
    let mut out = String::new();
    for (i, (name, ty)) in columns.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if ty.is_empty() {
            out.push_str(name);
        } else {
            out.push_str(&format!("{name}  ({ty})"));
        }
        out.push('\n');
        let value = row_obj
            .get(name)
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        for line in render_value_text(&value) {
            out.push_str("  ");
            out.push_str(&line);
            out.push('\n');
        }
    }
    Some(out)
}

/// Plain-text rendering of a JSON value for the body. Strings that
/// look like JSON (stringified objects/arrays — common with
/// Postgres `jsonb` over wire) are unwrapped + pretty-printed so
/// `metadata` columns aren't a single illegible blob.
fn render_value_text(v: &serde_json::Value) -> Vec<String> {
    match v {
        serde_json::Value::Null => vec!["NULL".into()],
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => vec![v.to_string()],
        serde_json::Value::String(s) => {
            let trimmed = s.trim();
            let looks_jsonish = (trimmed.starts_with('{') && trimmed.ends_with('}'))
                || (trimmed.starts_with('[') && trimmed.ends_with(']'));
            if looks_jsonish {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    return serde_json::to_string_pretty(&parsed)
                        .unwrap_or_default()
                        .lines()
                        .map(String::from)
                        .collect();
                }
            }
            if s.is_empty() {
                vec!["(empty)".into()]
            } else {
                s.lines().map(String::from).collect()
            }
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string_pretty(v)
                .unwrap_or_default()
                .lines()
                .map(String::from)
                .collect()
        }
    }
}

/// `y` inside the modal → copy the inspected row to the system
/// clipboard as pretty-printed JSON. Status hints differentiate the
/// success path ("row copied as JSON") from environments where no
/// clipboard backend is reachable (SSH without a forwarder, headless
/// container, sandbox).
fn apply_copy_db_row_detail_json(app: &mut App) {
    let Some(state) = app.db_row_detail.as_ref() else {
        return;
    };
    let Some(payload) = db_row_payload(app, state.segment_idx, state.row) else {
        app.set_status(StatusKind::Error, "row no longer available");
        return;
    };
    let text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string());
    match crate::clipboard::set_text(&text) {
        Ok(()) => app.set_status(StatusKind::Info, "row copied as JSON"),
        Err(msg) => app.set_status(StatusKind::Error, msg),
    }
}

/// Snapshot of a single result row as a `{column: value}` JSON
/// object. Source for the modal's `y` clipboard copy. Returns
/// `None` if the block / row vanished between the keystroke and
/// the dispatch (e.g. user re-ran the block in another tab).
fn db_row_payload(app: &App, segment_idx: usize, row: usize) -> Option<serde_json::Value> {
    let doc = app.document()?;
    let Segment::Block(block) = doc.segments().get(segment_idx)? else {
        return None;
    };
    let cached = block.cached_result.as_ref()?;
    let first = cached
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())?;
    if first.get("kind").and_then(|v| v.as_str()) != Some("select") {
        return None;
    }
    let columns: Vec<&str> = first
        .get("columns")
        .and_then(|v| v.as_array())?
        .iter()
        .filter_map(|c| c.get("name").and_then(|n| n.as_str()))
        .collect();
    let row_obj = first.get("rows").and_then(|v| v.as_array())?.get(row)?;
    let mut out = serde_json::Map::new();
    for name in columns {
        out.insert(
            name.to_string(),
            row_obj
                .get(name)
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
    }
    Some(serde_json::Value::Object(out))
}

// ───────────── HTTP response-detail modal ─────────────

/// Open the HTTP response-detail modal. Validates the cursor is on
/// an HTTP block with a cached response, snapshots the response
/// (status line + headers + body) into a fresh `Document`, flips
/// the mode. Pending vim state is reset to keep stale counts /
/// operators from leaking into the modal.
fn apply_open_http_response_detail(app: &mut App) {
    let Some(doc) = app.document() else { return };
    let Cursor::InBlockResult { segment_idx, .. } = doc.cursor() else {
        return;
    };
    let Some(seg) = doc.segments().get(segment_idx) else {
        return;
    };
    let Segment::Block(block) = seg else { return };
    if !block.is_http() {
        return;
    }
    let title = build_http_response_modal_title(block);
    let body_text = match build_http_response_body_text(block) {
        Some(t) => t,
        None => {
            app.set_status(StatusKind::Info, "no cached response on this block");
            return;
        }
    };
    // Sanitize triple-backticks so a body that carries ``` doesn't
    // open a fence and split the modal doc — we want a single Prose
    // segment for the motion engine to operate on.
    let safe_body = body_text.replace("```", "ʼʼʼ");
    let modal_doc = match crate::buffer::Document::from_markdown(&safe_body) {
        Ok(d) => d,
        Err(_) => return,
    };
    app.http_response_detail = Some(crate::app::HttpResponseDetailState {
        segment_idx,
        title,
        doc: modal_doc,
        viewport_height: 1,
        viewport_top: 0,
    });
    app.vim.mode = Mode::HttpResponseDetail;
    app.vim.reset_pending();
}

/// `Ctrl-c` inside the HTTP response-detail modal → drop the state
/// and return to normal mode.
fn apply_close_http_response_detail(app: &mut App) {
    app.http_response_detail = None;
    app.vim.enter_normal();
}

/// `Y` inside the modal → copy the full response body (raw, not the
/// rendered modal text) to the clipboard. Falls back gracefully when
/// the clipboard isn't reachable or no body is cached.
fn apply_copy_http_response_body(app: &mut App) {
    let Some(state) = app.http_response_detail.as_ref() else {
        return;
    };
    let Some(text) = http_response_raw_body(app, state.segment_idx) else {
        app.set_status(StatusKind::Error, "no response body to copy");
        return;
    };
    match crate::clipboard::set_text(&text) {
        Ok(()) => app.set_status(StatusKind::Info, "response body copied"),
        Err(msg) => app.set_status(StatusKind::Error, msg),
    }
}

/// Title line for the modal. Reuses the alias when present so the
/// header reads naturally: ` Response · 200 · 1.4 kB · login `.
fn build_http_response_modal_title(block: &BlockNode) -> String {
    let cached = match block.cached_result.as_ref() {
        Some(c) => c,
        None => return " Response ".to_string(),
    };
    let status = cached
        .get("status")
        .and_then(|v| v.as_u64())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "?".into());
    let size = cached
        .get("size_bytes")
        .and_then(|v| v.as_u64())
        .map(format_size)
        .unwrap_or_default();
    let mut parts: Vec<String> = vec![format!("Response · {status}")];
    if !size.is_empty() {
        parts.push(size);
    }
    if let Some(alias) = block.alias.as_deref() {
        parts.push(alias.to_string());
    }
    format!(" {} ", parts.join(" · "))
}

/// Render the cached response into the body text the modal's motion
/// engine will navigate. Layout (fixed): a status line, blank,
/// `Headers` section (`name: value` each), blank, `Body` heading,
/// then the body — pretty-printed JSON when possible, raw text
/// otherwise.
fn build_http_response_body_text(block: &BlockNode) -> Option<String> {
    let cached = block.cached_result.as_ref()?;
    let mut out = String::new();
    let status = cached.get("status").and_then(|v| v.as_u64()).unwrap_or(0);
    let status_text = cached
        .get("status_text")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let elapsed_ms = cached
        .get("timing")
        .and_then(|t| t.get("total_ms"))
        .and_then(|v| v.as_u64());
    let size_bytes = cached.get("size_bytes").and_then(|v| v.as_u64());
    out.push_str(&format!("{status} {status_text}"));
    if let Some(ms) = elapsed_ms {
        out.push_str(&format!("  ·  {ms} ms"));
    }
    if let Some(sz) = size_bytes {
        out.push_str(&format!("  ·  {}", format_size(sz)));
    }
    out.push('\n');

    out.push_str("\nHeaders\n");
    if let Some(headers) = cached.get("headers").and_then(|v| v.as_array()) {
        if headers.is_empty() {
            out.push_str("  (none)\n");
        } else {
            for h in headers {
                let key = h.get("key").and_then(|v| v.as_str()).unwrap_or("");
                let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
                out.push_str(&format!("  {key}: {value}\n"));
            }
        }
    } else {
        out.push_str("  (none)\n");
    }

    out.push_str("\nBody\n");
    let body_text = render_http_body(cached);
    if body_text.is_empty() {
        out.push_str("  (empty)\n");
    } else {
        // Indent each line by two spaces so the body lines up with
        // header values — a small visual cue that they share the same
        // "section body" role.
        for line in body_text.lines() {
            out.push_str("  ");
            out.push_str(line);
            out.push('\n');
        }
    }
    Some(out)
}

/// Format a byte count as a short human-readable string (`1.4 kB`,
/// `12 B`, `2.0 MB`). Mirrors the footer/status formatting in
/// `ui::blocks` so the modal title matches the editor chrome.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} kB");
    }
    let mb = kb / 1024.0;
    format!("{mb:.1} MB")
}

/// Render the body as text. Tries pretty-JSON first (covers the
/// common API path), falls back to whatever string representation
/// the cached value carries.
fn render_http_body(cached: &serde_json::Value) -> String {
    let body = cached.get("body");
    match body {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(v) => serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string()),
        None => String::new(),
    }
}

/// Raw body for the `Y`-copy path. Same fallback chain as
/// [`render_http_body`], minus the leading section labels.
fn http_response_raw_body(app: &App, segment_idx: usize) -> Option<String> {
    let doc = app.document()?;
    let Segment::Block(block) = doc.segments().get(segment_idx)? else {
        return None;
    };
    let cached = block.cached_result.as_ref()?;
    let body = render_http_body(cached);
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

// ───────────── window / split commands ─────────────

fn apply_window_cmd(app: &mut App, cmd: WindowCmd) {
    match cmd {
        WindowCmd::SplitVertical => split_focused(app, SplitDir::Vertical),
        WindowCmd::SplitHorizontal => split_focused(app, SplitDir::Horizontal),
        WindowCmd::FocusLeft => focus_dir(app, FocusDir::Left),
        WindowCmd::FocusRight => focus_dir(app, FocusDir::Right),
        WindowCmd::FocusUp => focus_dir(app, FocusDir::Up),
        WindowCmd::FocusDown => focus_dir(app, FocusDir::Down),
        WindowCmd::Cycle => {
            if let Some(tab) = app.active_tab_mut() {
                tab.cycle_focus();
            }
            app.refresh_viewport_for_cursor();
        }
        WindowCmd::Close => close_focused_pane(app),
        WindowCmd::Equalize => {
            if let Some(tab) = app.active_tab_mut() {
                tab.equalize();
            }
        }
    }
}

fn split_focused(app: &mut App, dir: SplitDir) {
    let Some(tab) = app.active_tab_mut() else {
        return;
    };
    let new_pane = tab.active_leaf().snapshot_clone();
    tab.split(dir, new_pane);
    app.refresh_viewport_for_cursor();
}

fn focus_dir(app: &mut App, dir: FocusDir) {
    if let Some(tab) = app.active_tab_mut() {
        tab.focus_dir(dir);
    }
    app.refresh_viewport_for_cursor();
}

/// Close the focused pane. When it's the only pane in the active tab,
/// closes the tab; when there are no tabs left, quits.
fn close_focused_pane(app: &mut App) {
    let leaf_count = app.active_tab().map(|t| t.leaf_count()).unwrap_or(0);
    if leaf_count > 1 {
        if app.document().is_some_and(|d| d.is_dirty()) {
            app.set_status(
                StatusKind::Error,
                "no write since last change (add ! to override)",
            );
            return;
        }
        if let Some(tab) = app.active_tab_mut() {
            tab.close_focused();
        }
        app.refresh_viewport_for_cursor();
        return;
    }
    match app.close_tab(false) {
        Ok(msg) => app.set_status(StatusKind::Info, msg),
        Err(msg) => {
            app.set_status(StatusKind::Error, msg);
            return;
        }
    }
    if app.tabs.is_empty() {
        app.should_quit = true;
    }
}

// ───────────── . repeat ─────────────

fn replay_last_change(app: &mut App, count: usize) {
    let Some(record) = app.vim.last_change.clone() else {
        return;
    };
    for _ in 0..count {
        replay_once(app, record.clone());
    }
}

fn replay_once(app: &mut App, record: ChangeRecord) {
    match record {
        ChangeRecord::OperatorMotion(op, motion, c) => {
            apply_op_motion(app, op, motion, c, false);
        }
        ChangeRecord::OperatorLinewise(op, c) => {
            apply_op_linewise(app, op, c, false);
        }
        ChangeRecord::OperatorTextObject(op, t, c) => {
            apply_op_textobject(app, op, t, c, false);
        }
        ChangeRecord::Paste(pos, c) => {
            apply_paste(app, pos, c, false);
        }
        ChangeRecord::Insert { pos, typed } => {
            replay_insert_session(app, Some(pos), None, &typed);
        }
        ChangeRecord::ChangeMotion {
            motion,
            op_count,
            typed,
        } => {
            apply_op_motion(app, Operator::Change, motion, op_count, false);
            replay_typed(app, &typed);
            // Replay's ExitInsert fires through dispatch only via real
            // keystrokes; here we exit synthetically.
            apply_action(app, Action::ExitInsert, false);
        }
        ChangeRecord::ChangeLinewise { op_count, typed } => {
            apply_op_linewise(app, Operator::Change, op_count, false);
            replay_typed(app, &typed);
            apply_action(app, Action::ExitInsert, false);
        }
        ChangeRecord::ChangeTextObject {
            textobj,
            op_count,
            typed,
        } => {
            apply_op_textobject(app, Operator::Change, textobj, op_count, false);
            replay_typed(app, &typed);
            apply_action(app, Action::ExitInsert, false);
        }
    }
}

fn replay_insert_session(app: &mut App, pos: Option<InsertPos>, _origin: Option<()>, typed: &str) {
    if let Some(p) = pos {
        apply_action(app, Action::EnterInsert(p), false);
    }
    replay_typed(app, typed);
    apply_action(app, Action::ExitInsert, false);
}

fn replay_typed(app: &mut App, typed: &str) {
    for c in typed.chars() {
        if c == '\n' {
            apply_action(app, Action::InsertNewline, false);
        } else {
            apply_action(app, Action::InsertChar(c), false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_prefetch_skips_when_backend_says_done() {
        // Cursor near the bottom but the server already exhausted
        // pages — no further fetch should fire.
        assert!(!should_prefetch(95, 100, false, 5));
        assert!(!should_prefetch(99, 100, false, 5));
    }

    #[test]
    fn should_prefetch_waits_for_threshold_when_more_pages_exist() {
        // Plenty of headroom: don't trigger.
        assert!(!should_prefetch(0, 100, true, 5));
        assert!(!should_prefetch(94, 100, true, 5));
        // Within the threshold band: trigger.
        assert!(should_prefetch(95, 100, true, 5));
        assert!(should_prefetch(99, 100, true, 5));
        // Past the loaded set (the engine reaches this momentarily
        // when motion overshoots before append finishes).
        assert!(should_prefetch(100, 100, true, 5));
    }

    #[test]
    fn should_prefetch_fires_immediately_for_small_initial_pages() {
        // 3 rows back, threshold 5, has_more true → trigger from
        // row 0 (page is smaller than the prefetch window).
        assert!(should_prefetch(0, 3, true, 5));
        assert!(should_prefetch(2, 3, true, 5));
    }

    #[test]
    fn should_prefetch_handles_empty_set() {
        // Defensive: no rows + has_more shouldn't crash on the
        // arithmetic and shouldn't fire (cursor can't be in an
        // empty result anyway).
        assert!(should_prefetch(0, 0, true, 5));
        assert!(!should_prefetch(0, 0, false, 5));
    }

    #[test]
    fn format_size_picks_unit_by_magnitude() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 kB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
    }

    #[test]
    fn build_http_response_body_text_lays_out_status_headers_and_body() {
        // Synthesize a fenced HTTP block + cached_result mimicking the
        // shape `http_response_to_json` emits, then check the rendered
        // modal body has the section headings, status line and an
        // indented body line.
        let block_md = "```http alias=req1\nGET https://api.example.com/users\n```\n";
        let doc = crate::buffer::Document::from_markdown(block_md).unwrap();
        let mut block = doc
            .segments()
            .iter()
            .find_map(|s| match s {
                Segment::Block(b) => Some(b.clone()),
                _ => None,
            })
            .expect("block segment in parsed doc");
        block.cached_result = Some(serde_json::json!({
            "status": 200,
            "status_text": "OK",
            "headers": [
                {"key": "content-type", "value": "application/json"},
                {"key": "x-trace", "value": "abc"},
            ],
            "cookies": [],
            "body": serde_json::json!({"ok": true}),
            "size_bytes": 42,
            "timing": {"total_ms": 142, "ttfb_ms": 30},
        }));

        let text = build_http_response_body_text(&block).expect("body text");
        // Status line is line zero with code and human label.
        assert!(text.starts_with("200 OK"), "got: {text}");
        assert!(text.contains("142 ms"));
        assert!(text.contains("42 B"));
        // Section heading + a header pair.
        assert!(text.contains("\nHeaders\n"));
        assert!(text.contains("  content-type: application/json"));
        // Body section + pretty JSON line.
        assert!(text.contains("\nBody\n"));
        assert!(text.contains("  \"ok\": true"));
    }

    #[test]
    fn build_http_response_modal_title_includes_status_size_alias() {
        let block_md = "```http alias=login\nPOST https://example.com/login\n```\n";
        let doc = crate::buffer::Document::from_markdown(block_md).unwrap();
        let mut block = doc
            .segments()
            .iter()
            .find_map(|s| match s {
                Segment::Block(b) => Some(b.clone()),
                _ => None,
            })
            .expect("block segment in parsed doc");
        block.cached_result = Some(serde_json::json!({
            "status": 201,
            "size_bytes": 1500,
        }));
        let title = build_http_response_modal_title(&block);
        // Padded with spaces (it's a window title, ratatui paints it
        // with a space-buffer so it doesn't kiss the corner).
        assert!(title.starts_with(' '));
        assert!(title.ends_with(' '));
        assert!(title.contains("201"));
        assert!(title.contains("1.5 kB"));
        assert!(title.contains("login"));
    }

    /// Build a Document with prose / block / prose / block / prose
    /// to exercise `apply_jump_block` against the segment iterator.
    fn doc_with_two_blocks() -> crate::buffer::Document {
        let md = "intro\n\n```http alias=a\nGET https://a.test\n```\n\nmid\n\n```http alias=b\nGET https://b.test\n```\n\nend\n";
        crate::buffer::Document::from_markdown(md).unwrap()
    }

    fn block_indices(doc: &crate::buffer::Document) -> Vec<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .filter_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
            .collect()
    }

    /// Stand-alone reimplementation of the navigator inside
    /// `apply_jump_block` — same predicate (skip current, no wrap),
    /// no `App` plumbing. Lets the test cover the search rule
    /// without spinning up a full `App`.
    fn next_block(doc: &crate::buffer::Document, cur: usize) -> Option<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .skip(cur + 1)
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
    }

    fn prev_block(doc: &crate::buffer::Document, cur: usize) -> Option<usize> {
        doc.segments()
            .iter()
            .enumerate()
            .take(cur)
            .rev()
            .find_map(|(i, s)| matches!(s, Segment::Block(_)).then_some(i))
    }

    #[test]
    fn jump_next_block_walks_forward_no_wrap() {
        let doc = doc_with_two_blocks();
        let blocks = block_indices(&doc);
        assert_eq!(blocks.len(), 2);
        // From the first prose segment (idx 0) → first block.
        assert_eq!(next_block(&doc, 0), Some(blocks[0]));
        // From the first block → second block.
        assert_eq!(next_block(&doc, blocks[0]), Some(blocks[1]));
        // From the second block → no more (no wrap).
        assert_eq!(next_block(&doc, blocks[1]), None);
    }

    #[test]
    fn jump_prev_block_walks_backward_no_wrap() {
        let doc = doc_with_two_blocks();
        let blocks = block_indices(&doc);
        // From the second block → first block.
        assert_eq!(prev_block(&doc, blocks[1]), Some(blocks[0]));
        // From the first block → no earlier block.
        assert_eq!(prev_block(&doc, blocks[0]), None);
        // From the last prose segment → previous block.
        let last = doc.segments().len() - 1;
        assert_eq!(prev_block(&doc, last), Some(blocks[1]));
    }

    #[test]
    fn jump_block_no_blocks_yields_none() {
        let md = "just prose\n\nno blocks at all\n";
        let doc = crate::buffer::Document::from_markdown(md).unwrap();
        assert_eq!(next_block(&doc, 0), None);
        assert_eq!(prev_block(&doc, 0), None);
    }
}
