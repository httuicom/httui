//! Action appliers, split by domain. Mechanically moved out of
//! `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p5/p6) with no logic
//! change. The `apply_action` router in `vim::dispatch` dispatches
//! each `Action` variant to the matching `apply_<group>` here.

pub mod completion;
pub mod misc;
pub mod modal_detail;
pub mod navigation;
pub mod operator;
pub mod pickers;
pub mod replay;
/// Standard-mode (non-modal) selection + clipboard handlers — a
/// fresh, fully-covered module (NOT `coverage:exclude`, unlike the
/// mechanically-relocated legacy groups). Added by tui-V1 / fase 3.
pub mod standard_sel;
/// Standard-mode undo-group snapshot policy — a fresh, fully-covered
/// module (NOT `coverage:exclude`). Added by tui-V1 / fase 4 p2.
pub mod standard_undo;
pub mod tree_nav;
pub mod window;
