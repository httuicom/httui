//! Action appliers, split by domain. The `apply_action` router in
//! `vim::dispatch` dispatches each `Action` variant to the matching
//! `apply_<group>` here.

pub mod completion;
/// Create-connection form handlers.
pub mod connection_form;
/// Connection picker handlers.
pub mod connection_picker;
/// Activate-env-by-index handler.
pub mod env_activate;
/// Environment picker handlers.
pub mod env_picker;
/// Clone-env form handlers (split from `envs_page`).
pub mod envs_clone;
pub mod envs_page;
/// Integration tests covering envs_page + envs_clone via real
/// `App` + `EnvironmentsStore` (tempdir).
#[cfg(test)]
mod envs_page_tests;
pub mod git_branch_picker;
pub mod git_conflict_resolver;
pub mod git_log_page;
pub mod git_panel;
pub mod git_share;
pub mod misc;
pub mod modal_detail;
pub mod navigation;
pub mod operator;
pub mod pickers;
pub mod replay;
/// Standard-mode `/` slash-trigger applier.
pub mod slash;
/// Standard-mode Backspace with cross-segment boundary semantics.
pub mod standard_delete;
/// Standard-mode (non-modal) selection + clipboard handlers.
pub mod standard_sel;
/// Standard-mode undo-group snapshot policy.
pub mod standard_undo;
pub mod tree_nav;
/// Vault picker + sub-modals (create / clone / open / missing).
pub mod vault_modals;
pub mod window;
