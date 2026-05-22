use sqlx::sqlite::SqlitePool;
use std::path::Path;

use httui_core::db::chat;

/// Result of the permission broker's check.
#[derive(Debug, Clone)]
pub enum PermissionVerdict {
    /// Tool use is allowed automatically (no UI prompt needed).
    Allow,
    /// Tool use is denied automatically (no UI prompt needed).
    Deny(String),
    /// Must ask the user via UI.
    AskUser,
}

pub struct PermissionBroker {
    pool: SqlitePool,
}

impl PermissionBroker {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Check whether a tool use should be auto-allowed, auto-denied, or prompted to the user.
    ///
    /// Cascading logic:
    /// 1. Hardcoded: Bash → always AskUser
    /// 2. Hardcoded: Edit/Write outside cwd → Deny
    /// 3. Hardcoded: Read/Glob/Grep inside cwd → Allow
    /// 4. DB: persisted rule (scope=always) → Allow/Deny
    /// 5. DB: session rule (scope=session) → Allow/Deny
    /// 6. Fallback: AskUser
    pub async fn check(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
        session_id: i64,
        cwd: Option<&str>,
    ) -> PermissionVerdict {
        // 1. Bash is never auto-approved
        if tool_name == "Bash" {
            return PermissionVerdict::AskUser;
        }

        // execute_block (DB/HTTP) always requires user confirmation
        if tool_name == "execute_block" {
            return PermissionVerdict::AskUser;
        }

        let input_path = extract_path(tool_name, tool_input);

        if let Some(cwd) = cwd {
            // 2. Edit/Write outside cwd → hard deny
            if tool_name == "Edit" || tool_name == "Write" {
                if let Some(ref p) = input_path {
                    if !is_within(p, cwd) {
                        return PermissionVerdict::Deny(format!(
                            "Cannot {tool_name} outside working directory: {p}"
                        ));
                    }
                }
            }

            // 3. Read/Glob/Grep inside cwd → auto-allow
            if tool_name == "Read" || tool_name == "Glob" || tool_name == "Grep" {
                if let Some(ref p) = input_path {
                    if is_within(p, cwd) {
                        return PermissionVerdict::Allow;
                    }
                }
            }
        }

        // 4-5. Check DB rules (persisted 'always' then 'session')
        let workspace = cwd.map(|s| s.to_string());
        if let Ok(Some(rule)) =
            chat::check_permission(&self.pool, tool_name, workspace.as_deref(), session_id).await
        {
            return match rule.behavior.as_str() {
                "allow" => PermissionVerdict::Allow,
                "deny" => PermissionVerdict::Deny("Denied by saved rule".to_string()),
                _ => PermissionVerdict::AskUser,
            };
        }

        // 6. Fallback
        PermissionVerdict::AskUser
    }
}

/// Extract the relevant filesystem path from tool input based on tool name.
fn extract_path(tool_name: &str, input: &serde_json::Value) -> Option<String> {
    match tool_name {
        "Read" | "Edit" | "Write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .map(String::from),
        "Glob" | "Grep" => input.get("path").and_then(|v| v.as_str()).map(String::from),
        _ => None,
    }
}

/// Check if a path is within the given directory (prefix check after normalization).
fn is_within(path: &str, dir: &str) -> bool {
    let p = Path::new(path);
    let d = Path::new(dir);

    // Use canonical forms if available, fall back to starts_with on raw paths
    if let (Ok(cp), Ok(cd)) = (p.canonicalize(), d.canonicalize()) {
        cp.starts_with(&cd)
    } else {
        p.starts_with(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_path_read() {
        let input = serde_json::json!({"file_path": "/home/user/file.rs"});
        assert_eq!(
            extract_path("Read", &input),
            Some("/home/user/file.rs".to_string())
        );
    }

    #[test]
    fn test_extract_path_grep() {
        let input = serde_json::json!({"pattern": "foo", "path": "/home/user"});
        assert_eq!(extract_path("Grep", &input), Some("/home/user".to_string()));
    }

    #[test]
    fn test_extract_path_bash() {
        let input = serde_json::json!({"command": "ls -la"});
        assert_eq!(extract_path("Bash", &input), None);
    }

    #[test]
    fn test_is_within() {
        assert!(is_within("/tmp/project/src/main.rs", "/tmp/project"));
        assert!(!is_within("/home/user/secrets.txt", "/tmp/project"));
        assert!(is_within("/tmp/project", "/tmp/project"));
    }

    #[tokio::test]
    async fn test_bash_always_ask_user() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = httui_core::db::init_db(tmp.path()).await.unwrap();
        let broker = PermissionBroker::new(pool);
        let input = serde_json::json!({"command": "rm -rf /"});

        let verdict = broker.check("Bash", &input, 1, Some("/tmp")).await;
        assert!(matches!(verdict, PermissionVerdict::AskUser));
    }

    #[tokio::test]
    async fn test_read_inside_cwd_auto_allow() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = httui_core::db::init_db(tmp.path()).await.unwrap();
        let broker = PermissionBroker::new(pool);

        let cwd = tmp.path().to_string_lossy().to_string();
        // Create a file so canonicalize works
        let file_path = tmp.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let input = serde_json::json!({"file_path": file_path.to_string_lossy()});
        let verdict = broker.check("Read", &input, 1, Some(&cwd)).await;
        assert!(matches!(verdict, PermissionVerdict::Allow));
    }

    #[tokio::test]
    async fn test_edit_outside_cwd_denied() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = httui_core::db::init_db(tmp.path()).await.unwrap();
        let broker = PermissionBroker::new(pool);

        let input = serde_json::json!({"file_path": "/etc/passwd"});
        let cwd = tmp.path().to_string_lossy().to_string();
        let verdict = broker.check("Edit", &input, 1, Some(&cwd)).await;
        assert!(matches!(verdict, PermissionVerdict::Deny(_)));
    }

    #[tokio::test]
    async fn test_persisted_rule_overrides() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = httui_core::db::init_db(tmp.path()).await.unwrap();

        // Create a session first
        let session = chat::create_session(&pool, None).await.unwrap();

        // Insert an 'always allow' rule for Edit
        chat::insert_permission(&pool, "Edit", None, None, "always", "allow", None)
            .await
            .unwrap();

        let broker = PermissionBroker::new(pool);
        // Edit inside cwd — would normally be AskUser without the rule,
        // but the persisted rule should make it Allow.
        // Note: no cwd → hardcoded rules don't trigger, goes to DB lookup.
        let input = serde_json::json!({"file_path": "/some/file.rs"});
        let verdict = broker.check("Edit", &input, session.id, None).await;
        assert!(matches!(verdict, PermissionVerdict::Allow));
    }

    #[tokio::test]
    async fn test_execute_block_always_ask_user() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = httui_core::db::init_db(tmp.path()).await.unwrap();
        let broker = PermissionBroker::new(pool);
        let input = serde_json::json!({
            "block_type": "db",
            "params": {"connection_id": "test", "query": "SELECT 1"}
        });

        let verdict = broker.check("execute_block", &input, 1, Some("/tmp")).await;
        assert!(matches!(verdict, PermissionVerdict::AskUser));
    }
}
