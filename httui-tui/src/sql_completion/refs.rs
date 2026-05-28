use super::*;

/// What the detector found inside an open `{{...}}` ref. Returned
/// by `detect_ref_context` when the cursor sits between an opener
/// and the matching `}}`. The completion engine uses it to switch
/// off the SQL path entirely (refs and SQL keywords don't mix).
///
/// Splitting on `.` matters because the engine surfaces *different*
/// items per segment:
/// - segment 1 (`{{|}}` or `{{q1|}}`) → alias names + env vars
/// - segment 2+ (`{{q1.|}}` or `{{q1.id|}}`) → keys of that
///   block's cached result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefDetect {
    /// Where the *prefix* word starts in the body (column on the
    /// current line). Used by the popup's accept handler to know
    /// how many characters to backspace before splicing the chosen
    /// label.
    pub anchor_offset: usize,
    /// What the user has typed since the last `{{` or `.` — the
    /// item the popup will filter on. Empty when the cursor sits
    /// right after `{{` or `.`.
    pub prefix: String,
    /// Path segments before the current one. `None` for the first
    /// segment after `{{`; `Some("q1")` for `{{q1.|}}`;
    /// `Some("q1.response")` for `{{q1.response.|}}`.
    pub path: Option<String>,
}

/// Detect whether the cursor sits inside an open `{{...}}` ref. V1
/// only walks the current line — refs spanning multiple lines are
/// rare in practice and would complicate detection a lot. Returns
/// `None` when there's no open `{{` to the left, or when there's
/// already a `}}` between the opener and the cursor (the ref is
/// closed; we're back in plain SQL).
pub fn detect_ref_context(body: &str, line: usize, cursor_offset: usize) -> Option<RefDetect> {
    let line_text = body.lines().nth(line)?;
    let chars: Vec<char> = line_text.chars().collect();
    let take = cursor_offset.min(chars.len());
    let head: String = chars[..take].iter().collect();

    // Find the *last* `{{` to the left, then make sure no `}}`
    // appears between it and the cursor. If a `}}` is present, the
    // ref is closed and we're back in plain SQL.
    let last_open = head.rfind("{{")?;
    let after_open = &head[last_open + 2..];
    if after_open.contains("}}") {
        return None;
    }

    // The current segment is everything since the last `.` (or the
    // whole `after_open` when there's no dot yet). The path is
    // everything before that dot.
    let (path, prefix) = match after_open.rfind('.') {
        Some(dot_idx) => (
            Some(after_open[..dot_idx].to_string()),
            after_open[dot_idx + 1..].to_string(),
        ),
        None => (None, after_open.to_string()),
    };

    let anchor_offset = cursor_offset.saturating_sub(prefix.chars().count());
    Some(RefDetect {
        anchor_offset,
        prefix,
        path,
    })
}

