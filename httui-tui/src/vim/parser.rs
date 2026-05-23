// `vim::parser` is now a pure re-export facade (fase 1 p1–p3): all
// decoders moved under `crate::input::parser`. These raw imports have
// no production use left here, but the in-file `mod tests`
// (`use super::*`, NOT moved — owned by p6) still resolves them, so
// they stay in scope behind `#[allow(unused_imports)]`.
#[allow(unused_imports)]
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[allow(unused_imports)]
use crate::vim::mode::Mode;
#[allow(unused_imports)]
use crate::vim::state::VimState;
// `FindKind` no longer used in production here (moved with the keymap
// helpers to `crate::input::parser` in fase 1 p2); kept in scope so the
// in-file `mod tests` (`use super::*`) keeps resolving it unchanged.
#[allow(unused_imports)]
use crate::vim::state::FindKind;

// `LineEdit` lives in `vim/lineedit.rs`; `parse_lineedit_prompt` below
// reuses it across cmdline / search / quickopen prompts.
#[allow(unused_imports)]
use crate::vim::lineedit::LineEdit;

// Pure types + the Action set live in `crate::input`; this facade
// re-exports them so the ~7 external `crate::vim::parser::{...}`
// call sites and the in-file `mod tests` (`use super::*`) keep
// resolving unchanged (tui-v2 vertical 1, fase 1 p1). `Action` lost
// its last production consumer when `vim::dispatch` became a facade
// and `crate::input::dispatch` started importing `Action` from its
// canonical `crate::input::action` path (fase 1 p6-router); the
// in-file `mod tests` (`use super::*`) still references it, so it
// stays re-exported behind `#[allow(unused_imports)]`.
#[allow(unused_imports)]
pub use crate::input::action::Action;
pub use crate::input::types::{
    InsertPos, Motion, MotionClass, Operator, PastePos, ScrollPos, TextObject,
};
// `WindowCmd` lost its last production consumer when the window/split
// appliers moved to `crate::input::apply::window` (fase 1 p5a). The
// in-file `mod tests` (`use super::*`) still references it, so it stays
// re-exported behind `#[allow(unused_imports)]`.
#[allow(unused_imports)]
pub use crate::input::types::WindowCmd;

// Keymap helper primitives + per-mode decoders now all live under
// `crate::input::parser` (fase 1 p2/p3); `vim::parser` keeps only the
// `pub use` facades + the in-file `mod tests` until p6.

// `parse_normal` now lives in `crate::input::parser::normal`;
// re-exported so external callers (`vim::dispatch`) and the
// in-file `mod tests` keep resolving it (tui-v2 vertical 1, fase 1 p3a).
pub use crate::input::parser::normal::parse_normal;

// `parse_visual` now lives in `crate::input::parser::visual`;
// re-exported so `vim::dispatch` and the in-file `mod tests` keep
// resolving it (tui-v2 vertical 1, fase 1 p3c).
pub use crate::input::parser::visual::parse_visual;

// Line-edit prompt decoders now live in
// `crate::input::parser::lineedit`; re-exported so `vim::dispatch`
// and the in-file `mod tests` keep resolving them (tui-v2 vertical 1, fase 1 p3b).
pub use crate::input::parser::lineedit::{
    parse_cmdline, parse_fence_edit, parse_quickopen, parse_search, parse_tree, parse_tree_prompt,
};

// Modal / picker decoders now live in
// `crate::input::parser::modals`; re-exported so `vim::dispatch`
// and the in-file `mod tests` keep resolving them (tui-v2 vertical 1, fase 1 p3d).
pub use crate::input::parser::modals::{
    parse_block_history, parse_block_template_picker, parse_connection_picker,
    parse_content_search, parse_db_export_picker, parse_db_row_detail,
    parse_db_settings_modal, parse_environment_picker, parse_http_response_detail,
    parse_tab_picker,
};

