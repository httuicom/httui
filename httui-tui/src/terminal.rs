// coverage:exclude file — 100% terminal I/O (raw mode, alt screen,
// mouse capture, kitty keyboard-protocol push/pop). Untestable
// without a PTY harness; no domain logic lives here. Debt logged
// 2026-05-21 in docs-llm/tui-v2/io-coverage-debt.md (tui-V03).
use crossterm::{
    event::{
        DisableMouseCapture, EnableMouseCapture, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, supports_keyboard_enhancement, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{TuiError, TuiResult};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

/// Tracks whether `setup` pushed the kitty keyboard-protocol flags so
/// teardown (and the panic hook) pop exactly once. `swap(false)` makes
/// the pop idempotent — teardown then the panic hook is harmless.
static KITTY_PUSHED: AtomicBool = AtomicBool::new(false);

/// Switch the terminal into alt screen + raw mode, optionally enabling
/// mouse capture. Returns a [`Terminal`] handle ready for drawing.
pub fn setup(mouse: bool) -> TuiResult<Tui> {
    enable_raw_mode().map_err(|e| TuiError::Terminal(format!("enable_raw_mode: {e}")))?;
    let mut stdout = io::stdout();
    if mouse {
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| TuiError::Terminal(format!("enter alt screen: {e}")))?;
    } else {
        execute!(stdout, EnterAlternateScreen)
            .map_err(|e| TuiError::Terminal(format!("enter alt screen: {e}")))?;
    }
    // Kitty keyboard protocol — when the terminal supports it, lets it
    // disambiguate `Ctrl+Shift+<key>` from `Ctrl+<key>`. Guarded by
    // the support probe: pushing it unconditionally regressed a
    // terminal that reports no support — it half-honored the push and
    // folded `Ctrl+Shift+M` onto `Enter`. Terminals without the
    // protocol keep the legacy scheme; reachable chords there come
    // from config-driven keybindings (`EditorConfig::toggle_mode_key`).
    if supports_keyboard_enhancement().unwrap_or(false)
        && execute!(
            stdout,
            PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
        )
        .is_ok()
    {
        KITTY_PUSHED.store(true, Ordering::SeqCst);
    }

    let backend = CrosstermBackend::new(stdout);
    let terminal =
        Terminal::new(backend).map_err(|e| TuiError::Terminal(format!("ratatui new: {e}")))?;
    Ok(terminal)
}

/// Restore the terminal to its previous state. Idempotent — calling twice
/// (e.g. once in normal teardown, once via the panic hook) is harmless.
pub fn teardown(terminal: &mut Tui) -> TuiResult<()> {
    if KITTY_PUSHED.swap(false, Ordering::SeqCst) {
        let _ = execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags);
    }
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();
    Ok(())
}

/// Restore the terminal directly via stdout, without owning a `Terminal`
/// handle. Used by the panic hook, which runs from a context where the
/// `Terminal` value is unreachable.
pub fn restore_raw_stdout() {
    if KITTY_PUSHED.swap(false, Ordering::SeqCst) {
        let _ = execute!(io::stdout(), PopKeyboardEnhancementFlags);
    }
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
}

/// Install a panic hook that restores the terminal *before* the default
/// hook prints the panic message. Without this, panics leave the terminal
/// in raw / alt-screen mode and the trace is unreadable.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_raw_stdout();
        prev(info);
    }));
}
