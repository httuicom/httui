//! Cross-block usage footer for BLOCKS view.
//!
//! For each captured ref the focused block exposes (alias + dot path),
//! shows where other blocks in the vault consume it. Shape per line:
//!
//! ```text
//! ↓ {path} = {short_value} used by {file:line} · {file:line} · +N more
//! ```
//!
//! The footer is rendered as N lines (one per ref path, capped). When
//! no consumer is found, `compute_footer_lines` returns an empty vec
//! and the caller drops the layout slot — empty rail never paints.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use std::collections::BTreeMap;
use std::path::Path;

use super::view::ParsedView;
use crate::app::{BlockMeta, FileBlocks};

pub(super) struct UsageLine {
    /// Sub-path on the captured value (e.g. `body.id`). Empty when
    /// consumers use the plain `{{alias}}` form (no dot).
    pub path: String,
    /// Short scalar from the cached result; `None` if the block hasn't
    /// run yet or the path can't be resolved.
    pub value: Option<String>,
    /// Consumers as `basename:line`. Caller-capped.
    pub consumers: Vec<String>,
}

const MAX_PATHS: usize = 3;
const MAX_CONSUMERS: usize = 3;
const VALUE_MAX_CHARS: usize = 32;

pub(super) fn compute_footer_lines(
    vault: &Path,
    file: &FileBlocks,
    block: &BlockMeta,
    parsed: &ParsedView,
) -> Vec<UsageLine> {
    let alias = match block.alias.as_deref() {
        Some(a) if !a.trim().is_empty() => a.trim(),
        _ => return Vec::new(),
    };
    let entries = match httui_core::var_uses::grep_var_uses(&vault.to_string_lossy(), alias) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let own_file = file.path.to_string_lossy().replace('\\', "/");
    let _ = block.line_start;

    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in entries {
        // Filter own file entirely — same-file refs are rarely useful
        // and the V1 line-range filter would need a re-parse to know
        // the block's end line.
        if entry.file_path == own_file {
            continue;
        }
        let paths = extract_ref_paths(&entry.snippet, alias);
        if paths.is_empty() {
            continue;
        }
        let consumer = format!("{}:{}", short_path(&entry.file_path), entry.line);
        for p in paths {
            grouped.entry(p).or_default().push(consumer.clone());
        }
    }

    let mut out: Vec<UsageLine> = grouped
        .into_iter()
        .map(|(path, mut consumers)| {
            consumers.sort();
            consumers.dedup();
            UsageLine {
                value: resolve_value(parsed.cached_json.as_ref(), &path),
                path,
                consumers,
            }
        })
        .collect();
    out.truncate(MAX_PATHS);
    out
}

