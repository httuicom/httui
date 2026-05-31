use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A parsed executable block extracted from a markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedBlock {
    pub block_type: String,
    pub alias: Option<String>,
    pub display_mode: Option<String>,
    pub params: serde_json::Value,
    pub line_start: usize,
    pub line_end: usize,
}

/// Known executable block types.
const EXECUTABLE_TYPES: &[&str] = &["http", "db", "db-postgres", "db-mysql", "db-sqlite"];

/// Parse all executable blocks from a markdown string.
///
/// Scans for fenced code blocks with known executable types in the info string.
///
/// Supported body formats per block type:
/// - `http`: HTTP-message body (post-redesign) or legacy JSON shim.
/// - `db-*`: heuristically detects legacy JSON body vs. raw SQL body.
///   - Legacy: body starts with `{` and parses as JSON containing a string `query` field.
///     `params` is the JSON as-is.
///   - New: body is raw SQL; `params` is synthesized as
///     `{query, connection_id?, limit?, timeout_ms?}` by merging info-string
///     tokens (`connection=`, `limit=`, `timeout=`) with the body.
pub fn parse_blocks(markdown: &str) -> Vec<ParsedBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = markdown.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if let Some(info) = strip_fence_open(trimmed) {
            let (block_type, attrs) = parse_info_string(info);

            if EXECUTABLE_TYPES.contains(&block_type.as_str()) {
                let line_start = i;

                let mut body_lines = Vec::new();
                i += 1;
                while i < lines.len() {
                    let l = lines[i].trim();
                    if l == "```" || l.starts_with("```") && l.chars().skip(3).all(|c| c == '`') {
                        break;
                    }
                    body_lines.push(lines[i]);
                    i += 1;
                }
                let line_end = i;

                let body = body_lines.join("\n");
                let params = if is_db_block(&block_type) {
                    parse_db_body(&body, &attrs)
                } else if block_type == "http" {
                    parse_http_body(&body, &attrs)
                } else {
                    serde_json::from_str(&body).unwrap_or(serde_json::Value::Null)
                };

                // `display=` (new canonical form) wins over legacy `displayMode=` when both present.
                let display_mode = attrs
                    .get("display")
                    .or_else(|| attrs.get("displayMode"))
                    .cloned();

                blocks.push(ParsedBlock {
                    block_type,
                    alias: attrs.get("alias").cloned(),
                    display_mode,
                    params,
                    line_start,
                    line_end,
                });
            }
        }

        i += 1;
    }

    blocks
}

fn is_db_block(block_type: &str) -> bool {
    block_type == "db" || block_type.starts_with("db-")
}

/// Parse a db-* block body, detecting legacy JSON vs. raw SQL format.
///
/// Legacy JSON format is detected when the trimmed body starts with `{` and parses
/// as a JSON object with a string `query` field. Otherwise the body is treated as
/// raw SQL and merged with info-string tokens.
fn parse_db_body(body: &str, attrs: &HashMap<String, String>) -> serde_json::Value {
    if let Some(legacy) = parse_legacy_db_body(body) {
        return legacy;
    }
    synthesize_db_params_from_info(body, attrs)
}

fn parse_legacy_db_body(body: &str) -> Option<serde_json::Value> {
    let trimmed = body.trim_start();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let obj = value.as_object()?;
    if !obj.get("query").is_some_and(|v| v.is_string()) {
        return None;
    }
    Some(value)
}

