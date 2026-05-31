//! Builds an outgoing `reqwest` request from parsed [`HttpParams`]:
//! query/header assembly (skipping disabled rows), Content-Type-driven
//! body interpretation (text / binary `< /path` / multipart), and the
//! cancellable file reads multipart + binary bodies need.

use std::path::PathBuf;
use std::time::Duration;

use reqwest::Client;
use tokio_util::sync::CancellationToken;

use super::HttpParams;
use crate::executor::ExecutorError;

pub(super) async fn build_request(
    client: &Client,
    p: &HttpParams,
    cancel: &CancellationToken,
) -> Result<reqwest::RequestBuilder, ExecutorError> {
    // Defaults: trim_whitespace ON, encode_url ON. `None` means "use default".
    let trim = p.trim_whitespace.unwrap_or(true);
    let encode_values = p.encode_url.unwrap_or(true);

    let trim_str = |s: &str| -> String {
        if trim {
            s.trim().to_string()
        } else {
            s.to_string()
        }
    };

    let raw_url = trim_str(&p.url);
    let mut url =
        reqwest::Url::parse(&raw_url).map_err(|e| ExecutorError(format!("Invalid URL: {e}")))?;

    if encode_values {
        // Default: percent-encode keys/values via the safe API.
        for kv in &p.params {
            if !kv.enabled {
                continue;
            }
            let key = trim_str(&kv.key);
            if key.is_empty() {
                continue;
            }
            url.query_pairs_mut()
                .append_pair(&key, &trim_str(&kv.value));
        }
    } else {
        // Opt-out: append raw query string. Caller is responsible for any
        // encoding. We still trim if requested — trimming is independent of
        // encoding. We rebuild the query manually so reqwest doesn't
        // double-encode.
        let mut existing: String = url.query().map(|s| s.to_string()).unwrap_or_default();
        for kv in &p.params {
            if !kv.enabled {
                continue;
            }
            let key = trim_str(&kv.key);
            if key.is_empty() {
                continue;
            }
            if !existing.is_empty() {
                existing.push('&');
            }
            existing.push_str(&key);
            existing.push('=');
            existing.push_str(&trim_str(&kv.value));
        }
        url.set_query(if existing.is_empty() {
            None
        } else {
            Some(&existing)
        });
    }

    let method = p
        .method
        .parse::<reqwest::Method>()
        .map_err(|e| ExecutorError(format!("Invalid method: {e}")))?;

    let mut req = client.request(method.clone(), url);
    if let Some(ms) = p.timeout_ms {
        req = req.timeout(Duration::from_millis(ms));
    }

    // Resolve Content-Type up-front so we can:
    // - decide which body branch to take (text vs binary vs multipart)
    // - skip emitting the user's `Content-Type: multipart/form-data` header
    //   when the body is multipart — reqwest will set its own with the right
    //   `boundary=` when `req.multipart(form)` is called, and a dangling
    //   user header without the boundary trips servers (and our wiremock).
    let user_content_type = p
        .headers
        .iter()
        .find(|kv| kv.enabled && kv.key.trim().eq_ignore_ascii_case("content-type"))
        .map(|kv| trim_str(&kv.value))
        .unwrap_or_default();
    let body_text_for_dispatch = trim_str(&p.body);
    let is_multipart = content_type_main(&user_content_type).starts_with("multipart/");

    for kv in &p.headers {
        if !kv.enabled {
            continue;
        }
        let key = trim_str(&kv.key);
        if key.is_empty() {
            continue;
        }
        if is_multipart && key.eq_ignore_ascii_case("content-type") {
            // Let reqwest's multipart layer own this header.
            continue;
        }
        req = req.header(&key, &trim_str(&kv.value));
    }

    let has_body = matches!(
        method,
        reqwest::Method::POST | reqwest::Method::PUT | reqwest::Method::PATCH
    );
    let body_text = body_text_for_dispatch;
    if has_body && !body_text.is_empty() {
        // Only inject default Content-Type when the user didn't set one and
        // we're not doing multipart (reqwest handles that one itself).
        if user_content_type.is_empty() && !is_multipart {
            req = req.header("Content-Type", "application/json");
        }

        let content_type = user_content_type.clone();

        match interpret_body(&content_type, &body_text) {
            InterpretedBody::Text(s) => req = req.body(s),
            InterpretedBody::Binary { path } => {
                let bytes = read_file_with_cancel(&path, cancel).await?;
                req = req.body(bytes);
            }
            InterpretedBody::Multipart { parts } => {
                let mut form = reqwest::multipart::Form::new();
                for part in parts {
                    match part {
                        Part::Text { name, value } => {
                            form = form.text(name, value);
                        }
                        Part::File {
                            name,
                            path,
                            filename,
                            content_type: ct,
                        } => {
                            let bytes = read_file_with_cancel(&path, cancel).await?;
                            // Build the Part once, then optionally apply the
                            // mime override. `mime_str` consumes Part on
                            // success — we keep the original via clone of
                            // the cheap shared bytes when we have to retry.
                            let bytes_for_fallback = bytes.clone();
                            let filename_for_fallback = filename.clone();
                            let part = reqwest::multipart::Part::bytes(bytes).file_name(filename);
                            let part = part.mime_str(&ct).unwrap_or_else(|_| {
                                reqwest::multipart::Part::bytes(bytes_for_fallback)
                                    .file_name(filename_for_fallback)
                            });
                            form = form.part(name, part);
                        }
                    }
                }
                // `req.multipart(form)` rewrites the Content-Type header to
                // include the boundary that reqwest itself generates — the
                // textual boundary in the .md is just for the user's eyes.
                req = req.multipart(form);
            }
        }
    }

    Ok(req)
}

