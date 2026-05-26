//! `{{ref}}` substitution inside HTTP block params. Block refs come
//! from `resolve_one_ref` (same source the SQL path uses); env vars
//! resolve as plain strings.

use crate::buffer::Segment;
use crate::commands::db::resolve_one_ref;

/// Walk the HTTP block's params object and replace every
/// `{{...}}` placeholder in URL / headers / params / body with its
/// resolved text value.
pub fn resolve_in_http_params(
    params: &mut serde_json::Value,
    segments: &[Segment],
    current_segment: usize,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    if let Some(s) = params.get("url").and_then(|v| v.as_str()).map(String::from) {
        let resolved = resolve_text_refs(&s, segments, current_segment, env_vars)?;
        if let Some(slot) = params.get_mut("url") {
            *slot = serde_json::Value::String(resolved);
        }
    }
    if let Some(arr) = params.get_mut("headers").and_then(|v| v.as_array_mut()) {
        for h in arr.iter_mut() {
            resolve_kv_in_place(h, segments, current_segment, env_vars)?;
        }
    }
    if let Some(arr) = params.get_mut("params").and_then(|v| v.as_array_mut()) {
        for p in arr.iter_mut() {
            resolve_kv_in_place(p, segments, current_segment, env_vars)?;
        }
    }
    if let Some(s) = params
        .get("body")
        .and_then(|v| v.as_str())
        .map(String::from)
    {
        let resolved = resolve_text_refs(&s, segments, current_segment, env_vars)?;
        if let Some(slot) = params.get_mut("body") {
            *slot = serde_json::Value::String(resolved);
        }
    }
    Ok(())
}

fn resolve_kv_in_place(
    obj: &mut serde_json::Value,
    segments: &[Segment],
    current_segment: usize,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<(), String> {
    for field in ["key", "value"] {
        if let Some(s) = obj.get(field).and_then(|v| v.as_str()).map(String::from) {
            let resolved = resolve_text_refs(&s, segments, current_segment, env_vars)?;
            if let Some(slot) = obj.get_mut(field) {
                *slot = serde_json::Value::String(resolved);
            }
        }
    }
    Ok(())
}

/// Substitute `{{ref}}` placeholders in `text` with their resolved
/// value as plain text. Strings unquote; other JSON values use
/// their JSON form. Used by HTTP for URL / header / param / body
/// substitution (DB uses `?`-bind placeholders instead).
pub(crate) fn resolve_text_refs(
    text: &str,
    segments: &[Segment],
    current_segment: usize,
    env_vars: &std::collections::HashMap<String, String>,
) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let close = match find_close(&bytes[i + 2..]) {
                Some(rel) => i + 2 + rel,
                None => {
                    out.push('{');
                    i += 1;
                    continue;
                }
            };
            let inner = std::str::from_utf8(&bytes[i + 2..close])
                .map_err(|_| "invalid utf-8 inside reference".to_string())?
                .trim();
            let value = resolve_one_ref(segments, current_segment, inner, env_vars)?;
            let s = match value {
                serde_json::Value::String(s) => s,
                other => other.to_string(),
            };
            out.push_str(&s);
            i = close + 2;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    Ok(out)
}

fn find_close(bytes: &[u8]) -> Option<usize> {
    (0..bytes.len().saturating_sub(1)).find(|&i| bytes[i] == b'}' && bytes[i + 1] == b'}')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_segs() -> Vec<Segment> {
        Vec::new()
    }

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn resolve_text_refs_substitutes_env_vars() {
        let segs = empty_segs();
        let env = env(&[("TOKEN", "abc123"), ("HOST", "api.x.com")]);
        let out = resolve_text_refs("https://{{HOST}}/v1?t={{TOKEN}}", &segs, 0, &env).unwrap();
        assert_eq!(out, "https://api.x.com/v1?t=abc123");
    }

    #[test]
    fn resolve_text_refs_passes_through_text_without_refs() {
        let segs = empty_segs();
        let env = env(&[]);
        let out = resolve_text_refs("plain text", &segs, 0, &env).unwrap();
        assert_eq!(out, "plain text");
    }

    #[test]
    fn resolve_text_refs_keeps_unmatched_open_brace() {
        let segs = empty_segs();
        let env = env(&[]);
        let out = resolve_text_refs("oops {{ nope", &segs, 0, &env).unwrap();
        assert!(out.contains("{ nope"), "got: {out}");
    }

    #[test]
    fn resolve_text_refs_errors_on_missing_env_var() {
        let segs = empty_segs();
        let env = env(&[]);
        let err = resolve_text_refs("{{MISSING}}", &segs, 0, &env).unwrap_err();
        assert!(
            err.contains("MISSING") || err.to_lowercase().contains("missing"),
            "got: {err}"
        );
    }

    #[test]
    fn resolve_in_http_params_walks_url_headers_body() {
        let segs = empty_segs();
        let env = env(&[("TOKEN", "secret")]);
        let mut params = serde_json::json!({
            "method": "POST",
            "url": "https://api.x.com?key={{TOKEN}}",
            "headers": [
                { "key": "Authorization", "value": "Bearer {{TOKEN}}" }
            ],
            "params": [
                { "key": "tok", "value": "{{TOKEN}}" }
            ],
            "body": "{\"t\":\"{{TOKEN}}\"}"
        });
        resolve_in_http_params(&mut params, &segs, 0, &env).unwrap();
        assert_eq!(
            params.get("url").and_then(|v| v.as_str()),
            Some("https://api.x.com?key=secret")
        );
        let auth = params
            .get("headers")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|h| h.get("value"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(auth, "Bearer secret");
        let body = params.get("body").and_then(|v| v.as_str()).unwrap();
        assert_eq!(body, "{\"t\":\"secret\"}");
    }
}
