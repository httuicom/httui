use std::collections::HashMap;

/// A resolved reference with its replacement value.
#[derive(Debug, Clone)]
pub struct ResolvedRef {
    pub placeholder: String,
    pub value: String,
}

/// Extract all `{{...}}` placeholders from a JSON value (recursively scanning all string values).
pub fn extract_placeholders(value: &serde_json::Value) -> Vec<String> {
    let mut placeholders = Vec::new();
    collect_placeholders(value, &mut placeholders);
    placeholders.sort();
    placeholders.dedup();
    placeholders
}

fn collect_placeholders(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            extract_from_string(s, out);
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_placeholders(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_placeholders(v, out);
            }
        }
        _ => {}
    }
}

fn extract_from_string(s: &str, out: &mut Vec<String>) {
    let mut start = 0;
    while let Some(open) = s[start..].find("{{") {
        let abs_open = start + open + 2;
        if let Some(close) = s[abs_open..].find("}}") {
            let inner = &s[abs_open..abs_open + close];
            if !inner.is_empty() {
                out.push(inner.to_string());
            }
            start = abs_open + close + 2;
        } else {
            break;
        }
    }
}

/// Determine if a placeholder is a block reference (has dots) or an env variable (no dots).
/// Block references: `alias.response.field` (contains at least one dot)
/// Env variables: `ENV_KEY` (no dots)
pub fn is_block_reference(placeholder: &str) -> bool {
    placeholder.contains('.')
}

/// Extract the alias from a block reference placeholder.
/// `login.response.token` -> `login`
pub fn extract_alias(placeholder: &str) -> Option<&str> {
    placeholder.split('.').next()
}

/// Navigate a JSON value by a dot-separated path.
/// `response.data.0.id` navigates into nested objects and arrays.
/// Property names blocked for prototype pollution defense.
const DANGEROUS_KEYS: &[&str] = &["__proto__", "constructor", "prototype"];

pub fn navigate_json(value: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = value.clone();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if DANGEROUS_KEYS.contains(&part) {
            return None;
        }
        if let Ok(idx) = part.parse::<usize>() {
            current = current.get(idx)?.clone();
        } else {
            current = current.get(part)?.clone();
        }
    }
    Some(current)
}

/// Resolve a block reference against a map of already-executed block results.
/// `login.response.token` -> look up `login` result, navigate `response.token`.
pub fn resolve_block_ref(
    placeholder: &str,
    results: &HashMap<String, serde_json::Value>,
) -> Option<String> {
    let alias = extract_alias(placeholder)?;
    let rest = &placeholder[alias.len()..].trim_start_matches('.');
    let result = results.get(alias)?;

    let value = if rest.is_empty() {
        result.clone()
    } else {
        navigate_json(result, rest)?
    };

    Some(value_to_string(&value))
}

/// Resolve all `{{...}}` placeholders in a JSON value.
/// Returns a new JSON value with all placeholders replaced.
pub fn resolve_all(
    params: &serde_json::Value,
    block_results: &HashMap<String, serde_json::Value>,
    env_vars: &HashMap<String, String>,
) -> serde_json::Value {
    resolve_value(params, block_results, env_vars)
}

fn resolve_value(
    value: &serde_json::Value,
    block_results: &HashMap<String, serde_json::Value>,
    env_vars: &HashMap<String, String>,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            let resolved = resolve_string(s, block_results, env_vars);
            serde_json::Value::String(resolved)
        }
        serde_json::Value::Array(arr) => {
            let resolved: Vec<serde_json::Value> = arr
                .iter()
                .map(|v| resolve_value(v, block_results, env_vars))
                .collect();
            serde_json::Value::Array(resolved)
        }
        serde_json::Value::Object(map) => {
            let resolved: serde_json::Map<String, serde_json::Value> = map
                .iter()
                .map(|(k, v)| (k.clone(), resolve_value(v, block_results, env_vars)))
                .collect();
            serde_json::Value::Object(resolved)
        }
        other => other.clone(),
    }
}

