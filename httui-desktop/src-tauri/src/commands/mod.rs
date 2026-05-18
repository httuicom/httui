//! Tauri command modules. The per-domain split is incremental —
//! `vault_stores` is the first module; environments and connections
//! command modules follow in a later cutover.

pub mod blocks;
pub mod connections;
pub mod environments;
pub mod files;
pub mod schema;
pub mod settings;
pub mod vault_stores;