pub(super) fn render_block_usage_footer(frame: &mut Frame, area: Rect, lines: &[UsageLine]) {
    if lines.is_empty() || area.height == 0 || area.width == 0 {
        return;
    }
    let constraints: Vec<Constraint> = (0..lines.len()).map(|_| Constraint::Length(1)).collect();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    for (i, ul) in lines.iter().enumerate().take(chunks.len()) {
        let row = chunks[i];
        if row.height == 0 {
            continue;
        }
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(8);
        spans.push(Span::styled(
            "↓ ".to_string(),
            Style::default().fg(crate::ui::palette::accent()),
        ));
        if !ul.path.is_empty() {
            spans.push(Span::styled(
                ul.path.clone(),
                Style::default()
                    .fg(crate::ui::palette::foreground())
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if let Some(v) = &ul.value {
            spans.push(Span::raw(" = ".to_string()));
            spans.push(Span::styled(
                v.clone(),
                Style::default().fg(crate::ui::palette::muted()),
            ));
        }
        let shown_count = ul.consumers.len().min(MAX_CONSUMERS);
        let mut joined = ul.consumers[..shown_count].join(" · ");
        if ul.consumers.len() > MAX_CONSUMERS {
            joined.push_str(&format!(" · +{} more", ul.consumers.len() - MAX_CONSUMERS));
        }
        spans.push(Span::raw(" used by ".to_string()));
        spans.push(Span::styled(
            joined,
            Style::default().fg(crate::ui::palette::muted()),
        ));
        frame.render_widget(Paragraph::new(Line::from(spans)), row);
    }
}

/// Scan a line for `{{alias}}` (path "") and `{{alias.<path>}}` and
/// return the path component (or empty string for the plain form).
fn extract_ref_paths(snippet: &str, alias: &str) -> Vec<String> {
    let needle = format!("{{{{{alias}");
    let mut paths = Vec::new();
    let mut i = 0usize;
    while i < snippet.len() {
        let Some(rel) = snippet[i..].find(&needle) else {
            break;
        };
        let after = i + rel + needle.len();
        let rest = &snippet[after..];
        if rest.starts_with("}}") {
            paths.push(String::new());
            i = after + 2;
            continue;
        }
        if rest.starts_with('.') {
            if let Some(end) = rest.find("}}") {
                let path = &rest[1..end];
                paths.push(path.to_string());
                i = after + end + 2;
                continue;
            }
        }
        // Either a longer alias prefix (e.g. matching "API" inside
        // "API_BASE") or an unterminated brace — skip past the prefix.
        i = after;
    }
    paths
}

fn resolve_value(json: Option<&serde_json::Value>, path: &str) -> Option<String> {
    let mut cur = json?;
    if !path.is_empty() {
        for seg in path.split('.') {
            cur = cur.get(seg)?;
        }
    }
    Some(short_value(cur))
}

fn short_value(v: &serde_json::Value) -> String {
    let raw = match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => "null".to_string(),
        _ => v.to_string(),
    };
    if raw.chars().count() > VALUE_MAX_CHARS {
        let s: String = raw.chars().take(VALUE_MAX_CHARS).collect();
        format!("{s}…")
    } else {
        raw
    }
}

fn short_path(rel: &str) -> String {
    rel.rsplit_once('/')
        .map(|(_, b)| b.to_string())
        .unwrap_or_else(|| rel.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use serde_json::json;
    use std::fs;
    use std::path::PathBuf;

    struct TempVault {
        path: PathBuf,
    }

    impl TempVault {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join("httui-blocks-footer")
                .join(format!("{label}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }
        fn write(&self, rel: &str, content: &str) {
            let p = self.path.join(rel);
            if let Some(parent) = p.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(&p, content).unwrap();
        }
    }

    impl Drop for TempVault {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn block(alias: &str) -> BlockMeta {
        BlockMeta {
            block_type: "http".into(),
            alias: Some(alias.into()),
            line_start: 0,
        }
    }

    fn file(rel: &str) -> FileBlocks {
        FileBlocks {
            path: PathBuf::from(rel),
            display: rel.into(),
            blocks: Vec::new(),
        }
    }

    fn parsed_with_json(json: Option<serde_json::Value>) -> ParsedView {
        let mut v = ParsedView::empty();
        v.cached_json = json;
        v
    }

    #[test]
    fn no_alias_yields_no_lines() {
        let v = TempVault::new("no-alias");
        v.write("a.md", "consumer: {{whatever.body}}");
        let mut b = block("ignored");
        b.alias = None;
        let lines = compute_footer_lines(&v.path, &file("self.md"), &b, &ParsedView::empty());
        assert!(lines.is_empty());
    }

    #[test]
    fn no_consumers_yields_no_lines() {
        let v = TempVault::new("none");
        v.write("a.md", "totally unrelated content");
        let lines = compute_footer_lines(
            &v.path,
            &file("self.md"),
            &block("createUser"),
            &ParsedView::empty(),
        );
        assert!(lines.is_empty());
    }

    #[test]
    fn excludes_refs_inside_own_file() {
        let v = TempVault::new("self");
        v.write(
            "self.md",
            "{{createUser.body.id}}\nfiller\n{{createUser}}\n",
        );
        let lines = compute_footer_lines(
            &v.path,
            &file("self.md"),
            &block("createUser"),
            &ParsedView::empty(),
        );
        assert!(lines.is_empty());
    }

    #[test]
    fn groups_consumers_by_ref_path() {
        let v = TempVault::new("group");
        v.write(
            "audit.md",
            "INSERT INTO log SELECT {{createUser.body.id}}\n",
        );
        v.write(
            "route.md",
            "PATCH /users/{{createUser.body.id}}\nDELETE /sess/{{createUser.body.session_id}}\n",
        );
        let lines = compute_footer_lines(
            &v.path,
            &file("self.md"),
            &block("createUser"),
            &parsed_with_json(Some(json!({
                "body": {"id": "usr_abc123", "session_id": "ses_xyz"}
            }))),
        );
        assert_eq!(lines.len(), 2);
        let by_path: BTreeMap<&str, &UsageLine> =
            lines.iter().map(|u| (u.path.as_str(), u)).collect();
        let id = by_path.get("body.id").expect("body.id line");
        assert_eq!(id.consumers.len(), 2);
        assert!(id.consumers.iter().any(|c| c.starts_with("audit.md:")));
        assert!(id.consumers.iter().any(|c| c.starts_with("route.md:")));
        assert_eq!(id.value.as_deref(), Some("usr_abc123"));
        let sid = by_path.get("body.session_id").expect("session_id line");
        assert_eq!(sid.value.as_deref(), Some("ses_xyz"));
    }

    #[test]
    fn caps_paths_at_three() {
        let v = TempVault::new("cap-paths");
        v.write("u.md", "{{a.p1}} {{a.p2}}\n{{a.p3}}\n{{a.p4}}\n{{a.p5}}\n");
        let lines =
            compute_footer_lines(&v.path, &file("self.md"), &block("a"), &ParsedView::empty());
        assert_eq!(lines.len(), MAX_PATHS);
    }

    #[test]
    fn extract_ref_paths_handles_plain_and_dotted() {
        let paths = extract_ref_paths("x {{api}} y {{api.body.id}} z {{api.body}}", "api");
        assert_eq!(paths, vec!["", "body.id", "body"]);
    }

    #[test]
    fn extract_ref_paths_skips_longer_alias_prefix() {
        // {{API_BASE}} is NOT a use of `{{API}}` — extractor must not
        // capture it as path "_BASE".
        let paths = extract_ref_paths("{{API_BASE}}/users {{API.body}}", "API");
        assert_eq!(paths, vec!["body".to_string()]);
    }

    #[test]
    fn resolve_value_navigates_dot_path() {
        let json = json!({"body": {"id": "abc", "n": 7}});
        assert_eq!(resolve_value(Some(&json), "body.id"), Some("abc".into()));
        assert_eq!(resolve_value(Some(&json), "body.n"), Some("7".into()));
        assert_eq!(resolve_value(Some(&json), "missing"), None);
        assert_eq!(resolve_value(None, "body"), None);
    }

    #[test]
    fn resolve_value_truncates_long_strings() {
        let long = "x".repeat(VALUE_MAX_CHARS + 20);
        let json = json!({"v": long});
        let v = resolve_value(Some(&json), "v").unwrap();
        assert!(v.ends_with('…'));
        assert_eq!(v.chars().count(), VALUE_MAX_CHARS + 1);
    }

    #[test]
    fn short_path_strips_directory_prefix() {
        assert_eq!(short_path("dir/sub/note.md"), "note.md");
        assert_eq!(short_path("note.md"), "note.md");
    }

    fn render(lines: &[UsageLine], w: u16, h: u16) -> String {
        let backend = TestBackend::new(w, h);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| {
                render_block_usage_footer(f, Rect::new(0, 0, w, h), lines);
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        (0..h)
            .map(|y| {
                let line: String = (0..w)
                    .map(|x| buf.cell((x, y)).unwrap().symbol().to_string())
                    .collect();
                line.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_arrow_path_value_and_consumers() {
        let lines = vec![UsageLine {
            path: "body.id".into(),
            value: Some("usr_abc123".into()),
            consumers: vec!["audit.md:5".into(), "route.md:12".into()],
        }];
        let text = render(&lines, 80, 1);
        assert!(text.contains("↓"));
        assert!(text.contains("body.id"));
        assert!(text.contains("usr_abc123"));
        assert!(text.contains("used by"));
        assert!(text.contains("audit.md:5"));
        assert!(text.contains("route.md:12"));
    }

    #[test]
    fn renders_plus_more_when_consumers_exceed_cap() {
        let lines = vec![UsageLine {
            path: "id".into(),
            value: None,
            consumers: (1..=5).map(|i| format!("f{i}.md:1")).collect(),
        }];
        let text = render(&lines, 80, 1);
        assert!(text.contains("+2 more"));
    }

    #[test]
    fn empty_lines_paints_nothing() {
        let text = render(&[], 40, 1);
        assert_eq!(text.trim(), "");
    }
}
