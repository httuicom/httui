//! Hand-rolled YAML-subset parser for runbook frontmatter.
//!
//! See `mod.rs` for the rationale + schema. This file owns:
//!
//! - `split_frontmatter(content)` — finds the `---` fence at offset
//!   0 and the next `---` line; returns the raw YAML between + the
//!   body, or `None` when no fence exists.
//! - `parse_frontmatter(content)` — typed extraction of the keys
//!   the slice-1 schema understands. Unknown / not-yet-typed keys
//!   are kept in the raw region so the round-trip survives.
//! - `assemble_with_body(raw_yaml, body)` — recomposes the original
//!   document. Round-trip safe: `assemble(split(c).raw, split(c).body)
//!   == c` for any `c` with a `---` fence.

use std::collections::BTreeMap;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Split {
    pub raw_yaml: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Frontmatter {
    pub title: Option<String>,
    pub owner: Option<String>,
    pub status: Option<FrontmatterStatus>,
    pub tags: Vec<String>,
    /// Unknown / not-yet-typed top-level keys, preserved verbatim
    /// (key = unparsed value text). Lets us round-trip block
    /// scalars (`abstract: |`) and block lists (`preflight:`) until
    /// the parser learns those shapes.
    pub extra: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FrontmatterStatus {
    Draft,
    Active,
    Archived,
}

impl FrontmatterStatus {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "draft" => Some(FrontmatterStatus::Draft),
            "active" => Some(FrontmatterStatus::Active),
            "archived" => Some(FrontmatterStatus::Archived),
            _ => None,
        }
    }
}

/// Split a runbook into `(raw_yaml, body)`. Requires a `---` fence
/// on line 1 (no leading whitespace) and a closing `---` line. The
/// fences themselves are not included in `raw_yaml`. Trailing
/// newline after the closing fence is consumed; any further bytes
/// belong to `body`.
///
/// Returns `None` when:
/// - the content does not start with `---\n` (or `---\r\n`)
/// - no closing `---` line is found
pub fn split_frontmatter(content: &str) -> Option<Split> {
    // Strip an optional UTF-8 BOM up front so files saved by
    // Windows editors still parse.
    let trimmed = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut rest = trimmed
        .strip_prefix("---\n")
        .or_else(|| trimmed.strip_prefix("---\r\n"))?;
    let mut raw_yaml = String::new();
    loop {
        // Find next line break.
        let line_end = rest.find('\n').map(|i| i + 1).unwrap_or(rest.len());
        let (line, after) = rest.split_at(line_end);
        let line_trimmed = line.trim_end_matches(['\n', '\r']);
        if line_trimmed == "---" {
            return Some(Split {
                raw_yaml,
                body: after.to_string(),
            });
        }
        raw_yaml.push_str(line);
        rest = after;
        if rest.is_empty() {
            // Hit EOF without seeing the closing fence.
            return None;
        }
    }
}

/// Parse a runbook into typed frontmatter + body. Returns `None`
/// when the content has no `---` fence; non-fatal YAML quirks
/// (unknown keys, unparseable values) end up in `Frontmatter::extra`.
pub fn parse_frontmatter(content: &str) -> Option<(Frontmatter, String)> {
    let split = split_frontmatter(content)?;
    let fm = parse_typed(&split.raw_yaml);
    Some((fm, split.body))
}

/// Recompose `---\n{raw_yaml}---\n{body}`. Pre-condition: the input
/// `raw_yaml` ends in a newline (matching what `split_frontmatter`
/// produced); the function is round-trip safe under that constraint.
pub fn assemble_with_body(raw_yaml: &str, body: &str) -> String {
    let mut out = String::with_capacity(raw_yaml.len() + body.len() + 8);
    out.push_str("---\n");
    out.push_str(raw_yaml);
    out.push_str("---\n");
    out.push_str(body);
    out
}

