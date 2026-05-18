//! Secret backends — abstraction layer over the OS keychain and
//! upcoming alternatives (Touch ID, Stronghold, 1Password CLI, pass).
//!
//! The MVP path lives in [`crate::db::keychain`] and uses the OS
//! keychain via the `keyring` crate. introduces this module
//! as the boundary against which Epics 14–16 add new backends. For
//! v1 every backend is local — there is no remote secret backend
//! shipped with the desktop app.
//!
//! ## Why a trait instead of just calling functions
//!
//! The old shape — free `store_secret`, `get_secret`, `delete_secret`
//! in `db::keychain` — couples every caller to "the OS keychain is
//! the answer". Once we add Touch ID, Windows Hello
//! and 1Password, the resolver needs to dispatch
//! by reference type (`{{keychain:...}}` vs `{{1password:...}}` vs
//! `{{pass:...}}`). The trait gives us a single object the resolver
//! can hold without import-tangling each backend's module.
//!
//! ## What ships in this commit
//!
//! - [`SecretBackend`] trait — minimal `store/get/delete` surface.
//! - [`Keychain`] — concrete impl that delegates to the existing
//!   `db::keychain` free functions. The body of those functions is
//!   not moved; this file is just the entry point that other
//!   subsystems (and the future resolver) target.
//! - [`parser`] submodule — pure ref-syntax parsing
//!   (`{{backend:address}}`). Existing free helpers in
//!   `db::keychain::resolve_secret_ref` and
//!   `vault_config::validate::is_secret_ref` keep working as thin
//!   wrappers.

pub mod parser;

/// Errors a backend can return. Kept as a plain `String` for now —
/// matches the existing `db::keychain` error type and keeps the
/// boundary low-friction. Will tighten in the prompt-fix
/// follow-up commit if we need to distinguish "user denied" from
/// "system error".
pub type SecretError = String;
pub type SecretResult<T> = Result<T, SecretError>;

/// Storage abstraction for a single named secret. Backends are
/// responsible for whatever locking, ACL, and prompting their
/// platform requires.
pub trait SecretBackend: Send + Sync {
    /// Store `value` under `key`. Overwrites any prior value silently.
    fn store(&self, key: &str, value: &str) -> SecretResult<()>;
    /// Read `key`. Returns `Ok(None)` when the key has no entry.
    fn get(&self, key: &str) -> SecretResult<Option<String>>;
    /// Remove `key`. Idempotent — already-absent keys return `Ok(())`.
    fn delete(&self, key: &str) -> SecretResult<()>;
    /// Stable identifier used as the backend prefix in
    /// `{{backend:address}}` references. e.g. `"keychain"`.
    fn id(&self) -> &'static str;
}

/// OS-keychain backend. Delegates to the legacy `db::keychain`
/// helpers so we don't fork the macOS-specific code paths.
#[derive(Debug, Clone, Copy, Default)]
pub struct Keychain;

impl SecretBackend for Keychain {
    fn store(&self, key: &str, value: &str) -> SecretResult<()> {
        crate::db::keychain::store_secret(key, value)
    }
    fn get(&self, key: &str) -> SecretResult<Option<String>> {
        crate::db::keychain::get_secret(key)
    }
    fn delete(&self, key: &str) -> SecretResult<()> {
        crate::db::keychain::delete_secret(key)
    }
    fn id(&self) -> &'static str {
        "keychain"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keychain_id_is_keychain() {
        assert_eq!(Keychain.id(), "keychain");
    }

    #[test]
    fn keychain_round_trips_via_trait_object() {
        // Hold the keychain test lock to serialize with other
        // tests touching the in-memory backing.
        let _g = crate::db::keychain::KEYCHAIN_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let backend: Box<dyn SecretBackend> = Box::new(Keychain);
        let key = "test:secrets:roundtrip";

        backend.store(key, "topsecret").unwrap();
        assert_eq!(backend.get(key).unwrap().as_deref(), Some("topsecret"));
        backend.delete(key).unwrap();
        assert!(backend.get(key).unwrap().is_none());
    }

    #[test]
    fn delete_missing_is_idempotent_via_trait() {
        let _g = crate::db::keychain::KEYCHAIN_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let backend = Keychain;
        // Best-effort: never seen this key, delete is a no-op `Ok`.
        backend.delete("test:secrets:never-existed").unwrap();
    }
}