/// Build a params JSON object for the new raw-SQL format by merging info-string
/// tokens with the body. Only emits fields that are explicitly present in the
/// info string (or the body), so downstream consumers can distinguish
/// "unspecified" from "explicitly zero".
fn synthesize_db_params_from_info(
    body: &str,
    attrs: &HashMap<String, String>,
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "query".to_string(),
        serde_json::Value::String(body.to_string()),
    );

    if let Some(conn) = attrs.get("connection") {
        if !conn.is_empty() {
            obj.insert(
                "connection_id".to_string(),
                serde_json::Value::String(conn.clone()),
            );
        }
    }
    if let Some(limit_str) = attrs.get("limit") {
        if let Ok(limit) = limit_str.parse::<u64>() {
            obj.insert("limit".to_string(), serde_json::Value::Number(limit.into()));
        }
    }
    if let Some(timeout_str) = attrs.get("timeout") {
        if let Ok(timeout) = timeout_str.parse::<u64>() {
            obj.insert(
                "timeout_ms".to_string(),
                serde_json::Value::Number(timeout.into()),
            );
        }
    }

    serde_json::Value::Object(obj)
}

/// Parse an http block body, detecting legacy JSON vs. new HTTP-message format.
///
/// Legacy JSON is detected when the trimmed body starts with `{` AND parses as
/// a JSON object with string `method` and `url` fields. Otherwise the body is
/// treated as an HTTP message (`METHOD URL` line, headers, blank line, body)
/// and synthesized into the same JSON shape downstream consumers expect.
fn parse_http_body(body: &str, attrs: &HashMap<String, String>) -> serde_json::Value {
    if let Some(legacy) = parse_legacy_http_body(body) {
        return legacy;
    }
    parse_http_message_body(body, attrs)
}

fn parse_legacy_http_body(body: &str) -> Option<serde_json::Value> {
    let trimmed = body.trim_start();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;
    let obj = value.as_object()?;
    if !obj.get("method").is_some_and(|v| v.is_string()) {
        return None;
    }
    if !obj.get("url").is_some_and(|v| v.is_string()) {
        return None;
    }
    Some(value)
}

/// Parse an HTTP-message-formatted body into the JSON shape expected by the
/// executor: `{ method, url, params: [{key, value, enabled?}], headers: [{key,
/// value, enabled?}], body, timeout_ms? }`.
///
/// Disabled rows (`# ` prefix) are preserved with `enabled: false` so the
/// editor can show them unchecked and the serializer round-trips the marker;
/// the executor skips them. `enabled` is omitted when true. Descriptions
/// (`# desc: ...`) are not modeled and are dropped.
fn parse_http_message_body(body: &str, attrs: &HashMap<String, String>) -> serde_json::Value {
    let lines: Vec<&str> = body.split('\n').collect();
    let mut i = 0;

    // Skip leading blanks and `#` comments.
    while i < lines.len() {
        let t = lines[i].trim();
        if t.is_empty() || t.starts_with('#') {
            i += 1;
        } else {
            break;
        }
    }

    if i >= lines.len() {
        return empty_http_params(attrs);
    }

    let first_line = lines[i].trim().to_string();
    i += 1;

    let (method, url, inline_params) = match parse_http_first_line(&first_line) {
        Some(v) => v,
        None => return empty_http_params(attrs),
    };

    // Phase 1: query continuations and headers, until first blank line.
    // Rows carry an `enabled` flag; inline query params are always enabled.
    let mut params: Vec<(String, String, bool)> = inline_params
        .into_iter()
        .map(|(k, v)| (k, v, true))
        .collect();
    let mut headers: Vec<(String, String, bool)> = Vec::new();
    let mut saw_header = false;

    while i < lines.len() {
        let raw = lines[i];
        let trimmed = raw.trim();

        if trimmed.is_empty() {
            i += 1;
            break;
        }

        // Descriptions are not modeled — drop them before the disabled-row
        // check (a description line also starts with `# `).
        if trimmed == "# desc:" || trimmed.starts_with("# desc: ") {
            i += 1;
            continue;
        }

        // Disabled row (`# ` prefix or bare `#`): preserve it as `enabled:
        // false` so the editor shows it unchecked and the serializer keeps the
        // marker. The executor skips disabled rows.
        if trimmed == "#" || trimmed.starts_with("# ") {
            let inner = if trimmed == "#" { "" } else { &trimmed[2..] };
            if !saw_header && (inner.starts_with('?') || inner.starts_with('&')) {
                if let Some((k, v)) = split_query_segment(&inner[1..]) {
                    params.push((k, v, false));
                }
            } else if let Some((k, v)) = split_header_line(inner) {
                headers.push((k, v, false));
                saw_header = true;
            }
            // else: free-form comment, dropped.
            i += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            // `#xxx` (no space) → free-form comment, ignored.
            i += 1;
            continue;
        }

        if !saw_header && (trimmed.starts_with('?') || trimmed.starts_with('&')) {
            if let Some((k, v)) = split_query_segment(&trimmed[1..]) {
                params.push((k, v, true));
            }
            i += 1;
            continue;
        }

        if let Some((k, v)) = split_header_line(trimmed) {
            headers.push((k, v, true));
            saw_header = true;
        }
        i += 1;
    }

    // Phase 2: body. Drop trailing blank lines for idempotency.
    let mut body_lines: Vec<&str> = lines[i..].to_vec();
    while body_lines.last().map(|s| s.is_empty()).unwrap_or(false) {
        body_lines.pop();
    }
    let body_text = body_lines.join("\n");

    let mut obj = serde_json::Map::new();
    obj.insert("method".to_string(), serde_json::Value::String(method));
    obj.insert("url".to_string(), serde_json::Value::String(url));
    obj.insert(
        "params".to_string(),
        serde_json::Value::Array(
            params
                .into_iter()
                .map(|(k, v, enabled)| http_kv_json(k, v, enabled))
                .collect(),
        ),
    );
    obj.insert(
        "headers".to_string(),
        serde_json::Value::Array(
            headers
                .into_iter()
                .map(|(k, v, enabled)| http_kv_json(k, v, enabled))
                .collect(),
        ),
    );
    obj.insert("body".to_string(), serde_json::Value::String(body_text));

    if let Some(timeout_str) = attrs.get("timeout") {
        if let Ok(timeout) = timeout_str.parse::<u64>() {
            obj.insert(
                "timeout_ms".to_string(),
                serde_json::Value::Number(timeout.into()),
            );
        }
    }

    serde_json::Value::Object(obj)
}

