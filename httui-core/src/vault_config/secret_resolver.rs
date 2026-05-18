//! Reference ↔ raw value plumbing for secrets.
//!
//! Both `ConnectionsStore` and `EnvironmentsStore` need the same flow:
//! take a raw value from the UI, store it in the keychain, keep only a
//! `{{keychain:NS:KEY}}` reference in the TOML file. And the reverse —
//! resolve a reference to a raw value at execution time. This module
//! owns that bridge so neither store repeats the parsing or keychain
//! plumbing inline.
//!
//! Extracted from `connections_store.rs` (where
//! it lived as `ensure_password_ref` / `parse_keychain_ref` /
//! `resolve_password_ref` / `format_password_ref`). The same module is
//! reused by `environments_store.rs` for env-var secrets.

use super::validate::is_secret_ref;
use crate::db::keychain::{get_secret, resolve_secret_ref, store_secret};

/// Format `key` as the canonical `{{keychain:KEY}}` reference written
/// into TOML files. Mirrors the parser in [`parse_keychain_address`].
pub fn format_keychain_ref(key: &str) -> String {
    format!("{{{{keychain:{key}}}}}")
}

/// Store `raw` in the keychain under `key` and return the canonical
/// reference. If `raw` is already a `{{...}}` secret reference, return
/// it verbatim — keeps the operation idempotent for "re-save same row"
/// flows where the UI hands us a reference instead of a plaintext.
pub fn ensure_keychain_ref(key: &str, raw: &str) -> Result<String, String> {
    if is_secret_ref(raw) {
        return Ok(raw.to_string());
    }
    store_secret(key, raw).map_err(|e| format!("Failed to store secret securely: {e}"))?;
    Ok(format_keychain_ref(key))
}

/// Returns the keychain address inside a `{{keychain:ADDRESS}}`
/// reference. `None` when:
///
/// - the input isn't a `{{...}}` reference at all
/// - the backend is something other than `keychain` (e.g.
///   `{{1password:...}}` belongs to a different resolver)
/// - the address part is empty
pub fn parse_keychain_address(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if !(trimmed.starts_with("{{") && trimmed.ends_with("}}")) {
        return None;
    }
    let inner = &trimmed[2..trimmed.len() - 2];
    let (backend, address) = inner.split_once(':')?;
    if backend != "keychain" || address.is_empty() {
        return None;
    }
    Some(address.to_string())
}

/// Best-effort: does the keychain entry behind this reference exist?
/// Returns `false` for non-references, non-keychain backends, missing
/// entries, or transient keychain failures (caller can re-prompt).
pub fn keychain_entry_exists(value: &str) -> bool {
    let Some(address) = parse_keychain_address(value) else {
        return false;
    };
    get_secret(&address).map(|v| v.is_some()).unwrap_or(false)
}

/// Resolve `value` into the raw string used at execution time.
/// - empty value → `Ok(None)` (caller treats as "no password / unset")
/// - plaintext (legacy, pre-keychain) → pass through unchanged
/// - `{{keychain:...}}` reference → look up via the keychain;
///   `Err` when the reference is well-formed but the keychain has no
///   entry, or when the backend errors
pub fn resolve_value(value: &str) -> Result<Option<String>, String> {
    if value.is_empty() {
        return Ok(None);
    }
    if !is_secret_ref(value) {
        return Ok(Some(value.to_string()));
    }
    match resolve_secret_ref(value) {
        Ok(Some(v)) => Ok(Some(v)),
        Ok(None) => Err(format!("secret reference {value} did not resolve")),
        Err(e) => Err(format!("keychain error resolving {value}: {e}")),
    }
}

#[cfg(test)]
#[allow(clippy::await_holding_lock)]
mod tests {
    use super::*;
    use crate::db::keychain::{delete_secret, force_keychain_failure, KEYCHAIN_TEST_LOCK};

    #[test]
    fn format_keychain_ref_wraps_key_in_braces() {
        assert_eq!(
            format_keychain_ref("conn:pg:password"),
            "{{keychain:conn:pg:password}}"
        );
        assert_eq!(format_keychain_ref(""), "{{keychain:}}");
    }

