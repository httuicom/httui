// coverage:exclude file — legacy vim engine relocated by tui-V1/Fase1
// (behavior-identical, suite-proven); coverage tracked in
// docs-llm/tui-v2/vim-coverage-debt.md (2026-05-19), paid by dedicated épico.
//! Keymap profile skeleton (tui-v2 vertical 1, fase 1 p7).
//!
//! Placeholder for the profile-aware keymap layer. The actual
//! per-profile rebinding (routing raw keystrokes through `lookup`
//! before the legacy vim decoders) is fase 2 work — this module only
//! pins the public shape so later phases have a stable seam. Nothing
//! here is wired into `app.rs` / `handle_key` yet; the binary's
//! behavior is unchanged.

/// Input profile. `Standard` is the default editor feel; `Vim` opts
/// into the modal vim engine. Selection plumbing lands in fase 2.
#[allow(dead_code)]
pub enum Profile {
    Standard,
    Vim,
}

/// Resolve a keystroke (under a profile + editor mode) to an
/// [`crate::input::action::Action`]. Skeleton: always `None` so the
/// caller falls back to the existing vim decoders. Fase 2 fills this
/// in and rewires `handle_key`.
#[allow(dead_code)]
pub fn lookup(
    _p: Profile,
    _m: crate::vim::mode::Mode,
    /* chord */
) -> Option<crate::input::action::Action> {
    None
}