/// Body shape after `Content-Type`-driven interpretation. The textual
/// branches (`Text`) carry the body verbatim — they include json/xml/text
/// and form-urlencoded, all of which the user already typed in the right
/// shape. `Binary` and `Multipart` require file I/O before the request can
/// go out.
#[derive(Debug)]
enum InterpretedBody {
    Text(String),
    Binary { path: PathBuf },
    Multipart { parts: Vec<Part> },
}

#[derive(Debug)]
enum Part {
    Text {
        name: String,
        value: String,
    },
    File {
        name: String,
        path: PathBuf,
        filename: String,
        content_type: String,
    },
}

/// Strip Content-Type parameters (`; charset=utf-8`, `; boundary=…`) and
/// lowercase. Used purely to dispatch the body shape — not to pick what
/// reqwest sees on the wire.
fn content_type_main(ct: &str) -> String {
    ct.split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase()
}

const FILE_PREFIX: &str = "< ";

/// Decide how to ship `body` based on the resolved Content-Type. Pure
/// function — caller does the file I/O afterwards.
fn interpret_body(content_type: &str, body: &str) -> InterpretedBody {
    let main = content_type_main(content_type);

    if main.starts_with("multipart/") {
        let parts = parse_multipart_textual(body);
        return InterpretedBody::Multipart { parts };
    }

    let is_binary_ct = matches!(
        main.as_str(),
        "application/octet-stream" | "application/pdf" | "application/zip" | "application/wasm"
    ) || main.starts_with("image/")
        || main.starts_with("audio/")
        || main.starts_with("video/");

    if is_binary_ct {
        let trimmed = body.trim();
        if let Some(path) = trimmed.strip_prefix(FILE_PREFIX) {
            let path = path.trim();
            if !path.is_empty() && trimmed.lines().filter(|l| !l.trim().is_empty()).count() == 1 {
                return InterpretedBody::Binary {
                    path: PathBuf::from(path),
                };
            }
        }
        // Binary content type but body isn't a `< /path` line — fall through
        // to Text so reqwest sends the literal bytes (probably the user is
        // still typing).
    }

    InterpretedBody::Text(body.to_string())
}

