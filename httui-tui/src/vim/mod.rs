//! Vim engine.
//!
//! Modes implemented: `Normal`, `Insert`, `CommandLine`. Round 2 also
//! brings the first ex commands (`:w`, `:q`, `:wq`, `:q!`). Operators,
//! text objects and find/till land later in round 2; visual / search /
//! registers / marks / macros are round 3.

pub mod change;
pub mod dispatch;
pub mod ex;
pub mod insert;
pub mod keybindings;
pub mod lineedit;
pub mod mode;
pub mod motions;
pub mod operator;
pub mod parser;
pub mod quickopen;
pub mod register;
pub mod search;
pub mod state;
pub mod textobject;
pub mod undo;

// `dispatch::dispatch` is no longer re-exported here: its last
// in-crate caller (`app::handle_key`) moved to
// `crate::input::route::route` (tui-V1 / fase 2 p5). The `dispatch`
// module itself stays public so `crate::vim::dispatch::apply_action`
// (used by `input::apply::replay`) keeps resolving through the facade.
pub use state::VimState;
