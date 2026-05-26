//! Bootstrap empty-state — runs before `app::run` when the vault
//! registry has no active vault (or the active path is gone). Owns
//! its own alt-screen Terminal + event loop; on success, persists the
//! chosen vault via `httui_core::vaults::set_active_vault` and returns
//! `ResolvedVault` to `main.rs`. On user cancel, returns
//! `TuiError::InvalidArg("vault required")` so the binary exits with
//! a clean message.

use sqlx::SqlitePool;
use std::path::PathBuf;
use std::time::Duration;
use tracing::{info, warn};

use crate::error::{TuiError, TuiResult};
use crate::event::{AppEvent, EventLoop};
use crate::terminal;
use crate::vault::ResolvedVault;

mod render;
mod state;

/// Outcome of one iteration of the bootstrap event loop. Pure
/// dispatch on `AppEvent` so the loop body in `run` stays a thin
/// wrapper over [`step`] — keeps the testable logic out of the
/// terminal-IO surface.
#[derive(Debug)]
enum StepResult {
    Continue,
    Quit,
    Activated(PathBuf),
}

fn step(event: Option<AppEvent>, state: &mut state::BootstrapState) -> StepResult {
    match event {
        Some(AppEvent::Key(key)) => match state.handle_key(key) {
            state::Outcome::Continue => StepResult::Continue,
            state::Outcome::Quit => StepResult::Quit,
            state::Outcome::Activated(path) => StepResult::Activated(path),
        },
        Some(AppEvent::Resize(_, _)) | Some(AppEvent::Tick) | Some(AppEvent::Paste(_)) => {
            StepResult::Continue
        }
        // Async events from background tasks can't fire here
        // (no executors running yet) — ignore everything else.
        Some(_) => StepResult::Continue,
        None => StepResult::Quit,
    }
}

/// Draw one frame + advance the cursor + dispatch one event into
/// the bootstrap state. Generic over the terminal backend so tests
/// drive it with `TestBackend` while production stays on
/// `CrosstermBackend`.
fn render_and_step<B: ratatui::backend::Backend>(
    term: &mut ratatui::Terminal<B>,
    state: &mut state::BootstrapState,
    event: Option<AppEvent>,
) -> TuiResult<StepResult> {
    let mut cursor: Option<(u16, u16)> = None;
    term.draw(|f| {
        cursor = render::render(f, state);
    })
    .map_err(|e| TuiError::Terminal(format!("draw bootstrap: {e}")))?;
    if let Some((x, y)) = cursor {
        let _ = term.set_cursor_position((x, y));
        let _ = term.show_cursor();
    } else {
        let _ = term.hide_cursor();
    }
    Ok(step(event, state))
}

