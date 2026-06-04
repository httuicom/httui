mod request;
pub mod types;

use async_trait::async_trait;
use base64::prelude::*;
use futures_util::StreamExt;
use reqwest::redirect::Policy;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use self::request::build_request;
use self::types::HttpChunk;

/// Hard cap on response body size. Above this, the executor returns a
/// `[body_too_large]` error before reading further bytes. Streaming via
/// `bytes_stream()` keeps memory bounded along the way — the cap exists so
/// a single accidental download can't fill the whole webview heap.
const MAX_BODY_BYTES: u64 = 100 * 1024 * 1024; // 100 MB

use self::types::{parse_set_cookie, Cookie, HttpResponse, TimingBreakdown};
use super::{BlockResult, Executor, ExecutorError};

#[derive(Debug, Deserialize)]
struct KeyValue {
    key: String,
    value: String,
    /// `false` → row is disabled (`# ` prefix in the fence) and skipped on
    /// dispatch. Absent defaults to `true` — desktop and legacy callers that
    /// pre-filter disabled rows send no flag and keep working unchanged.
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize)]
struct HttpParams {
    method: String,
    url: String,
    #[serde(default)]
    params: Vec<KeyValue>,
    #[serde(default)]
    headers: Vec<KeyValue>,
    #[serde(default)]
    body: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
    /// `None` (default) → follow redirects (reqwest default = 10 hops).
    /// `Some(false)` → no redirects, raw 3xx surface to the user.
    #[serde(default)]
    follow_redirects: Option<bool>,
    /// `None` / `Some(true)` (default) → enforce certificate validation.
    /// `Some(false)` → accept self-signed / invalid certificates (per request).
    #[serde(default)]
    verify_ssl: Option<bool>,
    /// `None` / `Some(true)` (default) → percent-encode query param values.
    /// `Some(false)` → append the raw value verbatim (user already encoded).
    #[serde(default)]
    encode_url: Option<bool>,
    /// `None` / `Some(true)` (default) → trim whitespace on header keys/values,
    /// query keys/values, and the body before sending. May break text/plain
    /// payloads that rely on leading/trailing whitespace; user opted in.
    #[serde(default)]
    trim_whitespace: Option<bool>,
}

fn is_binary_content_type(content_type: &str) -> bool {
    let ct = content_type.to_lowercase();
    ct.starts_with("image/")
        || ct.starts_with("video/")
        || ct.starts_with("audio/")
        || ct == "application/pdf"
        || ct == "application/octet-stream"
        || ct == "application/zip"
        || ct == "application/wasm"
}

fn classify_reqwest_error(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connection_failed"
    } else if e.is_redirect() {
        "too_many_redirects"
    } else if e.is_body() || e.is_decode() {
        "body_error"
    } else {
        "request_error"
    }
}

/// Bool flags that have to be baked into the reqwest `Client` because
/// reqwest does not expose them as per-request overrides
/// (`redirect::Policy` and `danger_accept_invalid_certs` are
/// `ClientBuilder`-only). We cache one client per combination so we don't
/// rebuild a fresh TLS pool on every request — the matrix is bounded at 4.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
struct ClientFlags {
    follow_redirects: bool,
    verify_ssl: bool,
}

impl Default for ClientFlags {
    fn default() -> Self {
        Self {
            follow_redirects: true,
            verify_ssl: true,
        }
    }
}

pub struct HttpExecutor {
    /// Lazy cache of `reqwest::Client` keyed by transport flags. Wrapped in
    /// a sync `Mutex` because lookup is fast and contention is rare (only
    /// hit on first use of each combo). `Client` clones share the same
    /// internal `Arc<ClientRef>` so cloning on the hot path is cheap.
    clients: Mutex<HashMap<ClientFlags, Client>>,
}

impl Default for HttpExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpExecutor {
    pub fn new() -> Self {
        let executor = Self {
            clients: Mutex::new(HashMap::new()),
        };
        // Eagerly populate the default-flags client so the warm path matches
        // the previous "single global client" behaviour.
        let _ = executor.client_for(ClientFlags::default());
        executor
    }

