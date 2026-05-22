//! Response shape for the `http` block type.

use serde::{Deserialize, Serialize};

/// Per-execution timing breakdown.
///
/// Ships `total_ms` + `ttfb_ms`. Sub-fields `dns_ms`/`connect_ms`/`tls_ms`
/// stay `None` — they require swapping `reqwest` for `isahc`/libcurl (see
/// `docs/http-timing-isahc-future.md`). Consumers should treat missing
/// fields as "unknown", not "zero". `connection_reused` is always `false`
/// without a custom connector that can observe pool-hits.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimingBreakdown {
    pub total_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dns_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttfb_ms: Option<u64>,
    #[serde(default)]
    pub connection_reused: bool,
}

/// A cookie captured from a `Set-Cookie` response header.
///
/// V1 stores raw attribute strings (`domain`, `path`, `expires`) without
/// RFC-6265 parsing. The persistent cookie jar (V2) will normalize these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
}

/// Top-level response shape for an `http` block execution.
///
/// `body` carries either the parsed JSON, the response text, or a
/// base64-encoded binary payload (`{ encoding: "base64", data: "..." }`),
/// matching the legacy shape so existing references (`{{req.response.body.foo}}`)
/// keep working.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub status_text: String,
    pub headers: std::collections::HashMap<String, String>,
    pub body: serde_json::Value,
    pub size_bytes: u64,
    pub elapsed_ms: u64,
    pub timing: TimingBreakdown,
    #[serde(default)]
    pub cookies: Vec<Cookie>,
}

/// Streaming chunk emitted to a `tauri::Channel<HttpChunk>` during execution.
///
/// Wire order on a successful execution:
///   `Headers { ... }` → `BodyChunk { ... }` × N → `Complete(HttpResponse)`
/// On failure / cancel: a single terminal `Error` / `Cancelled` after any
/// `Headers`/`BodyChunk` already emitted (cancel mid-body discards the
/// partial body — frontend should treat `Cancelled` as "no result").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HttpChunk {
    /// First chunk: status + headers as soon as `req.send()` returns.
    /// `ttfb_ms` is the elapsed time between dispatching the request and
    /// receiving the response headers (= Postman-style "Time to First Byte").
    /// Frontend can update statusbar dot + status text immediately.
    Headers {
        status_code: u16,
        status_text: String,
        headers: std::collections::HashMap<String, String>,
        ttfb_ms: u64,
    },
    /// Body chunk(s) emitted as `bytes_stream()` yields. `offset` is the
    /// total bytes received before this chunk (so receivers can verify
    /// continuity). V1 frontend ignores these except for a "downloading
    /// X kb…" progress counter — `Complete` carries the consolidated body.
    BodyChunk { offset: u64, bytes: Vec<u8> },
    /// Terminal chunk containing the full response. Consumer should close
    /// the subscription after receiving this. The cache writes from this.
    Complete(HttpResponse),
    /// Terminal chunk indicating the execution failed before completing.
    Error { message: String },
    /// Terminal chunk indicating the execution was cancelled.
    Cancelled,
}