// `parse_insert` now lives in `crate::input::parser::insert`.
pub use crate::input::parser::insert::parse_insert;

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    fn key_ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn h_l_j_k_are_motions() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('h'))),
            Action::Motion(Motion::Left, 1)
        );
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('j'))),
            Action::Motion(Motion::Down, 1)
        );
    }

    #[test]
    fn count_prefix_amplifies() {
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('5'))), Action::Noop);
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('j'))),
            Action::Motion(Motion::Down, 5)
        );
    }

    #[test]
    fn multi_digit_count() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('1')));
        parse_normal(&mut s, key(KeyCode::Char('2')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('w'))),
            Action::Motion(Motion::WordForward, 12)
        );
    }

    #[test]
    fn lone_zero_is_line_start() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('0'))),
            Action::Motion(Motion::LineStart, 1)
        );
    }

    #[test]
    fn zero_after_count_extends_count() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('1')));
        parse_normal(&mut s, key(KeyCode::Char('0')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('j'))),
            Action::Motion(Motion::Down, 10)
        );
    }

    #[test]
    fn gg_is_doc_start() {
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('g'))), Action::Noop);
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('g'))),
            Action::Motion(Motion::DocStart, 1)
        );
    }

    #[test]
    fn count_g_g_is_goto_line() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('5')));
        parse_normal(&mut s, key(KeyCode::Char('g')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('g'))),
            Action::Motion(Motion::GotoLine(5), 1)
        );
    }

    #[test]
    fn capital_g_is_doc_end() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('G'))),
            Action::Motion(Motion::DocEnd, 1)
        );
    }

    #[test]
    fn count_capital_g_is_goto_line() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('1')));
        parse_normal(&mut s, key(KeyCode::Char('2')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('G'))),
            Action::Motion(Motion::GotoLine(12), 1)
        );
    }

    #[test]
    fn ctrl_d_u_half_page() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('d'))),
            Action::Motion(Motion::HalfPageDown, 1)
        );
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('u'))),
            Action::Motion(Motion::HalfPageUp, 1)
        );
    }

    #[test]
    fn enter_insert_variants() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('i'))),
            Action::EnterInsert(InsertPos::Current)
        );
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('a'))),
            Action::EnterInsert(InsertPos::After)
        );
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('I'))),
            Action::EnterInsert(InsertPos::LineStart)
        );
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('A'))),
            Action::EnterInsert(InsertPos::LineEnd)
        );
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('o'))),
            Action::EnterInsert(InsertPos::LineBelow)
        );
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('O'))),
            Action::EnterInsert(InsertPos::LineAbove)
        );
    }

    #[test]
    fn r_in_normal_emits_run_block() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('r'))),
            Action::RunBlock
        );
    }

    #[test]
    fn v_in_normal_emits_enter_visual() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('v'))),
            Action::EnterVisual
        );
    }

    #[test]
    fn capital_v_emits_enter_visual_line() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('V'))),
            Action::EnterVisualLine
        );
    }

    #[test]
    fn parse_visual_motion_extends_selection() {
        let mut s = VimState::new();
        s.mode = Mode::Visual;
        assert_eq!(
            parse_visual(&mut s, key(KeyCode::Char('l'))),
            Action::Motion(Motion::Right, 1)
        );
        assert_eq!(
            parse_visual(&mut s, key(KeyCode::Char('w'))),
            Action::Motion(Motion::WordForward, 1)
        );
    }

    #[test]
    fn parse_visual_d_yanks_into_operator() {
        let mut s = VimState::new();
        s.mode = Mode::Visual;
        assert_eq!(
            parse_visual(&mut s, key(KeyCode::Char('d'))),
            Action::VisualOperator(Operator::Delete)
        );
        assert_eq!(
            parse_visual(&mut s, key(KeyCode::Char('y'))),
            Action::VisualOperator(Operator::Yank)
        );
        assert_eq!(
            parse_visual(&mut s, key(KeyCode::Char('c'))),
            Action::VisualOperator(Operator::Change)
        );
    }

    #[test]
    fn parse_visual_o_swaps() {
        let mut s = VimState::new();
        s.mode = Mode::Visual;
        assert_eq!(
            parse_visual(&mut s, key(KeyCode::Char('o'))),
            Action::VisualSwap
        );
    }

    #[test]
    fn parse_visual_text_object_chord() {
        // `va{` — `a` sets the text-object pending flag; `{` resolves
        // to a Pair around=true and emits VisualSelectTextObject.
        let mut s = VimState::new();
        s.mode = Mode::Visual;
        assert_eq!(parse_visual(&mut s, key(KeyCode::Char('a'))), Action::Noop);
        assert!(s.pending_textobj_inner == Some(false));
        let action = parse_visual(&mut s, key(KeyCode::Char('{')));
        assert!(
            matches!(
                action,
                Action::VisualSelectTextObject(TextObject::Pair {
                    open: '{',
                    close: '}',
                    around: true
                })
            ),
            "expected VisualSelectTextObject(Pair around), got {action:?}"
        );
        assert!(s.pending_textobj_inner.is_none());
    }

    #[test]
    fn parse_visual_inner_text_object_chord() {
        // `vi"` — `i` flags inner; `"` resolves to a Quote inner.
        let mut s = VimState::new();
        s.mode = Mode::Visual;
        assert_eq!(parse_visual(&mut s, key(KeyCode::Char('i'))), Action::Noop);
        let action = parse_visual(&mut s, key(KeyCode::Char('"')));
        assert!(
            matches!(
                action,
                Action::VisualSelectTextObject(TextObject::Quote {
                    delim: '"',
                    around: false
                })
            ),
            "expected VisualSelectTextObject(Quote inner), got {action:?}"
        );
    }

    #[test]
    fn parse_visual_v_exits_charwise() {
        let mut s = VimState::new();
        s.mode = Mode::Visual;
        assert_eq!(
            parse_visual(&mut s, key(KeyCode::Char('v'))),
            Action::ExitVisual
        );
    }

    #[test]
    fn parse_visual_capital_v_exits_linewise() {
        let mut s = VimState::new();
        s.mode = Mode::VisualLine;
        assert_eq!(
            parse_visual(&mut s, key(KeyCode::Char('V'))),
            Action::ExitVisual
        );
    }

    #[test]
    fn parse_visual_esc_exits() {
        let mut s = VimState::new();
        s.mode = Mode::Visual;
        assert_eq!(parse_visual(&mut s, key(KeyCode::Esc)), Action::ExitVisual);
    }

    #[test]
    fn ctrl_w_v_splits_vertical() {
        let mut s = VimState::new();
        // First `Ctrl+W` arms the prefix without emitting an action.
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('w'))),
            Action::Noop
        );
        assert!(s.pending_window);
        // The suffix resolves to a window command and clears the flag.
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('v'))),
            Action::Window(WindowCmd::SplitVertical)
        );
        assert!(!s.pending_window);
    }

    #[test]
    fn ctrl_w_hjkl_focus_moves() {
        for (suffix, expected) in [
            ('h', WindowCmd::FocusLeft),
            ('l', WindowCmd::FocusRight),
            ('k', WindowCmd::FocusUp),
            ('j', WindowCmd::FocusDown),
        ] {
            let mut s = VimState::new();
            parse_normal(&mut s, key_ctrl(KeyCode::Char('w')));
            assert_eq!(
                parse_normal(&mut s, key(KeyCode::Char(suffix))),
                Action::Window(expected),
            );
        }
    }

    #[test]
    fn ctrl_w_close_alias_q() {
        let mut s = VimState::new();
        parse_normal(&mut s, key_ctrl(KeyCode::Char('w')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('q'))),
            Action::Window(WindowCmd::Close)
        );
        let mut s = VimState::new();
        parse_normal(&mut s, key_ctrl(KeyCode::Char('w')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('c'))),
            Action::Window(WindowCmd::Close)
        );
    }

    #[test]
    fn ctrl_w_ctrl_w_cycles() {
        let mut s = VimState::new();
        parse_normal(&mut s, key_ctrl(KeyCode::Char('w')));
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('w'))),
            Action::Window(WindowCmd::Cycle)
        );
    }

    #[test]
    fn ctrl_w_unknown_suffix_clears_prefix() {
        let mut s = VimState::new();
        parse_normal(&mut s, key_ctrl(KeyCode::Char('w')));
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('z'))), Action::Noop);
        assert!(!s.pending_window);
        // After cancellation, normal motions resume immediately.
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('h'))),
            Action::Motion(Motion::Left, 1)
        );
    }

    #[test]
    fn ctrl_c_quits_in_normal() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('c'))),
            Action::Quit
        );
    }

    #[test]
    fn lowercase_q_is_no_longer_quit() {
        let mut s = VimState::new();
        // `q` is reserved (macros, future features). Quitting goes
        // through `:q` since round 2.
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('q'))), Action::Noop);
    }

    #[test]
    fn colon_enters_cmdline() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char(':'))),
            Action::EnterCmdline
        );
    }

    #[test]
    fn cmdline_chars_and_specials() {
        assert_eq!(
            parse_cmdline(key(KeyCode::Char('w'))),
            Action::CmdlineChar('w')
        );
        assert_eq!(
            parse_cmdline(key(KeyCode::Backspace)),
            Action::CmdlineBackspace
        );
        assert_eq!(parse_cmdline(key(KeyCode::Enter)), Action::CmdlineExecute);
        assert_eq!(parse_cmdline(key(KeyCode::Esc)), Action::CmdlineCancel);
        assert_eq!(
            parse_cmdline(key_ctrl(KeyCode::Char('c'))),
            Action::CmdlineCancel
        );
    }

    // ─── operator pending ───

    #[test]
    fn d_then_w_emits_operator_motion() {
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('d'))), Action::Noop);
        assert!(s.pending_operator.is_some());
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('w'))),
            Action::OperatorMotion(Operator::Delete, Motion::WordForward, 1)
        );
        assert!(s.pending_operator.is_none());
    }

    #[test]
    fn dd_emits_linewise_shortcut() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('d')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('d'))),
            Action::OperatorLinewise(Operator::Delete, 1)
        );
    }

    #[test]
    fn count_then_dd_multiplies() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('3')));
        parse_normal(&mut s, key(KeyCode::Char('d')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('d'))),
            Action::OperatorLinewise(Operator::Delete, 3)
        );
    }

    #[test]
    fn d3w_multiplies_counts() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('d')));
        parse_normal(&mut s, key(KeyCode::Char('3')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('w'))),
            Action::OperatorMotion(Operator::Delete, Motion::WordForward, 3)
        );
    }

    #[test]
    fn shorthand_x_is_delete_right() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('x'))),
            Action::OperatorMotion(Operator::Delete, Motion::Right, 1)
        );
    }

    #[test]
    fn ctrl_x_explains_focused_block() {
        // `<C-x>` is wired to EXPLAIN against the DB block at the
        // cursor. Plain `x` stays bound to delete-right (covered
        // above) — only the CONTROL modifier changes the action.
        // Replaces the old `:explain` ex command (per project
        // directive: keymap > ex command).
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('x'))),
            Action::ExplainBlock
        );
    }

    #[test]
    fn gd_cycles_focused_block_display_mode() {
        // `gd` chord — first `g` arms the prefix and is a no-op,
        // second `d` resolves to the display-mode cycle action. Uses
        // the same `pending_g` plumbing as `gg`/`gt`/`gT`.
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('g'))), Action::Noop);
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('d'))),
            Action::CycleDisplayMode
        );
    }

    #[test]
    fn ga_opens_alias_edit_prompt() {
        // `ga` chord opens the inline alias-edit popup for the
        // focused block. Picked over `<C-a>` because that's a tmux
        // prefix some users bind — letting tmux see the keystroke
        // before we do is the right call.
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('g'))), Action::Noop);
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('a'))),
            Action::OpenFenceEditAlias
        );
    }

    #[test]
    fn connection_picker_capital_d_deletes() {
        // Capital `D` triggers DeleteConnectionInPicker; lowercase
        // `d` would conflict with vim's `dd` reflex (not bound here
        // but easy to fat-finger) and is left as a no-op so the
        // user has to type the explicit capital.
        use crossterm::event::{KeyEvent, KeyModifiers};
        let mk = |mods, code| KeyEvent::new(code, mods);
        assert_eq!(
            parse_connection_picker(mk(KeyModifiers::SHIFT, KeyCode::Char('D'))),
            Action::DeleteConnectionInPicker,
        );
        assert_eq!(
            parse_connection_picker(mk(KeyModifiers::NONE, KeyCode::Char('D'))),
            Action::DeleteConnectionInPicker,
        );
        // Lowercase `d` MUST be a no-op — no accidental delete.
        assert_eq!(
            parse_connection_picker(mk(KeyModifiers::NONE, KeyCode::Char('d'))),
            Action::Noop,
        );
        // Ctrl-D would compose with HalfPageDown semantics elsewhere
        // — picker shouldn't surface delete on it.
        assert_eq!(
            parse_connection_picker(mk(KeyModifiers::CONTROL, KeyCode::Char('D'))),
            Action::Noop,
        );
    }

    #[test]
    fn ctrl_f_opens_content_search() {
        // <C-f> — global Find content. Bound on the normal-mode
        // shortcut layer (not via g-prefix) because it competes
        // with Quick-Open's <C-p> as a top-level finder.
        use crossterm::event::{KeyEvent, KeyModifiers};
        let mut s = VimState::new();
        let key = KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL);
        assert_eq!(parse_normal(&mut s, key), Action::OpenContentSearch);
    }

    #[test]
    fn content_search_routes_navigation_and_typing() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        let mk = |mods, code| KeyEvent::new(code, mods);
        // Selection nav: arrows + Ctrl-n/p only.
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Down)),
            Action::MoveContentSearchCursor(1),
        );
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Up)),
            Action::MoveContentSearchCursor(-1),
        );
        assert_eq!(
            parse_content_search(mk(KeyModifiers::CONTROL, KeyCode::Char('n'))),
            Action::MoveContentSearchCursor(1),
        );
        // j/k go INTO the buffer (FTS5 query can carry literal j/k).
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Char('j'))),
            Action::ContentSearchChar('j'),
        );
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Char('k'))),
            Action::ContentSearchChar('k'),
        );
        // Esc/Ctrl-C close; Enter confirms.
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Esc)),
            Action::CloseContentSearch,
        );
        assert_eq!(
            parse_content_search(mk(KeyModifiers::CONTROL, KeyCode::Char('c'))),
            Action::CloseContentSearch,
        );
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Enter)),
            Action::ConfirmContentSearch,
        );
        // LineEdit ops.
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Backspace)),
            Action::ContentSearchBackspace,
        );
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Left)),
            Action::ContentSearchCursorLeft,
        );
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Right)),
            Action::ContentSearchCursorRight,
        );
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::Home)),
            Action::ContentSearchCursorHome,
        );
        assert_eq!(
            parse_content_search(mk(KeyModifiers::NONE, KeyCode::End)),
            Action::ContentSearchCursorEnd,
        );
    }

    #[test]
    fn ctrl_shift_c_copies_as_curl() {
        // <C-S-c> express path — bypasses the gx picker. Both the
        // SHIFT-folded encoding (CTRL+SHIFT+'C') and the bare
        // CTRL+'C' fallback some terminals send map to CopyAsCurl.
        // A plain CTRL+'c' (lowercase) is NOT this chord — that
        // stays as the cancel intercept at dispatch top-level.
        use crossterm::event::{KeyEvent, KeyModifiers};
        let mut s = VimState::new();
        let shift_folded = KeyEvent::new(
            KeyCode::Char('C'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT,
        );
        assert_eq!(parse_normal(&mut s, shift_folded), Action::CopyAsCurl);

        let mut s = VimState::new();
        let bare_upper = KeyEvent::new(KeyCode::Char('C'), KeyModifiers::CONTROL);
        assert_eq!(parse_normal(&mut s, bare_upper), Action::CopyAsCurl);

        // Plain <C-c> (lowercase) is reserved for cancel semantics
        // at the dispatch level — must NOT route to CopyAsCurl.
        let mut s = VimState::new();
        let cancel = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_ne!(parse_normal(&mut s, cancel), Action::CopyAsCurl);
    }

    #[test]
    fn gh_opens_block_history() {
        // `gh` chord — sibling of gd/ga/gx/gs. Read-only modal that
        // lists the focused HTTP block's recent runs. Validation
        // (HTTP-only + aliased + has-rows) is in the dispatch
        // handler, not the parser.
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('g'))), Action::Noop);
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('h'))),
            Action::OpenBlockHistory
        );
    }

    #[test]
    fn block_history_navigation_keys() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        let mk = |mods, code| KeyEvent::new(code, mods);

        // j/k + arrows + Ctrl-n/p navigate.
        assert_eq!(
            parse_block_history(mk(KeyModifiers::NONE, KeyCode::Char('j'))),
            Action::MoveBlockHistoryCursor(1),
        );
        assert_eq!(
            parse_block_history(mk(KeyModifiers::NONE, KeyCode::Char('k'))),
            Action::MoveBlockHistoryCursor(-1),
        );
        assert_eq!(
            parse_block_history(mk(KeyModifiers::CONTROL, KeyCode::Char('n'))),
            Action::MoveBlockHistoryCursor(1),
        );
        assert_eq!(
            parse_block_history(mk(KeyModifiers::CONTROL, KeyCode::Char('p'))),
            Action::MoveBlockHistoryCursor(-1),
        );
        // Esc / Ctrl-C close.
        assert_eq!(
            parse_block_history(mk(KeyModifiers::NONE, KeyCode::Esc)),
            Action::CloseBlockHistory,
        );
        assert_eq!(
            parse_block_history(mk(KeyModifiers::CONTROL, KeyCode::Char('c'))),
            Action::CloseBlockHistory,
        );
        // Enter is NOT bound — V1 modal is view-only. Anything
        // unbound is a no-op so a stray keystroke can't leak
        // through to the editor underneath.
        assert_eq!(
            parse_block_history(mk(KeyModifiers::NONE, KeyCode::Enter)),
            Action::Noop,
        );
        assert_eq!(
            parse_block_history(mk(KeyModifiers::NONE, KeyCode::Char('x'))),
            Action::Noop,
        );
    }

    #[test]
    fn gx_opens_export_picker() {
        // `gx` chord — sibling of `gd` (display) and `ga` (alias).
        // Sidesteps `<C-x>` (already bound to ExplainBlock) and `y`
        // (yank chord prefix). Same `pending_g` plumbing.
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('g'))), Action::Noop);
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('x'))),
            Action::OpenDbExportPicker
        );
    }

    #[test]
    fn gs_opens_settings_modal() {
        // `gs` chord — sibling of gd/ga/gx. Stays in the g-prefix
        // family per the chord-constraints memory; one chord opens
        // a multi-input modal instead of one chord per field.
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('g'))), Action::Noop);
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('s'))),
            Action::OpenDbSettingsModal
        );
    }

    #[test]
    fn settings_modal_routes_navigation_and_typing() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        let mk = |mods, code| KeyEvent::new(code, mods);

        // Tab/BackTab + arrows cycle focused field.
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Tab)),
            Action::DbSettingsFocusNext,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::BackTab)),
            Action::DbSettingsFocusPrev,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Down)),
            Action::DbSettingsFocusNext,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Up)),
            Action::DbSettingsFocusPrev,
        );
        // Enter saves; Esc / Ctrl-C cancel.
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Enter)),
            Action::ConfirmDbSettingsModal,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Esc)),
            Action::CloseDbSettingsModal,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::CONTROL, KeyCode::Char('c'))),
            Action::CloseDbSettingsModal,
        );
        // Plain digit goes into the focused buffer.
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Char('5'))),
            Action::DbSettingsChar('5'),
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::SHIFT, KeyCode::Char('A'))),
            Action::DbSettingsChar('A'),
        );
        // Control + char rejected — terminal-emulator chord like
        // <C-x> shouldn't accidentally land in the input buffer.
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::CONTROL, KeyCode::Char('x'))),
            Action::Noop,
        );
        // LineEdit ops route through dedicated actions.
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Backspace)),
            Action::DbSettingsBackspace,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Delete)),
            Action::DbSettingsDelete,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Left)),
            Action::DbSettingsCursorLeft,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Right)),
            Action::DbSettingsCursorRight,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::Home)),
            Action::DbSettingsCursorHome,
        );
        assert_eq!(
            parse_db_settings_modal(mk(KeyModifiers::NONE, KeyCode::End)),
            Action::DbSettingsCursorEnd,
        );
    }

    #[test]
    fn export_picker_navigation_keys() {
        // Vertical-only: j/k, arrows, Ctrl-n/p all move; Enter
        // confirms; Esc/Ctrl-C close. Anything else is a no-op so
        // a stray motion can't leak through to the editor.
        use crossterm::event::{KeyEvent, KeyModifiers};
        let mk = |mods, code| KeyEvent::new(code, mods);

        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::NONE, KeyCode::Char('j'))),
            Action::MoveDbExportPickerCursor(1),
        );
        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::NONE, KeyCode::Char('k'))),
            Action::MoveDbExportPickerCursor(-1),
        );
        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::NONE, KeyCode::Down)),
            Action::MoveDbExportPickerCursor(1),
        );
        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::NONE, KeyCode::Up)),
            Action::MoveDbExportPickerCursor(-1),
        );
        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::CONTROL, KeyCode::Char('n'))),
            Action::MoveDbExportPickerCursor(1),
        );
        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::CONTROL, KeyCode::Char('p'))),
            Action::MoveDbExportPickerCursor(-1),
        );
        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::NONE, KeyCode::Enter)),
            Action::ConfirmDbExportPicker,
        );
        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::NONE, KeyCode::Esc)),
            Action::CloseDbExportPicker,
        );
        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::CONTROL, KeyCode::Char('c'))),
            Action::CloseDbExportPicker,
        );
        // A motion key (e.g. `h`) is a no-op — must not leak into
        // the editor while the picker is up.
        assert_eq!(
            parse_db_export_picker(mk(KeyModifiers::NONE, KeyCode::Char('h'))),
            Action::Noop,
        );
    }

    #[test]
    fn parse_fence_edit_routes_typeable_chars() {
        // Plain typing (no CONTROL) goes into the input buffer; the
        // dispatch arm appends each char to `LineEdit`. Mirrors the
        // tree-prompt behavior so users get the same feel.
        assert_eq!(
            parse_fence_edit(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Action::FenceEditChar('q')
        );
    }

    #[test]
    fn parse_fence_edit_routes_control_keys() {
        // Enter / Esc / Backspace / Delete + the emacs-style cursor
        // shortcuts. Lists the surface so a future refactor of the
        // dispatch arms doesn't silently drop a binding.
        assert_eq!(
            parse_fence_edit(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Action::FenceEditConfirm
        );
        assert_eq!(
            parse_fence_edit(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Action::FenceEditCancel
        );
        assert_eq!(
            parse_fence_edit(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::FenceEditCancel
        );
        assert_eq!(
            parse_fence_edit(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            Action::FenceEditBackspace
        );
        assert_eq!(
            parse_fence_edit(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            Action::FenceEditCursorHome
        );
        assert_eq!(
            parse_fence_edit(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL)),
            Action::FenceEditCursorEnd
        );
    }

    #[test]
    fn gd_drops_stale_count_prefix() {
        // `5gd` shouldn't cycle five times — the count is meaningful
        // for `5gg` (goto line 5) but not for the per-press mode
        // cycle. We drain it instead of leaking it into the next
        // keystroke.
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('5')));
        parse_normal(&mut s, key(KeyCode::Char('g')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('d'))),
            Action::CycleDisplayMode
        );
        // Count drained — next plain `j` is a 1-step Down, not 5.
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('j'))),
            Action::Motion(Motion::Down, 1)
        );
    }

    #[test]
    fn shorthand_capital_d_is_delete_eol() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('D'))),
            Action::OperatorMotion(Operator::Delete, Motion::LineEnd, 1)
        );
    }

    #[test]
    fn shorthand_capital_y_is_yank_line() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('Y'))),
            Action::OperatorLinewise(Operator::Yank, 1)
        );
    }

    #[test]
    fn p_and_capital_p_are_paste() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('p'))),
            Action::Paste(PastePos::After, 1)
        );
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('P'))),
            Action::Paste(PastePos::Before, 1)
        );
    }

    #[test]
    fn ctrl_d_does_not_become_delete_operator() {
        let mut s = VimState::new();
        // Regression: with naive `d` matching, Ctrl+D would set
        // pending_operator instead of producing HalfPageDown.
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('d'))),
            Action::Motion(Motion::HalfPageDown, 1)
        );
        assert!(s.pending_operator.is_none());
    }

    #[test]
    fn esc_cancels_pending_operator() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('d')));
        assert!(s.pending_operator.is_some());
        parse_normal(&mut s, key(KeyCode::Esc));
        assert!(s.pending_operator.is_none());
    }

    // ─── text-object trigrams ───

    #[test]
    fn diw_emits_operator_text_object() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('d')));
        parse_normal(&mut s, key(KeyCode::Char('i')));
        assert!(s.pending_textobj_inner == Some(true));
        let action = parse_normal(&mut s, key(KeyCode::Char('w')));
        assert_eq!(
            action,
            Action::OperatorTextObject(Operator::Delete, TextObject::Word { around: false }, 1)
        );
        assert!(s.pending_operator.is_none());
        assert!(s.pending_textobj_inner.is_none());
    }

    #[test]
    fn ca_quote_emits_around_quote() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('c')));
        parse_normal(&mut s, key(KeyCode::Char('a')));
        let action = parse_normal(&mut s, key(KeyCode::Char('"')));
        assert_eq!(
            action,
            Action::OperatorTextObject(
                Operator::Change,
                TextObject::Quote {
                    delim: '"',
                    around: true,
                },
                1
            )
        );
    }

    #[test]
    fn yi_paren_emits_inner_pair() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('y')));
        parse_normal(&mut s, key(KeyCode::Char('i')));
        let action = parse_normal(&mut s, key(KeyCode::Char('(')));
        assert_eq!(
            action,
            Action::OperatorTextObject(
                Operator::Yank,
                TextObject::Pair {
                    open: '(',
                    close: ')',
                    around: false,
                },
                1
            )
        );
    }

    #[test]
    fn dib_aliases_to_paren_pair() {
        // `b` is vim's alias for `()`.
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('d')));
        parse_normal(&mut s, key(KeyCode::Char('i')));
        let action = parse_normal(&mut s, key(KeyCode::Char('b')));
        assert_eq!(
            action,
            Action::OperatorTextObject(
                Operator::Delete,
                TextObject::Pair {
                    open: '(',
                    close: ')',
                    around: false,
                },
                1
            )
        );
    }

    #[test]
    fn di_capital_b_aliases_to_brace_pair() {
        // `B` is vim's alias for `{}`.
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('d')));
        parse_normal(&mut s, key(KeyCode::Char('i')));
        let action = parse_normal(&mut s, key(KeyCode::Char('B')));
        assert_eq!(
            action,
            Action::OperatorTextObject(
                Operator::Delete,
                TextObject::Pair {
                    open: '{',
                    close: '}',
                    around: false,
                },
                1
            )
        );
    }

    #[test]
    fn unknown_text_object_target_cancels() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('d')));
        parse_normal(&mut s, key(KeyCode::Char('i')));
        let action = parse_normal(&mut s, key(KeyCode::Char('z')));
        assert_eq!(action, Action::Noop);
        assert!(s.pending_operator.is_none());
        assert!(s.pending_textobj_inner.is_none());
    }

    #[test]
    fn esc_during_text_object_prefix_cancels() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('d')));
        parse_normal(&mut s, key(KeyCode::Char('i')));
        parse_normal(&mut s, key(KeyCode::Esc));
        assert!(s.pending_operator.is_none());
        assert!(s.pending_textobj_inner.is_none());
    }

    #[test]
    fn standalone_i_still_enters_insert_when_no_operator() {
        let mut s = VimState::new();
        // Without a pending operator, `i` is the regular insert command.
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('i'))),
            Action::EnterInsert(InsertPos::Current)
        );
    }

    // ─── find / till ───

    #[test]
    fn f_then_char_emits_find_forward() {
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('f'))), Action::Noop);
        assert_eq!(s.pending_find_kind, Some(FindKind::F));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('o'))),
            Action::Motion(Motion::FindForward('o'), 1)
        );
        assert!(s.pending_find_kind.is_none());
    }

    #[test]
    fn capital_f_then_char_searches_backward() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('F')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('o'))),
            Action::Motion(Motion::FindBackward('o'), 1)
        );
    }

    #[test]
    fn t_and_capital_t_emit_till_motions() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('t')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('o'))),
            Action::Motion(Motion::TillForward('o'), 1)
        );
        parse_normal(&mut s, key(KeyCode::Char('T')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('o'))),
            Action::Motion(Motion::TillBackward('o'), 1)
        );
    }

    #[test]
    fn count_amplifies_find() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('3')));
        parse_normal(&mut s, key(KeyCode::Char('f')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('o'))),
            Action::Motion(Motion::FindForward('o'), 3)
        );
    }

    #[test]
    fn df_emits_operator_motion_with_find() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('d')));
        parse_normal(&mut s, key(KeyCode::Char('f')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('.'))),
            Action::OperatorMotion(Operator::Delete, Motion::FindForward('.'), 1)
        );
        assert!(s.pending_operator.is_none());
    }

    #[test]
    fn semicolon_repeats_last_find() {
        let mut s = VimState::new();
        s.last_find = Some(Motion::FindForward('o'));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char(';'))),
            Action::Motion(Motion::FindForward('o'), 1)
        );
    }

    #[test]
    fn comma_reverses_last_find() {
        let mut s = VimState::new();
        s.last_find = Some(Motion::FindForward('o'));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char(','))),
            Action::Motion(Motion::FindBackward('o'), 1)
        );
    }

    #[test]
    fn semicolon_with_no_history_is_noop() {
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char(';'))), Action::Noop);
    }

    #[test]
    fn esc_during_pending_find_cancels() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('f')));
        parse_normal(&mut s, key(KeyCode::Esc));
        assert!(s.pending_find_kind.is_none());
    }

    // ─── undo / redo / repeat ───

    #[test]
    fn u_emits_undo() {
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Char('u'))), Action::Undo);
    }

    #[test]
    fn ctrl_r_emits_redo() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('r'))),
            Action::Redo
        );
    }

    #[test]
    fn dot_emits_repeat_change() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('.'))),
            Action::RepeatChange(1)
        );
    }

    #[test]
    fn count_dot_repeats_n_times() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('5')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('.'))),
            Action::RepeatChange(5)
        );
    }

    // ─── search ───

    #[test]
    fn slash_enters_forward_search() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('/'))),
            Action::EnterSearch(true)
        );
    }

    #[test]
    fn question_enters_backward_search() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('?'))),
            Action::EnterSearch(false)
        );
    }

    #[test]
    fn n_repeats_search_capital_n_reverses() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('n'))),
            Action::SearchRepeat { reverse: false }
        );
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('N'))),
            Action::SearchRepeat { reverse: true }
        );
    }

    #[test]
    fn search_prompt_keys() {
        assert_eq!(
            parse_search(key(KeyCode::Char('a'))),
            Action::SearchChar('a')
        );
        assert_eq!(
            parse_search(key(KeyCode::Backspace)),
            Action::SearchBackspace
        );
        assert_eq!(parse_search(key(KeyCode::Enter)), Action::SearchExecute);
        assert_eq!(parse_search(key(KeyCode::Esc)), Action::SearchCancel);
    }

    // ─── quick open ───

    #[test]
    fn ctrl_p_enters_quick_open() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('p'))),
            Action::EnterQuickOpen
        );
    }

    #[test]
    fn lowercase_p_is_still_paste() {
        // Regression: Ctrl+P shouldn't shadow plain `p`.
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('p'))),
            Action::Paste(PastePos::After, 1)
        );
    }

    // ─── tree ───

    #[test]
    fn ctrl_e_toggles_tree() {
        let mut s = VimState::new();
        assert_eq!(
            parse_normal(&mut s, key_ctrl(KeyCode::Char('e'))),
            Action::TreeToggle
        );
    }

    #[test]
    fn tab_emits_focus_swap() {
        let mut s = VimState::new();
        assert_eq!(parse_normal(&mut s, key(KeyCode::Tab)), Action::FocusSwap);
    }

    #[test]
    fn tree_navigation_keys() {
        assert_eq!(parse_tree(key(KeyCode::Char('j'))), Action::TreeSelectNext);
        assert_eq!(parse_tree(key(KeyCode::Char('k'))), Action::TreeSelectPrev);
        assert_eq!(parse_tree(key(KeyCode::Char('g'))), Action::TreeSelectFirst);
        assert_eq!(parse_tree(key(KeyCode::Char('G'))), Action::TreeSelectLast);
        assert_eq!(parse_tree(key(KeyCode::Enter)), Action::TreeActivate);
        assert_eq!(parse_tree(key(KeyCode::Char('l'))), Action::TreeActivate);
        assert_eq!(parse_tree(key(KeyCode::Char('h'))), Action::TreeCollapse);
        assert_eq!(parse_tree(key(KeyCode::Char('R'))), Action::TreeRefresh);
        assert_eq!(parse_tree(key(KeyCode::Tab)), Action::FocusSwap);
        assert_eq!(parse_tree(key(KeyCode::Esc)), Action::FocusSwap);
        assert_eq!(parse_tree(key_ctrl(KeyCode::Char('e'))), Action::TreeToggle);
    }

    #[test]
    fn tree_shortcuts_for_file_ops() {
        assert_eq!(parse_tree(key(KeyCode::Char('a'))), Action::TreeCreate);
        assert_eq!(parse_tree(key(KeyCode::Char('r'))), Action::TreeRename);
        assert_eq!(parse_tree(key(KeyCode::Char('d'))), Action::TreeDelete);
        assert_eq!(parse_tree(key(KeyCode::Char('D'))), Action::TreeDelete);
    }

    // ─── tabs ───

    #[test]
    fn gt_emits_tab_next() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('g')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('t'))),
            Action::TabNext
        );
    }

    #[test]
    fn capital_gt_emits_tab_prev() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('g')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('T'))),
            Action::TabPrev
        );
    }

    #[test]
    fn count_gt_jumps_to_tab() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('3')));
        parse_normal(&mut s, key(KeyCode::Char('g')));
        assert_eq!(
            parse_normal(&mut s, key(KeyCode::Char('t'))),
            Action::TabGoto(3)
        );
    }

    #[test]
    fn quickopen_prompt_keys() {
        assert_eq!(
            parse_quickopen(key(KeyCode::Char('a'))),
            Action::QuickOpenChar('a')
        );
        assert_eq!(
            parse_quickopen(key(KeyCode::Backspace)),
            Action::QuickOpenBackspace
        );
        assert_eq!(
            parse_quickopen(key(KeyCode::Up)),
            Action::QuickOpenSelectPrev
        );
        assert_eq!(
            parse_quickopen(key(KeyCode::Down)),
            Action::QuickOpenSelectNext
        );
        assert_eq!(
            parse_quickopen(key_ctrl(KeyCode::Char('n'))),
            Action::QuickOpenSelectNext
        );
        assert_eq!(
            parse_quickopen(key_ctrl(KeyCode::Char('p'))),
            Action::QuickOpenSelectPrev
        );
        assert_eq!(
            parse_quickopen(key(KeyCode::Enter)),
            Action::QuickOpenExecute
        );
        assert_eq!(parse_quickopen(key(KeyCode::Esc)), Action::QuickOpenCancel);
    }

    #[test]
    fn insert_translates_chars_and_specials() {
        assert_eq!(
            parse_insert(key(KeyCode::Char('x'))),
            Action::InsertChar('x')
        );
        assert_eq!(parse_insert(key(KeyCode::Enter)), Action::InsertNewline);
        assert_eq!(
            parse_insert(key(KeyCode::Backspace)),
            Action::DeleteBackward
        );
        assert_eq!(parse_insert(key(KeyCode::Delete)), Action::DeleteForward);
        assert_eq!(parse_insert(key(KeyCode::Esc)), Action::ExitInsert);
        assert_eq!(
            parse_insert(key_ctrl(KeyCode::Char('c'))),
            Action::ExitInsert
        );
    }

    #[test]
    fn esc_in_normal_clears_pending() {
        let mut s = VimState::new();
        parse_normal(&mut s, key(KeyCode::Char('5')));
        parse_normal(&mut s, key(KeyCode::Esc));
        assert!(s.pending_count.is_none());
    }

    #[test]
    fn enter_in_normal_opens_db_row_detail() {
        let mut s = VimState::new();
        let action = parse_normal(&mut s, key(KeyCode::Enter));
        assert!(matches!(action, Action::OpenDbRowDetail));
    }

    #[test]
    fn db_row_detail_close_keys() {
        // Modal close is `Ctrl-C` only. `Esc` and `q` keep their
        // vim semantics so they don't accidentally yank the user
        // out of the modal mid-chord.
        let mut s = VimState::new();
        assert!(matches!(
            parse_db_row_detail(&mut s, key_ctrl(KeyCode::Char('c'))),
            Action::CloseDbRowDetail
        ));
        // `Esc` falls through to parse_normal which returns Noop
        // (and resets pending state — same as vim).
        let mut s = VimState::new();
        assert!(matches!(
            parse_db_row_detail(&mut s, key(KeyCode::Esc)),
            Action::Noop
        ));
        // `q` falls through to parse_normal. There's no `q` binding
        // in normal mode (macros aren't implemented), so it lands
        // on Noop too.
        let mut s = VimState::new();
        assert!(matches!(
            parse_db_row_detail(&mut s, key(KeyCode::Char('q'))),
            Action::Noop
        ));
    }

    #[test]
    fn db_row_detail_uppercase_y_copies_row_as_json() {
        let mut s = VimState::new();
        // `Y` is the row-as-JSON shortcut; `y` stays free so the
        // standard yank chord family (`yi{`, `yy`, `y$` …) works.
        let action = parse_db_row_detail(
            &mut s,
            KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT),
        );
        assert!(matches!(action, Action::CopyDbRowDetailJson));
    }

    #[test]
    fn db_row_detail_lowercase_y_starts_yank_chord() {
        // Pressing `y` alone must NOT trigger the row-JSON copy —
        // it should set up the operator-pending state so the next
        // keystroke (motion / textobj) completes the yank.
        let mut s = VimState::new();
        let action = parse_db_row_detail(&mut s, key(KeyCode::Char('y')));
        assert!(
            matches!(action, Action::Noop),
            "expected Noop (operator-pending), got {action:?}"
        );
        // `i` after `y` → text-object pending.
        let action = parse_db_row_detail(&mut s, key(KeyCode::Char('i')));
        assert!(matches!(action, Action::Noop));
        // `{` completes `yi{` → OperatorTextObject(Yank, ...).
        let action = parse_db_row_detail(&mut s, key(KeyCode::Char('{')));
        assert!(
            matches!(action, Action::OperatorTextObject(Operator::Yank, _, _)),
            "expected yank text-object, got {action:?}"
        );
    }

    #[test]
    fn db_row_detail_forwards_motions_from_normal() {
        // The modal piggybacks on `parse_normal`, so j/k/h/l/wbe/0/$
        // and friends all generate Motion actions just like in the
        // editor.
        let mut s = VimState::new();
        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Char('h'),
            KeyCode::Char('l'),
            KeyCode::Char('w'),
            KeyCode::Char('b'),
            KeyCode::Char('e'),
            KeyCode::Char('$'),
            KeyCode::Char('0'),
            KeyCode::Char('G'),
        ] {
            let action = parse_db_row_detail(&mut s, key(code));
            assert!(
                matches!(action, Action::Motion(_, _)),
                "expected Motion for {code:?}, got {action:?}"
            );
        }
    }

    #[test]
    fn db_row_detail_blocks_mutations_and_focus_escapes() {
        // Insert / paste / undo / ex / search / run-block / etc.
        // must NOT leak through — modal is read-only and owns input.
        // Search and ex would transition mode away from DbRowDetail
        // and break the render path. Visual is allowed (handled
        // separately) because the modal renders independently of
        // mode.
        for code in [
            KeyCode::Char('i'),
            KeyCode::Char('a'),
            KeyCode::Char('o'),
            KeyCode::Char('p'),
            KeyCode::Char(':'),
            KeyCode::Char('/'),
            KeyCode::Char('?'),
            KeyCode::Char('u'),
            KeyCode::Char('r'),
        ] {
            let mut s = VimState::new();
            let action = parse_db_row_detail(&mut s, key(code));
            assert!(
                matches!(action, Action::Noop),
                "expected Noop for {code:?}, got {action:?}"
            );
        }
    }

    #[test]
    fn db_row_detail_allows_visual_mode_entry() {
        // The modal renders independently of `app.vim.mode`, so
        // `v`/`V` flow through to enter visual selection. Yank
        // (`y{motion}` or `viwy`) then captures the highlighted
        // range from the modal's body doc.
        let mut s = VimState::new();
        assert!(matches!(
            parse_db_row_detail(&mut s, key(KeyCode::Char('v'))),
            Action::EnterVisual
        ));
        let mut s = VimState::new();
        assert!(matches!(
            parse_db_row_detail(&mut s, key(KeyCode::Char('V'))),
            Action::EnterVisualLine
        ));
    }

    #[test]
    fn http_response_detail_close_keys() {
        // Modal close is `Ctrl-C` only, mirroring the DB row-detail
        // modal — `Esc` and `q` must keep their normal vim semantics
        // so a stray keystroke during `yi{` doesn't teleport-close.
        let mut s = VimState::new();
        assert!(matches!(
            parse_http_response_detail(&mut s, key_ctrl(KeyCode::Char('c'))),
            Action::CloseHttpResponseDetail
        ));
        let mut s = VimState::new();
        assert!(matches!(
            parse_http_response_detail(&mut s, key(KeyCode::Esc)),
            Action::Noop
        ));
        let mut s = VimState::new();
        assert!(matches!(
            parse_http_response_detail(&mut s, key(KeyCode::Char('q'))),
            Action::Noop
        ));
    }

    #[test]
    fn http_response_detail_uppercase_y_copies_body() {
        let mut s = VimState::new();
        let action = parse_http_response_detail(
            &mut s,
            KeyEvent::new(KeyCode::Char('Y'), KeyModifiers::SHIFT),
        );
        assert!(matches!(action, Action::CopyHttpResponseBody));
    }

    #[test]
    fn http_response_detail_forwards_motions_from_normal() {
        let mut s = VimState::new();
        for code in [
            KeyCode::Char('j'),
            KeyCode::Char('k'),
            KeyCode::Char('h'),
            KeyCode::Char('l'),
            KeyCode::Char('$'),
            KeyCode::Char('0'),
            KeyCode::Char('G'),
        ] {
            let action = parse_http_response_detail(&mut s, key(code));
            assert!(
                matches!(action, Action::Motion(_, _)),
                "expected Motion for {code:?}, got {action:?}"
            );
        }
    }

    #[test]
    fn http_response_detail_blocks_mutations_and_focus_escapes() {
        // Same read-only contract as `db_row_detail`: no insert,
        // paste, undo, ex, search, run-block, etc.
        for code in [
            KeyCode::Char('i'),
            KeyCode::Char('a'),
            KeyCode::Char('o'),
            KeyCode::Char('p'),
            KeyCode::Char(':'),
            KeyCode::Char('/'),
            KeyCode::Char('?'),
            KeyCode::Char('u'),
            KeyCode::Char('r'),
        ] {
            let mut s = VimState::new();
            let action = parse_http_response_detail(&mut s, key(code));
            assert!(
                matches!(action, Action::Noop),
                "expected Noop for {code:?}, got {action:?}"
            );
        }
    }
}
