//! Secret backends — abstraction layer over the OS keychain and
//! alternatives (Touch ID, 1Password CLI, pass).
//!
//! The primary path lives in [`crate::db::keychain`] and uses the OS
//! keychain via the `keyring` crate. This module is the dispatch
//! boundary: the resolver holds a `dyn SecretBackend` rather than
//! importing each backend's module directly, allowing new backends to
//! be added without touching callers.
//!
//! [`parser`] handles the ref-syntax (`{{backend:address}}`).

pub mod parser;

/// Errors a backend can return. Plain `String` — matches the existing
/// `db::keychain` error type and keeps the boundary low-friction.
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
