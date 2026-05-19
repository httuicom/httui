use clap::Parser;
use directories::ProjectDirs;
use std::path::PathBuf;
use tracing_appender::rolling;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

mod app;
mod buffer;
mod cli;
mod clipboard;
mod commands;
mod config;
mod document_loader;
mod error;
mod event;
mod fs_watch;
mod input;
mod pane;
mod schema;
mod sql_completion;
mod terminal;
mod tree;
mod ui;
mod vault;
mod vim;

use crate::cli::Cli;
use crate::error::{TuiError, TuiResult};

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::from(2)
        }
    }
}

async fn run(cli: Cli) -> TuiResult<()> {
    let _guard = init_tracing(&cli.log_level)?;

    let config_path = match &cli.config {
        Some(p) => p.clone(),
        None => config::default_config_path()?,
    };
    let cfg = config::load_or_init(&config_path)?;

    let data_dir = match cli.data_dir.clone() {
        Some(p) => p,
        None => httui_core::paths::default_data_dir()?,
    };
    std::fs::create_dir_all(&data_dir)?;
    let pool = httui_core::db::init_db(&data_dir)
        .await
        .map_err(|e| TuiError::Config(format!("init database at {data_dir:?}: {e}")))?;

    let resolved = vault::resolve(&pool).await?;

    app::run(cfg, resolved, pool).await
}

/// Set up `tracing` to write to a rolling log file under the user's XDG
/// state dir. **Nothing** is written to stderr while the TUI is up — the
/// alternate screen would corrupt log lines and vice versa.
fn init_tracing(level: &str) -> TuiResult<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = log_dir()?;
    std::fs::create_dir_all(&log_dir)?;
    let file_appender = rolling::daily(&log_dir, "notes-tui.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(format!("notes_tui={level},httui_tui={level}")))
        .map_err(|e| TuiError::Config(format!("invalid log level: {e}")))?;

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .init();

    Ok(guard)
}

fn log_dir() -> TuiResult<PathBuf> {
    let dirs = ProjectDirs::from("com", "httui", "notes-tui")
        .ok_or_else(|| TuiError::Config("could not resolve project dirs (no $HOME?)".into()))?;
    // `state_dir` is Linux-only; on macOS/Windows fall back to data_local_dir.
    let path = dirs
        .state_dir()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| dirs.data_local_dir().to_path_buf())
        .join("logs");
    Ok(path)
}
