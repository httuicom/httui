pub mod block_examples;
pub mod block_history;
pub mod block_results;
pub mod block_settings;
pub mod blocks;
pub mod captures_cache;
pub mod config;
pub mod connection_uses;
pub mod db;
pub mod dotenv;
pub mod error;
pub mod executor;
pub mod explain;
pub mod frontmatter;
pub mod fs;
pub mod git;
pub mod paths;
pub mod preflight;
pub mod references;
pub mod run_bodies;
pub mod runner;
pub mod search;
pub mod secrets;
pub mod tag_index;
pub mod templates;
pub mod var_uses;
pub mod vault_config;
pub mod vaults;

pub use error::{CoreError, CoreResult};

// Compat re-export: external consumers (`httui-mcp`) historically imported
// `httui_core::parser`. The module moved under `blocks::parser`; the alias
// keeps the old path working until those crates migrate.
pub use blocks::parser;
