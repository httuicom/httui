//! Action appliers, split by domain. Mechanically moved out of
//! `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p5/p6) with no logic
//! change. The `apply_action` router in `vim::dispatch` dispatches
//! each `Action` variant to the matching `apply_<group>` here.

pub mod completion;
/// V3 P3 (2026-05-23): create-connection form modal handlers.
pub mod connection_form;
/// V10 (tui-V10): connection picker handlers, split out of `pickers.rs`
/// to keep that file under the 600-line size gate.
pub mod connection_picker;
/// V4 P6 (2026-05-23): activate-env-by-index handler.
pub mod env_activate;
/// V10 (tui-V10): environment picker handlers, split out of `pickers.rs`.
pub mod env_picker;
pub mod git_branch_picker;
pub mod git_conflict_resolver;
pub mod git_log_page;
pub mod git_panel;
/// V4 P5 (2026-05-23): handlers do clone-env form. Extraído de
/// `envs_page` pra respeitar size limit do DoD.
pub mod envs_clone;
pub mod envs_page;
/// V4 débito-de-cobertura (2026-05-23): tests integrados que cobrem
/// envs_page + envs_clone via App+EnvironmentsStore reais (tempdir).
#[cfg(test)]
mod envs_page_tests;
pub mod misc;
pub mod modal_detail;
pub mod navigation;
pub mod operator;
pub mod pickers;
pub mod replay;
/// Standard-mode `/` slash-trigger applier — a fresh, fully-covered
/// module (NOT `coverage:exclude`). Added by tui-V2 / vertical 2.
pub mod slash;
/// Standard-mode Backspace applier with cross-segment boundary
/// semantics — a fresh, fully-covered module (NOT `coverage:exclude`).
/// Added by tui-V2 / vertical 2 / cenário 4.
pub mod standard_delete;
/// Standard-mode (non-modal) selection + clipboard handlers — a
/// fresh, fully-covered module (NOT `coverage:exclude`, unlike the
/// mechanically-relocated legacy groups). Added by tui-V1 / fase 3.
pub mod standard_sel;
/// Standard-mode undo-group snapshot policy — a fresh, fully-covered
/// module (NOT `coverage:exclude`). Added by tui-V1 / fase 4 p2.
pub mod standard_undo;
pub mod tree_nav;
/// V10 (tui-V10): vault picker + sub-modals (create/clone/open/missing),
/// split out of `pickers.rs`.
pub mod vault_modals;
pub mod window;
