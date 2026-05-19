//! `.` repeat — replay the last recorded change. Mechanically moved
//! out of `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p6-replay) with
//! no logic change. The bodies are copied verbatim; `apply_action`
//! and the operator appliers are reached via their canonical module
//! paths instead of the old bare call sites.

use crate::app::App;
use crate::input::action::Action;
use crate::input::apply::operator::{
    apply_op_linewise, apply_op_motion, apply_op_textobject, apply_paste,
};
use crate::input::types::{InsertPos, Operator};
use crate::vim::change::ChangeRecord;
use crate::vim::dispatch::apply_action;

// ───────────── . repeat ─────────────

pub(crate) fn replay_last_change(app: &mut App, count: usize) {
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