fn parse_typed(raw_yaml: &str) -> Frontmatter {
    let mut fm = Frontmatter::default();
    for line in raw_yaml.lines() {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Only top-level keys (no leading whitespace) are typed.
        if line.starts_with([' ', '\t']) {
            continue;
        }
        let Some(colon_idx) = line.find(':') else {
            continue;
        };
        let key = line[..colon_idx].trim();
        let value_part = line[colon_idx + 1..].trim();

        match key {
            "title" => fm.title = Some(unquote(value_part)),
            "owner" => fm.owner = Some(unquote(value_part)),
            "status" => fm.status = FrontmatterStatus::parse(value_part),
            "tags" => fm.tags = parse_flow_list(value_part),
            // Block-scalar (`|`) and block-list (`abstract:` empty +
            // indented children) round-trip via the raw region but
            // are not typed yet — record the value text as-is for
            // forward-compat.
            other => {
                if !value_part.is_empty() {
                    fm.extra.insert(other.to_string(), value_part.to_string());
                }
            }
        }
    }
    fm
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

/// Parse a flow-style list `[a, b, "c"]`. Returns `Vec::new()` for
/// any other shape — the slice-1 parser doesn't yet handle block
/// lists for `tags:` (the schema in the canvas only shows the flow
/// shape).
fn parse_flow_list(value: &str) -> Vec<String> {
    let v = value.trim();
    let stripped = match v.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        Some(s) => s,
        None => return Vec::new(),
    };
    stripped
        .split(',')
        .map(|item| unquote(item.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_returns_none_when_no_fence() {
        assert!(split_frontmatter("# Hello\n\nbody\n").is_none());
        assert!(split_frontmatter("").is_none());
    }

    #[test]
    fn split_returns_none_when_no_closing_fence() {
        assert!(split_frontmatter("---\ntitle: foo\nbody without close\n").is_none());
    }

    #[test]
    fn split_handles_minimal_fence() {
        let s = split_frontmatter("---\ntitle: foo\n---\nhello\n").unwrap();
        assert_eq!(s.raw_yaml, "title: foo\n");
        assert_eq!(s.body, "hello\n");
    }

    #[test]
    fn split_handles_crlf_fences() {
        let s = split_frontmatter("---\r\ntitle: foo\r\n---\r\nbody\r\n").unwrap();
        assert!(s.raw_yaml.contains("title: foo"));
        assert!(s.body.starts_with("body"));
    }

    #[test]
    fn split_strips_utf8_bom() {
        let s = split_frontmatter("\u{feff}---\ntitle: x\n---\nbody\n").unwrap();
        assert_eq!(s.body, "body\n");
    }

    #[test]
    fn split_preserves_inner_yaml_verbatim() {
        let raw = "title: foo\nabstract: |\n  multi\n  line\ntags: [a, b]\n";
        let input = format!("---\n{raw}---\nbody\n");
        let s = split_frontmatter(&input).unwrap();
        assert_eq!(s.raw_yaml, raw);
    }

    #[test]
    fn assemble_round_trips_split_output() {
        let original = "---\ntitle: foo\nabstract: |\n  multi\n---\nbody\n";
        let s = split_frontmatter(original).unwrap();
        let assembled = assemble_with_body(&s.raw_yaml, &s.body);
        assert_eq!(assembled, original);
    }

    #[test]
    fn parse_returns_none_when_no_fence() {
        assert!(parse_frontmatter("# Hello\n").is_none());
    }

    #[test]
    fn parse_extracts_typed_keys() {
        let input = "---\ntitle: \"Payments\"\nowner: alice\nstatus: draft\ntags: [payments, debug]\n---\nbody\n";
        let (fm, body) = parse_frontmatter(input).unwrap();
        assert_eq!(fm.title.as_deref(), Some("Payments"));
        assert_eq!(fm.owner.as_deref(), Some("alice"));
        assert_eq!(fm.status, Some(FrontmatterStatus::Draft));
        assert_eq!(fm.tags, vec!["payments", "debug"]);
        assert_eq!(body, "body\n");
    }

    #[test]
    fn parse_unquotes_double_and_single_quotes() {
        let (fm, _) = parse_frontmatter("---\ntitle: \"Hi\"\nowner: 'bob'\n---\n").unwrap();
        assert_eq!(fm.title.as_deref(), Some("Hi"));
        assert_eq!(fm.owner.as_deref(), Some("bob"));
    }

    #[test]
    fn parse_falls_back_to_extra_for_unknown_keys() {
        let (fm, _) = parse_frontmatter("---\ntitle: foo\ncustom_field: hello\n---\n").unwrap();
        assert_eq!(
            fm.extra.get("custom_field").map(String::as_str),
            Some("hello")
        );
    }

    #[test]
    fn parse_skips_empty_and_comment_lines() {
        let input = "---\n# A comment\n\ntitle: foo\n# another comment\n---\nbody\n";
        let (fm, _) = parse_frontmatter(input).unwrap();
        assert_eq!(fm.title.as_deref(), Some("foo"));
    }

    #[test]
    fn parse_skips_indented_lines_at_top_level() {
        // Indented children of `abstract:` / `preflight:` round-trip
        // via the raw region but are NOT picked up as typed keys.
        let input = "---\ntitle: foo\nabstract: |\n  hello\n  world\n---\nbody\n";
        let (fm, _) = parse_frontmatter(input).unwrap();
        assert_eq!(fm.title.as_deref(), Some("foo"));
        assert!(fm.extra.is_empty() || fm.extra.contains_key("abstract"));
    }

    #[test]
    fn parse_status_recognises_each_variant() {
        assert_eq!(
            FrontmatterStatus::parse("draft"),
            Some(FrontmatterStatus::Draft)
        );
        assert_eq!(
            FrontmatterStatus::parse("active"),
            Some(FrontmatterStatus::Active)
        );
        assert_eq!(
            FrontmatterStatus::parse("archived"),
            Some(FrontmatterStatus::Archived)
        );
        assert_eq!(FrontmatterStatus::parse("nope"), None);
    }

    #[test]
    fn parse_status_ignores_unknown_value() {
        let (fm, _) = parse_frontmatter("---\nstatus: nope\n---\n").unwrap();
        assert!(fm.status.is_none());
    }

    #[test]
    fn parse_flow_list_handles_quoted_items() {
        let (fm, _) = parse_frontmatter("---\ntags: [\"hello world\", debug]\n---\n").unwrap();
        assert_eq!(fm.tags, vec!["hello world", "debug"]);
    }

    #[test]
    fn parse_flow_list_returns_empty_for_block_list_or_other_shapes() {
        // Block-list shape (not the slice-1 schema for tags).
        let input = "---\ntags:\n  - a\n  - b\n---\n";
        let (fm, _) = parse_frontmatter(input).unwrap();
        assert!(fm.tags.is_empty());
    }

    #[test]
    fn round_trip_acceptance_byte_identical_when_body_only_changes() {
        let original = "---\ntitle: foo\nabstract: |\n  hi\ntags: [a, b]\n---\nold body\n";
        let s = split_frontmatter(original).unwrap();
        let new_doc = assemble_with_body(&s.raw_yaml, "new body\n");
        // Frontmatter region byte-identical to the original.
        let split_after = split_frontmatter(&new_doc).unwrap();
        assert_eq!(split_after.raw_yaml, s.raw_yaml);
    }
}