fn resolve_string(
    s: &str,
    block_results: &HashMap<String, serde_json::Value>,
    env_vars: &HashMap<String, String>,
) -> String {
    let mut result = s.to_string();
    let mut placeholders = Vec::new();
    extract_from_string(s, &mut placeholders);

    for placeholder in placeholders {
        let replacement = if is_block_reference(&placeholder) {
            resolve_block_ref(&placeholder, block_results)
        } else {
            env_vars.get(&placeholder).cloned()
        };

        if let Some(value) = replacement {
            let pattern = format!("{{{{{}}}}}", placeholder);
            result = result.replace(&pattern, &value);
        }
    }

    result
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_placeholders() {
        let value = serde_json::json!({
            "url": "https://api.com/users/{{login.response.id}}",
            "headers": [
                {"key": "Authorization", "value": "Bearer {{login.response.token}}"},
                {"key": "X-Env", "value": "{{BASE_URL}}"}
            ],
            "body": "no placeholders here"
        });

        let placeholders = extract_placeholders(&value);
        assert_eq!(placeholders.len(), 3);
        assert!(placeholders.contains(&"BASE_URL".to_string()));
        assert!(placeholders.contains(&"login.response.id".to_string()));
        assert!(placeholders.contains(&"login.response.token".to_string()));
    }

    #[test]
    fn test_is_block_reference() {
        assert!(is_block_reference("login.response.token"));
        assert!(is_block_reference("auth.data.id"));
        assert!(!is_block_reference("BASE_URL"));
        assert!(!is_block_reference("API_KEY"));
    }

    #[test]
    fn test_extract_alias() {
        assert_eq!(extract_alias("login.response.token"), Some("login"));
        assert_eq!(extract_alias("auth.data.0.id"), Some("auth"));
        assert_eq!(extract_alias("BASE_URL"), Some("BASE_URL"));
    }

    #[test]
    fn test_navigate_json() {
        let value = serde_json::json!({
            "response": {
                "data": [
                    {"id": 1, "name": "Alice"},
                    {"id": 2, "name": "Bob"}
                ],
                "token": "abc123"
            }
        });

        assert_eq!(
            navigate_json(&value, "response.token"),
            Some(serde_json::json!("abc123"))
        );
        assert_eq!(
            navigate_json(&value, "response.data.0.name"),
            Some(serde_json::json!("Alice"))
        );
        assert_eq!(
            navigate_json(&value, "response.data.1.id"),
            Some(serde_json::json!(2))
        );
        assert_eq!(navigate_json(&value, "response.missing"), None);
    }

    #[test]
    fn test_resolve_block_ref() {
        let mut results = HashMap::new();
        results.insert(
            "login".to_string(),
            serde_json::json!({
                "response": {
                    "token": "jwt-123",
                    "user": {"id": 42}
                }
            }),
        );

        assert_eq!(
            resolve_block_ref("login.response.token", &results),
            Some("jwt-123".to_string())
        );
        assert_eq!(
            resolve_block_ref("login.response.user.id", &results),
            Some("42".to_string())
        );
        assert_eq!(resolve_block_ref("missing.response.token", &results), None);
    }

    #[test]
    fn test_resolve_all() {
        let params = serde_json::json!({
            "url": "{{BASE_URL}}/users/{{login.response.id}}",
            "headers": [
                {"key": "Authorization", "value": "Bearer {{login.response.token}}"}
            ]
        });

        let mut block_results = HashMap::new();
        block_results.insert(
            "login".to_string(),
            serde_json::json!({
                "response": {"id": 42, "token": "jwt-abc"}
            }),
        );

        let mut env_vars = HashMap::new();
        env_vars.insert(
            "BASE_URL".to_string(),
            "https://api.example.com".to_string(),
        );

        let resolved = resolve_all(&params, &block_results, &env_vars);

        assert_eq!(resolved["url"], "https://api.example.com/users/42");
        assert_eq!(resolved["headers"][0]["value"], "Bearer jwt-abc");
    }

    #[test]
    fn test_resolve_with_no_placeholders() {
        let params = serde_json::json!({"url": "https://example.com", "method": "GET"});
        let resolved = resolve_all(&params, &HashMap::new(), &HashMap::new());
        assert_eq!(resolved, params);
    }

    #[test]
    fn test_resolve_unresolved_placeholder_stays() {
        let params = serde_json::json!({"url": "{{UNKNOWN}}/api"});
        let resolved = resolve_all(&params, &HashMap::new(), &HashMap::new());
        assert_eq!(resolved["url"], "{{UNKNOWN}}/api");
    }

    #[test]
    fn navigate_json_blocks_prototype_pollution_keys() {
        // Even if the response payload literally contains a key called
        // `__proto__` (or `constructor` / `prototype`), references that
        // try to navigate into it must return None — never the value.
        // Mitigates a vault that authors `{{login.response.__proto__}}`
        // assuming Rust JSON behaves like JS prototype-walked objects.
        let value = serde_json::json!({
            "response": {
                "__proto__": "should-not-be-readable",
                "constructor": "ditto",
                "prototype": "ditto",
                "safe_key": "ok",
            }
        });

        assert_eq!(
            navigate_json(&value, "response.__proto__"),
            None,
            "__proto__ must be blocked even when present in payload",
        );
        assert_eq!(navigate_json(&value, "response.constructor"), None);
        assert_eq!(navigate_json(&value, "response.prototype"), None);
        // Sanity: regular keys still resolve.
        assert_eq!(
            navigate_json(&value, "response.safe_key"),
            Some(serde_json::json!("ok")),
        );
    }

    #[test]
    fn navigate_json_blocks_prototype_pollution_mid_path() {
        // Mid-path: the dangerous key is not the leaf.
        let value = serde_json::json!({
            "__proto__": { "leaked": "value" }
        });
        assert_eq!(navigate_json(&value, "__proto__.leaked"), None);
    }
}