    fn client_for(&self, flags: ClientFlags) -> Client {
        let mut guard = self.clients.lock().expect("HttpExecutor.clients poisoned");
        if let Some(existing) = guard.get(&flags) {
            return existing.clone();
        }
        let mut builder = Client::builder().timeout(Duration::from_secs(30));
        if !flags.follow_redirects {
            builder = builder.redirect(Policy::none());
        }
        if !flags.verify_ssl {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let client = builder.build().unwrap_or_default();
        guard.insert(flags, client.clone());
        client
    }

    /// Cancel-aware execution returning the typed `HttpResponse` directly.
    ///
    /// Wraps `execute_streamed` with a no-op chunk callback for callers that
    /// don't care about progress (legacy `execute()` Tauri path, internal
    /// uses, tests). The `cancel` token semantics are unchanged: when fired,
    /// the in-flight request or body stream future is dropped and the caller
    /// gets `Err("Request cancelled")`. HTTP-level errors (4xx/5xx) are
    /// surfaced as `Ok(response)` with the status code preserved — only
    /// transport/encoding/cancellation failures map to `Err`.
    pub async fn execute_with_cancel(
        &self,
        params: serde_json::Value,
        cancel: CancellationToken,
    ) -> Result<HttpResponse, ExecutorError> {
        self.execute_streamed(params, cancel, |_| {}).await
    }

    /// Streaming, cancel-aware execution. Emits `HttpChunk::Headers` once
    /// the response status line is available, then `HttpChunk::BodyChunk`
    /// per body chunk yielded by `Response::bytes_stream()`, and finally
    /// `HttpChunk::Complete(response)` with the consolidated body.
    ///
    /// Memory bound: total accumulated body is capped at `MAX_BODY_BYTES`
    /// (100 MB). Above that, the executor returns a `[body_too_large]`
    /// error and stops reading further bytes — no `Complete` is emitted.
    ///
    /// Cancel mid-body returns `Err("Request cancelled")` and emits no
    /// terminal chunk on the callback (the `executions.rs` Tauri command
    /// converts this `Err` into a `HttpChunk::Cancelled` on the wire).
    /// Partial bytes received before the cancel are discarded.
    pub async fn execute_streamed<F>(
        &self,
        params: serde_json::Value,
        cancel: CancellationToken,
        on_chunk: F,
    ) -> Result<HttpResponse, ExecutorError>
    where
        F: Fn(HttpChunk) + Send,
    {
        let p: HttpParams = serde_json::from_value(params)
            .map_err(|e| ExecutorError(format!("Invalid params: {e}")))?;
        let flags = ClientFlags {
            follow_redirects: p.follow_redirects.unwrap_or(true),
            verify_ssl: p.verify_ssl.unwrap_or(true),
        };
        let client = self.client_for(flags);
        let req = build_request(&client, &p, &cancel).await?;

        let t0 = Instant::now();
        let response = tokio::select! {
            biased;
            _ = cancel.cancelled() => {
                return Err(ExecutorError("Request cancelled".to_string()));
            }
            res = req.send() => {
                res.map_err(|e| {
                    ExecutorError(format!("[{}] {}", classify_reqwest_error(&e), e))
                })?
            }
        };
        let ttfb_ms = t0.elapsed().as_millis() as u64;

        // Headers + cookies + content-type captured up-front so the partial
        // shape is stable and we don't re-borrow `response` after starting
        // the body stream.
        let status_code = response.status().as_u16();
        let status_text = response
            .status()
            .canonical_reason()
            .unwrap_or("")
            .to_string();
        let cookies: Vec<Cookie> = response
            .headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(parse_set_cookie)
            .collect();
        let resp_headers: HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let content_type = resp_headers
            .get("content-type")
            .map(|s| s.as_str())
            .unwrap_or("")
            .to_string();

        on_chunk(HttpChunk::Headers {
            status_code,
            status_text: status_text.clone(),
            headers: resp_headers.clone(),
            ttfb_ms,
        });

        // Stream the body in chunks. We accumulate into `body_bytes` so the
        // terminal `Complete` carries the consolidated payload (the cache
        // and the body viewer both want a single value, not a stream of
        // chunks). The cap fires before we copy into `body_bytes`, so a
        // pathological response can grow `body_bytes` to at most
        // `MAX_BODY_BYTES + last_chunk_size` (chunks from reqwest are
        // typically ≤ 16 KB).
        let mut stream = response.bytes_stream();
        let mut body_bytes: Vec<u8> = Vec::with_capacity(8 * 1024);
        let mut offset: u64 = 0;
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    return Err(ExecutorError("Request cancelled".to_string()));
                }
                next = stream.next() => {
                    match next {
                        None => break,
                        Some(Err(e)) => {
                            return Err(ExecutorError(format!(
                                "[{}] {}",
                                classify_reqwest_error(&e),
                                e
                            )));
                        }
                        Some(Ok(bytes)) => {
                            let new_total = offset + bytes.len() as u64;
                            if new_total > MAX_BODY_BYTES {
                                return Err(ExecutorError(format!(
                                    "[body_too_large] response exceeded {} MB cap",
                                    MAX_BODY_BYTES / 1024 / 1024
                                )));
                            }
                            let chunk_vec = bytes.to_vec();
                            body_bytes.extend_from_slice(&chunk_vec);
                            on_chunk(HttpChunk::BodyChunk {
                                offset,
                                bytes: chunk_vec,
                            });
                            offset = new_total;
                        }
                    }
                }
            }
        }

        let size_bytes = body_bytes.len() as u64;
        let body_value = if is_binary_content_type(&content_type) {
            serde_json::json!({
                "encoding": "base64",
                "data": BASE64_STANDARD.encode(&body_bytes),
            })
        } else {
            let text = String::from_utf8_lossy(&body_bytes).into_owned();
            serde_json::from_str::<serde_json::Value>(&text)
                .unwrap_or(serde_json::Value::String(text))
        };

        let total_ms = t0.elapsed().as_millis() as u64;
        let response = HttpResponse {
            status_code,
            status_text,
            headers: resp_headers,
            body: body_value,
            size_bytes,
            elapsed_ms: total_ms,
            timing: TimingBreakdown {
                total_ms,
                ttfb_ms: Some(ttfb_ms),
                connection_reused: false,
                ..Default::default()
            },
            cookies,
        };
        on_chunk(HttpChunk::Complete(response.clone()));
        Ok(response)
    }
}

