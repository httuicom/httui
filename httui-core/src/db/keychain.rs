#[cfg(not(test))]
use keyring::Entry;

#[cfg(not(test))]
const SERVICE: &str = "httui-notes";

/// Sentinel value stored in SQLite when the real value is in the keychain.
pub const KEYCHAIN_SENTINEL: &str = "__KEYCHAIN__";

/// Test-only override: when set to `true`, every keychain operation
/// (`store_secret` / `get_secret` / `delete_secret`) returns `Err`
/// immediately, without touching the OS keyring. This lets us assert the
/// fail-secure invariant — no plaintext fallback when the keychain is
/// unavailable — from integration tests that don't have a way to disable
/// the real keyring backend.
///
/// Hidden behind `#[cfg(test)]` so production builds can't toggle it. Use
/// the `force_keychain_failure` helper to flip it; tests that flip it
/// should hold `KEYCHAIN_TEST_LOCK` so concurrent tests don't see a half-
/// flipped state.
#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(test)]
static FORCE_FAIL: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
pub static KEYCHAIN_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Test-only: force every keychain op in this process to error until reset.
/// Hold `KEYCHAIN_TEST_LOCK` while the override is active.
#[cfg(test)]
pub fn force_keychain_failure(enable: bool) {
    FORCE_FAIL.store(enable, Ordering::SeqCst);
}

#[cfg(test)]
fn forced_failure() -> Option<String> {
    if FORCE_FAIL.load(Ordering::SeqCst) {
        Some("Keychain unavailable (test override)".to_string())
    } else {
        None
    }
}

#[cfg(not(test))]
fn forced_failure() -> Option<String> {
    None
}

/// Test-only in-memory keychain: a process-wide `HashMap` that
/// `store_secret`/`get_secret`/`delete_secret` route through when built
/// with `cfg(test)`. Why this exists: on macOS the OS keychain prompts
/// for the login password on every test run because `cargo test` rebuilds
/// the test binary with a new hash and the "Always Allow" ACL is bound
/// to the binary signature. The `keyring` crate's own `mock` backend is
/// `EntryOnly` (each `Entry::new` returns a fresh credential with no
/// shared store), so we need our own map for store/get to be symmetric.
#[cfg(test)]
static TEST_KEYCHAIN: std::sync::Mutex<Option<std::collections::HashMap<String, String>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
fn with_test_keychain<R>(f: impl FnOnce(&mut std::collections::HashMap<String, String>) -> R) -> R {
    let mut guard = TEST_KEYCHAIN.lock().expect("test keychain mutex poisoned");
    let map = guard.get_or_insert_with(std::collections::HashMap::new);
    f(map)
}

/// Store a secret in the OS keychain.
pub fn store_secret(key: &str, value: &str) -> Result<(), String> {
    if let Some(err) = forced_failure() {
        return Err(err);
    }
    #[cfg(test)]
    {
        with_test_keychain(|m| m.insert(key.to_string(), value.to_string()));
        Ok(())
    }
    #[cfg(not(test))]
    {
        let entry = Entry::new(SERVICE, key).map_err(|e| format!("Keychain error: {}", e))?;
        entry
            .set_password(value)
            .map_err(|e| format!("Failed to store secret: {}", e))
    }
}

/// Retrieve a secret from the OS keychain.
pub fn get_secret(key: &str) -> Result<Option<String>, String> {
    if let Some(err) = forced_failure() {
        return Err(err);
    }
    #[cfg(test)]
    {
        Ok(with_test_keychain(|m| m.get(key).cloned()))
    }
    #[cfg(not(test))]
    {
        let entry = Entry::new(SERVICE, key).map_err(|e| format!("Keychain error: {}", e))?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("Failed to get secret: {}", e)),
        }
    }
}

/// Delete a secret from the OS keychain.
pub fn delete_secret(key: &str) -> Result<(), String> {
    if let Some(err) = forced_failure() {
        return Err(err);
    }
    #[cfg(test)]
    {
        with_test_keychain(|m| m.remove(key));
        Ok(())
    }
    #[cfg(not(test))]
    {
        let entry = Entry::new(SERVICE, key).map_err(|e| format!("Keychain error: {}", e))?;
        match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // already gone
            Err(e) => Err(format!("Failed to delete secret: {}", e)),
        }
    }
}

/// Build the keychain key for a connection password.
pub fn conn_password_key(connection_id: &str) -> String {
    format!("conn:{}:password", connection_id)
}

/// Build the keychain key for an environment variable.
pub fn env_var_key(env_id: &str, var_key: &str) -> String {
    format!("env:{}:{}", env_id, var_key)
}

/// Resolve a value that may be stored in keychain.
/// If the value is the sentinel, fetch from keychain. Otherwise return as-is.
pub fn resolve_value(db_value: &str, keychain_key: &str) -> Result<String, String> {
    if db_value == KEYCHAIN_SENTINEL {
        get_secret(keychain_key)?
            .ok_or_else(|| format!("Secret not found in keychain for key: {}", keychain_key))
    } else {
        Ok(db_value.to_string())
    }
}