    #[test]
    fn ensure_keychain_ref_passes_existing_ref_through() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let existing = "{{keychain:conn:already-stored:password}}";
        let out = ensure_keychain_ref("ignored", existing).unwrap();
        assert_eq!(out, existing);
    }

    #[test]
    fn ensure_keychain_ref_passes_other_backend_ref_through() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let existing = "{{1password:Personal/x/y}}";
        let out = ensure_keychain_ref("ignored", existing).unwrap();
        assert_eq!(out, existing);
    }

    #[test]
    fn ensure_keychain_ref_stores_raw_and_returns_canonical_ref() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let key = "conn:test-ensure:password";
        let _ = delete_secret(key);

        let out = ensure_keychain_ref(key, "hunter2").unwrap();
        assert_eq!(out, format_keychain_ref(key));
        assert!(keychain_entry_exists(&out));

        let _ = delete_secret(key);
    }

    #[test]
    fn ensure_keychain_ref_propagates_keychain_failure() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        force_keychain_failure(true);
        let result = ensure_keychain_ref("conn:fail:password", "hunter2");
        force_keychain_failure(false);
        let err = result.expect_err("must fail when keychain unavailable");
        assert!(err.contains("Failed to store secret securely"));
    }

    #[test]
    fn parse_keychain_address_extracts_namespaced_key() {
        assert_eq!(
            parse_keychain_address("{{keychain:conn:pg:password}}"),
            Some("conn:pg:password".to_string())
        );
        assert_eq!(
            parse_keychain_address("{{keychain:env:staging:DB_URL}}"),
            Some("env:staging:DB_URL".to_string())
        );
    }

    #[test]
    fn parse_keychain_address_trims_whitespace() {
        assert_eq!(
            parse_keychain_address("  {{keychain:x:y}}  "),
            Some("x:y".to_string())
        );
    }

    #[test]
    fn parse_keychain_address_rejects_non_reference() {
        assert_eq!(parse_keychain_address("plaintext"), None);
        assert_eq!(parse_keychain_address(""), None);
        assert_eq!(parse_keychain_address("{{not closed"), None);
    }

    #[test]
    fn parse_keychain_address_rejects_non_keychain_backend() {
        assert_eq!(parse_keychain_address("{{1password:Personal/x}}"), None);
        assert_eq!(parse_keychain_address("{{env:GITHUB_TOKEN}}"), None);
    }

    #[test]
    fn parse_keychain_address_rejects_empty_address() {
        assert_eq!(parse_keychain_address("{{keychain:}}"), None);
    }

    #[test]
    fn parse_keychain_address_rejects_missing_separator() {
        assert_eq!(parse_keychain_address("{{keychain}}"), None);
    }

    #[test]
    fn keychain_entry_exists_true_after_store() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let key = "conn:test-exists:password";
        let _ = delete_secret(key);

        let reference = ensure_keychain_ref(key, "value").unwrap();
        assert!(keychain_entry_exists(&reference));

        let _ = delete_secret(key);
        // After deletion the entry no longer exists.
        assert!(!keychain_entry_exists(&reference));
    }

    #[test]
    fn keychain_entry_exists_false_for_non_reference() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        assert!(!keychain_entry_exists("plaintext"));
        assert!(!keychain_entry_exists(""));
        assert!(!keychain_entry_exists("{{1password:x}}"));
    }

    #[test]
    fn keychain_entry_exists_false_under_keychain_failure() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        force_keychain_failure(true);
        let result = keychain_entry_exists("{{keychain:conn:any:password}}");
        force_keychain_failure(false);
        assert!(!result);
    }

    #[test]
    fn resolve_value_passes_plaintext_through() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        assert_eq!(
            resolve_value("plain-value").unwrap(),
            Some("plain-value".to_string())
        );
    }

    #[test]
    fn resolve_value_returns_none_for_empty() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        assert_eq!(resolve_value("").unwrap(), None);
    }

    #[test]
    fn resolve_value_resolves_stored_reference() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let key = "conn:test-resolve:password";
        let _ = delete_secret(key);

        let reference = ensure_keychain_ref(key, "secret-value").unwrap();
        let resolved = resolve_value(&reference).unwrap();
        assert_eq!(resolved, Some("secret-value".to_string()));

        let _ = delete_secret(key);
    }

    #[test]
    fn resolve_value_errors_on_missing_keychain_entry() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        let key = "conn:test-missing:password";
        let _ = delete_secret(key);

        let reference = format_keychain_ref(key);
        let err = resolve_value(&reference).expect_err("must error on missing entry");
        assert!(err.contains("did not resolve"));
    }

    #[test]
    fn resolve_value_errors_on_keychain_backend_failure() {
        let _g = KEYCHAIN_TEST_LOCK.lock().unwrap();
        force_keychain_failure(true);
        let result = resolve_value("{{keychain:conn:any:password}}");
        force_keychain_failure(false);
        let err = result.expect_err("must surface backend failure");
        assert!(err.contains("keychain error"));
    }
}
