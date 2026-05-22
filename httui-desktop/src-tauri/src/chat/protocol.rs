use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_to_bytes(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

/// Compute HMAC-SHA256 of a message payload.
pub fn compute_hmac(secret: &str, payload: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    bytes_to_hex(&mac.finalize().into_bytes())
}

/// Verify HMAC-SHA256 of a message payload (constant-time comparison).
pub fn verify_hmac(secret: &str, payload: &str, expected_hmac: &str) -> bool {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    let expected_bytes = hex_to_bytes(expected_hmac).unwrap_or_default();
    mac.verify_slice(&expected_bytes).is_ok()
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutgoingMessage {
    Chat {
        request_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        claude_session_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        allowed_tools: Vec<String>,
        content: Vec<serde_json::Value>,
    },
    PermissionResponse {
        permission_id: String,
        decision: PermissionDecision,
    },
    Abort {
        request_id: String,
    },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub behavior: PermissionBehavior,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionBehavior {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IncomingMessage {
    Session {
        request_id: String,
        claude_session_id: String,
    },
    TextDelta {
        request_id: String,
        text: String,
    },
    ToolUse {
        request_id: String,
        tool_use_id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        request_id: String,
        tool_use_id: String,
        content: Vec<serde_json::Value>,
        is_error: bool,
    },
    PermissionRequest {
        request_id: String,
        permission_id: String,
        tool_name: String,
        tool_input: serde_json::Value,
    },
    Done {
        request_id: String,
        usage: Option<UsageInfo>,
        stop_reason: Option<String>,
    },
    Error {
        request_id: String,
        category: String,
        message: String,
    },
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageInfo {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatDeltaEvent {
    pub session_id: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatToolUseEvent {
    pub session_id: i64,
    pub tool_use_id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatToolResultEvent {
    pub session_id: i64,
    pub tool_use_id: String,
    pub content: Vec<serde_json::Value>,
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatPermissionRequestEvent {
    pub session_id: i64,
    pub permission_id: String,
    pub tool_name: String,
    pub tool_input: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatDoneEvent {
    pub session_id: i64,
    pub usage: Option<UsageInfo>,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatErrorEvent {
    pub session_id: i64,
    pub category: String,
    pub message: String,
}

impl OutgoingMessage {
    /// Serialize to a single NDJSON line (no trailing newline).
    pub fn to_ndjson(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

impl IncomingMessage {
    /// Parse a single NDJSON line.
    pub fn from_ndjson(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }

    /// Extract the request_id if present.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Session { request_id, .. } => Some(request_id),
            Self::TextDelta { request_id, .. } => Some(request_id),
            Self::ToolUse { request_id, .. } => Some(request_id),
            Self::ToolResult { request_id, .. } => Some(request_id),
            Self::PermissionRequest { request_id, .. } => Some(request_id),
            Self::Done { request_id, .. } => Some(request_id),
            Self::Error { request_id, .. } => Some(request_id),
            Self::Pong => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outgoing_chat_serialization() {
        let msg = OutgoingMessage::Chat {
            request_id: "req-123".to_string(),
            claude_session_id: None,
            cwd: Some("/Users/me/project".to_string()),
            allowed_tools: vec!["Read".to_string(), "Grep".to_string()],
            content: vec![serde_json::json!({"type": "text", "text": "hello"})],
        };
        let json = msg.to_ndjson().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "chat");
        assert_eq!(parsed["request_id"], "req-123");
        assert!(parsed.get("claude_session_id").is_none());
        assert_eq!(parsed["cwd"], "/Users/me/project");
    }

    #[test]
    fn test_outgoing_permission_response_serialization() {
        let msg = OutgoingMessage::PermissionResponse {
            permission_id: "perm-abc".to_string(),
            decision: PermissionDecision {
                behavior: PermissionBehavior::Allow,
                message: None,
            },
        };
        let json = msg.to_ndjson().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "permission_response");
        assert_eq!(parsed["decision"]["behavior"], "allow");
    }

    #[test]
    fn test_outgoing_abort_serialization() {
        let msg = OutgoingMessage::Abort {
            request_id: "req-456".to_string(),
        };
        let json = msg.to_ndjson().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["type"], "abort");
    }

    #[test]
    fn test_outgoing_ping_serialization() {
        let msg = OutgoingMessage::Ping;
        let json = msg.to_ndjson().unwrap();
        assert_eq!(json, r#"{"type":"ping"}"#);
    }

    #[test]
    fn test_incoming_session_deserialization() {
        let json = r#"{"type":"session","request_id":"req-1","claude_session_id":"sess_xyz"}"#;
        let msg = IncomingMessage::from_ndjson(json).unwrap();
        match msg {
            IncomingMessage::Session {
                request_id,
                claude_session_id,
            } => {
                assert_eq!(request_id, "req-1");
                assert_eq!(claude_session_id, "sess_xyz");
            }
            _ => panic!("Expected Session variant"),
        }
    }

    #[test]
    fn test_incoming_text_delta_deserialization() {
        let json = r#"{"type":"text_delta","request_id":"req-1","text":"Hello "}"#;
        let msg = IncomingMessage::from_ndjson(json).unwrap();
        match msg {
            IncomingMessage::TextDelta { text, .. } => assert_eq!(text, "Hello "),
            _ => panic!("Expected TextDelta variant"),
        }
    }

    #[test]
    fn test_incoming_tool_use_deserialization() {
        let json = r#"{"type":"tool_use","request_id":"req-1","tool_use_id":"toolu_01","name":"Read","input":{"file_path":"/tmp/foo"}}"#;
        let msg = IncomingMessage::from_ndjson(json).unwrap();
        match msg {
            IncomingMessage::ToolUse { name, input, .. } => {
                assert_eq!(name, "Read");
                assert_eq!(input["file_path"], "/tmp/foo");
            }
            _ => panic!("Expected ToolUse variant"),
        }
    }

    #[test]
    fn test_incoming_permission_request_deserialization() {
        let json = r#"{"type":"permission_request","request_id":"req-1","permission_id":"perm_abc","tool_name":"Bash","tool_input":{"command":"ls"}}"#;
        let msg = IncomingMessage::from_ndjson(json).unwrap();
        match msg {
            IncomingMessage::PermissionRequest {
                tool_name,
                tool_input,
                ..
            } => {
                assert_eq!(tool_name, "Bash");
                assert_eq!(tool_input["command"], "ls");
            }
            _ => panic!("Expected PermissionRequest variant"),
        }
    }

    #[test]
    fn test_incoming_done_deserialization() {
        let json = r#"{"type":"done","request_id":"req-1","usage":{"input_tokens":100,"output_tokens":50,"cache_read_tokens":80},"stop_reason":"end_turn"}"#;
        let msg = IncomingMessage::from_ndjson(json).unwrap();
        match msg {
            IncomingMessage::Done {
                usage, stop_reason, ..
            } => {
                let u = usage.unwrap();
                assert_eq!(u.input_tokens, 100);
                assert_eq!(u.output_tokens, 50);
                assert_eq!(u.cache_read_tokens, 80);
                assert_eq!(stop_reason.unwrap(), "end_turn");
            }
            _ => panic!("Expected Done variant"),
        }
    }

    #[test]
    fn test_incoming_error_deserialization() {
        let json =
            r#"{"type":"error","request_id":"req-1","category":"auth","message":"Not logged in"}"#;
        let msg = IncomingMessage::from_ndjson(json).unwrap();
        match msg {
            IncomingMessage::Error {
                category, message, ..
            } => {
                assert_eq!(category, "auth");
                assert_eq!(message, "Not logged in");
            }
            _ => panic!("Expected Error variant"),
        }
    }

    #[test]
    fn test_incoming_pong_deserialization() {
        let json = r#"{"type":"pong"}"#;
        let msg = IncomingMessage::from_ndjson(json).unwrap();
        assert!(matches!(msg, IncomingMessage::Pong));
    }

    #[test]
    fn test_request_id_extraction() {
        let delta = IncomingMessage::TextDelta {
            request_id: "req-42".to_string(),
            text: "hi".to_string(),
        };
        assert_eq!(delta.request_id(), Some("req-42"));

        let pong = IncomingMessage::Pong;
        assert_eq!(pong.request_id(), None);
    }
}