#[async_trait]
impl Executor for HttpExecutor {
    fn block_type(&self) -> &str {
        "http"
    }

    async fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        let p: HttpParams =
            serde_json::from_value(params.clone()).map_err(|e| format!("Invalid params: {e}"))?;
        if p.url.trim().is_empty() {
            return Err("URL is required".to_string());
        }
        Ok(())
    }

    async fn execute(&self, params: serde_json::Value) -> Result<BlockResult, ExecutorError> {
        let response = self
            .execute_with_cancel(params, CancellationToken::new())
            .await?;

        let status = if response.status_code < 400 {
            "success"
        } else {
            "error"
        };

        Ok(BlockResult {
            status: status.to_string(),
            data: serde_json::json!({
                "status_code": response.status_code,
                "status_text": response.status_text,
                "headers": response.headers,
                "body": response.body,
                "size_bytes": response.size_bytes,
            }),
            duration_ms: response.elapsed_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_http_executor_validate_empty_url() {
        let executor = HttpExecutor::new();
        let params = serde_json::json!({ "method": "GET", "url": "" });
        let result = executor.validate(&params).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("URL is required"));
    }

    #[tokio::test]
    async fn test_http_executor_validate_missing_url() {
        let executor = HttpExecutor::new();
        let params = serde_json::json!({ "method": "GET" });
        let result = executor.validate(&params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_http_executor_get_json() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/users/1"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("x-request-id", "abc123")
                    .set_body_json(serde_json::json!({"id": 1, "name": "Alice"})),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/users/1", server.uri()),
        });

        let result = executor.execute(params).await.unwrap();
        assert_eq!(result.status, "success");

        let data = &result.data;
        assert_eq!(data["status_code"], 200);
        assert_eq!(data["status_text"], "OK");
        assert_eq!(data["body"]["id"], 1);
        assert_eq!(data["body"]["name"], "Alice");
        assert_eq!(data["headers"]["x-request-id"], "abc123");
        assert!(data["size_bytes"].as_u64().unwrap() > 0);
        assert!(result.duration_ms < 5000);
    }

    #[tokio::test]
    async fn test_http_executor_post_with_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/users"))
            .and(header("content-type", "application/json"))
            .respond_with(
                ResponseTemplate::new(201)
                    .set_body_json(serde_json::json!({"id": 2, "created": true})),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "POST",
            "url": format!("{}/users", server.uri()),
            "body": r#"{"name":"Bob"}"#,
        });

        let result = executor.execute(params).await.unwrap();
        assert_eq!(result.status, "success");
        assert_eq!(result.data["status_code"], 201);
        assert_eq!(result.data["body"]["created"], true);
    }

    #[tokio::test]
    async fn test_http_executor_error_status() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/not-found"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/not-found", server.uri()),
        });

        let result = executor.execute(params).await.unwrap();
        assert_eq!(result.status, "error");
        assert_eq!(result.data["status_code"], 404);
    }

    #[tokio::test]
    async fn test_http_executor_binary_response() {
        let server = MockServer::start().await;
        let png_bytes: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

        Mock::given(method("GET"))
            .and(path("/avatar.png"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "image/png")
                    .set_body_bytes(png_bytes.clone()),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/avatar.png", server.uri()),
        });

        let result = executor.execute(params).await.unwrap();
        assert_eq!(result.status, "success");
        assert_eq!(result.data["body"]["encoding"], "base64");

        let decoded = BASE64_STANDARD
            .decode(result.data["body"]["data"].as_str().unwrap())
            .unwrap();
        assert_eq!(decoded, png_bytes);
        assert_eq!(result.data["size_bytes"], png_bytes.len());
    }

    #[tokio::test]
    async fn test_http_executor_timeout() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(60)))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/slow", server.uri()),
            "timeout_ms": 200,
        });

        let result = executor.execute(params).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("[timeout]"),
            "Error should be classified as timeout: {err}"
        );
    }

    #[tokio::test]
    async fn test_http_executor_custom_headers() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api"))
            .and(header("authorization", "Bearer tok123"))
            .and(header("x-custom", "value"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/api", server.uri()),
            "headers": [
                { "key": "Authorization", "value": "Bearer tok123" },
                { "key": "X-Custom", "value": "value" },
            ],
        });

        let result = executor.execute(params).await.unwrap();
        assert_eq!(result.status, "success");
        assert_eq!(result.data["status_code"], 200);
    }

    #[tokio::test]
    async fn test_execute_with_cancel_returns_typed_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/typed"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("set-cookie", "sid=abc; Path=/; HttpOnly")
                    .set_body_json(serde_json::json!({"ok": true})),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/typed", server.uri()),
        });
        let token = CancellationToken::new();

        let response = executor
            .execute_with_cancel(params, token)
            .await
            .expect("execute_with_cancel should succeed");

        assert_eq!(response.status_code, 200);
        assert_eq!(response.body["ok"], true);
        assert_eq!(response.cookies.len(), 1);
        assert_eq!(response.cookies[0].name, "sid");
        assert_eq!(response.cookies[0].path.as_deref(), Some("/"));
        assert!(response.cookies[0].http_only);
        assert_eq!(response.timing.total_ms, response.elapsed_ms);
    }

    #[tokio::test]
    async fn test_execute_with_cancel_aborts_in_flight_request() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(60)))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/slow", server.uri()),
        });
        let token = CancellationToken::new();
        let token_clone = token.clone();

        // Cancel the request after a short delay so the future is mid-flight.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            token_clone.cancel();
        });

        let result = executor.execute_with_cancel(params, token).await;
        let err = result.expect_err("cancel should produce an error");
        assert_eq!(err.to_string(), "Request cancelled");
    }

    #[tokio::test]
    async fn test_execute_with_cancel_4xx_is_ok() {
        // HTTP-level errors (4xx/5xx) are still "successful executions" on
        // the typed path — only transport / cancel failures map to Err.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing"))
            .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/missing", server.uri()),
        });

        let response = executor
            .execute_with_cancel(params, CancellationToken::new())
            .await
            .expect("4xx should not be an Err on the typed path");
        assert_eq!(response.status_code, 404);
    }

    #[tokio::test]
    async fn test_execute_with_cancel_captures_multiple_cookies() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/cookies"))
            .respond_with(
                ResponseTemplate::new(200)
                    .append_header("set-cookie", "a=1; Path=/")
                    .append_header("set-cookie", "b=2; Domain=example.com; Secure")
                    .set_body_string("ok"),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/cookies", server.uri()),
        });
        let response = executor
            .execute_with_cancel(params, CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(response.cookies.len(), 2);
        let names: Vec<&str> = response.cookies.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[tokio::test]
    async fn test_execute_legacy_path_still_works_via_cancel_aware_impl() {
        // The legacy `execute` BlockResult path now goes through
        // `execute_with_cancel`; we double-check the conversion preserves
        // the shape consumers depend on.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/echo"))
            .respond_with(
                ResponseTemplate::new(201).set_body_json(serde_json::json!({"received": "yes"})),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "POST",
            "url": format!("{}/echo", server.uri()),
            "body": "{}",
        });
        let result = executor.execute(params).await.unwrap();
        assert_eq!(result.status, "success");
        assert_eq!(result.data["status_code"], 201);
        assert_eq!(result.data["body"]["received"], "yes");
    }

    #[tokio::test]
    async fn http_params_accepts_new_flags() {
        // Just exercise the deserialization — the executor reads these via
        // `serde_json::from_value` deeper down; flag-aware behaviour is
        // covered by the dedicated tests below.
        let json = serde_json::json!({
            "method": "GET",
            "url": "https://example.com",
            "follow_redirects": false,
            "verify_ssl": false,
            "encode_url": false,
            "trim_whitespace": false,
        });
        let parsed: HttpParams = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.follow_redirects, Some(false));
        assert_eq!(parsed.verify_ssl, Some(false));
        assert_eq!(parsed.encode_url, Some(false));
        assert_eq!(parsed.trim_whitespace, Some(false));
    }

    #[tokio::test]
    async fn http_params_omits_flags_yields_none() {
        // Defaults via serde — backwards-compatible with old payloads.
        let json = serde_json::json!({
            "method": "GET",
            "url": "https://example.com",
        });
        let parsed: HttpParams = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.follow_redirects, None);
        assert_eq!(parsed.verify_ssl, None);
        assert_eq!(parsed.encode_url, None);
        assert_eq!(parsed.trim_whitespace, None);
    }

    #[tokio::test]
    async fn client_cache_is_keyed_by_flags() {
        let executor = HttpExecutor::new();
        // Default client populated by `new()`.
        let n_default = executor.clients.lock().unwrap().len();
        assert!(n_default >= 1);
        let _ = executor.client_for(ClientFlags {
            follow_redirects: false,
            verify_ssl: true,
        });
        let _ = executor.client_for(ClientFlags {
            follow_redirects: false,
            verify_ssl: true,
        });
        // Same combo — no new client cached.
        assert_eq!(executor.clients.lock().unwrap().len(), n_default + 1);
        let _ = executor.client_for(ClientFlags {
            follow_redirects: false,
            verify_ssl: false,
        });
        assert_eq!(executor.clients.lock().unwrap().len(), n_default + 2);
    }

    #[tokio::test]
    async fn follow_redirects_false_returns_3xx_to_caller() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/start"))
            .respond_with(ResponseTemplate::new(302).insert_header("location", "/elsewhere"))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/start", server.uri()),
            "follow_redirects": false,
        });
        let response = executor
            .execute_with_cancel(params, CancellationToken::new())
            .await
            .unwrap();
        // Without follow, the 302 surfaces directly.
        assert_eq!(response.status_code, 302);
        assert_eq!(
            response.headers.get("location").map(|s| s.as_str()),
            Some("/elsewhere"),
        );
    }

    #[tokio::test]
    async fn trim_whitespace_strips_header_and_body() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/trim"))
            .and(header("x-trimmed", "value"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "POST",
            "url": format!("{}/trim", server.uri()),
            "headers": [
                { "key": "  X-Trimmed  ", "value": "  value  " },
            ],
            "body": "  payload  ",
            // trim_whitespace defaults ON; we pass it explicitly here for clarity.
            "trim_whitespace": true,
        });
        let result = executor.execute(params).await.unwrap();
        assert_eq!(result.status, "success");
        assert_eq!(result.data["status_code"], 200);
    }

    #[tokio::test]
    async fn encode_url_off_preserves_raw_query() {
        // With encode_url=true (default), `q=a b` becomes `q=a+b` or `q=a%20b`.
        // With encode_url=false, the raw value is appended verbatim.
        let server = MockServer::start().await;

        // Wiremock matches the raw query string the server sees.
        Mock::given(method("GET"))
            .and(path("/q"))
            .and(wiremock::matchers::query_param("q", "a%20b"))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/q", server.uri()),
            "params": [{ "key": "q", "value": "a%20b" }],
            "encode_url": false,
        });
        let result = executor.execute(params).await;
        assert!(result.is_ok(), "expected query to pass through verbatim");
    }

    #[tokio::test]
    async fn binary_body_reads_file_and_uploads_bytes() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let server = MockServer::start().await;

        // Mock asserts the request body bytes literally — the executor must
        // read from disk and ship the file content, not the textual
        // `< /path` placeholder.
        Mock::given(method("POST"))
            .and(path("/upload"))
            .and(wiremock::matchers::header(
                "content-type",
                "application/octet-stream",
            ))
            .and(wiremock::matchers::body_bytes(
                b"\x89PNG\r\n\x1a\n".to_vec(),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let mut tmp = NamedTempFile::new().unwrap();
        tmp.as_file_mut().write_all(b"\x89PNG\r\n\x1a\n").unwrap();
        tmp.as_file_mut().flush().unwrap();
        let path = tmp.path().to_string_lossy().to_string();

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "POST",
            "url": format!("{}/upload", server.uri()),
            "headers": [{ "key": "Content-Type", "value": "application/octet-stream" }],
            "body": format!("< {path}"),
        });
        let result = executor.execute(params).await.unwrap();
        assert_eq!(result.status, "success");
    }

    #[tokio::test]
    async fn multipart_body_uploads_with_file_part() {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let server = MockServer::start().await;

        // The mock only checks that the request actually shipped as multipart
        // (Content-Type header carries `multipart/form-data; boundary=…`),
        // because asserting against the streamed multipart body bytes is
        // brittle (CRLF normalization, boundary auto-generation). The shape
        // of each part is covered by the unit tests on `parse_multipart_textual`
        // above.
        Mock::given(method("POST"))
            .and(path("/form"))
            .and(wiremock::matchers::header_regex(
                "content-type",
                "^multipart/form-data; boundary=",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
            .mount(&server)
            .await;

        let mut tmp = NamedTempFile::new().unwrap();
        // Write file content via std::io (sync) so we don't fight tokio's
        // owned-handle expectation; we only need the bytes on disk.
        tmp.as_file_mut().write_all(b"FILECONTENT").unwrap();
        tmp.as_file_mut().flush().unwrap();
        let path = tmp.path().to_string_lossy().to_string();

        let body = format!("name=alice\nfile=< {path}");

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "POST",
            "url": format!("{}/form", server.uri()),
            "headers": [
                { "key": "Content-Type", "value": "multipart/form-data" },
            ],
            "body": body,
        });
        let result = executor.execute(params).await.unwrap();
        assert_eq!(result.status, "success");
    }

    #[tokio::test]
    async fn binary_body_missing_file_returns_error() {
        let server = MockServer::start().await;
        // No mock — request never goes out because file read fails first.

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "POST",
            "url": format!("{}/x", server.uri()),
            "headers": [{ "key": "Content-Type", "value": "application/octet-stream" }],
            "body": "< /this/path/does/not/exist.bin",
        });
        let result = executor.execute(params).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Read body file"), "got: {err}");
    }

    /// Capture all chunks emitted on the callback. Returned `Arc<Mutex<Vec>>`
    /// is shared with the closure passed to `execute_streamed`.
    fn capture_chunks() -> (
        std::sync::Arc<std::sync::Mutex<Vec<HttpChunk>>>,
        impl Fn(HttpChunk) + Send,
    ) {
        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<HttpChunk>::new()));
        let buf_clone = buf.clone();
        let cb = move |chunk: HttpChunk| {
            buf_clone.lock().unwrap().push(chunk);
        };
        (buf, cb)
    }

    fn count_kinds(chunks: &[HttpChunk]) -> (usize, usize, usize, usize, usize) {
        let mut h = 0;
        let mut b = 0;
        let mut c = 0;
        let mut e = 0;
        let mut x = 0;
        for ch in chunks {
            match ch {
                HttpChunk::Headers { .. } => h += 1,
                HttpChunk::BodyChunk { .. } => b += 1,
                HttpChunk::Complete(_) => c += 1,
                HttpChunk::Error { .. } => e += 1,
                HttpChunk::Cancelled => x += 1,
            }
        }
        (h, b, c, e, x)
    }

    #[tokio::test]
    async fn streaming_emits_headers_then_body_then_complete() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/stream"))
            .respond_with(ResponseTemplate::new(200).set_body_string("hello world"))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/stream", server.uri()),
        });
        let (buf, cb) = capture_chunks();
        let result = executor
            .execute_streamed(params, CancellationToken::new(), cb)
            .await
            .expect("streamed execution should succeed");

        assert_eq!(result.status_code, 200);
        let chunks = buf.lock().unwrap();
        let (h, b, c, e, x) = count_kinds(&chunks);
        assert_eq!(h, 1, "exactly one Headers chunk");
        assert!(b >= 1, "at least one BodyChunk (got {b})");
        assert_eq!(c, 1, "exactly one Complete chunk");
        assert_eq!(e, 0);
        assert_eq!(x, 0);
        // First chunk MUST be Headers, last MUST be Complete.
        assert!(matches!(chunks.first(), Some(HttpChunk::Headers { .. })));
        assert!(matches!(chunks.last(), Some(HttpChunk::Complete(_))));
    }

    #[tokio::test]
    async fn streaming_handles_large_body_without_oom() {
        // 10 MB response — well under the 100 MB cap. Verifies that the
        // bytes_stream loop completes and that BodyChunk offsets cover the
        // whole payload.
        let server = MockServer::start().await;
        let payload: Vec<u8> = vec![0xAB; 10 * 1024 * 1024];
        Mock::given(method("GET"))
            .and(path("/big"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/octet-stream")
                    .set_body_bytes(payload.clone()),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/big", server.uri()),
        });
        let (buf, cb) = capture_chunks();
        let result = executor
            .execute_streamed(params, CancellationToken::new(), cb)
            .await
            .expect("10MB streaming should succeed");

        assert_eq!(result.status_code, 200);
        assert_eq!(result.size_bytes, 10 * 1024 * 1024);

        // Sum of BodyChunk byte lengths must equal the total payload.
        let chunks = buf.lock().unwrap();
        let body_total: usize = chunks
            .iter()
            .filter_map(|c| match c {
                HttpChunk::BodyChunk { bytes, .. } => Some(bytes.len()),
                _ => None,
            })
            .sum();
        assert_eq!(body_total, 10 * 1024 * 1024);
    }

    #[tokio::test]
    async fn streaming_enforces_body_cap() {
        // 100 MB + 1 byte payload — exceeds the cap by 1 byte. Validates
        // that the executor returns the typed `[body_too_large]` error and
        // does NOT emit a Complete chunk on the channel.
        let server = MockServer::start().await;
        let payload: Vec<u8> = vec![0u8; (100 * 1024 * 1024) + 1];
        Mock::given(method("GET"))
            .and(path("/over"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/octet-stream")
                    .set_body_bytes(payload),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/over", server.uri()),
        });
        let (buf, cb) = capture_chunks();
        let result = executor
            .execute_streamed(params, CancellationToken::new(), cb)
            .await;

        let err = result.expect_err("oversized body must error").to_string();
        assert!(
            err.contains("[body_too_large]"),
            "expected body_too_large classification, got: {err}"
        );
        let chunks = buf.lock().unwrap();
        let (_h, _b, c, _e, _x) = count_kinds(&chunks);
        assert_eq!(c, 0, "Complete must NOT be emitted when cap fires");
    }

    #[tokio::test]
    async fn streaming_cancel_mid_body_emits_no_complete() {
        // Slow-trickle response — the response delay holds the body before
        // it streams, giving us a window to cancel mid-flight. The cancel
        // future fires before the body finishes, and the executor returns
        // `Err("Request cancelled")`. No Complete chunk should appear on
        // the callback.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/slow-body"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(500))
                    .set_body_bytes(vec![0u8; 4 * 1024 * 1024]),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/slow-body", server.uri()),
        });
        let token = CancellationToken::new();
        let token_clone = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(80)).await;
            token_clone.cancel();
        });

        let (buf, cb) = capture_chunks();
        let result = executor.execute_streamed(params, token, cb).await;
        let err = result
            .expect_err("cancel must surface as error")
            .to_string();
        assert_eq!(err, "Request cancelled");

        let chunks = buf.lock().unwrap();
        let (_h, _b, c, _e, _x) = count_kinds(&chunks);
        assert_eq!(c, 0, "Complete must NOT be emitted on cancel");
    }

    #[tokio::test]
    async fn ttfb_is_less_than_or_equal_to_total() {
        // Wiremock honors the per-request delay before sending the response,
        // so TTFB picks up at least ~50ms. Total includes TTFB + body
        // streaming, so the ≤ relation must hold.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/timed"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_millis(50))
                    .set_body_string("ok"),
            )
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "GET",
            "url": format!("{}/timed", server.uri()),
        });
        let (buf, cb) = capture_chunks();
        let response = executor
            .execute_streamed(params, CancellationToken::new(), cb)
            .await
            .unwrap();

        let chunks = buf.lock().unwrap();
        let header_ttfb = chunks
            .iter()
            .find_map(|c| match c {
                HttpChunk::Headers { ttfb_ms, .. } => Some(*ttfb_ms),
                _ => None,
            })
            .expect("Headers chunk must carry ttfb_ms");

        assert!(
            header_ttfb <= response.timing.total_ms,
            "ttfb_ms ({}) > total_ms ({})",
            header_ttfb,
            response.timing.total_ms,
        );
        assert_eq!(
            response.timing.ttfb_ms,
            Some(header_ttfb),
            "TimingBreakdown.ttfb_ms must match the Headers chunk value",
        );
        // V1 invariant — sub-fields stay None.
        assert_eq!(response.timing.dns_ms, None);
        assert_eq!(response.timing.connect_ms, None);
        assert_eq!(response.timing.tls_ms, None);
        assert!(!response.timing.connection_reused);
    }

    #[tokio::test]
    async fn streaming_legacy_execute_with_cancel_still_works() {
        // The legacy entry point now wraps execute_streamed with a no-op
        // callback. Behaviour for callers (legacy `execute()` Tauri command,
        // existing tests) must be byte-for-byte identical aside from the
        // newly populated `ttfb_ms` field.
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/legacy"))
            .respond_with(ResponseTemplate::new(201).set_body_json(serde_json::json!({"ok": true})))
            .mount(&server)
            .await;

        let executor = HttpExecutor::new();
        let params = serde_json::json!({
            "method": "POST",
            "url": format!("{}/legacy", server.uri()),
            "body": "{}",
        });
        let response = executor
            .execute_with_cancel(params, CancellationToken::new())
            .await
            .expect("legacy entry point must keep working");

        assert_eq!(response.status_code, 201);
        assert_eq!(response.body["ok"], true);
        assert_eq!(response.timing.total_ms, response.elapsed_ms);
        assert!(response.timing.ttfb_ms.is_some());
    }
}