/// Parse a single `Set-Cookie` header value into a `Cookie`.
///
/// RFC 6265-aware enough for V1 — splits on `;`, lowercases attribute names,
/// preserves case for the cookie name/value. Returns `None` if the header
/// has no `name=value` pair.
pub fn parse_set_cookie(header: &str) -> Option<Cookie> {
    let mut parts = header.split(';');
    let nv = parts.next()?.trim();
    let eq = nv.find('=')?;
    let name = nv[..eq].trim().to_string();
    let value = nv[eq + 1..].trim().to_string();
    if name.is_empty() {
        return None;
    }

    let mut cookie = Cookie {
        name,
        value,
        domain: None,
        path: None,
        expires: None,
        secure: false,
        http_only: false,
    };

    for attr in parts {
        let attr = attr.trim();
        let (key, val) = match attr.find('=') {
            Some(i) => (&attr[..i], Some(attr[i + 1..].trim().to_string())),
            None => (attr, None),
        };
        match key.to_ascii_lowercase().as_str() {
            "domain" => cookie.domain = val,
            "path" => cookie.path = val,
            "expires" => cookie.expires = val,
            "secure" => cookie.secure = true,
            "httponly" => cookie.http_only = true,
            _ => {}
        }
    }

    Some(cookie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_chunk_complete_serializes_inline() {
        let response = HttpResponse {
            status_code: 200,
            status_text: "OK".to_string(),
            headers: Default::default(),
            body: serde_json::json!({"hello": "world"}),
            size_bytes: 17,
            elapsed_ms: 42,
            timing: TimingBreakdown {
                total_ms: 42,
                ..Default::default()
            },
            cookies: vec![],
        };
        let chunk = HttpChunk::Complete(response);
        let v = serde_json::to_value(&chunk).unwrap();
        assert_eq!(v["kind"], "complete");
        assert_eq!(v["status_code"], 200);
        assert_eq!(v["body"]["hello"], "world");
        assert_eq!(v["timing"]["total_ms"], 42);
    }

    #[test]
    fn http_chunk_error_and_cancelled_serialize() {
        let err = HttpChunk::Error {
            message: "boom".to_string(),
        };
        let v = serde_json::to_value(&err).unwrap();
        assert_eq!(v["kind"], "error");
        assert_eq!(v["message"], "boom");

        let cancelled = HttpChunk::Cancelled;
        let v = serde_json::to_value(&cancelled).unwrap();
        assert_eq!(v["kind"], "cancelled");
    }

    #[test]
    fn parse_set_cookie_basic() {
        let c = parse_set_cookie("session=abc123").unwrap();
        assert_eq!(c.name, "session");
        assert_eq!(c.value, "abc123");
        assert_eq!(c.domain, None);
        assert!(!c.secure);
    }

    #[test]
    fn parse_set_cookie_with_attrs() {
        let c = parse_set_cookie("sid=xyz; Domain=example.com; Path=/; Secure; HttpOnly").unwrap();
        assert_eq!(c.name, "sid");
        assert_eq!(c.value, "xyz");
        assert_eq!(c.domain.as_deref(), Some("example.com"));
        assert_eq!(c.path.as_deref(), Some("/"));
        assert!(c.secure);
        assert!(c.http_only);
    }

    #[test]
    fn parse_set_cookie_with_expires() {
        let c = parse_set_cookie("foo=bar; Expires=Wed, 21 Oct 2026 07:28:00 GMT").unwrap();
        assert_eq!(c.expires.as_deref(), Some("Wed, 21 Oct 2026 07:28:00 GMT"));
    }

    #[test]
    fn parse_set_cookie_rejects_no_equals() {
        assert!(parse_set_cookie("no-equals").is_none());
    }

    #[test]
    fn parse_set_cookie_rejects_empty_name() {
        assert!(parse_set_cookie("=value").is_none());
    }

    #[test]
    fn timing_breakdown_default_omits_optional_fields() {
        let t = TimingBreakdown::default();
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["total_ms"], 0);
        assert!(v.get("dns_ms").is_none());
        assert!(v.get("connect_ms").is_none());
        // connection_reused is not Option — always present, defaults to false.
        assert_eq!(v["connection_reused"], false);
    }

    #[test]
    fn http_chunk_headers_serializes() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        let chunk = HttpChunk::Headers {
            status_code: 200,
            status_text: "OK".to_string(),
            headers,
            ttfb_ms: 42,
        };
        let v = serde_json::to_value(&chunk).unwrap();
        assert_eq!(v["kind"], "headers");
        assert_eq!(v["status_code"], 200);
        assert_eq!(v["status_text"], "OK");
        assert_eq!(v["ttfb_ms"], 42);
        assert_eq!(v["headers"]["content-type"], "application/json");
    }

    #[test]
    fn http_chunk_body_chunk_serializes() {
        let chunk = HttpChunk::BodyChunk {
            offset: 8,
            bytes: vec![0xDE, 0xAD, 0xBE, 0xEF],
        };
        let v = serde_json::to_value(&chunk).unwrap();
        assert_eq!(v["kind"], "body_chunk");
        assert_eq!(v["offset"], 8);
        // serde_json serializes Vec<u8> as a JSON array of numbers.
        assert_eq!(v["bytes"], serde_json::json!([0xDE, 0xAD, 0xBE, 0xEF]));
    }
}
