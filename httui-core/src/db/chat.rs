use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: i64,
    pub claude_session_id: Option<String>,
    pub title: String,
    pub cwd: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub archived_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    pub session_id: i64,
    pub role: String,
    pub turn_index: i64,
    pub content_json: String,
    pub tokens_in: Option<i64>,
    pub tokens_out: Option<i64>,
    pub is_partial: bool,
    pub created_at: i64,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: i64,
    pub tool_use_id: String,
    pub tool_name: String,
    pub input_json: String,
    pub result_json: Option<String>,
    pub is_error: bool,
    pub created_at: i64,
}

fn row_to_session(row: &sqlx::sqlite::SqliteRow) -> Session {
    Session {
        id: row.get("id"),
        claude_session_id: row.get("claude_session_id"),
        title: row.get("title"),
        cwd: row.get("cwd"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        archived_at: row.get("archived_at"),
    }
}

fn row_to_message(row: &sqlx::sqlite::SqliteRow) -> Message {
    Message {
        id: row.get("id"),
        session_id: row.get("session_id"),
        role: row.get("role"),
        turn_index: row.get("turn_index"),
        content_json: row.get("content_json"),
        tokens_in: row.get("tokens_in"),
        tokens_out: row.get("tokens_out"),
        is_partial: row.get::<i32, _>("is_partial") != 0,
        created_at: row.get("created_at"),
        tool_calls: vec![],
    }
}

fn row_to_tool_call(row: &sqlx::sqlite::SqliteRow) -> ToolCall {
    ToolCall {
        id: row.get("id"),
        tool_use_id: row.get("tool_use_id"),
        tool_name: row.get("tool_name"),
        input_json: row.get("input_json"),
        result_json: row.get("result_json"),
        is_error: row.get::<i32, _>("is_error") != 0,
        created_at: row.get("created_at"),
    }
}

pub async fn create_session(pool: &SqlitePool, cwd: Option<String>) -> Result<Session, String> {
    let row = sqlx::query("INSERT INTO sessions (cwd) VALUES (?) RETURNING *")
        .bind(&cwd)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to create session: {e}"))?;

    Ok(row_to_session(&row))
}

pub async fn list_sessions(pool: &SqlitePool) -> Result<Vec<Session>, String> {
    let rows =
        sqlx::query("SELECT * FROM sessions WHERE archived_at IS NULL ORDER BY updated_at DESC")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list sessions: {e}"))?;

    Ok(rows.iter().map(row_to_session).collect())
}

pub async fn get_session(pool: &SqlitePool, session_id: i64) -> Result<Session, String> {
    let row = sqlx::query("SELECT * FROM sessions WHERE id = ?")
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to get session: {e}"))?
        .ok_or_else(|| format!("Session {session_id} not found"))?;

    Ok(row_to_session(&row))
}

pub async fn archive_session(pool: &SqlitePool, session_id: i64) -> Result<(), String> {
    let result = sqlx::query(
        "UPDATE sessions SET archived_at = unixepoch() WHERE id = ? AND archived_at IS NULL",
    )
    .bind(session_id)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to archive session: {e}"))?;

    if result.rows_affected() == 0 {
        return Err(format!(
            "Session {session_id} not found or already archived"
        ));
    }
    Ok(())
}

pub async fn update_session_claude_id(
    pool: &SqlitePool,
    session_id: i64,
    claude_session_id: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET claude_session_id = ?, updated_at = unixepoch() WHERE id = ?")
        .bind(claude_session_id)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update claude_session_id: {e}"))?;
    Ok(())
}

pub async fn update_session_title(
    pool: &SqlitePool,
    session_id: i64,
    title: &str,
) -> Result<(), String> {
    sqlx::query("UPDATE sessions SET title = ?, updated_at = unixepoch() WHERE id = ?")
        .bind(title)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update session title: {e}"))?;
    Ok(())
}

pub async fn insert_message(
    pool: &SqlitePool,
    session_id: i64,
    role: &str,
    content_json: &str,
    tokens_in: Option<i64>,
    tokens_out: Option<i64>,
    is_partial: bool,
) -> Result<Message, String> {
    let max_turn: Option<i64> =
        sqlx::query_scalar("SELECT MAX(turn_index) FROM messages WHERE session_id = ?")
            .bind(session_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to get max turn_index: {e}"))?;

    let turn_index = max_turn.map_or(0, |t| t + 1);

    let row = sqlx::query(
        "INSERT INTO messages (session_id, role, turn_index, content_json, tokens_in, tokens_out, is_partial) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *",
    )
    .bind(session_id)
    .bind(role)
    .bind(turn_index)
    .bind(content_json)
    .bind(tokens_in)
    .bind(tokens_out)
    .bind(is_partial as i32)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to insert message: {e}"))?;

    let _ = sqlx::query("UPDATE sessions SET updated_at = unixepoch() WHERE id = ?")
        .bind(session_id)
        .execute(pool)
        .await;

    Ok(row_to_message(&row))
}

pub async fn list_messages(pool: &SqlitePool, session_id: i64) -> Result<Vec<Message>, String> {
    let msg_rows =
        sqlx::query("SELECT * FROM messages WHERE session_id = ? ORDER BY turn_index ASC")
            .bind(session_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Failed to list messages: {e}"))?;

    let message_ids: Vec<i64> = msg_rows.iter().map(|r| r.get("id")).collect();

    if message_ids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders = message_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let query_str = format!(
        "SELECT * FROM tool_calls WHERE message_id IN ({placeholders}) ORDER BY created_at ASC"
    );
    let mut query = sqlx::query(&query_str);
    for id in &message_ids {
        query = query.bind(id);
    }
    let tc_rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list tool_calls: {e}"))?;

    let mut tc_map: std::collections::HashMap<i64, Vec<ToolCall>> =
        std::collections::HashMap::new();
    for row in &tc_rows {
        let msg_id: i64 = row.get("message_id");
        tc_map
            .entry(msg_id)
            .or_default()
            .push(row_to_tool_call(row));
    }

    let messages = msg_rows
        .iter()
        .map(|row| {
            let mut msg = row_to_message(row);
            if let Some(tcs) = tc_map.remove(&msg.id) {
                msg.tool_calls = tcs;
            }
            msg
        })
        .collect();

    Ok(messages)
}

pub async fn insert_tool_call(
    pool: &SqlitePool,
    message_id: i64,
    tool_use_id: &str,
    tool_name: &str,
    input_json: &str,
) -> Result<ToolCall, String> {
    let row = sqlx::query(
        "INSERT INTO tool_calls (message_id, tool_use_id, tool_name, input_json) VALUES (?, ?, ?, ?) RETURNING *",
    )
    .bind(message_id)
    .bind(tool_use_id)
    .bind(tool_name)
    .bind(input_json)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to insert tool_call: {e}"))?;

    Ok(row_to_tool_call(&row))
}

pub async fn update_tool_call_result(
    pool: &SqlitePool,
    tool_use_id: &str,
    result_json: &str,
    is_error: bool,
) -> Result<(), String> {
    sqlx::query("UPDATE tool_calls SET result_json = ?, is_error = ? WHERE tool_use_id = ?")
        .bind(result_json)
        .bind(is_error as i32)
        .bind(tool_use_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to update tool_call result: {e}"))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub id: i64,
    pub tool_name: String,
    pub path_pattern: Option<String>,
    pub workspace: Option<String>,
    pub scope: String,
    pub behavior: String,
    pub session_id: Option<i64>,
    pub created_at: i64,
}

fn row_to_permission(row: &sqlx::sqlite::SqliteRow) -> PermissionRule {
    PermissionRule {
        id: row.get("id"),
        tool_name: row.get("tool_name"),
        path_pattern: row.get("path_pattern"),
        workspace: row.get("workspace"),
        scope: row.get("scope"),
        behavior: row.get("behavior"),
        session_id: row.get("session_id"),
        created_at: row.get("created_at"),
    }
}

/// Check for a matching permission rule. Checks persisted ('always') first, then session-scoped.
pub async fn check_permission(
    pool: &SqlitePool,
    tool_name: &str,
    workspace: Option<&str>,
    session_id: i64,
) -> Result<Option<PermissionRule>, String> {
    // 1. Check 'always' rules (workspace-specific first, then global)
    let row = sqlx::query(
        "SELECT * FROM tool_permissions WHERE tool_name = ? AND scope = 'always' \
         AND (workspace = ? OR workspace IS NULL) \
         ORDER BY CASE WHEN workspace IS NOT NULL THEN 0 ELSE 1 END LIMIT 1",
    )
    .bind(tool_name)
    .bind(workspace)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to check permission: {e}"))?;

    if let Some(row) = row {
        return Ok(Some(row_to_permission(&row)));
    }

    // 2. Check 'session' rules
    let row = sqlx::query(
        "SELECT * FROM tool_permissions WHERE tool_name = ? AND scope = 'session' \
         AND session_id = ? LIMIT 1",
    )
    .bind(tool_name)
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("Failed to check session permission: {e}"))?;

    Ok(row.as_ref().map(row_to_permission))
}

pub async fn insert_permission(
    pool: &SqlitePool,
    tool_name: &str,
    path_pattern: Option<&str>,
    workspace: Option<&str>,
    scope: &str,
    behavior: &str,
    session_id: Option<i64>,
) -> Result<PermissionRule, String> {
    let row = sqlx::query(
        "INSERT INTO tool_permissions (tool_name, path_pattern, workspace, scope, behavior, session_id) \
         VALUES (?, ?, ?, ?, ?, ?) RETURNING *"
    )
    .bind(tool_name)
    .bind(path_pattern)
    .bind(workspace)
    .bind(scope)
    .bind(behavior)
    .bind(session_id)
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to insert permission: {e}"))?;

    Ok(row_to_permission(&row))
}

pub async fn delete_permission(pool: &SqlitePool, id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM tool_permissions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to delete permission: {e}"))?;
    Ok(())
}

pub async fn list_permissions(
    pool: &SqlitePool,
    workspace: Option<&str>,
) -> Result<Vec<PermissionRule>, String> {
    let rows = if let Some(ws) = workspace {
        sqlx::query(
            "SELECT * FROM tool_permissions WHERE (workspace = ? OR workspace IS NULL) \
             ORDER BY created_at DESC",
        )
        .bind(ws)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query("SELECT * FROM tool_permissions ORDER BY created_at DESC")
            .fetch_all(pool)
            .await
    }
    .map_err(|e| format!("Failed to list permissions: {e}"))?;

    Ok(rows.iter().map(row_to_permission).collect())
}

pub async fn clear_session_permissions(pool: &SqlitePool, session_id: i64) -> Result<(), String> {
    sqlx::query("DELETE FROM tool_permissions WHERE scope = 'session' AND session_id = ?")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to clear session permissions: {e}"))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    pub date: String,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
}

pub async fn upsert_usage(
    pool: &SqlitePool,
    session_id: i64,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO usage_stats (date, session_id, input_tokens, output_tokens, cache_read_tokens) \
         VALUES (date('now'), ?, ?, ?, ?) \
         ON CONFLICT (date, session_id) DO UPDATE SET \
             input_tokens = input_tokens + excluded.input_tokens, \
             output_tokens = output_tokens + excluded.output_tokens, \
             cache_read_tokens = cache_read_tokens + excluded.cache_read_tokens"
    )
    .bind(session_id)
    .bind(input_tokens)
    .bind(output_tokens)
    .bind(cache_read_tokens)
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to upsert usage: {e}"))?;
    Ok(())
}

pub async fn get_usage_by_date_range(
    pool: &SqlitePool,
    from: &str,
    to: &str,
) -> Result<Vec<DailyUsage>, String> {
    let rows = sqlx::query(
        "SELECT date, SUM(input_tokens) as input_tokens, SUM(output_tokens) as output_tokens, \
         SUM(cache_read_tokens) as cache_read_tokens \
         FROM usage_stats WHERE date >= ? AND date <= ? \
         GROUP BY date ORDER BY date ASC",
    )
    .bind(from)
    .bind(to)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to get usage: {e}"))?;

    Ok(rows
        .iter()
        .map(|r| DailyUsage {
            date: r.get("date"),
            input_tokens: r.get("input_tokens"),
            output_tokens: r.get("output_tokens"),
            cache_read_tokens: r.get("cache_read_tokens"),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use tempfile::TempDir;

    async fn setup() -> (SqlitePool, TempDir) {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();
        (pool, tmp)
    }

    #[tokio::test]
    async fn test_create_and_get_session() {
        let (pool, _tmp) = setup().await;

        let session = create_session(&pool, Some("/tmp/project".to_string()))
            .await
            .unwrap();

        assert_eq!(session.title, "Nova conversa");
        assert_eq!(session.cwd, Some("/tmp/project".to_string()));
        assert!(session.claude_session_id.is_none());
        assert!(session.archived_at.is_none());

        let fetched = get_session(&pool, session.id).await.unwrap();
        assert_eq!(fetched.id, session.id);
        assert_eq!(fetched.title, "Nova conversa");
    }

    #[tokio::test]
    async fn test_list_sessions_excludes_archived() {
        let (pool, _tmp) = setup().await;

        let s1 = create_session(&pool, None).await.unwrap();
        let s2 = create_session(&pool, None).await.unwrap();
        archive_session(&pool, s1.id).await.unwrap();

        let sessions = list_sessions(&pool).await.unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, s2.id);
    }

    #[tokio::test]
    async fn test_update_session_claude_id() {
        let (pool, _tmp) = setup().await;

        let session = create_session(&pool, None).await.unwrap();
        update_session_claude_id(&pool, session.id, "sess_abc123")
            .await
            .unwrap();

        let fetched = get_session(&pool, session.id).await.unwrap();
        assert_eq!(fetched.claude_session_id, Some("sess_abc123".to_string()));
    }

    #[tokio::test]
    async fn test_update_session_title() {
        let (pool, _tmp) = setup().await;

        let session = create_session(&pool, None).await.unwrap();
        update_session_title(&pool, session.id, "Analise de TODOs")
            .await
            .unwrap();

        let fetched = get_session(&pool, session.id).await.unwrap();
        assert_eq!(fetched.title, "Analise de TODOs");
    }

    #[tokio::test]
    async fn test_insert_and_list_messages() {
        let (pool, _tmp) = setup().await;

        let session = create_session(&pool, None).await.unwrap();

        let m1 = insert_message(
            &pool,
            session.id,
            "user",
            r#"[{"type":"text","text":"hello"}]"#,
            None,
            None,
            false,
        )
        .await
        .unwrap();
        assert_eq!(m1.turn_index, 0);
        assert_eq!(m1.role, "user");

        let m2 = insert_message(
            &pool,
            session.id,
            "assistant",
            r#"[{"type":"text","text":"Hi!"}]"#,
            Some(100),
            Some(50),
            false,
        )
        .await
        .unwrap();
        assert_eq!(m2.turn_index, 1);

        let messages = list_messages(&pool, session.id).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].tokens_in, Some(100));
    }

    #[tokio::test]
    async fn test_insert_tool_call_and_list_with_messages() {
        let (pool, _tmp) = setup().await;

        let session = create_session(&pool, None).await.unwrap();
        let msg = insert_message(
            &pool,
            session.id,
            "assistant",
            r#"[{"type":"text","text":"Let me check."}]"#,
            None,
            None,
            false,
        )
        .await
        .unwrap();

        let tc = insert_tool_call(
            &pool,
            msg.id,
            "toolu_01",
            "Read",
            r#"{"file_path":"/tmp/foo.rs"}"#,
        )
        .await
        .unwrap();
        assert_eq!(tc.tool_name, "Read");
        assert!(!tc.is_error);

        update_tool_call_result(&pool, "toolu_01", r#"{"content":"fn main() {}"}"#, false)
            .await
            .unwrap();

        let messages = list_messages(&pool, session.id).await.unwrap();
        assert_eq!(messages[0].tool_calls.len(), 1);
        assert_eq!(messages[0].tool_calls[0].tool_name, "Read");
        assert!(messages[0].tool_calls[0].result_json.is_some());
    }

    #[tokio::test]
    async fn test_partial_message() {
        let (pool, _tmp) = setup().await;

        let session = create_session(&pool, None).await.unwrap();
        let msg = insert_message(
            &pool,
            session.id,
            "assistant",
            r#"[{"type":"text","text":"Partial resp"}]"#,
            None,
            None,
            true,
        )
        .await
        .unwrap();

        assert!(msg.is_partial);

        let messages = list_messages(&pool, session.id).await.unwrap();
        assert!(messages[0].is_partial);
    }

    #[tokio::test]
    async fn test_archive_nonexistent_session() {
        let (pool, _tmp) = setup().await;
        let result = archive_session(&pool, 99999).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cascade_delete() {
        let (pool, _tmp) = setup().await;

        let session = create_session(&pool, None).await.unwrap();
        let msg = insert_message(&pool, session.id, "user", "[]", None, None, false)
            .await
            .unwrap();
        insert_tool_call(&pool, msg.id, "toolu_99", "Bash", "{}")
            .await
            .unwrap();

        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session.id)
            .execute(&pool)
            .await
            .unwrap();

        // Messages and tool_calls should be gone (CASCADE)
        let messages = list_messages(&pool, session.id).await.unwrap();
        assert!(messages.is_empty());
    }
}
