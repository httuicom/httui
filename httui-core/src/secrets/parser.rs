//! Pure parser for `{{backend:address}}` secret references (ADR 0002).
//!
//! No I/O, no backend dispatch. Two things live here:
//!
//! - [`is_secret_ref`] — fast structural check (does this string look
//!   like a reference at all?)
//! - [`parse_secret_ref`] — split into `(backend, address)` if the
//!   reference is well-formed, otherwise an `Err` describing why
//!
//! The legacy entry points
//! ([`crate::db::keychain::resolve_secret_ref`] and
//! [`crate::vault_config::validate::is_secret_ref`]) stay as thin
//! wrappers so existing callers don't break — they delegate here in a
//! follow-up commit (or earlier opportunistic work).

/// Backends recognised by the parser. New backends are added when their
/// epic lands (introduces `1password`, etc.).
const KNOWN_BACKENDS: &[&str] = &["keychain", "1password", "pass", "env"];

/// Returns `true` when `s` is a structurally valid `{{backend:address}}`
/// reference with a known backend. Whitespace is trimmed before the
/// check so callers don't have to.
pub fn is_secret_ref(s: &str) -> bool {
    parse_secret_ref(s).is_ok()
}

/// Split `s` into `(backend, address)`. Returns an `Err` describing
/// what's missing when the reference is malformed; backends that the
/// parser doesn't yet know about return `Err` so callers can fail
/// loudly rather than silently mis-routing.
pub fn parse_secret_ref(s: &str) -> Result<(&str, &str), ParseError> {
    let trimmed = s.trim();
    if !(trimmed.starts_with("{{") && trimmed.ends_with("}}")) {
        return Err(ParseError::NotAReference);
    }
    let inner = &trimmed[2..trimmed.len() - 2];
    let (backend, address) = inner
        .split_once(':')
        .ok_or(ParseError::MissingBackendSeparator)?;
    if address.is_empty() {
        return Err(ParseError::EmptyAddress);
    }
    if !KNOWN_BACKENDS.contains(&backend) {
        return Err(ParseError::UnknownBackend(backend.to_string()));
    }
    Ok((backend, address))
}

/// Why parsing failed. Carry the offending backend name for
/// `UnknownBackend` so the caller can put it in a user-facing error
/// without re-parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Doesn't start/end with `{{...}}` or is empty.
    NotAReference,
    /// Inside `{{...}}` but no `:` to split backend from address.
    MissingBackendSeparator,
    /// `{{backend:}}` — no address.
    EmptyAddress,
    /// `{{unknown:foo}}` where `unknown` isn't in the supported list.
    UnknownBackend(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAReference => write!(f, "not a secret reference"),
            Self::MissingBackendSeparator => {
                write!(f, "malformed reference (no backend separator)")
            }
            Self::EmptyAddress => write!(f, "empty address in reference"),
            Self::UnknownBackend(b) => write!(f, "unknown secret backend `{b}`"),
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_keychain_reference() {
        let (backend, addr) = parse_secret_ref("{{keychain:conn:pg:password}}").unwrap();
        assert_eq!(backend, "keychain");
        assert_eq!(addr, "conn:pg:password");
    }

    #[test]
    fn parses_1password_reference() {
        let (backend, addr) = parse_secret_ref("{{1password:op://Personal/db/password}}").unwrap();
        assert_eq!(backend, "1password");
        assert_eq!(addr, "op://Personal/db/password");
    }

    #[test]
    fn parses_pass_reference() {
        let (backend, addr) = parse_secret_ref("{{pass:databases/staging}}").unwrap();
        assert_eq!(backend, "pass");
        assert_eq!(addr, "databases/staging");
    }

    #[test]
    fn parses_env_reference() {
        let (backend, addr) = parse_secret_ref("{{env:DB_PASSWORD}}").unwrap();
        assert_eq!(backend, "env");
        assert_eq!(addr, "DB_PASSWORD");
    }

    #[test]
    fn trims_whitespace() {
        assert!(parse_secret_ref("  {{keychain:x}}  ").is_ok());
    }

    #[test]
    fn rejects_plain_value() {
        assert_eq!(parse_secret_ref("hunter2"), Err(ParseError::NotAReference));
    }

    #[test]
    fn rejects_empty_string() {
        assert_eq!(parse_secret_ref(""), Err(ParseError::NotAReference));
    }

    #[test]
    fn rejects_missing_separator() {
        assert_eq!(
            parse_secret_ref("{{nobackend}}"),
            Err(ParseError::MissingBackendSeparator)
        );
    }

    #[test]
    fn rejects_empty_address() {
        assert_eq!(
            parse_secret_ref("{{keychain:}}"),
            Err(ParseError::EmptyAddress)
        );
    }

    #[test]
    fn rejects_unknown_backend() {
        match parse_secret_ref("{{vault:secret/foo}}") {
            Err(ParseError::UnknownBackend(b)) => assert_eq!(b, "vault"),
            other => panic!("expected UnknownBackend, got {other:?}"),
        }
    }

    #[test]
    fn is_secret_ref_matches_parse_secret_ref() {
        assert!(is_secret_ref("{{keychain:x}}"));
        assert!(!is_secret_ref("plain"));
        assert!(!is_secret_ref("{{nope}}"));
    }

    #[test]
    fn parse_error_display_is_human_readable() {
        assert_eq!(
            ParseError::NotAReference.to_string(),
            "not a secret reference"
        );
        assert_eq!(
            ParseError::MissingBackendSeparator.to_string(),
            "malformed reference (no backend separator)"
        );
        assert_eq!(
            ParseError::EmptyAddress.to_string(),
            "empty address in reference"
        );
        assert_eq!(
            ParseError::UnknownBackend("vault".into()).to_string(),
            "unknown secret backend `vault`"
        );
    }
}
