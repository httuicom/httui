//! Per-key handlers for the git-specific modals. Built on top of
//! `super::handlers::*` primitives.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::input::action::Action;

use super::handlers::{list_picker_key, ListPickerKey};
use super::ModalOutcome;

/// j/k/arrows navigate commits; Esc closes; PageUp/PageDown scroll
/// the diff pane.
pub(super) fn log_page_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::CloseGitLogPage),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => ModalOutcome::Emit(Action::CloseGitLogPage),
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            ModalOutcome::Emit(Action::MoveGitLogPageCursor(1))
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            ModalOutcome::Emit(Action::MoveGitLogPageCursor(-1))
        }
        (_, KeyCode::PageDown) => ModalOutcome::Emit(Action::ScrollGitLogDiff(10)),
        (_, KeyCode::PageUp) => ModalOutcome::Emit(Action::ScrollGitLogDiff(-10)),
        (KeyModifiers::CONTROL, KeyCode::Char('d')) => {
            ModalOutcome::Emit(Action::ScrollGitLogDiff(10))
        }
        (KeyModifiers::CONTROL, KeyCode::Char('u')) => {
            ModalOutcome::Emit(Action::ScrollGitLogDiff(-10))
        }
        _ => ModalOutcome::Continue,
    }
}

/// j/k/arrows navigate; Enter checks out; Esc closes.
pub(super) fn branch_picker_handle_key(key: KeyEvent) -> ModalOutcome {
    match list_picker_key(key) {
        ListPickerKey::Up => ModalOutcome::Emit(Action::MoveGitBranchPickerCursor(-1)),
        ListPickerKey::Down => ModalOutcome::Emit(Action::MoveGitBranchPickerCursor(1)),
        ListPickerKey::Cancel => ModalOutcome::Emit(Action::CloseGitBranchPicker),
        ListPickerKey::Confirm => ModalOutcome::Emit(Action::ConfirmGitBranchPicker),
        ListPickerKey::Other => ModalOutcome::Continue,
    }
}

/// 3-way conflict resolver: j/k move file cursor; 1/2/3 pick the
/// version (base/ours/theirs) and apply it; Esc closes.
pub(super) fn conflict_resolver_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::CloseGitConflictResolver),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::CloseGitConflictResolver)
        }
        (_, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            ModalOutcome::Emit(Action::MoveGitConflictResolverFile(1))
        }
        (_, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            ModalOutcome::Emit(Action::MoveGitConflictResolverFile(-1))
        }
        (KeyModifiers::NONE, KeyCode::Char('1')) => ModalOutcome::Emit(Action::ResolveGitConflict(
            crate::git::ConflictVersion::Base,
        )),
        (KeyModifiers::NONE, KeyCode::Char('2')) => ModalOutcome::Emit(Action::ResolveGitConflict(
            crate::git::ConflictVersion::Ours,
        )),
        (KeyModifiers::NONE, KeyCode::Char('3')) => ModalOutcome::Emit(Action::ResolveGitConflict(
            crate::git::ConflictVersion::Theirs,
        )),
        _ => ModalOutcome::Continue,
    }
}

