//! System clipboard wrapper around `arboard`.
//!
//! Yank operations (`y{motion}`, `yy`, `yiw`, visual+`y`, the
//! row-detail modal's `y`) call `set_text` after writing to the
//! unnamed register so the user can paste outside the TUI. Failures
//! are returned as `Err(String)` for the caller to surface on the
//! status bar — common reasons: SSH session without an X/Wayland
//! forward, headless container, sandbox.
//!
//! Fase 3 (tui-V1) adds the read side (`get_text`) plus a
//! [`SystemClipboard`] trait so the Standard-mode Copy/Cut/Paste
//! handlers depend on an injectable seam instead of poking the OS
//! directly — that keeps the new logic unit-testable with a
//! [`FakeClipboard`] (no X/Wayland needed in CI) while the legacy
//! free-function path (`set_text`, used by the vim yank engine) is
//! untouched so Cenário 2 stays byte-identical.

use arboard::Clipboard;

/// Map an `arboard` open failure to a stable user-facing string.
/// Pure so the error branch is unit-testable without a real
/// display server (the OS call itself is environment-dependent).
fn unavailable_msg(e: impl std::fmt::Display) -> String {
    format!("clipboard unavailable: {e}")
}

/// Map an `arboard` read/write failure to a stable user-facing
/// string. `verb` is `"read"` / `"write"` so the message reflects
/// which direction failed.
fn io_msg(verb: &str, e: impl std::fmt::Display) -> String {
    format!("clipboard {verb} failed: {e}")
}

/// Push `text` to the OS clipboard. Each call opens a fresh
/// `Clipboard` handle — the alternative (caching one in `App`)
/// keeps a system resource alive for the whole TUI lifetime, which
/// can interact poorly with screen lockers / paste daemons. Yank is
/// rare enough that the per-call open is invisible.
pub fn set_text(text: &str) -> Result<(), String> {
    let mut clip = Clipboard::new().map_err(unavailable_msg)?;
    clip.set_text(text.to_string())
        .map_err(|e| io_msg("write", e))
}

/// Read the current OS clipboard contents. Mirrors [`set_text`] —
/// fresh handle per call, same failure surface. Used by the
/// Standard-mode paste (`Ctrl+V`) path via the [`SystemClipboard`]
/// seam.
pub fn get_text() -> Result<String, String> {
    let mut clip = Clipboard::new().map_err(unavailable_msg)?;
    clip.get_text().map_err(|e| io_msg("read", e))
}

/// Injectable clipboard seam. The Standard-mode Copy/Cut/Paste
/// handlers take `&mut impl SystemClipboard` so unit tests pass a
/// [`FakeClipboard`] (deterministic, no display server) while
/// production passes [`ArboardClipboard`] (thin pass-through to the
/// free functions above).
pub trait SystemClipboard {
    /// Current clipboard text, or a user-facing error string.
    fn get(&mut self) -> Result<String, String>;
    /// Replace the clipboard text, or a user-facing error string.
    fn set(&mut self, text: &str) -> Result<(), String>;
}

/// Production [`SystemClipboard`] — each call delegates to the
/// per-call-handle free functions, so behaviour is byte-identical
/// to the legacy direct calls.
#[derive(Debug, Default, Clone, Copy)]
pub struct ArboardClipboard;

impl SystemClipboard for ArboardClipboard {
    fn get(&mut self) -> Result<String, String> {
        get_text()
    }
    fn set(&mut self, text: &str) -> Result<(), String> {
        set_text(text)
    }
}

/// In-memory [`SystemClipboard`] for unit tests. Holds the last
/// `set` value; `get` returns it (empty string until the first
/// `set`, mirroring an empty OS clipboard). `pub` + `#[cfg(test)]`
/// so any module's `mod tests` can inject it without a real
/// display server.
#[cfg(test)]
#[derive(Debug, Default, Clone)]
pub struct FakeClipboard {
    contents: String,
}

#[cfg(test)]
impl FakeClipboard {
    /// A fake seeded with an initial clipboard value (so paste
    /// tests don't need a prior `set`).
    pub fn with(contents: impl Into<String>) -> Self {
        Self {
            contents: contents.into(),
        }
    }
}

#[cfg(test)]
impl SystemClipboard for FakeClipboard {
    fn get(&mut self) -> Result<String, String> {
        Ok(self.contents.clone())
    }
    fn set(&mut self, text: &str) -> Result<(), String> {
        self.contents = text.to_string();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_round_trips_set_then_get() {
        let mut clip = FakeClipboard::default();
        assert_eq!(clip.get(), Ok(String::new()), "empty until first set");
        clip.set("hello").unwrap();
        assert_eq!(clip.get(), Ok("hello".to_string()));
        // Overwrite replaces, not appends.
        clip.set("world").unwrap();
        assert_eq!(clip.get(), Ok("world".to_string()));
    }

    #[test]
    fn fake_with_seeds_initial_contents() {
        let mut clip = FakeClipboard::with("seed");
        assert_eq!(clip.get(), Ok("seed".to_string()));
    }

    #[test]
    fn unavailable_msg_is_stable() {
        // The `Clipboard::new()` failure branch of get_text/set_text
        // funnels through this pure mapper — assert its shape
        // deterministically (the OS call itself is env-dependent).
        assert_eq!(
            unavailable_msg("no display"),
            "clipboard unavailable: no display"
        );
    }

    #[test]
    fn io_msg_reflects_direction() {
        assert_eq!(io_msg("read", "boom"), "clipboard read failed: boom");
        assert_eq!(io_msg("write", "boom"), "clipboard write failed: boom");
    }

    #[test]
    fn arboard_clipboard_is_a_thin_passthrough() {
        // We can't assert the OS result in CI (no display server),
        // but calling through the trait must not panic and must
        // return a `Result` whose error — if any — is one of the
        // stable strings above (proving it routed through the
        // mappers, not an arbitrary arboard Debug string).
        let mut clip = ArboardClipboard;
        for r in [
            SystemClipboard::set(&mut clip, "x").err(),
            SystemClipboard::get(&mut clip).err(),
        ]
        .into_iter()
        .flatten()
        {
            assert!(
                r.starts_with("clipboard unavailable: ")
                    || r.starts_with("clipboard read failed: ")
                    || r.starts_with("clipboard write failed: "),
                "unexpected error shape: {r:?}"
            );
        }
    }
}
