//! Schema validation: anti-cleartext-secret checks and structural
//! constraints (e.g. `[secrets]` values must be references).
//!
//! Two surfaces:
//!
//! - [`is_secret_ref`] — fast structural check ("is this string a
//!   `{{backend:address}}` reference?")
//! - [`validate_*`] family — schema-level checks that produce a
//!   `Vec<Issue>` summarizing both errors (must-fix) and warnings.
//!
//! The validator runs after `serde` deserialization succeeds. Anything
//! that breaks deserialization (wrong type, unknown variant, bad TOML)
//! surfaces as a parse error before we get here.

use std::fmt;

use once_cell::sync::Lazy;
use regex::Regex;

use super::connections::ConnectionsFile;
use super::envs::EnvFile;

/// True when `s` is a `{{backend:address}}` secret reference (ADR 0002).
pub fn is_secret_ref(s: &str) -> bool {
    let trimmed = s.trim();
    if !(trimmed.starts_with("{{") && trimmed.ends_with("}}")) {
        return false;
    }
    let inner = &trimmed[2..trimmed.len() - 2];
    let Some((backend, rest)) = inner.split_once(':') else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    matches!(backend, "keychain" | "1password" | "pass" | "env")
}

/// Field names that nudge the validator toward "this should probably be
/// a `{{...}}` reference, not a literal." The list intentionally errs on
/// the side of false positives — the `# httui:allow-cleartext` escape
/// hatch (ADR 0002) is the user's way to silence a warning.
const SENSITIVE_FIELD_NAMES: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "token",
    "secret",
    "auth",
    "authorization",
    "api_key",
    "apikey",
    "x-api-key",
    "credentials",
    "credentials_path",
    "credentials_json",
    "user",
    "username",
];

fn is_sensitive_field_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    SENSITIVE_FIELD_NAMES.iter().any(|s| lower == *s)
}

/// High-confidence patterns matching well-known credential formats.
/// A literal value matching any of these (in any field) raises a
/// warning even if the field name doesn't look sensitive.
static SECRET_PATTERNS: Lazy<Vec<(&'static str, Regex)>> = Lazy::new(|| {
    vec![
        ("AWS access key", Regex::new(r"^AKIA[0-9A-Z]{16}$").unwrap()),
        (
            "GitHub token",
            Regex::new(r"^gh[pousr]_[A-Za-z0-9]{36,}$").unwrap(),
        ),
        (
            "Slack token",
            Regex::new(r"^xox[abprs]-[A-Za-z0-9-]{10,}$").unwrap(),
        ),
        (
            "JSON Web Token",
            Regex::new(r"^eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$").unwrap(),
        ),
        (
            "PEM private key",
            Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----").unwrap(),
        ),
    ]
});

fn matches_known_secret(value: &str) -> Option<&'static str> {
    SECRET_PATTERNS
        .iter()
        .find(|(_, re)| re.is_match(value))
        .map(|(name, _)| *name)
}

/// A validation finding. Errors must be fixed; warnings nudge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub severity: Severity,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "[{tag}] {}: {}", self.path, self.message)
    }
}

/// Groups issues from one validation pass.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub issues: Vec<Issue>,
}

impl Report {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    fn push_error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
        });
    }

    fn push_warning(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(Issue {
            severity: Severity::Warning,
            path: path.into(),
            message: message.into(),
        });
    }
}

pub(super) fn check_field(report: &mut Report, path: &str, name: &str, value: &str) {
    if is_secret_ref(value) {
        return;
    }
    if let Some(secret_kind) = matches_known_secret(value) {
        report.push_warning(
            path,
            format!(
                "value matches {secret_kind} pattern; replace with a {{{{...}}}} reference \
                 (ADR 0002) or add `# httui:allow-cleartext` if it's truly safe"
            ),
        );
        return;
    }
    if is_sensitive_field_name(name) {
        report.push_warning(
            path,
            format!(
                "field `{name}` typically holds a secret; consider replacing the literal \
                 with a {{{{keychain:...}}}} reference (ADR 0002)"
            ),
        );
    }
}

/// Validate an `envs/{name}.toml` post-deserialization.
///
/// - `[secrets]` entries MUST be references (hard error).
/// - `[vars]` entries warn on sensitive-looking values.
/// - Cross-section key collision warns.
pub fn validate_env_file(file: &EnvFile) -> Report {
    let mut report = Report::default();

    for (key, value) in &file.secrets {
        let path = format!("[secrets].{key}");
        if !is_secret_ref(value) {
            report.push_error(
                &path,
                "value in [secrets] must be a {{backend:address}} reference (ADR 0002), \
                 not a literal — move the literal to [vars] if it isn't actually secret",
            );
        }
    }

    for (key, value) in &file.vars {
        let path = format!("[vars].{key}");
        check_field(&mut report, &path, key, value);
    }

    for key in file.vars.keys() {
        if file.secrets.contains_key(key) {
            report.push_warning(
                format!("[vars]/[secrets].{key}"),
                "key is defined in both [vars] and [secrets]; resolution will prefer \
                 [secrets] but the duplicate is almost certainly a mistake",
            );
        }
    }

    report
}

