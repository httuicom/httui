use crate::blocks::parser::ParsedBlock;

/// Serialize a [`ParsedBlock`] back to its fenced markdown form.
///
/// Output is canonical and deterministic: parsing the result and
/// re-serializing yields byte-identical bytes (idempotent), and parsing
/// preserves the semantic shape of the original block.
///
/// Format per block type:
/// - `db` family — info string `<type> [alias=…] [connection=…] [limit=…] [timeout=…] [display=…]`,
///   body is the raw SQL stored in `params.query`. Mirrors the canonical
///   form documented in `src/lib/blocks/db-fence.ts`.
/// - `http` — info string `<type> [alias=…] [displayMode=…]`,
///   body is `params` rendered as compact JSON.
/// - Unknown types — same as http (JSON body fallback). New block
///   types can ship without a dedicated serializer until their fence
///   shape is finalized.
pub fn serialize_block(block: &ParsedBlock) -> String {
    if is_db_block(&block.block_type) {
        serialize_db_block(block)
    } else if block.block_type == "http" {
        serialize_http_block(block)
    } else {
        serialize_json_block(block)
    }
}

fn is_db_block(block_type: &str) -> bool {
    block_type == "db" || block_type.starts_with("db-")
}

/// Serialize an http block in canonical HTTP-message form. Mirrors
/// the desktop/CodeMirror writer in `src/lib/blocks/http-fence.ts`:
///
/// - Line 1: `<METHOD> <URL>` with query params re-attached as
///   `?key=value&key=value` (URL-encoded values).
/// - One line per header: `Name: value`.
/// - Blank line, then the request body, when the body is non-empty.
///
/// The legacy JSON-body shape is still accepted by the parser (round-
/// trip via `parse_legacy_http_body`), so existing vaults keep
/// opening; new saves all use the canonical form.
fn serialize_http_block(block: &ParsedBlock) -> String {
    let mut info = block.block_type.clone();
    if let Some(alias) = block.alias.as_deref().filter(|s| !s.is_empty()) {
        info.push_str(" alias=");
        info.push_str(alias);
    }
    if let Some(display) = block.display_mode.as_deref().filter(|s| !s.is_empty()) {
        info.push_str(" displayMode=");
        info.push_str(display);
    }

    let params = block.params.as_object();
    let method = params
        .and_then(|o| o.get("method"))
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_uppercase();
    let url = params
        .and_then(|o| o.get("url"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let query = params
        .and_then(|o| o.get("params"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.as_slice())
        .unwrap_or(&[]);
    let headers = params
        .and_then(|o| o.get("headers"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.as_slice())
        .unwrap_or(&[]);
    let body = params
        .and_then(|o| o.get("body"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut request_line = String::new();
    request_line.push_str(&method);
    request_line.push(' ');
    request_line.push_str(url);

    // When every param is enabled we inline them on the request line
    // (`?a=1&b=2`). A single disabled param forces the continuation form — one
    // `?`/`&` line per param — so the `# ` marker has a line to live on.
    // Mirrors `canInlineQuery` in the desktop stringifier.
    let any_disabled_param = query.iter().any(|p| {
        !kv_enabled(p)
            && !p
                .get("key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty()
    });
    let mut param_lines = String::new();
    if any_disabled_param {
        let mut first = true;
        for p in query {
            let key = p.get("key").and_then(|v| v.as_str()).unwrap_or("");
            if key.is_empty() {
                continue;
            }
            let value = p.get("value").and_then(|v| v.as_str()).unwrap_or("");
            let seg = if value.is_empty() {
                key.to_string()
            } else {
                format!("{key}={value}")
            };
            param_lines.push('\n');
            if !kv_enabled(p) {
                param_lines.push_str("# ");
            }
            param_lines.push(if first { '?' } else { '&' });
            param_lines.push_str(&seg);
            first = false;
        }
    } else {
        let query_string = build_query_string(query);
        if !query_string.is_empty() {
            // Append `?` only when the URL doesn't already carry one;
            // otherwise the params extend an existing query (`&`).
            let sep = if url.contains('?') { '&' } else { '?' };
            request_line.push(sep);
            request_line.push_str(&query_string);
        }
    }

    let mut header_lines = String::new();
    for h in headers {
        let key = h.get("key").and_then(|v| v.as_str()).unwrap_or("");
        let value = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
        if key.is_empty() {
            continue;
        }
        header_lines.push('\n');
        if !kv_enabled(h) {
            header_lines.push_str("# ");
        }
        header_lines.push_str(key);
        header_lines.push_str(": ");
        header_lines.push_str(value);
    }

    let mut body_block = String::new();
    let trimmed_body = body.trim_end_matches('\n');
    if !trimmed_body.is_empty() {
        body_block.push_str("\n\n");
        body_block.push_str(trimmed_body);
    }

    format!("```{info}\n{request_line}{param_lines}{header_lines}{body_block}\n```")
}

/// Render a query-param array as `k1=v1&k2=v2`. Empty keys are
/// dropped (same as the parser's behavior on the read path).
///
/// Values are passed through verbatim — no percent-encoding. The
/// parser doesn't decode either (`?q=hello%20world` parses as value
/// `"hello%20world"`), so users author whatever they want on the URL
/// and it round-trips literally. This matches the desktop / TS
/// implementation in `src/lib/blocks/http-fence.ts:formatParam`.
/// Read a row's `enabled` flag, defaulting to `true` when absent — matches the
/// parser's omit-when-true convention and the executor's serde default.
fn kv_enabled(row: &serde_json::Value) -> bool {
    row.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true)
}

fn build_query_string(params: &[serde_json::Value]) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(params.len());
    for p in params {
        let key = p.get("key").and_then(|v| v.as_str()).unwrap_or("");
        if key.is_empty() {
            continue;
        }
        let value = p.get("value").and_then(|v| v.as_str()).unwrap_or("");
        if value.is_empty() {
            parts.push(key.to_string());
        } else {
            parts.push(format!("{key}={value}"));
        }
    }
    parts.join("&")
}

fn serialize_db_block(block: &ParsedBlock) -> String {
    let mut info = block.block_type.clone();

    if let Some(alias) = block.alias.as_deref().filter(|s| !s.is_empty()) {
        info.push_str(" alias=");
        info.push_str(alias);
    }

    let params = block.params.as_object();
    if let Some(obj) = params {
        // Read `connection` (canonical, written by the TUI's
        // connection picker after redesign) first, falling back to
        // `connection_id` (legacy / parser-emitted). Without this
        // fallback, a freshly-edited block whose connection came
        // from the picker would lose the connection on serialize.
        if let Some(conn) = obj
            .get("connection")
            .or_else(|| obj.get("connection_id"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
        {
            info.push_str(" connection=");
            info.push_str(conn);
        }
        if let Some(limit) = obj.get("limit").and_then(|v| v.as_u64()) {
            info.push_str(" limit=");
            info.push_str(&limit.to_string());
        }
        if let Some(timeout) = obj.get("timeout_ms").and_then(|v| v.as_u64()) {
            info.push_str(" timeout=");
            info.push_str(&timeout.to_string());
        }
    }

    if let Some(display) = block.display_mode.as_deref().filter(|s| !s.is_empty()) {
        info.push_str(" display=");
        info.push_str(display);
    }

    let body = params
        .and_then(|o| o.get("query"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    format!("```{info}\n{body}\n```")
}

fn serialize_json_block(block: &ParsedBlock) -> String {
    let mut info = block.block_type.clone();

    if let Some(alias) = block.alias.as_deref().filter(|s| !s.is_empty()) {
        info.push_str(" alias=");
        info.push_str(alias);
    }
    if let Some(display) = block.display_mode.as_deref().filter(|s| !s.is_empty()) {
        info.push_str(" displayMode=");
        info.push_str(display);
    }

    let body = serde_json::to_string(&block.params).unwrap_or_else(|_| "null".to_string());

    format!("```{info}\n{body}\n```")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::parser::parse_blocks;
    use serde_json::json;

    fn assert_semantic_roundtrip(md: &str) {
        let parsed = parse_blocks(md);
        assert_eq!(parsed.len(), 1, "expected exactly 1 block in fixture");
        let serialized = serialize_block(&parsed[0]);
        let reparsed = parse_blocks(&serialized);
        assert_eq!(reparsed.len(), 1, "roundtrip must yield 1 block");
        assert_eq!(reparsed[0].block_type, parsed[0].block_type);
        assert_eq!(reparsed[0].alias, parsed[0].alias);
        assert_eq!(reparsed[0].display_mode, parsed[0].display_mode);
        assert_eq!(reparsed[0].params, parsed[0].params);
    }

    fn assert_idempotent(md: &str) {
        let parsed = parse_blocks(md);
        let s1 = serialize_block(&parsed[0]);
        let reparsed = parse_blocks(&s1);
        let s2 = serialize_block(&reparsed[0]);
        assert_eq!(s1, s2, "serialization must be idempotent");
    }

    #[test]
    fn roundtrip_http_simple() {
        let md = "```http alias=login\n{\"method\":\"POST\",\"url\":\"https://api.test.com/login\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        assert_semantic_roundtrip(md);
        assert_idempotent(md);
    }

    #[test]
    fn roundtrip_http_disabled_header() {
        let md = "```http alias=req\nGET https://api.test.com/x\nAccept: application/json\n# X-Debug: on\n```\n";
        assert_semantic_roundtrip(md);
        assert_idempotent(md);
        let parsed = parse_blocks(md);
        let out = serialize_block(&parsed[0]);
        assert!(
            out.contains("\n# X-Debug: on"),
            "disabled header keeps `# ` marker: {out}"
        );
        assert!(
            out.contains("\nAccept: application/json"),
            "enabled header has no marker: {out}"
        );
    }

    #[test]
    fn roundtrip_http_disabled_param_uses_continuation() {
        let md = "```http\nGET https://api.test.com/search\n?q=hello\n# &page=2\n```\n";
        assert_semantic_roundtrip(md);
        assert_idempotent(md);
        let parsed = parse_blocks(md);
        let out = serialize_block(&parsed[0]);
        assert!(
            out.contains("\n?q=hello"),
            "first param on its own line: {out}"
        );
        assert!(
            out.contains("\n# &page=2"),
            "disabled param keeps `# &`: {out}"
        );
        assert!(
            !out.contains("?q=hello&page=2"),
            "must not inline when a param is disabled: {out}"
        );
    }

    #[test]
    fn roundtrip_http_with_display_mode() {
        let md = "```http alias=login displayMode=split\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        assert_semantic_roundtrip(md);
        assert_idempotent(md);
    }

    #[test]
    fn roundtrip_http_without_alias() {
        let md = "```http\n{\"method\":\"GET\",\"url\":\"https://x.com\",\"params\":[],\"headers\":[],\"body\":\"\"}\n```\n";
        assert_semantic_roundtrip(md);
        assert_idempotent(md);
    }

    #[test]
    fn roundtrip_db_postgres_full() {
        let md = "```db-postgres alias=db1 connection=prod limit=100 timeout=30000 display=split\nSELECT *\nFROM users\nWHERE id > 10\n```\n";
        assert_semantic_roundtrip(md);
        assert_idempotent(md);
    }

    #[test]
    fn roundtrip_db_minimal() {
        let md = "```db-mysql\nSELECT 1\n```\n";
        assert_semantic_roundtrip(md);
        assert_idempotent(md);
    }

    #[test]
    fn roundtrip_db_with_display_only() {
        let md = "```db alias=q display=output\nSELECT 1\n```\n";
        assert_semantic_roundtrip(md);
        assert_idempotent(md);
    }

    #[test]
    fn db_info_string_emits_canonical_order() {
        let parsed = parse_blocks(
            "```db-postgres alias=a display=split timeout=5000 limit=50 connection=prod\nSELECT 1\n```\n",
        );
        let out = serialize_block(&parsed[0]);
        assert!(
            out.starts_with(
                "```db-postgres alias=a connection=prod limit=50 timeout=5000 display=split\n"
            ),
            "got: {out}"
        );
    }

    #[test]
    fn db_info_string_omits_missing_fields() {
        let parsed = parse_blocks("```db-postgres alias=a\nSELECT 1\n```\n");
        let out = serialize_block(&parsed[0]);
        assert_eq!(out, "```db-postgres alias=a\nSELECT 1\n```");
    }

    #[test]
    fn db_legacy_body_normalizes_to_canonical_form() {
        // Legacy JSON body: `params` already has connection_id/query/timeout_ms.
        // Serializer must emit the new raw-SQL canonical form regardless.
        let parsed = parse_blocks(
            "```db-postgres alias=u\n{\"connection_id\":\"x\",\"query\":\"SELECT 1\",\"timeout_ms\":5000}\n```\n",
        );
        let out = serialize_block(&parsed[0]);
        assert_eq!(
            out,
            "```db-postgres alias=u connection=x timeout=5000\nSELECT 1\n```"
        );
    }

    #[test]
    fn http_emits_request_line_with_method_and_url() {
        let parsed = parse_blocks("```http alias=h\nGET https://example.com/users\n```\n");
        let out = serialize_block(&parsed[0]);
        assert_eq!(out, "```http alias=h\nGET https://example.com/users\n```",);
    }

    #[test]
    fn http_emits_headers_one_per_line() {
        let parsed = parse_blocks(
            "```http alias=h\nGET https://example.com/u\nAuthorization: Bearer abc\nAccept: application/json\n```\n",
        );
        let out = serialize_block(&parsed[0]);
        assert_eq!(
            out,
            "```http alias=h\nGET https://example.com/u\nAuthorization: Bearer abc\nAccept: application/json\n```",
        );
    }

    #[test]
    fn http_emits_body_after_blank_separator() {
        let parsed = parse_blocks(
            "```http alias=h\nPOST https://example.com/u\nContent-Type: application/json\n\n{\"name\":\"alice\"}\n```\n",
        );
        let out = serialize_block(&parsed[0]);
        assert_eq!(
            out,
            "```http alias=h\nPOST https://example.com/u\nContent-Type: application/json\n\n{\"name\":\"alice\"}\n```",
        );
    }

    #[test]
    fn http_legacy_json_body_normalizes_to_http_message_form() {
        // Legacy JSON shape (pre-redesign) must serialize as the new
        // HTTP-message canonical form, just like DB blocks normalize
        // their legacy JSON bodies to raw SQL.
        let parsed = parse_blocks(
            "```http alias=u\n{\"method\":\"POST\",\"url\":\"https://api.test.com/login\",\"params\":[],\"headers\":[{\"key\":\"Accept\",\"value\":\"application/json\"}],\"body\":\"{\\\"u\\\":1}\"}\n```\n",
        );
        let out = serialize_block(&parsed[0]);
        assert_eq!(
            out,
            "```http alias=u\nPOST https://api.test.com/login\nAccept: application/json\n\n{\"u\":1}\n```",
        );
    }

    #[test]
    fn http_query_params_inlined_verbatim() {
        // Legacy JSON body with structured `params` array. The
        // serializer inlines them onto the URL with `?k=v&...`,
        // values verbatim — no url-encoding (paritied with TS at
        // `src/lib/blocks/http-fence.ts:formatParam`, which also
        // passes values through unchanged).
        let parsed = parse_blocks(
            "```http alias=q\n{\"method\":\"GET\",\"url\":\"https://api.test.com/search\",\"params\":[{\"key\":\"q\",\"value\":\"hello%20world\"},{\"key\":\"page\",\"value\":\"2\"}],\"headers\":[],\"body\":\"\"}\n```\n",
        );
        let out = serialize_block(&parsed[0]);
        assert!(
            out.contains("?q=hello%20world&page=2"),
            "expected query string verbatim in output, got: {out}",
        );
        // Re-parse round-trips: the structural URL is preserved.
        let reparsed = parse_blocks(&out);
        assert_eq!(reparsed.len(), 1);
    }

    #[test]
    fn unknown_block_type_serializes_as_json() {
        // Future block types (e.g. graphql) without a dedicated serializer
        // fall back to JSON-body. This guarantees adding a new type to the
        // parser doesn't break round-trip.
        let block = ParsedBlock {
            block_type: "graphql".to_string(),
            alias: Some("q1".to_string()),
            display_mode: None,
            params: json!({"query": "{ user { id } }"}),
            line_start: 0,
            line_end: 0,
        };
        let out = serialize_block(&block);
        assert_eq!(
            out,
            "```graphql alias=q1\n{\"query\":\"{ user { id } }\"}\n```"
        );
    }
}
