//! Parser for the `preflight:` block-list inside YAML frontmatter.
//!
//! Input shape (one item per line under the `preflight:` parent):
//!
//! ```text
//! preflight:
//!   - connection: payments-db
//!   - env_var: API_TOKEN
//!   - branch: main
//!   - file_exists: ./schema/payments.sql
//!   - command: psql --version
//! ```
//!
//! The parser is deliberately tolerant: any line that doesn't match
//! the expected shape is dropped (with no error) so the surrounding
//! frontmatter parser stays generic. Items with unknown keys are
//! recorded as `PreflightItem::Unknown` for forward-compat — the
//! evaluator (Story 02) can choose to surface them as `Skip { reason
//! "unknown check kind" }` later.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PreflightItem {
    Connection { name: String },
    EnvVar { name: String },
    Branch { name: String },
    FileExists { path: String },
    Command { command: String },
    Unknown { key: String, value: String },
}

/// Parse the typed `preflight:` items out of a raw YAML region.
/// `raw_yaml` is the inner content of the `---` fence as produced
/// by `crate::frontmatter::split_frontmatter`. Returns `Vec::new()`
/// when no `preflight:` section exists.
pub fn parse_preflight(raw_yaml: &str) -> Vec<PreflightItem> {
    let mut out = Vec::new();
    let mut lines = raw_yaml.lines().peekable();
    while let Some(line) = lines.next() {
        if !line_is_preflight_header(line) {
            continue;
        }
        // Collect the indented children that follow.
        while let Some(&next) = lines.peek() {
            // Stop at the next un-indented top-level key or blank
            // line (block-list ends).
            if next.trim().is_empty() {
                lines.next();
                continue;
            }
            if !next.starts_with([' ', '\t']) {
                break;
            }
            // Consume the line.
            lines.next();
            if let Some(item) = parse_item(next) {
                out.push(item);
            }
        }
        break;
    }
    out
}

fn line_is_preflight_header(line: &str) -> bool {
    // `preflight:` at the start of the line, optionally followed by
    // whitespace and nothing else (no inline value — block lists
    // require an empty value).
    let trimmed = line.trim_end_matches(['\r', '\n']);
    trimmed == "preflight:"
}

fn parse_item(line: &str) -> Option<PreflightItem> {
    // Strip leading whitespace + the `- ` marker.
    let trimmed = line.trim_start();
    let body = trimmed.strip_prefix("- ")?;
    let colon_idx = body.find(':')?;
    let key = body[..colon_idx].trim().to_string();
    let value = body[colon_idx + 1..].trim();
    let unquoted = unquote(value);
    if key.is_empty() || unquoted.is_empty() {
        return None;
    }
    Some(match key.as_str() {
        "connection" => PreflightItem::Connection { name: unquoted },
        "env_var" => PreflightItem::EnvVar { name: unquoted },
        "branch" => PreflightItem::Branch { name: unquoted },
        "file_exists" => PreflightItem::FileExists { path: unquoted },
        "command" => PreflightItem::Command { command: unquoted },
        _ => PreflightItem::Unknown {
            key,
            value: unquoted,
        },
    })
}