/// Validate a `connections.toml` post-deserialization.
///
/// Per-variant field checks live on each `impl DbConnection` —
/// adding a new connection type adds its own `validate_fields`
/// override and this function dispatches automatically. SQLite, WS,
/// gRPC, GraphQL, and Shell variants don't override (no
/// conventionally-sensitive fields), so they fall through to the
/// trait's default empty implementation.
pub fn validate_connections_file(file: &ConnectionsFile) -> Report {
    let mut report = Report::default();
    for (name, conn) in &file.connections {
        conn.as_dyn().validate_fields(name, &mut report);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault_config::connections::ConnectionsFile;
    use crate::vault_config::envs::EnvFile;

    #[test]
    fn detects_known_backends() {
        assert!(is_secret_ref("{{keychain:ns:key}}"));
        assert!(is_secret_ref("{{1password:op://vault/item/field}}"));
        assert!(is_secret_ref("{{pass:work/db}}"));
        assert!(is_secret_ref("{{env:GITHUB_TOKEN}}"));
    }

    #[test]
    fn rejects_non_refs() {
        assert!(!is_secret_ref("literal value"));
        assert!(!is_secret_ref("{{NO_COLON}}"));
        assert!(!is_secret_ref("{{unknown:something}}"));
        assert!(!is_secret_ref("{{keychain:}}"));
        assert!(!is_secret_ref(""));
        assert!(!is_secret_ref("{{req1.response.body.id}}"));
    }

    #[test]
    fn env_with_only_refs_in_secrets_is_clean() {
        let raw = r#"
version = "1"
[vars]
BASE_URL = "https://api.example.com"
[secrets]
TOKEN = "{{keychain:env:staging:TOKEN}}"
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        let r = validate_env_file(&f);
        assert!(r.is_clean(), "expected clean, got: {:?}", r.issues);
    }

    #[test]
    fn env_rejects_literal_in_secrets() {
        let raw = r#"
version = "1"
[secrets]
TOKEN = "raw-token-value-here-not-good"
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        let r = validate_env_file(&f);
        assert!(r.has_errors());
        assert!(r.issues.iter().any(|i| i.path == "[secrets].TOKEN"));
    }

    #[test]
    fn env_warns_on_sensitive_var_name() {
        let raw = r#"
version = "1"
[vars]
PASSWORD = "literal-bad"
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        let r = validate_env_file(&f);
        assert!(!r.has_errors());
        assert!(r.issues.iter().any(|i| i.severity == Severity::Warning));
    }

    #[test]
    fn env_warns_on_jwt_in_var() {
        // JWT pattern in a non-sensitively-named var still triggers regex check.
        let raw = r#"
version = "1"
[vars]
SOMETHING = "eyJabc.eyJxyz.signature"
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        let r = validate_env_file(&f);
        assert!(r
            .issues
            .iter()
            .any(|i| i.message.contains("JSON Web Token")));
    }

    #[test]
    fn env_warns_on_aws_key_pattern() {
        let raw = r#"
version = "1"
[vars]
KEY = "AKIAIOSFODNN7EXAMPLE"
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        let r = validate_env_file(&f);
        assert!(r
            .issues
            .iter()
            .any(|i| i.message.contains("AWS access key")));
    }

    #[test]
    fn env_collision_between_vars_and_secrets_warns() {
        let raw = r#"
version = "1"
[vars]
TOKEN = "literal"
[secrets]
TOKEN = "{{keychain:env:staging:TOKEN}}"
"#;
        let f: EnvFile = toml::from_str(raw).unwrap();
        let r = validate_env_file(&f);
        assert!(r
            .issues
            .iter()
            .any(|i| i.path.contains("[vars]/[secrets].TOKEN")));
    }

    #[test]
    fn connections_with_refs_are_clean() {
        let raw = r#"
version = "1"
[connections.pg]
type = "postgres"
host = "h"
port = 5432
database = "d"
user = "{{keychain:pg:user}}"
password = "{{keychain:pg:password}}"
"#;
        let f: ConnectionsFile = toml::from_str(raw).unwrap();
        let r = validate_connections_file(&f);
        assert!(r.is_clean(), "expected clean, got: {:?}", r.issues);
    }

    #[test]
    fn connections_warn_on_literal_user_and_password() {
        let raw = r#"
version = "1"
[connections.pg]
type = "postgres"
host = "h"
port = 5432
database = "d"
user = "app"
password = "hunter2"
"#;
        let f: ConnectionsFile = toml::from_str(raw).unwrap();
        let r = validate_connections_file(&f);
        let warnings: Vec<_> = r
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .collect();
        assert!(warnings.iter().any(|i| i.path.ends_with(".user")));
        assert!(warnings.iter().any(|i| i.path.ends_with(".password")));
    }

    #[test]
    fn connections_http_default_headers_authorization_warns() {
        let raw = r#"
version = "1"
[connections.api]
type = "http"
base_url = "https://api.example.com"
default_headers = { "Authorization" = "Bearer ghp_abcdefghijklmnopqrstuvwxyz0123456789" }
"#;
        let f: ConnectionsFile = toml::from_str(raw).unwrap();
        let r = validate_connections_file(&f);
        // Either GitHub-token regex or sensitive name catches it.
        assert!(!r.is_clean());
    }
}