/// Build a `{key, value}` row, adding `enabled: false` only for disabled rows.
/// Enabled rows omit the flag so the canonical shape (and parity fixtures)
/// stay `{key, value}`; the executor defaults a missing `enabled` to `true`.
fn http_kv_json(key: String, value: String, enabled: bool) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    m.insert("key".to_string(), serde_json::Value::String(key));
    m.insert("value".to_string(), serde_json::Value::String(value));
    if !enabled {
        m.insert("enabled".to_string(), serde_json::Value::Bool(false));
    }
    serde_json::Value::Object(m)
}

fn empty_http_params(attrs: &HashMap<String, String>) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert(
        "method".to_string(),
        serde_json::Value::String("GET".to_string()),
    );
    obj.insert("url".to_string(), serde_json::Value::String(String::new()));
    obj.insert("params".to_string(), serde_json::Value::Array(vec![]));
    obj.insert("headers".to_string(), serde_json::Value::Array(vec![]));
    obj.insert("body".to_string(), serde_json::Value::String(String::new()));
    if let Some(timeout_str) = attrs.get("timeout") {
        if let Ok(timeout) = timeout_str.parse::<u64>() {
            obj.insert(
                "timeout_ms".to_string(),
                serde_json::Value::Number(timeout.into()),
            );
        }
    }
    serde_json::Value::Object(obj)
}

type HttpRequestLine = (String, String, Vec<(String, String)>);

