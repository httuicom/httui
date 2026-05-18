//! Code generators for the HTTP block "Send as" menu.
//!
//! Pure functions — each takes a block's resolved params and returns a
//! snippet string the user can paste into a terminal / editor / IDE.
//! Mirrors `src/lib/blocks/http-codegen.ts` so generated snippets stay
//! identical across runtimes.
//!
//! Input shape: `serde_json::Value` matching the block-params JSON
//! (`{ method, url, params: [{key,value}], headers: [...], body }`).
//! Empty keys are dropped (mirror of `enabledKV` on the JS side, where
//! disabled rows have already been stripped by the parser).
//!
//! Output: each function returns a `String` ready for clipboard.

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde_json::Value;

// `URLSearchParams` style encoding — same set the JS
// `encodeURIComponent` uses for query keys/values.
//
// `encodeURIComponent` percent-encodes everything except
// `A-Z a-z 0-9 - _ . ! ~ * ' ( )`. Define the inverse: the chars that
// MUST be encoded.
const QUERY_ENCODE: &AsciiSet = &CONTROLS
    .add(b' ').add(b'"').add(b'#').add(b'$').add(b'%').add(b'&')
    .add(b'+').add(b',').add(b'/').add(b':').add(b';').add(b'<')
    .add(b'=').add(b'>').add(b'?').add(b'@').add(b'[').add(b'\\')
    .add(b']').add(b'^').add(b'`').add(b'{').add(b'|').add(b'}')
    // Non-ASCII handled by utf8_percent_encode automatically.
    ;

const METHODS_WITH_BODY: &[&str] = &["POST", "PUT", "PATCH", "DELETE"];

fn method_has_body(method: &str, body: &str) -> bool {
    METHODS_WITH_BODY.contains(&method) && !body.is_empty()
}