fn unquote(value: &str) -> String {
    let v = value.trim();
    if (v.starts_with('"') && v.ends_with('"') && v.len() >= 2)
        || (v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2)
    {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_returns_empty_when_no_preflight_section() {
        let raw = "title: foo\nowner: alice\n";
        assert_eq!(parse_preflight(raw), Vec::<PreflightItem>::new());
    }

    #[test]
    fn parse_returns_empty_when_preflight_is_inline_scalar() {
        // `preflight: foo` is not a valid block-list header.
        let raw = "preflight: foo\n";
        assert_eq!(parse_preflight(raw), Vec::<PreflightItem>::new());
    }

    #[test]
    fn parse_extracts_all_five_documented_kinds() {
        let raw = "preflight:\n  - connection: payments-db\n  - env_var: API_TOKEN\n  - branch: main\n  - file_exists: ./schema/payments.sql\n  - command: psql --version\n";
        let items = parse_preflight(raw);
        assert_eq!(items.len(), 5);
        assert_eq!(
            items[0],
            PreflightItem::Connection {
                name: "payments-db".into()
            }
        );
        assert_eq!(
            items[1],
            PreflightItem::EnvVar {
                name: "API_TOKEN".into()
            }
        );
        assert_eq!(
            items[2],
            PreflightItem::Branch {
                name: "main".into()
            }
        );
        assert_eq!(
            items[3],
            PreflightItem::FileExists {
                path: "./schema/payments.sql".into()
            }
        );
        assert_eq!(
            items[4],
            PreflightItem::Command {
                command: "psql --version".into()
            }
        );
    }

    #[test]
    fn parse_records_keychain_as_unknown_kind() {
        // V6 cenário 9: keychain was removed from the typed set. Legacy
        // YAML that still uses it falls through to Unknown so the file
        // doesn't break — the pill renders as a skip with the key.
        let raw = "preflight:\n  - keychain: payments-db.password\n";
        let items = parse_preflight(raw);
        assert_eq!(
            items,
            vec![PreflightItem::Unknown {
                key: "keychain".into(),
                value: "payments-db.password".into()
            }]
        );
    }

    #[test]
    fn parse_unquotes_double_and_single_quoted_values() {
        let raw = "preflight:\n  - connection: \"my-db\"\n  - branch: 'main'\n";
        let items = parse_preflight(raw);
        assert_eq!(
            items,
            vec![
                PreflightItem::Connection {
                    name: "my-db".into()
                },
                PreflightItem::Branch {
                    name: "main".into()
                },
            ]
        );
    }

    #[test]
    fn parse_records_unknown_keys_for_forward_compat() {
        let raw = "preflight:\n  - new_kind: hello\n";
        let items = parse_preflight(raw);
        assert_eq!(
            items,
            vec![PreflightItem::Unknown {
                key: "new_kind".into(),
                value: "hello".into()
            }]
        );
    }

    #[test]
    fn parse_drops_lines_without_dash_marker() {
        let raw = "preflight:\n  not_a_list_item\n  - connection: ok\n";
        let items = parse_preflight(raw);
        assert_eq!(items, vec![PreflightItem::Connection { name: "ok".into() }]);
    }

    #[test]
    fn parse_drops_items_with_empty_key_or_value() {
        let raw = "preflight:\n  - : hello\n  - connection:\n  - connection: ok\n";
        let items = parse_preflight(raw);
        assert_eq!(items, vec![PreflightItem::Connection { name: "ok".into() }]);
    }

    #[test]
    fn parse_stops_at_next_top_level_key() {
        let raw = "preflight:\n  - connection: a\n  - connection: b\nowner: alice\n  - connection: ignored\n";
        let items = parse_preflight(raw);
        assert_eq!(
            items,
            vec![
                PreflightItem::Connection { name: "a".into() },
                PreflightItem::Connection { name: "b".into() },
            ]
        );
    }

    #[test]
    fn parse_tolerates_blank_lines_between_items() {
        let raw = "preflight:\n  - connection: a\n\n  - connection: b\n";
        let items = parse_preflight(raw);
        assert_eq!(
            items,
            vec![
                PreflightItem::Connection { name: "a".into() },
                PreflightItem::Connection { name: "b".into() },
            ]
        );
    }

    #[test]
    fn parse_handles_tab_indentation() {
        let raw = "preflight:\n\t- connection: a\n";
        let items = parse_preflight(raw);
        assert_eq!(items, vec![PreflightItem::Connection { name: "a".into() }]);
    }

    #[test]
    fn parse_only_handles_first_preflight_section() {
        // Two `preflight:` sections is not valid YAML; the parser
        // stops after the first one and ignores the rest.
        let raw = "preflight:\n  - connection: a\nowner: alice\npreflight:\n  - connection: b\n";
        let items = parse_preflight(raw);
        assert_eq!(items, vec![PreflightItem::Connection { name: "a".into() }]);
    }

    #[test]
    fn parse_serializes_kind_via_serde() {
        let item = PreflightItem::Connection { name: "db".into() };
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("\"kind\":\"connection\""));
        assert!(json.contains("\"name\":\"db\""));
    }
}