/// Parse the simplified KV multipart body. Mirrors `parseMultipartBody`
/// from `src/lib/blocks/http-fence.ts`. Each line is one part:
///
///   name=value           ← text part
///   name=< /path/to/file ← file part
///   # name=value         ← disabled (skipped here, never sent)
///   # desc: …            ← description (ignored at this layer)
///
/// Disabled parts and free-form comments are dropped — they exist only
/// for the user's reference in the .md.
fn parse_multipart_textual(body: &str) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();

    for raw in body.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Disabled or comment line — drop. Never ship.
        if trimmed.starts_with('#') {
            continue;
        }
        let eq = match trimmed.find('=') {
            Some(idx) if idx > 0 => idx,
            _ => continue,
        };
        let name = trimmed[..eq].trim().to_string();
        if name.is_empty() {
            continue;
        }
        let raw_value = &trimmed[eq + 1..];

        if let Some(rest) = raw_value.strip_prefix(FILE_PREFIX) {
            let path = rest.trim().to_string();
            if path.is_empty() {
                continue;
            }
            let filename = path_basename(&path).unwrap_or_else(|| name.clone());
            let content_type = infer_content_type(&filename);
            parts.push(Part::File {
                name,
                path: PathBuf::from(path),
                filename,
                content_type,
            });
        } else {
            parts.push(Part::Text {
                name,
                value: raw_value.to_string(),
            });
        }
    }

    parts
}

/// Best-effort MIME inference from filename extension. Mirrors
/// `inferContentType` on the frontend so file parts land with the same
/// declared type whether the request goes through the form mode or the
/// raw body.
fn infer_content_type(filename: &str) -> String {
    let ext = filename
        .rsplit('.')
        .next()
        .filter(|e| !e.contains('/'))
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "json" => "application/json",
        "xml" => "application/xml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "txt" | "log" | "md" => "text/plain",
        "csv" => "text/csv",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "wav" => "audio/wav",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
    .to_string()
}

#[allow(dead_code)]
fn parse_content_disposition(cd: &str) -> (Option<String>, Option<String>) {
    if cd.is_empty() {
        return (None, None);
    }
    let grab = |key: &str| -> Option<String> {
        // Quoted form first.
        let needle_q = format!("{key}=\"");
        if let Some(start) = cd.find(&needle_q) {
            let after = &cd[start + needle_q.len()..];
            if let Some(end) = after.find('"') {
                return Some(after[..end].to_string());
            }
        }
        // Bare form.
        let needle_b = format!("{key}=");
        if let Some(start) = cd.find(&needle_b) {
            let after = &cd[start + needle_b.len()..];
            let end = after.find(';').unwrap_or(after.len());
            return Some(after[..end].trim().to_string());
        }
        None
    };
    (grab("name"), grab("filename"))
}

fn path_basename(p: &str) -> Option<String> {
    let last = p.rsplit(['/', '\\']).next();
    last.map(|s| s.to_string()).filter(|s| !s.is_empty())
}

