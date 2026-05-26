use httui_core::blocks::parser;
use httui_core::executor::ExecutorRegistry;
use httui_core::runner::BlockRunner;
use serde_json::json;
use sqlx::sqlite::SqlitePool;
use std::sync::Arc;

pub fn list_blocks(vault_path: &str, note_path: &str) -> String {
    let content = match httui_core::fs::read_note(vault_path, note_path) {
        Ok(c) => c,
        Err(e) => return json!({"error": e}).to_string(),
    };

    let blocks = parser::parse_blocks(&content);

    let summary: Vec<serde_json::Value> = blocks
        .iter()
        .map(|b| {
            json!({
                "type": b.block_type,
                "alias": b.alias,
                "display_mode": b.display_mode,
                "line": b.line_start,
            })
        })
        .collect();

    json!({"blocks": summary}).to_string()
}

pub async fn execute_block(
    vault_path: &str,
    note_path: &str,
    alias: &str,
    registry: &Arc<ExecutorRegistry>,
    pool: &SqlitePool,
) -> String {
    if note_path.contains("..") || std::path::Path::new(note_path).is_absolute() {
        return json!({"error": "Invalid note path"}).to_string();
    }

    match httui_core::fs::read_note(vault_path, note_path) {
        Ok(content) => {
            let blocks = parser::parse_blocks(&content);
            if !blocks.iter().any(|b| b.alias.as_deref() == Some(alias)) {
                return json!({"error": format!("Block alias '{}' not found in note", alias)})
                    .to_string();
            }
        }
        Err(e) => return json!({"error": e}).to_string(),
    }

    let runner = BlockRunner::new(registry.clone(), pool.clone());

    match runner.execute(vault_path, note_path, alias).await {
        Ok(result) => json!({
            "status": result.status,
            "data": result.data,
            "duration_ms": result.duration_ms,
        })
        .to_string(),
        Err(e) => json!({"error": e.to_string()}).to_string(),
    }
}
