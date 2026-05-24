//! Bootstrap empty-state — runs before `app::run` when the vault
//! registry has no active vault (or the active path is gone). Owns
//! its own alt-screen Terminal + event loop; on success, persists the
//! chosen vault via `httui_core::vaults::set_active_vault` and returns
//! `ResolvedVault` to `main.rs`. On user cancel, returns
//! `TuiError::InvalidArg("vault required")` so the binary exits with
//! a clean message.

use sqlx::SqlitePool;
use std::time::Duration;
use tracing::{info, warn};

use crate::error::{TuiError, TuiResult};
use crate::event::{AppEvent, EventLoop};
use crate::terminal;
use crate::vault::ResolvedVault;

mod render;
mod state;

pub async fn run(pool: &SqlitePool) -> TuiResult<ResolvedVault> {
    terminal::install_panic_hook();
    let mut term = terminal::setup(false)?;
    let mut events = EventLoop::start(Duration::from_millis(500))?;
    let mut state = state::BootstrapState::new();
    let mut cursor: Option<(u16, u16)>;

    let outcome = loop {
        cursor = None;
        term.draw(|f| {
            cursor = render::render(f, &state);
        })
        .map_err(|e| TuiError::Terminal(format!("draw bootstrap: {e}")))?;
        if let Some((x, y)) = cursor {
            let _ = term.set_cursor_position((x, y));
            let _ = term.show_cursor();
        } else {
            let _ = term.hide_cursor();
        }

        match events.next().await {
            Some(AppEvent::Key(key)) => match state.handle_key(key) {
                state::Outcome::Continue => {}
                state::Outcome::Quit => break Err(()),
                state::Outcome::Activated(path) => break Ok(path),
            },
            Some(AppEvent::Resize(_, _)) | Some(AppEvent::Tick) | Some(AppEvent::Paste(_)) => {}
            // Async events from background tasks can't fire here
            // (no executors running yet) — ignore everything else.
            Some(_) => {}
            None => break Err(()),
        }
    };

    let _ = terminal::teardown(&mut term);

    let path = match outcome {
        Ok(p) => p,
        Err(()) => {
            info!("bootstrap cancelled by user");
            return Err(TuiError::InvalidArg(
                "no vault selected (run httui again to pick one)".into(),
            ));
        }
    };

    let path_str = path.to_string_lossy().to_string();
    if let Err(e) = httui_core::vaults::set_active_vault(pool, &path_str).await {
        warn!(?path, ?e, "could not persist active vault");
        return Err(TuiError::Config(format!(
            "persist active vault {}: {e}",
            path.display()
        )));
    }

    Ok(ResolvedVault { vault: path })
}