async fn read_file_with_cancel(
    path: &std::path::Path,
    cancel: &CancellationToken,
) -> Result<Vec<u8>, ExecutorError> {
    let read_future = tokio::fs::read(path);
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(ExecutorError("Request cancelled".to_string())),
        res = read_future => res.map_err(|e| {
            ExecutorError(format!("Read body file {}: {e}", path.display()))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn build_request_skips_disabled_header_and_param() {
        let client = Client::new();
        let cancel = CancellationToken::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": "https://api.test.com/x",
            "headers": [
                {"key": "Accept", "value": "application/json"},
                {"key": "X-Debug", "value": "on", "enabled": false}
            ],
            "params": [
                {"key": "a", "value": "1"},
                {"key": "b", "value": "2", "enabled": false}
            ]
        });
        let p: HttpParams = serde_json::from_value(params).unwrap();
        let req = build_request(&client, &p, &cancel)
            .await
            .unwrap()
            .build()
            .unwrap();
        assert!(req.headers().contains_key("accept"), "enabled header sent");
        assert!(
            !req.headers().contains_key("x-debug"),
            "disabled header must be skipped"
        );
        let q = req.url().query().unwrap_or("");
        assert!(q.contains("a=1"), "enabled param present: {q}");
        assert!(!q.contains("b="), "disabled param must be skipped: {q}");
    }

    #[test]
    fn interpret_body_text_for_json() {
        let out = interpret_body("application/json", r#"{"a":1}"#);
        assert!(matches!(out, InterpretedBody::Text(s) if s == r#"{"a":1}"#));
    }

    #[test]
    fn interpret_body_text_for_form_urlencoded() {
        let out = interpret_body("application/x-www-form-urlencoded", "a=1&b=2");
        assert!(matches!(out, InterpretedBody::Text(s) if s == "a=1&b=2"));
    }

    #[test]
    fn interpret_body_binary_for_octet_stream_with_path() {
        let out = interpret_body("application/octet-stream", "< /tmp/data.bin");
        match out {
            InterpretedBody::Binary { path } => {
                assert_eq!(path, PathBuf::from("/tmp/data.bin"));
            }
            other => panic!("expected Binary, got {other:?}"),
        }
    }

    #[test]
    fn interpret_body_text_when_binary_ct_but_no_path_marker() {
        let out = interpret_body("image/png", "garbage that isn't a path");
        assert!(matches!(out, InterpretedBody::Text(_)));
    }

    #[test]
    fn interpret_body_strips_content_type_parameters() {
        let out = interpret_body("application/json; charset=utf-8", r#"{"a":1}"#);
        assert!(matches!(out, InterpretedBody::Text(_)));
    }

    #[test]
    fn parse_multipart_textual_basic() {
        let body = "username=alice\navatar=< /tmp/avatar.png";
        let parts = parse_multipart_textual(body);
        assert_eq!(parts.len(), 2);
        match &parts[0] {
            Part::Text { name, value } => {
                assert_eq!(name, "username");
                assert_eq!(value, "alice");
            }
            other => panic!("expected Text, got {other:?}"),
        }
        match &parts[1] {
            Part::File {
                name,
                path,
                filename,
                content_type,
            } => {
                assert_eq!(name, "avatar");
                assert_eq!(path, &PathBuf::from("/tmp/avatar.png"));
                assert_eq!(filename, "avatar.png");
                assert_eq!(content_type, "image/png");
            }
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn parse_multipart_textual_skips_disabled_parts() {
        // Disabled rows (prefixed with `#`) and free-form comments are
        // dropped — they exist only for the user's reference in the .md.
        let body = "name=alice\n# secret=hidden\n# this is a free-form comment\n# desc: foo";
        let parts = parse_multipart_textual(body);
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            Part::Text { name, value } => {
                assert_eq!(name, "name");
                assert_eq!(value, "alice");
            }
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn parse_multipart_returns_empty_for_non_multipart_body() {
        assert!(parse_multipart_textual("just plain text").is_empty());
        assert!(parse_multipart_textual("").is_empty());
        // Lines without `=` are dropped silently.
        assert!(parse_multipart_textual("nokey\nalso-no-key").is_empty());
    }

    #[test]
    fn parse_multipart_text_value_can_be_empty() {
        let parts = parse_multipart_textual("empty=");
        assert_eq!(parts.len(), 1);
        match &parts[0] {
            Part::Text { name, value } => {
                assert_eq!(name, "empty");
                assert_eq!(value, "");
            }
            _ => panic!("expected Text"),
        }
    }

    #[test]
    fn parse_multipart_file_picks_up_inferred_content_type() {
        let parts = parse_multipart_textual("doc=< /tmp/report.pdf");
        match &parts[0] {
            Part::File { content_type, .. } => {
                assert_eq!(content_type, "application/pdf");
            }
            _ => panic!("expected File"),
        }
    }
}
