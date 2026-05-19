//! Action appliers, split by domain. Mechanically moved out of
//! `vim/dispatch.rs` (tui-v2 vertical 1, fase 1 p5) with no logic
//! change. The `apply_action` router itself stays in `vim::dispatch`
//! until p6 (owner decision); these submodules hold the per-domain
//! helpers it delegates to.

pub mod completion;
pub mod operator;
pub mod pickers;
pub mod window;