/// Returns `(method, url, inline_query_params)` or `None` if the line is not a
/// valid request line.
fn parse_http_first_line(line: &str) -> Option<HttpRequestLine> {
    let mut parts = line.splitn(2, char::is_whitespace);
    let method = parts.next()?.to_string();
    let rest = parts.next()?.trim().to_string();
    if !is_known_http_method(&method) {
        return None;
    }
    let mut params = Vec::new();
    let (url, inline_query) = match rest.find('?') {
        Some(idx) => (rest[..idx].to_string(), Some(rest[idx + 1..].to_string())),
        None => (rest, None),
    };
    if let Some(q) = inline_query {
        for seg in q.split('&') {
            if seg.is_empty() {
                continue;
            }
            if let Some(kv) = split_query_segment(seg) {
                params.push(kv);
            }
        }
    }
    Some((method, url, params))
}

fn is_known_http_method(method: &str) -> bool {
    matches!(
        method,
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS"
    )
}

fn split_query_segment(seg: &str) -> Option<(String, String)> {
    if seg.is_empty() {
        return None;
    }
    match seg.find('=') {
        Some(idx) => {
            let key = seg[..idx].to_string();
            if key.is_empty() {
                return None;
            }
            Some((key, seg[idx + 1..].to_string()))
        }
        None => Some((seg.to_string(), String::new())),
    }
}

fn split_header_line(line: &str) -> Option<(String, String)> {
    let idx = line.find(':')?;
    if idx == 0 {
        return None;
    }
    let key = line[..idx].trim().to_string();
    let value = line[idx + 1..].trim().to_string();
    if key.is_empty() {
        return None;
    }
    Some((key, value))
}

/// Find a block by alias in a list of parsed blocks.
pub fn find_block_by_alias<'a>(blocks: &'a [ParsedBlock], alias: &str) -> Option<&'a ParsedBlock> {
    blocks.iter().find(|b| b.alias.as_deref() == Some(alias))
}

/// Find all blocks above a given line position (for dependency resolution).
pub fn blocks_above(blocks: &[ParsedBlock], line: usize) -> Vec<&ParsedBlock> {
    blocks.iter().filter(|b| b.line_start < line).collect()
}

/// Strip the opening fence and return the info string, if any.
/// Supports ``` and ~~~.
fn strip_fence_open(line: &str) -> Option<&str> {
    if (line.starts_with("```") || line.starts_with("~~~")) && line.len() > 3 {
        Some(line[3..].trim())
    } else {
        None
    }
}