/// Drive the bootstrap loop until the user activates a vault or
/// cancels. Generic over the backend (test) + receiver type (tokio
/// mpsc receiver — same shape as `EventLoop`'s inner channel).
async fn bootstrap_loop<B>(
    term: &mut ratatui::Terminal<B>,
    state: &mut state::BootstrapState,
    events: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> TuiResult<Option<PathBuf>>
where
    B: ratatui::backend::Backend,
{
    loop {
        match render_and_step(term, state, events.recv().await)? {
            StepResult::Continue => {}
            StepResult::Quit => return Ok(None),
            StepResult::Activated(p) => return Ok(Some(p)),
        }
    }
}

/// Persist the chosen vault path and return a `ResolvedVault`.
/// Extracted from [`run`] so the success / failure branches are
/// reachable in tests without setting up a terminal.
async fn persist_active_vault(pool: &SqlitePool, path: PathBuf) -> TuiResult<ResolvedVault> {
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

/// Translate the loop's `Option<PathBuf>` outcome into the final
/// `ResolvedVault`. Cancel (`None`) becomes `InvalidArg`; otherwise
/// persist the path and wrap.
async fn finalize(pool: &SqlitePool, outcome: Option<PathBuf>) -> TuiResult<ResolvedVault> {
    let path = match outcome {
        Some(p) => p,
        None => {
            info!("bootstrap cancelled by user");
            return Err(TuiError::InvalidArg(
                "no vault selected (run httui again to pick one)".into(),
            ));
        }
    };
    persist_active_vault(pool, path).await
}

pub async fn run(pool: &SqlitePool) -> TuiResult<ResolvedVault> {
    terminal::install_panic_hook();
    let mut term = terminal::setup(false)?;
    let mut events = EventLoop::start(Duration::from_millis(500))?;
    let mut state = state::BootstrapState::new();
    let outcome = bootstrap_loop(&mut term, &mut state, events.receiver_mut()).await?;
    let _ = terminal::teardown(&mut term);
    finalize(pool, outcome).await
}


#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use httui_core::db::init_db;
    use tempfile::TempDir;
    
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }
    
    #[test]
    fn step_none_breaks_with_quit() {
        let mut s = state::BootstrapState::new();
        assert!(matches!(step(None, &mut s), StepResult::Quit));
    }
    
    #[test]
    fn step_resize_continues() {
        let mut s = state::BootstrapState::new();
        assert!(matches!(
            step(Some(AppEvent::Resize(80, 24)), &mut s),
            StepResult::Continue
        ));
    }
    
    #[test]
    fn step_tick_continues() {
        let mut s = state::BootstrapState::new();
        assert!(matches!(
            step(Some(AppEvent::Tick), &mut s),
            StepResult::Continue
        ));
    }
    
    #[test]
    fn step_paste_continues() {
        let mut s = state::BootstrapState::new();
        assert!(matches!(
            step(Some(AppEvent::Paste("clipboard".into())), &mut s),
            StepResult::Continue
        ));
    }
    
    #[test]
    fn step_catchall_other_event_continues() {
        let mut s = state::BootstrapState::new();
        assert!(matches!(step(Some(AppEvent::Quit), &mut s), StepResult::Continue));
    }

    #[test]
    fn step_result_debug_format_covers_all_variants() {
        // Exercise the derived Debug impl for every variant — keeps
        // the auto-generated branches under coverage.
        let c = format!("{:?}", StepResult::Continue);
        let q = format!("{:?}", StepResult::Quit);
        let a = format!("{:?}", StepResult::Activated(PathBuf::from("/x")));
        assert!(c.contains("Continue"));
        assert!(q.contains("Quit"));
        assert!(a.contains("Activated"));
    }

    #[test]
    fn step_resize_zero_dimensions_still_continues() {
        let mut s = state::BootstrapState::new();
        assert!(matches!(
            step(Some(AppEvent::Resize(0, 0)), &mut s),
            StepResult::Continue
        ));
    }

    #[test]
    fn step_paste_empty_string_continues() {
        let mut s = state::BootstrapState::new();
        assert!(matches!(
            step(Some(AppEvent::Paste(String::new())), &mut s),
            StepResult::Continue
        ));
    }
    
    #[test]
    fn step_key_quit_propagates_quit() {
        let mut s = state::BootstrapState::new();
        assert!(matches!(
            step(Some(AppEvent::Key(key(KeyCode::Esc))), &mut s),
            StepResult::Quit
        ));
    }
    
    #[test]
    fn step_key_continue_keeps_loop_running() {
        let mut s = state::BootstrapState::new();
        assert!(matches!(
            step(Some(AppEvent::Key(key(KeyCode::Char('z')))), &mut s),
            StepResult::Continue
        ));
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn persist_active_vault_returns_resolved_on_success() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = persist_active_vault(&pool, vault.path().to_path_buf())
            .await
            .expect("ok");
        assert_eq!(resolved.vault, vault.path());
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn persist_active_vault_returns_config_err_on_pool_failure() {
        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        pool.close().await;
        let err = persist_active_vault(&pool, PathBuf::from("/tmp/x"))
            .await
            .unwrap_err();
        assert!(
            matches!(err, TuiError::Config(ref m) if m.contains("persist active vault")),
            "got {err:?}"
        );
    }
    
    fn test_terminal() -> ratatui::Terminal<ratatui::backend::TestBackend> {
        ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap()
    }
    
    #[test]
    fn render_and_step_draws_and_dispatches_continue_on_tick() {
        let mut term = test_terminal();
        let mut state = state::BootstrapState::new();
        let outcome = render_and_step(&mut term, &mut state, Some(AppEvent::Tick)).unwrap();
        assert!(matches!(outcome, StepResult::Continue));
    }
    
    #[test]
    fn render_and_step_propagates_quit_on_esc_key() {
        let mut term = test_terminal();
        let mut state = state::BootstrapState::new();
        let outcome =
            render_and_step(&mut term, &mut state, Some(AppEvent::Key(key(KeyCode::Esc))))
                .unwrap();
        assert!(matches!(outcome, StepResult::Quit));
    }
    
    #[test]
    fn render_and_step_none_yields_quit() {
        let mut term = test_terminal();
        let mut state = state::BootstrapState::new();
        let outcome = render_and_step(&mut term, &mut state, None).unwrap();
        assert!(matches!(outcome, StepResult::Quit));
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_loop_returns_none_when_channel_closes() {
        let mut term = test_terminal();
        let mut state = state::BootstrapState::new();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        drop(tx);
        let result = bootstrap_loop(&mut term, &mut state, &mut rx).await.unwrap();
        assert!(result.is_none());
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_loop_processes_ticks_then_quits_on_esc() {
        let mut term = test_terminal();
        let mut state = state::BootstrapState::new();
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        tx.send(AppEvent::Tick).unwrap();
        tx.send(AppEvent::Resize(80, 24)).unwrap();
        tx.send(AppEvent::Key(key(KeyCode::Esc))).unwrap();
        let result = bootstrap_loop(&mut term, &mut state, &mut rx).await.unwrap();
        assert!(result.is_none());
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn finalize_none_returns_invalid_arg_err() {
        let data = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let err = finalize(&pool, None).await.unwrap_err();
        assert!(
            matches!(err, TuiError::InvalidArg(ref m) if m.contains("no vault selected")),
            "got {err:?}"
        );
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn finalize_some_path_delegates_to_persist() {
        let data = TempDir::new().unwrap();
        let vault = TempDir::new().unwrap();
        let pool = init_db(data.path()).await.unwrap();
        let resolved = finalize(&pool, Some(vault.path().to_path_buf()))
            .await
            .expect("ok");
        assert_eq!(resolved.vault, vault.path());
    }
    
    #[test]
    fn step_key_activated_returns_activated_with_path() {
        use crate::app::{VaultCreateFormFocus, VaultCreateFormState};
        use crate::vim::lineedit::LineEdit;
        let parent_dir = TempDir::new().unwrap();
        let mut s = state::BootstrapState::new();
        s.screen = state::Screen::Create(VaultCreateFormState {
            parent: LineEdit::from_str(parent_dir.path().to_string_lossy().to_string()),
            name: LineEdit::from_str("v"),
            focus: VaultCreateFormFocus::Name,
            error: None,
        });
        match step(Some(AppEvent::Key(key(KeyCode::Enter))), &mut s) {
            StepResult::Activated(p) => {
                assert!(p.ends_with("v"), "got {p:?}");
            }
            other => panic!("expected Activated, got {other:?}"),
        }
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn bootstrap_loop_returns_activated_path_on_enter_in_create_form() {
        use crate::app::{VaultCreateFormFocus, VaultCreateFormState};
        use crate::vim::lineedit::LineEdit;
        let parent_dir = TempDir::new().unwrap();
        let mut term = test_terminal();
        let mut s = state::BootstrapState::new();
        s.screen = state::Screen::Create(VaultCreateFormState {
            parent: LineEdit::from_str(parent_dir.path().to_string_lossy().to_string()),
            name: LineEdit::from_str("vault-via-loop"),
            focus: VaultCreateFormFocus::Name,
            error: None,
        });
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AppEvent>();
        tx.send(AppEvent::Key(key(KeyCode::Enter))).unwrap();
        let result = bootstrap_loop(&mut term, &mut s, &mut rx).await.unwrap();
        let p = result.expect("Some path");
        assert!(p.ends_with("vault-via-loop"), "got {p:?}");
    }
}