/// Build the candidate list for the ref popup. When `detect.path`
/// is `None`, surfaces aliases of blocks above `current_segment`
/// plus env vars from the active environment.
///
/// When `detect.path` is set, walks a synthetic
/// `{response: <cached>, status: "..."}` envelope — same shape the
/// desktop builds in `references.ts:140-143` — and emits the keys
/// of whatever value the path lands on. Pure JSON walk: arrays
/// contribute their numeric indices, objects their keys, primitives
/// nothing (popup closes). The legacy `{{alias.col}}` first-row
/// shim is *not* surfaced here — it's a runtime resolver shim, not
/// an autocomplete suggestion.
pub fn complete_refs(
    detect: &RefDetect,
    segments: &[crate::buffer::Segment],
    current_segment: usize,
    env_vars: &std::collections::HashMap<String, String>,
) -> Vec<CompletionItem> {
    let prefix_lower = detect.prefix.to_ascii_lowercase();
    let mut out: Vec<CompletionItem> = Vec::new();

    let Some(path) = detect.path.as_deref() else {
        // Top-level: aliases first (most-typed), env vars after.
        for seg in segments.iter().take(current_segment) {
            if let crate::buffer::Segment::Block(b) = seg {
                if let Some(alias) = b.alias.as_deref() {
                    if alias.to_ascii_lowercase().starts_with(&prefix_lower) {
                        let cached = if b.cached_result.is_some() {
                            "cached"
                        } else {
                            "no result"
                        };
                        out.push(CompletionItem {
                            label: alias.to_string(),
                            kind: CompletionKind::Reference,
                            detail: Some(format!("{} · {cached}", b.block_type)),
                        });
                    }
                }
            }
        }
        for key in env_vars.keys() {
            if key.to_ascii_lowercase().starts_with(&prefix_lower) {
                out.push(CompletionItem {
                    label: key.clone(),
                    kind: CompletionKind::Reference,
                    detail: Some("env".into()),
                });
            }
        }
        out.sort_by(|a, b| a.label.cmp(&b.label));
        out.dedup_by(|a, b| a.label == b.label);
        return out;
    };

    // Path is set — first segment is the alias.
    let path_segs: Vec<&str> = path.split('.').collect();
    let alias = match path_segs.first() {
        Some(h) => *h,
        None => return out,
    };
    let block = segments
        .iter()
        .take(current_segment)
        .filter_map(|s| match s {
            crate::buffer::Segment::Block(b) => Some(b),
            _ => None,
        })
        .find(|b| b.alias.as_deref() == Some(alias));
    let Some(block) = block else { return out };
    let Some(cached) = block.cached_result.as_ref() else {
        return out;
    };

    // Synthesize the navigation envelope — matches desktop's
    // `references.ts:140-143`. The autocomplete walks *this* shape,
    // not `cached_result` directly, so `{{alias.|}}` shows
    // `response` + `status` (the envelope's keys), not the keys of
    // the underlying response.
    let status_str = match &block.state {
        crate::buffer::block::ExecutionState::Success
        | crate::buffer::block::ExecutionState::Cached => "success",
        crate::buffer::block::ExecutionState::Error(_) => "error",
        crate::buffer::block::ExecutionState::Running => "running",
        crate::buffer::block::ExecutionState::Idle => "idle",
    };
    let synthetic_root = serde_json::json!({
        "response": cached,
        "status": status_str,
    });

    // Walk every path segment after the alias against the synthetic
    // root. Arrays support both string-key (`.results`) and numeric
    // index (`.0`) navigation.
    let mut cursor: &serde_json::Value = &synthetic_root;
    for seg in &path_segs[1..] {
        let next = cursor.get(seg).or_else(|| {
            seg.parse::<usize>()
                .ok()
                .and_then(|i| cursor.as_array().and_then(|a| a.get(i)))
        });
        match next {
            Some(v) => cursor = v,
            None => return out,
        }
    }

    // Emit the children of `cursor` based on its shape. Detail
    // mirrors desktop's hint format (`Array(N)`, `{N keys}`,
    // `"string"`, `42`, etc.) so the popup feels familiar.
    if let Some(obj) = cursor.as_object() {
        for (key, val) in obj {
            if key.to_ascii_lowercase().starts_with(&prefix_lower) {
                out.push(CompletionItem {
                    label: key.clone(),
                    kind: CompletionKind::Reference,
                    detail: Some(value_hint(val)),
                });
            }
        }
    } else if let Some(arr) = cursor.as_array() {
        for (i, val) in arr.iter().enumerate() {
            let label = i.to_string();
            if label.starts_with(&detect.prefix) {
                out.push(CompletionItem {
                    label,
                    kind: CompletionKind::Reference,
                    detail: Some(value_hint(val)),
                });
            }
        }
    }
    // Primitives have no children — `out` stays empty and the
    // dispatcher closes the popup on its own.

    // Numeric labels (array indices) sort numerically so `9` comes
    // before `10`; mixed / text labels fall back to alpha.
    out.sort_by(
        |a, b| match (a.label.parse::<usize>(), b.label.parse::<usize>()) {
            (Ok(n), Ok(m)) => n.cmp(&m),
            _ => a.label.cmp(&b.label),
        },
    );
    out.dedup_by(|a, b| a.label == b.label);
    out
}

/// Compact one-liner describing a JSON value's shape — used as the
/// `detail` field for ref completion items so the popup shows
/// `Array(12)`, `{3 keys}`, `"select"`, `42`, etc. Mirrors the
/// strings shown in the desktop popup; long strings get truncated
/// so a row-text column doesn't blow up the popup width.
fn value_hint(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".into(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => {
            if s.chars().count() > 40 {
                let trimmed: String = s.chars().take(37).collect();
                format!("\"{trimmed}...\"")
            } else {
                format!("\"{s}\"")
            }
        }
        serde_json::Value::Array(a) => format!("Array({})", a.len()),
        serde_json::Value::Object(o) => {
            if o.len() == 1 {
                "{1 key}".into()
            } else {
                format!("{{{} keys}}", o.len())
            }
        }
    }
}
