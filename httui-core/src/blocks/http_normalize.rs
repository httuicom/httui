//! Lazy on-read normalization of legacy JSON-bodied http blocks into the
//! canonical HTTP-message form.
//!
//! Background: pre-redesign http blocks stored the request as a single JSON
//! object inside a ```http``` fence (`{"method":"…","url":"…",…}`). The current
//! format uses an HTTP-message body (`METHOD URL\nHeader: Value\n\nbody`).
//!
//! The frontend has a runtime shim (`parseLegacyHttpBody` +
//! `legacyToHttpMessage` in `src/lib/blocks/http-fence.ts`) that converts in
//! memory, so the form view always renders correctly. But the raw CodeMirror
//! doc still holds the legacy JSON until the user *interacts* with the panel
//! (toggle raw↔form, edit a field), which only then writes the canonical body
//! back via `replaceBody`. Result: the raw view shows JSON until interaction,
//! and the file watcher reload exposes the same gap.
//!
//! This module fixes the root cause by normalizing on every read at the
//! backend layer. Any markdown coming out of `httui-core::fs::read_note_normalized`
//! has its http-block bodies in canonical form. The frontend keeps its TS
//! shim as a safety net for now.
//!
//! Behavior:
//! - Only ```http``` fences are touched. Other block types are left alone.
//! - Bodies that don't look like legacy JSON are left untouched (preserves
//!   exact bytes, including HTTP-message bodies the user is editing manually).
//! - Conversion is idempotent: feeding the output back through this function
//!   yields the same string.
//!
//! See `src/lib/blocks/http-fence.ts` for the TypeScript reference of
//! `stringifyHttpMessageBody` — this module's `stringify_http_message`
//! mirrors its layout rules.

use serde_json::Value;

const URL_INLINE_LIMIT: usize = 80;
const HTTP_METHODS: &[&str] = &[
    "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS",
];

/// Walk a markdown string, find ```http fences, and rewrite legacy JSON bodies
/// in canonical HTTP-message form. Bodies that aren't legacy JSON are
/// preserved byte-for-byte.
pub fn normalize_http_blocks(markdown: &str) -> String {
    if !markdown.contains("```http") {
        return markdown.to_string();
    }

    let lines: Vec<&str> = markdown.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        if !is_http_fence_open(line) {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        // Opening fence — emit it and consume body lines up to the next
        // closing fence (or EOF).
        out.push(line.to_string());
        i += 1;

        let body_start = i;
        while i < lines.len() && !is_fence_close(lines[i]) {
            i += 1;
        }
        let body_end = i;

        let body = lines[body_start..body_end].join("\n");

        match try_normalize_legacy(&body) {
            Some(canonical) => {
                // Re-emit canonical body line-by-line; preserves the
                // join-on-newline contract.
                for nl in canonical.split('\n') {
                    out.push(nl.to_string());
                }
            }
            None => {
                // Not legacy → preserve original bytes. (split+push is a
                // no-op on the body's internal newlines.)
                for nl in &lines[body_start..body_end] {
                    out.push((*nl).to_string());
                }
            }
        }

        // Emit closing fence (if found) and advance past it.
        if i < lines.len() {
            out.push(lines[i].to_string());
            i += 1;
        }
    }

    out.join("\n")
}

fn is_http_fence_open(line: &str) -> bool {
    let trimmed = line.trim_start();
    let after = match trimmed.strip_prefix("```http") {
        Some(s) => s,
        None => return false,
    };
    after.is_empty() || after.starts_with(char::is_whitespace)
}

fn is_fence_close(line: &str) -> bool {
    let t = line.trim();
    t.len() >= 3 && t.chars().all(|c| c == '`')
}

/// Returns `Some(canonical_body)` if `body` parses as a legacy JSON request,
/// `None` otherwise.
fn try_normalize_legacy(body: &str) -> Option<String> {
    let trimmed = body.trim_start();
    if !trimmed.starts_with('{') {
        return None;
    }

    let value: Value = serde_json::from_str(trimmed).ok()?;
    let obj = value.as_object()?;

    let method_raw = obj.get("method")?.as_str()?;
    let method = method_raw.to_ascii_uppercase();
    if !HTTP_METHODS.contains(&method.as_str()) {
        return None;
    }

    let url = obj.get("url")?.as_str()?.to_string();
    let params = extract_kv_array(obj.get("params"));
    let headers = extract_kv_array(obj.get("headers"));
    let body_text = obj
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Some(stringify_http_message(&method, &url, &params, &headers, &body_text))
}

fn extract_kv_array(value: Option<&Value>) -> Vec<(String, String)> {
    let arr = match value.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let obj = match item.as_object() {
            Some(o) => o,
            None => continue,
        };
        let key = obj.get("key").and_then(|v| v.as_str()).unwrap_or("");
        if key.is_empty() {
            continue;
        }
        let value = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
        out.push((key.to_string(), value.to_string()));
    }
    out
}