/// Resolve a `{{keychain:NS:KEY}}` reference (ADR 0002 syntax) against
/// the OS keychain. Returns:
///
/// - `Ok(Some(value))` when the reference points at a stored secret
/// - `Ok(None)` when the reference is well-formed but the keychain
///   has no entry (caller decides whether that's an error)
/// - `Err(...)` for malformed input or backend errors
///
/// Non-keychain backends (`1password`, `pass`, `env`) are NOT handled
/// here — they need their own resolvers that this module does not own.
pub fn resolve_secret_ref(value: &str) -> Result<Option<String>, String> {
    let trimmed = value.trim();
    if !(trimmed.starts_with("{{") && trimmed.ends_with("}}")) {
        return Err(format!("not a secret reference: {value}"));
    }
    let inner = &trimmed[2..trimmed.len() - 2];
    let (backend, address) = inner
        .split_once(':')
        .ok_or_else(|| format!("malformed reference (no backend separator): {value}"))?;
    if backend != "keychain" {
        return Err(format!(
            "secret backend `{backend}` is not handled by this resolver"
        ));
    }
    if address.is_empty() {
        return Err(format!("empty address in reference: {value}"));
    }
    get_secret(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_failure_makes_store_secret_error() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        force_keychain_failure(true);
        let result = store_secret("test:key", "value");
        force_keychain_failure(false); // reset before any assert that could panic
        let err = result.expect_err("store_secret must fail when forced");
        assert!(
            err.contains("Keychain unavailable"),
            "expected forced-failure marker in error, got: {err}"
        );
    }

    #[test]
    fn forced_failure_makes_get_secret_error() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        force_keychain_failure(true);
        let result = get_secret("test:key");
        force_keychain_failure(false);
        assert!(result.is_err());
    }

    #[test]
    fn forced_failure_makes_resolve_value_error_for_sentinel() {
        // Non-sentinel values bypass keychain entirely and must keep working
        // even when the keychain is forced to fail — they're plaintext by
        // design (Tier 2 in the security model).
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        force_keychain_failure(true);
        let plain = resolve_value("plaintext", "any:key");
        let sentinel = resolve_value(KEYCHAIN_SENTINEL, "missing:key");
        force_keychain_failure(false);
        assert_eq!(plain.unwrap(), "plaintext");
        assert!(
            sentinel.is_err(),
            "sentinel must NOT silently fall back to plaintext when keychain is unavailable"
        );
    }

    #[test]
    fn forced_failure_makes_delete_secret_error() {
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        force_keychain_failure(true);
        let result = delete_secret("test:key");
        force_keychain_failure(false);
        assert!(result.is_err());
    }

    #[test]
    fn conn_password_key_format() {
        assert_eq!(conn_password_key("pg-prod"), "conn:pg-prod:password");
        assert_eq!(conn_password_key(""), "conn::password");
    }

    #[test]
    fn env_var_key_format() {
        assert_eq!(env_var_key("staging", "DB_URL"), "env:staging:DB_URL");
        assert_eq!(env_var_key("", ""), "env::");
    }

    #[test]
    fn resolve_secret_ref_rejects_non_reference() {
        let err = resolve_secret_ref("plain-value").expect_err("must reject");
        assert!(err.contains("not a secret reference"));
    }

    #[test]
    fn resolve_secret_ref_rejects_missing_separator() {
        let err = resolve_secret_ref("{{nobackend}}").expect_err("must reject");
        assert!(err.contains("malformed reference"));
    }

    #[test]
    fn resolve_secret_ref_rejects_unhandled_backend() {
        let err = resolve_secret_ref("{{1password:Personal/x}}").expect_err("must reject");
        assert!(err.contains("not handled by this resolver"));
    }

    #[test]
    fn resolve_secret_ref_rejects_empty_address() {
        let err = resolve_secret_ref("{{keychain:}}").expect_err("must reject");
        assert!(err.contains("empty address"));
    }

    #[test]
    fn resolve_secret_ref_propagates_keychain_error() {
        // Forcing keychain failure exercises the success-path branch up to
        // the point where get_secret runs — verifies the parser reaches
        // backend dispatch correctly, and that errors propagate.
        let _guard = KEYCHAIN_TEST_LOCK.lock().unwrap();
        force_keychain_failure(true);
        let result = resolve_secret_ref("{{keychain:conn:pg:password}}");
        force_keychain_failure(false);
        assert!(result.is_err());
    }

    #[test]
    fn resolve_secret_ref_trims_whitespace() {
        // The parser trims surrounding whitespace before checking braces.
        let err = resolve_secret_ref("   plain   ").expect_err("must reject");
        assert!(err.contains("not a secret reference"));
    }
}
