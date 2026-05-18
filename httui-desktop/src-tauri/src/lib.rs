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

// Git panel.
pub mod git_commands;

// Vault-wide tag index.
pub mod tag_commands;

// Pre-flight evaluator for the DocHeader pill row.
pub mod preflight_commands;

// Run-body filesystem cache.
pub mod run_body_commands;

// Captures persistence.
pub mod captures_commands;

// Template registry.
pub mod templates_commands;

// File-backed config (epic 09 foundation; cutover in epic 19).
pub mod vault_config_commands;

// Per-domain Tauri command split (lands the full split; this
// `commands/` tree starts with the helpers
// introduced in audit-015).
pub mod commands;

// Re-export the schemas frontend code needs at the IPC boundary.
pub use httui_core::vault_config;