/// Parse info string into block_type and key=value attributes.
/// Example: "http alias=login displayMode=split" -> ("http", {"alias": "login", "displayMode": "split"})
fn parse_info_string(info: &str) -> (String, HashMap<String, String>) {
    let mut parts = info.split_whitespace();
    let block_type = parts.next().unwrap_or("").to_string();
    let mut attrs = HashMap::new();

    for part in parts {
        if let Some((key, value)) = part.split_once('=') {
            attrs.insert(key.to_string(), value.to_string());
        }
    }

    (block_type, attrs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_disabled_header_as_enabled_false() {
        let md =
            "```http alias=req\nGET https://api.test.com/x\nAccept: application/json\n# X-Debug: on\n```\n";
        let blocks = parse_blocks(md);
        let headers = blocks[0].params["headers"].as_array().unwrap();
        assert_eq!(headers.len(), 2);
        assert_eq!(headers[0]["key"], "Accept");
        assert!(
            headers[0].get("enabled").is_none(),
            "enabled rows omit the flag"
        );
        assert_eq!(headers[1]["key"], "X-Debug");
        assert_eq!(headers[1]["value"], "on");
        assert_eq!(headers[1]["enabled"], false);
    }

    #[test]
    fn drops_descriptions_and_freeform_comments() {
        let md = "```http\nGET https://api.test.com/x\n# desc: human note\n#freeform\nAccept: text/plain\n```\n";
        let blocks = parse_blocks(md);
        let headers = blocks[0].params["headers"].as_array().unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers[0]["key"], "Accept");
    }

    #[test]
    fn parses_disabled_query_param() {
        let md = "```http\nGET https://api.test.com/search\n?q=hello\n# &page=2\n```\n";
        let blocks = parse_blocks(md);
        let params = blocks[0].params["params"].as_array().unwrap();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0]["key"], "q");
        assert!(params[0].get("enabled").is_none());
        assert_eq!(params[1]["key"], "page");
        assert_eq!(params[1]["value"], "2");
        assert_eq!(params[1]["enabled"], false);
    }

    #[test]
    fn test_parse_http_block() {
        let md = r#"# My API Doc

Some text here.

```http alias=login displayMode=split
{"method":"POST","url":"https://api.test.com/login","params":[],"headers":[],"body":"{\"user\":\"admin\"}"}
```

More text.
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, "http");
        assert_eq!(blocks[0].alias.as_deref(), Some("login"));
        assert_eq!(blocks[0].display_mode.as_deref(), Some("split"));
        assert_eq!(blocks[0].params["method"], "POST");
        assert_eq!(blocks[0].params["url"], "https://api.test.com/login");
        assert_eq!(blocks[0].line_start, 4);
        assert_eq!(blocks[0].line_end, 6);
    }

    #[test]
    fn test_parse_db_block() {
        let md = r#"```db alias=users
{"connection_id":"abc-123","query":"SELECT * FROM users","bind_values":[]}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, "db");
        assert_eq!(blocks[0].alias.as_deref(), Some("users"));
        assert_eq!(blocks[0].params["query"], "SELECT * FROM users");
    }

    #[test]
    fn test_parse_multiple_blocks() {
        let md = r#"# API

```http alias=auth
{"method":"POST","url":"https://api.test.com/login","params":[],"headers":[],"body":""}
```

Some text between blocks.

```http alias=users
{"method":"GET","url":"https://api.test.com/users","params":[],"headers":[],"body":""}
```

```db alias=query
{"connection_id":"1","query":"SELECT 1","bind_values":[]}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].alias.as_deref(), Some("auth"));
        assert_eq!(blocks[1].alias.as_deref(), Some("users"));
        assert_eq!(blocks[2].alias.as_deref(), Some("query"));
    }

    #[test]
    fn test_ignores_non_executable_blocks() {
        let md = r#"```javascript
console.log("hello");
```

```http alias=req
{"method":"GET","url":"https://example.com","params":[],"headers":[],"body":""}
```

```python
print("world")
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, "http");
    }

    #[test]
    fn test_find_block_by_alias() {
        let md = r#"```http alias=first
{"method":"GET","url":"https://a.com","params":[],"headers":[],"body":""}
```

```http alias=second
{"method":"GET","url":"https://b.com","params":[],"headers":[],"body":""}
```
"#;
        let blocks = parse_blocks(md);
        let found = find_block_by_alias(&blocks, "second");
        assert!(found.is_some());
        assert_eq!(found.unwrap().params["url"], "https://b.com");

        let missing = find_block_by_alias(&blocks, "nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_blocks_above() {
        let md = r#"```http alias=a
{"method":"GET","url":"https://a.com","params":[],"headers":[],"body":""}
```

```http alias=b
{"method":"GET","url":"https://b.com","params":[],"headers":[],"body":""}
```

```http alias=c
{"method":"GET","url":"https://c.com","params":[],"headers":[],"body":""}
```
"#;
        let blocks = parse_blocks(md);
        let above = blocks_above(&blocks, blocks[2].line_start);
        assert_eq!(above.len(), 2);
        assert_eq!(above[0].alias.as_deref(), Some("a"));
        assert_eq!(above[1].alias.as_deref(), Some("b"));
    }

    #[test]
    fn test_block_without_alias() {
        let md = r#"```http
{"method":"GET","url":"https://example.com","params":[],"headers":[],"body":""}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].alias.is_none());
    }

    #[test]
    fn test_invalid_json_body() {
        // Post-redesign: a non-JSON, non-HTTP-message body falls back to an
        // empty HTTP shape (executor receives a well-formed shape it can
        // gracefully fail on).
        let md = r#"```http alias=broken
not valid json
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].params["method"], "GET");
        assert_eq!(blocks[0].params["url"], "");
    }

    #[test]
    fn test_db_variant_types() {
        let md = r#"```db-postgres alias=pg
{"connection_id":"1","query":"SELECT 1"}
```

```db-mysql alias=my
{"connection_id":"2","query":"SELECT 1"}
```

```db-sqlite alias=sl
{"connection_id":"3","query":"SELECT 1"}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].block_type, "db-postgres");
        assert_eq!(blocks[1].block_type, "db-mysql");
        assert_eq!(blocks[2].block_type, "db-sqlite");
    }

    #[test]
    fn test_empty_markdown() {
        let blocks = parse_blocks("");
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_markdown_with_no_blocks() {
        let md = "# Title\n\nSome regular markdown text.\n\n- Item 1\n- Item 2\n";
        let blocks = parse_blocks(md);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_parse_db_new_format_raw_sql() {
        let md = r#"```db-postgres alias=db1 connection=prod limit=100 timeout=30000 display=split
SELECT *
FROM users
WHERE id > 10
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, "db-postgres");
        assert_eq!(blocks[0].alias.as_deref(), Some("db1"));
        assert_eq!(blocks[0].display_mode.as_deref(), Some("split"));
        assert_eq!(
            blocks[0].params["query"],
            "SELECT *\nFROM users\nWHERE id > 10"
        );
        assert_eq!(blocks[0].params["connection_id"], "prod");
        assert_eq!(blocks[0].params["limit"], 100);
        assert_eq!(blocks[0].params["timeout_ms"], 30000);
        assert!(blocks[0].params.get("session").is_none());
    }

    #[test]
    fn test_parse_db_new_format_minimal() {
        let md = "```db-mysql\nSELECT 1\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].params["query"], "SELECT 1");
        // Only query is present; other fields absent.
        assert!(blocks[0].params.get("connection_id").is_none());
        assert!(blocks[0].params.get("limit").is_none());
        assert!(blocks[0].params.get("timeout_ms").is_none());
    }

    #[test]
    fn test_parse_db_new_format_preserves_blank_lines() {
        let md = "```db-postgres alias=x\nSELECT 1;\n\nSELECT 2;\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].params["query"], "SELECT 1;\n\nSELECT 2;");
    }

    #[test]
    fn test_parse_db_new_format_display_wins_over_legacy_key() {
        let md = "```db-postgres alias=x displayMode=input display=output\nSELECT 1\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].display_mode.as_deref(), Some("output"));
    }

    #[test]
    fn test_parse_db_new_format_accepts_legacy_display_mode_alone() {
        let md = "```db-postgres alias=x displayMode=input\nSELECT 1\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].display_mode.as_deref(), Some("input"));
    }

    #[test]
    fn test_parse_db_new_format_ignores_invalid_limit() {
        let md = "```db-postgres limit=abc timeout=-5\nSELECT 1\n```\n";
        let blocks = parse_blocks(md);
        assert!(blocks[0].params.get("limit").is_none());
        assert!(blocks[0].params.get("timeout_ms").is_none());
    }

    #[test]
    fn test_parse_db_legacy_format_still_works() {
        let md = r#"```db-postgres alias=users displayMode=split
{"connection_id":"abc-uuid","query":"SELECT * FROM users","timeout_ms":5000}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, "db-postgres");
        assert_eq!(blocks[0].alias.as_deref(), Some("users"));
        assert_eq!(blocks[0].display_mode.as_deref(), Some("split"));
        assert_eq!(blocks[0].params["query"], "SELECT * FROM users");
        assert_eq!(blocks[0].params["connection_id"], "abc-uuid");
        assert_eq!(blocks[0].params["timeout_ms"], 5000);
    }

    #[test]
    fn test_parse_db_legacy_format_preserves_json_shape() {
        // Legacy bodies may carry extra fields (bind_values, offset, fetch_size);
        // the parser must pass them through untouched.
        let md = r#"```db alias=users
{"connection_id":"x","query":"SELECT 1","bind_values":[1,2],"offset":10,"fetch_size":50}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["bind_values"], serde_json::json!([1, 2]));
        assert_eq!(blocks[0].params["offset"], 10);
        assert_eq!(blocks[0].params["fetch_size"], 50);
    }

    #[test]
    fn test_parse_db_json_without_query_field_is_treated_as_sql() {
        // Body is JSON-looking but has no `query` string → treated as raw SQL.
        let md = "```db-postgres\n{\"foo\":\"bar\"}\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["query"], "{\"foo\":\"bar\"}");
    }

    #[test]
    fn test_parse_db_sql_starting_with_brace_comment_is_treated_as_sql() {
        // A SQL body whose first non-whitespace char is `{` but which is not
        // valid JSON falls back to raw SQL.
        let md = "```db-postgres\n{ not json\nSELECT 1\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["query"], "{ not json\nSELECT 1");
    }

    #[test]
    fn test_parse_db_sql_with_leading_comment_is_raw_sql() {
        let md = "```db-postgres\n-- a comment\nSELECT 1\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["query"], "-- a comment\nSELECT 1");
    }

    #[test]
    fn test_parse_db_generic_dialect_still_works() {
        // `db` without dialect suffix should behave the same as db-*.
        let md = "```db alias=x\nSELECT 1\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].block_type, "db");
        assert_eq!(blocks[0].params["query"], "SELECT 1");
    }

    #[test]
    fn test_http_block_body_still_parsed_as_json() {
        let md = r#"```http alias=x
{"method":"GET","url":"https://api.test.com","params":[],"headers":[],"body":""}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].block_type, "http");
        assert_eq!(blocks[0].params["method"], "GET");
    }

    #[test]
    fn test_http_block_ignores_db_heuristic() {
        // If an http body looks db-like, the http parser falls back to an
        // empty HTTP shape (it never synthesizes a SQL query).
        let md = "```http alias=x\nSELECT 1\n```\n";
        let blocks = parse_blocks(md);
        assert!(blocks[0].params.get("query").is_none());
        assert_eq!(blocks[0].params["url"], "");
    }

    #[test]
    fn test_parse_http_new_format_get_simple() {
        let md = "```http alias=req1\nGET https://api.example.com/users\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].block_type, "http");
        assert_eq!(blocks[0].alias.as_deref(), Some("req1"));
        assert_eq!(blocks[0].params["method"], "GET");
        assert_eq!(blocks[0].params["url"], "https://api.example.com/users");
        assert_eq!(blocks[0].params["params"], serde_json::json!([]));
        assert_eq!(blocks[0].params["headers"], serde_json::json!([]));
        assert_eq!(blocks[0].params["body"], "");
    }

    #[test]
    fn test_parse_http_new_format_inline_query() {
        let md = "```http\nGET https://api.example.com/users?page=1&limit=10\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["url"], "https://api.example.com/users");
        assert_eq!(
            blocks[0].params["params"],
            serde_json::json!([
                {"key": "page", "value": "1"},
                {"key": "limit", "value": "10"},
            ])
        );
    }

    #[test]
    fn test_parse_http_new_format_query_continuation() {
        let md = "```http\nGET https://api.example.com/users\n?page=1\n&limit=10\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(
            blocks[0].params["params"],
            serde_json::json!([
                {"key": "page", "value": "1"},
                {"key": "limit", "value": "10"},
            ])
        );
    }

    #[test]
    fn test_parse_http_new_format_with_headers_and_body() {
        let md = r#"```http alias=createUser
POST https://api.example.com/users
Authorization: Bearer xyz
Content-Type: application/json

{"name":"alice"}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["method"], "POST");
        assert_eq!(blocks[0].params["url"], "https://api.example.com/users");
        assert_eq!(
            blocks[0].params["headers"],
            serde_json::json!([
                {"key": "Authorization", "value": "Bearer xyz"},
                {"key": "Content-Type", "value": "application/json"},
            ])
        );
        assert_eq!(blocks[0].params["body"], "{\"name\":\"alice\"}");
    }

    #[test]
    fn test_parse_http_new_format_preserves_disabled_rows() {
        let md = r#"```http
GET https://example.com
?page=1
# &cursor=abc
Authorization: Bearer x
# X-Debug: 1
```
"#;
        let blocks = parse_blocks(md);
        // Disabled rows survive as `enabled: false` so the editor can show them
        // unchecked; the executor skips them at dispatch. Enabled rows omit the
        // flag, keeping the canonical shape unchanged.
        assert_eq!(
            blocks[0].params["params"],
            serde_json::json!([
                {"key": "page", "value": "1"},
                {"key": "cursor", "value": "abc", "enabled": false},
            ])
        );
        assert_eq!(
            blocks[0].params["headers"],
            serde_json::json!([
                {"key": "Authorization", "value": "Bearer x"},
                {"key": "X-Debug", "value": "1", "enabled": false},
            ])
        );
    }

    #[test]
    fn test_parse_http_new_format_descriptions_stripped() {
        let md = r#"```http
GET https://example.com
# desc: page index
?page=1
# desc: bearer
Authorization: Bearer x
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(
            blocks[0].params["params"],
            serde_json::json!([{"key": "page", "value": "1"}])
        );
        assert_eq!(
            blocks[0].params["headers"],
            serde_json::json!([{"key": "Authorization", "value": "Bearer x"}])
        );
    }

    #[test]
    fn test_parse_http_new_format_timeout_from_info_string() {
        let md = "```http alias=x timeout=5000\nGET https://example.com\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["timeout_ms"], 5000);
    }

    #[test]
    fn test_parse_http_new_format_preserves_body_blank_lines() {
        let md =
            "```http\nPOST https://example.com\nContent-Type: text/plain\n\nline1\n\nline3\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["body"], "line1\n\nline3");
    }

    #[test]
    fn test_parse_http_new_format_unknown_method_falls_back() {
        let md = "```http\nFETCH https://example.com\n```\n";
        let blocks = parse_blocks(md);
        // FETCH is not a known method, so first-line parse fails and we get
        // an empty shape.
        assert_eq!(blocks[0].params["method"], "GET");
        assert_eq!(blocks[0].params["url"], "");
    }

    #[test]
    fn test_parse_http_legacy_format_still_works() {
        let md = r#"```http alias=login displayMode=split
{"method":"POST","url":"https://api.test.com/login","params":[],"headers":[],"body":"{\"user\":\"admin\"}"}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["method"], "POST");
        assert_eq!(blocks[0].params["url"], "https://api.test.com/login");
        assert_eq!(blocks[0].params["body"], "{\"user\":\"admin\"}");
    }

    #[test]
    fn test_parse_http_legacy_format_preserves_extra_fields() {
        // Legacy bodies may carry extra fields; the parser passes them through.
        let md = r#"```http
{"method":"GET","url":"https://example.com","params":[{"key":"a","value":"1"}],"headers":[],"body":"","timeout_ms":5000}
```
"#;
        let blocks = parse_blocks(md);
        assert_eq!(
            blocks[0].params["params"],
            serde_json::json!([{"key": "a", "value": "1"}])
        );
        assert_eq!(blocks[0].params["timeout_ms"], 5000);
    }

    #[test]
    fn test_parse_http_json_without_method_is_treated_as_message() {
        // A JSON-looking body that lacks `method`/`url` is not legacy; it
        // tries the message parser, which fails, and we get the empty shape.
        let md = "```http\n{\"foo\":\"bar\"}\n```\n";
        let blocks = parse_blocks(md);
        assert_eq!(blocks[0].params["url"], "");
    }
}
