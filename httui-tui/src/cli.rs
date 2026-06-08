use clap::Parser;
use std::path::PathBuf;

/// Notes — terminal edition.
///
/// Edits a vault of markdown notes with executable HTTP / DB / E2E blocks.
/// The vault to open is the `active_vault` registered in the shared
/// database; switching/adding vaults is done from inside the TUI
/// (mirrors the desktop). On the very first run you'll be prompted for
/// a path on stdin.
#[derive(Debug, Parser)]
#[command(name = "httui-tui", version, about)]
pub struct Cli {
    /// Override the config file location.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Override the data directory (where the SQLite registry lives).
    /// Defaults to `httui_core::paths::default_data_dir()`. Useful for
    /// running the binary against a sandboxed database in tests / dev.
    #[arg(long, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Logging verbosity: trace | debug | info | warn | error.
    #[arg(long, default_value = "info")]
    pub log_level: String,
}