fn stringify_http_message(
    method: &str,
    url: &str,
    params: &[(String, String)],
    headers: &[(String, String)],
    body: &str,
) -> String {
    let mut out: Vec<String> = Vec::new();

    let inline = can_inline_query(method, url, params);
    if inline {
        if params.is_empty() {
            out.push(format!("{} {}", method, url));
        } else {
            let q = params
                .iter()
                .map(|(k, v)| format_param(k, v))
                .collect::<Vec<_>>()
                .join("&");
            out.push(format!("{} {}?{}", method, url, q));
        }
    } else {
        out.push(format!("{} {}", method, url));
        let mut first = true;
        for (k, v) in params {
            let lead = if first { "?" } else { "&" };
            first = false;
            out.push(format!("{}{}", lead, format_param(k, v)));
        }
    }

    for (k, v) in headers {
        out.push(format!("{}: {}", k, v));
    }

    if !body.is_empty() {
        out.push(String::new());
        out.push(body.to_string());
    }

    out.join("\n")
}

fn format_param(key: &str, value: &str) -> String {
    if value.is_empty() {
        key.to_string()
    } else {
        format!("{}={}", key, value)
    }
}

fn can_inline_query(method: &str, url: &str, params: &[(String, String)]) -> bool {
    if params.is_empty() {
        return true;
    }
    let inline_len: usize = params
        .iter()
        .map(|(k, v)| {
            if v.is_empty() {
                k.len()
            } else {
                k.len() + 1 + v.len()
            }
        })
        .sum::<usize>()
        + params.len().saturating_sub(1); // separators
    let total = method.len() + 1 + url.len() + 1 + inline_len;
    total <= URL_INLINE_LIMIT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_when_no_http_fences() {
        let md = "# title\n\nsome text\n\n```db-postgres\nSELECT 1\n```\n";
        assert_eq!(normalize_http_blocks(md), md);
    }

    #[test]
    fn empty_input() {
        assert_eq!(normalize_http_blocks(""), "");
    }

    #[test]
    fn rewrites_minimal_legacy_body() {
        let md = "```http alias=req1\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        let expected = "```http alias=req1\nGET https://x.com\n```\n";
        assert_eq!(normalize_http_blocks(md), expected);
    }

    #[test]
    fn rewrites_legacy_with_inline_query() {
        let md = concat!(
            "```http alias=r\n",
            "{\"method\":\"POST\",\"url\":\"https://api.x/users\",\"params\":[{\"key\":\"page\",\"value\":\"1\"}],\"headers\":[],\"body\":\"\"}\n",
            "```\n",
        );
        let expected = "```http alias=r\nPOST https://api.x/users?page=1\n```\n";
        assert_eq!(normalize_http_blocks(md), expected);
    }

    #[test]
    fn rewrites_legacy_with_headers_and_body() {
        let md = concat!(
            "```http alias=r\n",
            "{\"method\":\"POST\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[{\"key\":\"Content-Type\",\"value\":\"application/json\"}],\"body\":\"{\\\"name\\\":\\\"alice\\\"}\"}\n",
            "```\n",
        );
        let expected = concat!(
            "```http alias=r\n",
            "POST https://x.com\n",
            "Content-Type: application/json\n",
            "\n",
            "{\"name\":\"alice\"}\n",
            "```\n",
        );
        assert_eq!(normalize_http_blocks(md), expected);
    }

    #[test]
    fn rewrites_real_world_block_from_bug_report() {
        // Mirrors the exact body shape from the user's screenshot — escapes the
        // newline in `body` and uses `{{...}}` placeholders.
        let md = concat!(
            "```http alias=testd\n",
            "{\"body\":\"asdf=asdf\\nasdffdsa=asdf\",\"headers\":[{\"key\":\"Teste\",\"value\":\"{{BASE_URL}}\"},{\"key\":\"Content-Type\",\"value\":\"multipart/form-data\"}],\"method\":\"POST\",\"params\":[{\"key\":\"page\",\"value\":\"{{BASE_URL}}\"}],\"url\":\"https://httpbin.org/post\"}\n",
            "```\n",
        );
        let expected = concat!(
            "```http alias=testd\n",
            "POST https://httpbin.org/post?page={{BASE_URL}}\n",
            "Teste: {{BASE_URL}}\n",
            "Content-Type: multipart/form-data\n",
            "\n",
            "asdf=asdf\n",
            "asdffdsa=asdf\n",
            "```\n",
        );
        assert_eq!(normalize_http_blocks(md), expected);
    }

    #[test]
    fn falls_back_to_continuation_when_inline_too_long() {
        // URL alone is 75 chars; with `GET ` + `?param=…` it tips over the
        // 80-char inline budget so each param must move to a continuation line.
        let long_url = "https://example.com/api/v1/some/really/quite/long/resource/path/here/now";
        assert!(long_url.len() > 70, "fixture sanity: url must be long");
        let body = format!(
            "```http\n{{\"method\":\"GET\",\"url\":\"{}\",\"params\":[{{\"key\":\"alpha\",\"value\":\"one\"}},{{\"key\":\"beta\",\"value\":\"two\"}}],\"headers\":[],\"body\":\"\"}}\n```\n",
            long_url
        );
        let out = normalize_http_blocks(&body);
        assert!(
            out.contains(&format!("GET {}\n?alpha=one\n", long_url)),
            "got: {out}"
        );
        assert!(out.contains("&beta=two\n"), "got: {out}");
    }

    #[test]
    fn leaves_canonical_body_untouched() {
        let md = concat!(
            "```http alias=req1\n",
            "POST https://x.com\n",
            "Content-Type: application/json\n",
            "\n",
            "{\"name\":\"alice\"}\n",
            "```\n",
        );
        assert_eq!(normalize_http_blocks(md), md);
    }

    #[test]
    fn leaves_invalid_json_untouched() {
        let md = "```http\n{not real json\n```\n";
        assert_eq!(normalize_http_blocks(md), md);
    }

    #[test]
    fn leaves_json_without_method_untouched() {
        let md = "```http\n{\"url\":\"x\"}\n```\n";
        assert_eq!(normalize_http_blocks(md), md);
    }

    #[test]
    fn leaves_json_with_unknown_method_untouched() {
        let md = "```http\n{\"method\":\"BREW\",\"url\":\"x\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        assert_eq!(normalize_http_blocks(md), md);
    }

    #[test]
    fn does_not_touch_db_blocks() {
        let md = concat!(
            "```db-postgres alias=q\n",
            "{\"connection_id\":\"c\",\"query\":\"SELECT 1\"}\n",
            "```\n",
        );
        assert_eq!(normalize_http_blocks(md), md);
    }

    #[test]
    fn handles_multiple_blocks_in_one_doc() {
        let md = concat!(
            "# Notes\n",
            "\n",
            "```http alias=a\n",
            "{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n",
            "```\n",
            "\n",
            "Some prose.\n",
            "\n",
            "```http alias=b\n",
            "POST https://y.com\n",
            "\n",
            "hi\n",
            "```\n",
        );
        let expected = concat!(
            "# Notes\n",
            "\n",
            "```http alias=a\n",
            "GET https://x.com\n",
            "```\n",
            "\n",
            "Some prose.\n",
            "\n",
            "```http alias=b\n",
            "POST https://y.com\n",
            "\n",
            "hi\n",
            "```\n",
        );
        assert_eq!(normalize_http_blocks(md), expected);
    }

    #[test]
    fn idempotent_on_canonical_input() {
        let md = concat!(
            "```http alias=r\n",
            "{\"method\":\"POST\",\"url\":\"https://api.x\",\"params\":[{\"key\":\"k\",\"value\":\"v\"}],\"headers\":[{\"key\":\"H\",\"value\":\"X\"}],\"body\":\"hi\"}\n",
            "```\n",
        );
        let once = normalize_http_blocks(md);
        let twice = normalize_http_blocks(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn supports_method_in_lowercase() {
        let md = "```http\n{\"method\":\"post\",\"url\":\"https://x\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        let expected = "```http\nPOST https://x\n```\n";
        assert_eq!(normalize_http_blocks(md), expected);
    }

    #[test]
    fn skips_kv_pairs_with_empty_keys() {
        let md = "```http\n{\"method\":\"GET\",\"url\":\"x\",\"params\":[{\"key\":\"\",\"value\":\"v\"}],\"headers\":[],\"body\":\"\"}\n```\n";
        let expected = "```http\nGET x\n```\n";
        assert_eq!(normalize_http_blocks(md), expected);
    }

    #[test]
    fn handles_param_with_empty_value() {
        let md = "```http\n{\"method\":\"GET\",\"url\":\"x\",\"params\":[{\"key\":\"flag\",\"value\":\"\"}],\"headers\":[],\"body\":\"\"}\n```\n";
        let expected = "```http\nGET x?flag\n```\n";
        assert_eq!(normalize_http_blocks(md), expected);
    }

    #[test]
    fn unclosed_fence_at_eof_still_normalizes_body() {
        let md = "```http\n{\"method\":\"GET\",\"url\":\"x\",\"params\":[],\"headers\":[],\"body\":\"\"}";
        // No closing fence — we still rewrite the body before EOF.
        let expected = "```http\nGET x";
        assert_eq!(normalize_http_blocks(md), expected);
    }
}