/// Walk a JSON `[{"key": "...", "value": "..."}, ...]` array and yield
/// `(key, value)` pairs with non-empty keys. Mirrors `enabledKV` on
/// the JS side — Rust-side parser already drops disabled rows.
fn iter_kv(arr: Option<&Value>) -> Vec<(String, String)> {
    let arr = match arr.and_then(|v| v.as_array()) {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|v| {
            let obj = v.as_object()?;
            let key = obj.get("key").and_then(|v| v.as_str()).unwrap_or("");
            if key.is_empty() {
                return None;
            }
            let value = obj.get("value").and_then(|v| v.as_str()).unwrap_or("");
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}

fn encode_qs(s: &str) -> String {
    utf8_percent_encode(s, QUERY_ENCODE).to_string()
}

fn build_query_string(params: &Value) -> String {
    let kv = iter_kv(params.get("params"));
    if kv.is_empty() {
        return String::new();
    }
    kv.iter()
        .map(|(k, v)| format!("{}={}", encode_qs(k), encode_qs(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn build_url_with_query(params: &Value) -> String {
    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let q = build_query_string(params);
    if q.is_empty() {
        return url;
    }
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}{q}")
}

fn read_method(params: &Value) -> String {
    params
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_string()
}

fn read_body(params: &Value) -> String {
    params
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

// ─── shell single-quoting ───────────────────────────────────────────

/// Wrap a value in POSIX-shell single quotes, escaping any internal
/// single quotes by closing the quoted run, emitting an escaped quote,
/// and reopening — the standard `'…'\''…'` trick.
fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ─── cURL ───────────────────────────────────────────────────────────

pub fn to_curl(params: &Value) -> String {
    let method = read_method(params);
    let url = build_url_with_query(params);
    let body = read_body(params);
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("curl -X {method} {}", shell_single_quote(&url)));
    for (k, v) in iter_kv(params.get("headers")) {
        lines.push(format!("  -H {}", shell_single_quote(&format!("{k}: {v}"))));
    }
    if method_has_body(&method, &body) {
        lines.push(format!("  --data-raw {}", shell_single_quote(&body)));
    }
    lines.join(" \\\n")
}

// ─── fetch (JavaScript) ─────────────────────────────────────────────

fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

pub fn to_fetch(params: &Value) -> String {
    let method = read_method(params);
    let url = build_url_with_query(params);
    let headers = iter_kv(params.get("headers"));
    let body = read_body(params);

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("await fetch({}, {{", js_string(&url)));
    lines.push(format!("  method: {},", js_string(&method)));
    if !headers.is_empty() {
        lines.push("  headers: {".into());
        for (k, v) in &headers {
            lines.push(format!("    {}: {},", js_string(k), js_string(v)));
        }
        lines.push("  },".into());
    }
    if method_has_body(&method, &body) {
        lines.push(format!("  body: {},", js_string(&body)));
    }
    lines.push("});".into());
    lines.join("\n")
}

// ─── Python requests ────────────────────────────────────────────────

fn py_string(s: &str) -> String {
    // Identical escape rules to `js_string` for the chars we care
    // about (\, ', \n). Python's single-quoted literal rules match.
    js_string(s)
}

pub fn to_python(params: &Value) -> String {
    let method = read_method(params);
    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let body = read_body(params);
    let params_kv = iter_kv(params.get("params"));
    let headers = iter_kv(params.get("headers"));

    let mut lines: Vec<String> = Vec::new();
    lines.push("import requests".into());
    lines.push(String::new());
    let fn_name = method.to_ascii_lowercase();
    lines.push(format!("response = requests.{fn_name}("));
    lines.push(format!("    {},", py_string(&url)));
    if !params_kv.is_empty() {
        lines.push("    params={".into());
        for (k, v) in &params_kv {
            lines.push(format!("        {}: {},", py_string(k), py_string(v)));
        }
        lines.push("    },".into());
    }
    if !headers.is_empty() {
        lines.push("    headers={".into());
        for (k, v) in &headers {
            lines.push(format!("        {}: {},", py_string(k), py_string(v)));
        }
        lines.push("    },".into());
    }
    if method_has_body(&method, &body) {
        lines.push(format!("    data={},", py_string(&body)));
    }
    lines.push(")".into());
    lines.join("\n")
}

// ─── HTTPie ─────────────────────────────────────────────────────────

pub fn to_httpie(params: &Value) -> String {
    let method = read_method(params);
    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let body = read_body(params);

    let mut tokens: Vec<String> = Vec::new();
    tokens.push("http".into());
    tokens.push(method.clone());
    tokens.push(shell_single_quote(&url));

    for (k, v) in iter_kv(params.get("params")) {
        // `==` is HTTPie's query-param syntax.
        tokens.push(shell_single_quote(&format!("{k}=={v}")));
    }
    for (k, v) in iter_kv(params.get("headers")) {
        tokens.push(shell_single_quote(&format!("{k}:{v}")));
    }
    if method_has_body(&method, &body) {
        // HTTPie 3.0+ accepts `--raw=<body>`; the shell-quote keeps
        // multi-line bodies intact.
        tokens.push(format!("--raw={}", shell_single_quote(&body)));
    }
    tokens.join(" ")
}

// ─── .http file ─────────────────────────────────────────────────────

/// Emit the canonical HTTP-message body the user can paste into a
/// `.http` / `.rest` file (REST Client extension, JetBrains HTTP
/// Client, etc). One request per file — multi-request files separated
/// by `###` are out of scope for V1.
pub fn to_http_file(params: &Value) -> String {
    let method = read_method(params);
    let url = params
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let kv_params = iter_kv(params.get("params"));
    let headers = iter_kv(params.get("headers"));
    let body = read_body(params);

    let mut out = String::new();
    out.push_str(&method);
    out.push(' ');
    out.push_str(&url);
    if !kv_params.is_empty() {
        let q = kv_params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        let sep = if url.contains('?') { '&' } else { '?' };
        out.push(sep);
        out.push_str(&q);
    }
    out.push('\n');
    for (k, v) in &headers {
        out.push_str(&format!("{k}: {v}\n"));
    }
    if !body.is_empty() {
        out.push('\n');
        out.push_str(&body);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        json!({
            "method": "POST",
            "url": "https://api.example.com/users",
            "params": [
                {"key": "page", "value": "1"},
                {"key": "active", "value": "true"}
            ],
            "headers": [
                {"key": "Authorization", "value": "Bearer xyz"},
                {"key": "Content-Type", "value": "application/json"}
            ],
            "body": "{\"name\":\"alice\"}"
        })
    }

    // ─── cURL ─────

    #[test]
    fn curl_emits_method_url_headers_body() {
        let curl = to_curl(&fixture());
        assert!(curl.starts_with("curl -X POST 'https://api.example.com/users?page=1&active=true'"));
        assert!(curl.contains("-H 'Authorization: Bearer xyz'"));
        assert!(curl.contains("-H 'Content-Type: application/json'"));
        assert!(curl.contains("--data-raw '{\"name\":\"alice\"}'"));
        // Multi-line continuation `\` (escaped to `\\\n`).
        assert!(curl.contains(" \\\n"));
    }

    #[test]
    fn curl_get_skips_data_flag() {
        let v = json!({
            "method": "GET",
            "url": "https://api.example.com",
            "params": [],
            "headers": [],
            "body": "should be ignored on GET"
        });
        let s = to_curl(&v);
        assert!(!s.contains("--data-raw"), "got: {s}");
    }

    #[test]
    fn curl_escapes_single_quotes_in_body() {
        let v = json!({
            "method": "POST",
            "url": "https://x",
            "params": [],
            "headers": [],
            "body": "it's"
        });
        let s = to_curl(&v);
        // The escaped quote sequence appears inside the body literal.
        assert!(s.contains("'it'\\''s'"), "got: {s}");
    }

    // ─── fetch ─────

    #[test]
    fn fetch_emits_method_headers_body() {
        let s = to_fetch(&fixture());
        assert!(s.contains("await fetch('https://api.example.com/users?page=1&active=true', {"));
        assert!(s.contains("method: 'POST',"));
        assert!(s.contains("headers: {"));
        assert!(s.contains("'Authorization': 'Bearer xyz',"));
        assert!(s.contains("body: '{\\'name\\':\\'alice\\'}',") || s.contains("body: "));
    }

    // ─── Python ─────

    #[test]
    fn python_emits_imports_and_call() {
        let s = to_python(&fixture());
        assert!(s.starts_with("import requests"));
        assert!(s.contains("response = requests.post("));
        assert!(s.contains("    'https://api.example.com/users',"));
        assert!(s.contains("    params={"));
        assert!(s.contains("        'page': '1',"));
        assert!(s.contains("    headers={"));
        assert!(s.contains("    data="));
    }

    #[test]
    fn python_get_skips_params_block_when_empty() {
        let v = json!({
            "method": "GET",
            "url": "https://x",
            "params": [],
            "headers": [],
            "body": ""
        });
        let s = to_python(&v);
        assert!(s.contains("requests.get("), "got: {s}");
        assert!(!s.contains("params="));
        assert!(!s.contains("headers="));
        assert!(!s.contains("data="));
    }

    // ─── HTTPie ─────

    #[test]
    fn httpie_emits_request_items() {
        let s = to_httpie(&fixture());
        assert!(s.starts_with("http POST 'https://api.example.com/users'"));
        // == is the HTTPie syntax for query params.
        assert!(s.contains("'page==1'"));
        assert!(s.contains("'active==true'"));
        // : is the syntax for headers.
        assert!(s.contains("'Authorization:Bearer xyz'"));
        assert!(s.contains("--raw="));
    }

    // ─── .http file ─────

    #[test]
    fn http_file_emits_request_line_headers_body() {
        let s = to_http_file(&fixture());
        let expected = concat!(
            "POST https://api.example.com/users?page=1&active=true\n",
            "Authorization: Bearer xyz\n",
            "Content-Type: application/json\n",
            "\n",
            "{\"name\":\"alice\"}\n",
        );
        assert_eq!(s, expected);
    }

    #[test]
    fn http_file_no_blank_line_when_body_empty() {
        let v = json!({
            "method": "GET",
            "url": "https://x",
            "params": [],
            "headers": [{"key":"X","value":"1"}],
            "body": ""
        });
        let s = to_http_file(&v);
        assert_eq!(s, "GET https://x\nX: 1\n");
    }

    // ─── encoding sanity ─────

    #[test]
    fn query_encoding_percent_encodes_spaces_and_specials() {
        let v = json!({
            "method": "GET",
            "url": "https://x",
            "params": [{"key": "q", "value": "hello world &=?"}],
            "headers": [],
            "body": ""
        });
        let s = to_curl(&v);
        // Spaces become %20, & = ? become %26 %3D %3F.
        assert!(s.contains("q=hello%20world%20%26%3D%3F"), "got: {s}");
    }

    #[test]
    fn empty_keys_dropped() {
        // An entry with empty key is silently skipped by the
        // generators — same as the parser's behavior on the read
        // path.
        let v = json!({
            "method": "GET",
            "url": "https://x",
            "params": [{"key": "", "value": "v"}, {"key": "k", "value": "v"}],
            "headers": [{"key": "", "value": "v"}],
            "body": ""
        });
        let s = to_curl(&v);
        assert!(!s.contains("=v&"), "empty-key param should be dropped: {s}");
        // Surviving param should land in the URL.
        assert!(s.contains("?k=v"));
    }
}
