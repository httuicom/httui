// Re-export shared core modules
pub use httui_core::block_examples;
pub use httui_core::block_history;
pub use httui_core::block_results;
pub use httui_core::block_settings;
pub use httui_core::config;
pub use httui_core::db;
pub use httui_core::executor;
pub use httui_core::search;
pub use httui_core::var_uses;

// fs re-exports core + local watcher
pub mod fs {
    pub use httui_core::fs::*;
    pub mod watcher;
}

// Chat sidecar integration
pub mod chat;

// Cancel-aware DB execution plumbing (stage 3 of db block redesign)
pub mod executions;

// Git panel (Epic 20).
pub mod git_commands;

// Vault-wide tag index (Epic 52 Story 04).
pub mod tag_commands;

// Run-body filesystem cache (Epic 47 Story 01).
pub mod run_body_commands;

// Captures persistence (Epic 46 Story 03).
pub mod captures_commands;

// Template registry (Epic 41 Story 04).
pub mod templates_commands;

// File-backed config (epic 09 foundation; cutover in epic 19).
pub mod vault_config_commands;

// Per-domain Tauri command split (Epic 20a Story 05 lands the full
// split; this `commands/` tree starts with the cutover helpers
// introduced in audit-015).
pub mod commands;

// Re-export the schemas frontend code needs at the IPC boundary.
pub use httui_core::vault_config;