/// `y` / `Enter` → confirm push -u; `n` / `Esc` / `Ctrl-C` → cancel.
pub(super) fn set_upstream_confirm_handle_key(key: KeyEvent) -> ModalOutcome {
    let KeyEvent {
        code, modifiers, ..
    } = key;
    match (modifiers, code) {
        (_, KeyCode::Esc) => ModalOutcome::Emit(Action::GitCancelSetUpstream),
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => {
            ModalOutcome::Emit(Action::GitCancelSetUpstream)
        }
        (KeyModifiers::NONE, KeyCode::Char('n')) | (KeyModifiers::NONE, KeyCode::Char('N')) => {
            ModalOutcome::Emit(Action::GitCancelSetUpstream)
        }
        (KeyModifiers::NONE, KeyCode::Char('y'))
        | (KeyModifiers::NONE, KeyCode::Char('Y'))
        | (_, KeyCode::Enter) => ModalOutcome::Emit(Action::GitConfirmSetUpstream),
        _ => ModalOutcome::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn key_mod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }
    fn emitted(outcome: ModalOutcome) -> Action {
        match outcome {
            ModalOutcome::Emit(a) => a,
            other => panic!("expected Emit, got {other:?}"),
        }
    }

    // ---- log_page_handle_key ----------------------------------------

    #[test]
    fn log_page_esc_and_ctrl_c_close() {
        assert_eq!(
            emitted(log_page_handle_key(key(KeyCode::Esc))),
            Action::CloseGitLogPage,
        );
        assert_eq!(
            emitted(log_page_handle_key(key_mod(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL
            ))),
            Action::CloseGitLogPage,
        );
    }

    #[test]
    fn log_page_jk_and_arrows_move_cursor() {
        assert_eq!(
            emitted(log_page_handle_key(key(KeyCode::Down))),
            Action::MoveGitLogPageCursor(1),
        );
        assert_eq!(
            emitted(log_page_handle_key(key(KeyCode::Char('j')))),
            Action::MoveGitLogPageCursor(1),
        );
        assert_eq!(
            emitted(log_page_handle_key(key(KeyCode::Up))),
            Action::MoveGitLogPageCursor(-1),
        );
        assert_eq!(
            emitted(log_page_handle_key(key(KeyCode::Char('k')))),
            Action::MoveGitLogPageCursor(-1),
        );
    }

    #[test]
    fn log_page_paging_keys_scroll_diff() {
        assert_eq!(
            emitted(log_page_handle_key(key(KeyCode::PageDown))),
            Action::ScrollGitLogDiff(10),
        );
        assert_eq!(
            emitted(log_page_handle_key(key(KeyCode::PageUp))),
            Action::ScrollGitLogDiff(-10),
        );
        assert_eq!(
            emitted(log_page_handle_key(key_mod(
                KeyCode::Char('d'),
                KeyModifiers::CONTROL
            ))),
            Action::ScrollGitLogDiff(10),
        );
        assert_eq!(
            emitted(log_page_handle_key(key_mod(
                KeyCode::Char('u'),
                KeyModifiers::CONTROL
            ))),
            Action::ScrollGitLogDiff(-10),
        );
    }

    #[test]
    fn log_page_unknown_keys_continue() {
        assert!(matches!(
            log_page_handle_key(key(KeyCode::Char('z'))),
            ModalOutcome::Continue
        ));
    }

    // ---- branch_picker_handle_key -----------------------------------

    #[test]
    fn branch_picker_routes_list_keys() {
        assert_eq!(
            emitted(branch_picker_handle_key(key(KeyCode::Up))),
            Action::MoveGitBranchPickerCursor(-1),
        );
        assert_eq!(
            emitted(branch_picker_handle_key(key(KeyCode::Down))),
            Action::MoveGitBranchPickerCursor(1),
        );
        assert_eq!(
            emitted(branch_picker_handle_key(key(KeyCode::Esc))),
            Action::CloseGitBranchPicker,
        );
        assert_eq!(
            emitted(branch_picker_handle_key(key(KeyCode::Enter))),
            Action::ConfirmGitBranchPicker,
        );
    }

    #[test]
    fn branch_picker_unknown_continues() {
        assert!(matches!(
            branch_picker_handle_key(key(KeyCode::Char('z'))),
            ModalOutcome::Continue
        ));
    }

    // ---- conflict_resolver_handle_key -------------------------------

    #[test]
    fn conflict_resolver_close_keys() {
        assert_eq!(
            emitted(conflict_resolver_handle_key(key(KeyCode::Esc))),
            Action::CloseGitConflictResolver,
        );
        assert_eq!(
            emitted(conflict_resolver_handle_key(key_mod(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL
            ))),
            Action::CloseGitConflictResolver,
        );
    }

    #[test]
    fn conflict_resolver_file_cursor() {
        assert_eq!(
            emitted(conflict_resolver_handle_key(key(KeyCode::Down))),
            Action::MoveGitConflictResolverFile(1),
        );
        assert_eq!(
            emitted(conflict_resolver_handle_key(key(KeyCode::Char('j')))),
            Action::MoveGitConflictResolverFile(1),
        );
        assert_eq!(
            emitted(conflict_resolver_handle_key(key(KeyCode::Up))),
            Action::MoveGitConflictResolverFile(-1),
        );
        assert_eq!(
            emitted(conflict_resolver_handle_key(key(KeyCode::Char('k')))),
            Action::MoveGitConflictResolverFile(-1),
        );
    }

    #[test]
    fn conflict_resolver_numeric_keys_pick_version() {
        assert_eq!(
            emitted(conflict_resolver_handle_key(key(KeyCode::Char('1')))),
            Action::ResolveGitConflict(crate::git::ConflictVersion::Base),
        );
        assert_eq!(
            emitted(conflict_resolver_handle_key(key(KeyCode::Char('2')))),
            Action::ResolveGitConflict(crate::git::ConflictVersion::Ours),
        );
        assert_eq!(
            emitted(conflict_resolver_handle_key(key(KeyCode::Char('3')))),
            Action::ResolveGitConflict(crate::git::ConflictVersion::Theirs),
        );
    }

    #[test]
    fn conflict_resolver_unknown_continues() {
        assert!(matches!(
            conflict_resolver_handle_key(key(KeyCode::Char('9'))),
            ModalOutcome::Continue
        ));
    }

    // ---- set_upstream_confirm_handle_key ----------------------------

    #[test]
    fn set_upstream_confirm_keys() {
        assert_eq!(
            emitted(set_upstream_confirm_handle_key(key(KeyCode::Esc))),
            Action::GitCancelSetUpstream,
        );
        assert_eq!(
            emitted(set_upstream_confirm_handle_key(key_mod(
                KeyCode::Char('c'),
                KeyModifiers::CONTROL
            ))),
            Action::GitCancelSetUpstream,
        );
        assert_eq!(
            emitted(set_upstream_confirm_handle_key(key(KeyCode::Char('n')))),
            Action::GitCancelSetUpstream,
        );
        assert_eq!(
            emitted(set_upstream_confirm_handle_key(key(KeyCode::Char('N')))),
            Action::GitCancelSetUpstream,
        );
        assert_eq!(
            emitted(set_upstream_confirm_handle_key(key(KeyCode::Char('y')))),
            Action::GitConfirmSetUpstream,
        );
        assert_eq!(
            emitted(set_upstream_confirm_handle_key(key(KeyCode::Char('Y')))),
            Action::GitConfirmSetUpstream,
        );
        assert_eq!(
            emitted(set_upstream_confirm_handle_key(key(KeyCode::Enter))),
            Action::GitConfirmSetUpstream,
        );
    }

    #[test]
    fn set_upstream_confirm_unknown_continues() {
        assert!(matches!(
            set_upstream_confirm_handle_key(key(KeyCode::Char('z'))),
            ModalOutcome::Continue
        ));
    }
}
