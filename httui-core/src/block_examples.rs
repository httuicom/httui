//! Pinned response examples per HTTP block.
//!
//! Examples are user-curated snapshots ("happy path 200", "422 invalid
//! email"). Unlike the cache (`block_results`) they survive cache
//! trimming and are never auto-evicted; the user explicitly pins and
//! unpins them via the drawer.
//!
//! Storage: full `HttpResponse` shape stringified as JSON. We don't
//! re-execute or transform — clicking an example in the drawer just
//! restores the snapshot into the React result panel.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockExample {
    pub id: i64,
    pub file_path: String,
    pub block_alias: String,
    pub name: String,
    pub response_json: String,
    pub saved_at: String,
}

/// Insert (or replace if `(file, alias, name)` already exists). Returns
/// the row id of the saved example.
pub async fn save_example(
    pool: &SqlitePool,
    file_path: &str,
    block_alias: &str,
    name: &str,
    response_json: &str,
) -> Result<i64, sqlx::Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let row = sqlx::query(
        "INSERT INTO block_examples (file_path, block_alias, name, response_json, saved_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(file_path, block_alias, name) DO UPDATE SET
            response_json = excluded.response_json,
            saved_at      = excluded.saved_at
         RETURNING id",
    )
    .bind(file_path)
    .bind(block_alias)
    .bind(name)
    .bind(response_json)
    .bind(&now)
    .fetch_one(pool)
    .await?;
    use sqlx::Row;
    row.try_get::<i64, _>("id")
}

/// List examples for a (file, alias), most recent first.
pub async fn list_examples(
    pool: &SqlitePool,
    file_path: &str,
    block_alias: &str,
) -> Result<Vec<BlockExample>, sqlx::Error> {
    let rows = sqlx::query_as::<_, (i64, String, String, String, String, String)>(
        "SELECT id, file_path, block_alias, name, response_json, saved_at
         FROM block_examples
         WHERE file_path = ? AND block_alias = ?
         ORDER BY saved_at DESC",
    )
    .bind(file_path)
    .bind(block_alias)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| BlockExample {
            id: r.0,
            file_path: r.1,
            block_alias: r.2,
            name: r.3,
            response_json: r.4,
            saved_at: r.5,
        })
        .collect())
}

/// Delete a single example by id.
pub async fn delete_example(pool: &SqlitePool, id: i64) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM block_examples WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Cascade-delete all examples for a file. Called when the host note is
/// removed from the vault.
pub async fn purge_examples_for_file(
    pool: &SqlitePool,
    file_path: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM block_examples WHERE file_path = ?")
        .bind(file_path)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Cascade-delete all examples for a (file, alias). Called when a single
/// block is removed from a document.
pub async fn purge_examples_for_block(
    pool: &SqlitePool,
    file_path: &str,
    block_alias: &str,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query("DELETE FROM block_examples WHERE file_path = ? AND block_alias = ?")
        .bind(file_path)
        .bind(block_alias)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE block_examples (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path     TEXT NOT NULL,
                block_alias   TEXT NOT NULL,
                name          TEXT NOT NULL,
                response_json TEXT NOT NULL,
                saved_at      TEXT NOT NULL,
                UNIQUE (file_path, block_alias, name)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn save_and_list() {
        let pool = setup().await;
        let id = save_example(&pool, "/a.md", "req1", "happy", "{\"status\":200}")
            .await
            .unwrap();
        assert!(id > 0);
        let rows = list_examples(&pool, "/a.md", "req1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "happy");
        assert_eq!(rows[0].response_json, "{\"status\":200}");
    }

    #[tokio::test]
    async fn upsert_replaces_on_conflict() {
        let pool = setup().await;
        save_example(&pool, "/a.md", "req1", "v1", "{\"status\":200}")
            .await
            .unwrap();
        save_example(&pool, "/a.md", "req1", "v1", "{\"status\":201}")
            .await
            .unwrap();
        let rows = list_examples(&pool, "/a.md", "req1").await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].response_json, "{\"status\":201}");
    }

    #[tokio::test]
    async fn isolates_by_file_and_alias() {
        let pool = setup().await;
        save_example(&pool, "/a.md", "req1", "x", "1")
            .await
            .unwrap();
        save_example(&pool, "/b.md", "req1", "x", "2")
            .await
            .unwrap();
        save_example(&pool, "/a.md", "other", "x", "3")
            .await
            .unwrap();
        assert_eq!(
            list_examples(&pool, "/a.md", "req1").await.unwrap().len(),
            1
        );
        assert_eq!(
            list_examples(&pool, "/b.md", "req1").await.unwrap().len(),
            1
        );
        assert_eq!(
            list_examples(&pool, "/a.md", "other").await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn delete_by_id() {
        let pool = setup().await;
        let id = save_example(&pool, "/a.md", "req1", "x", "1")
            .await
            .unwrap();
        let removed = delete_example(&pool, id).await.unwrap();
        assert_eq!(removed, 1);
        assert!(list_examples(&pool, "/a.md", "req1")
            .await
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn purge_for_file_drops_all_aliases() {
        let pool = setup().await;
        save_example(&pool, "/a.md", "req1", "x", "1")
            .await
            .unwrap();
        save_example(&pool, "/a.md", "req2", "y", "2")
            .await
            .unwrap();
        save_example(&pool, "/b.md", "req1", "z", "3")
            .await
            .unwrap();
        let removed = purge_examples_for_file(&pool, "/a.md").await.unwrap();
        assert_eq!(removed, 2);
        assert!(list_examples(&pool, "/a.md", "req1")
            .await
            .unwrap()
            .is_empty());
        assert!(list_examples(&pool, "/a.md", "req2")
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            list_examples(&pool, "/b.md", "req1").await.unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn purge_for_block_only_drops_that_alias() {
        let pool = setup().await;
        save_example(&pool, "/a.md", "req1", "x", "1")
            .await
            .unwrap();
        save_example(&pool, "/a.md", "req2", "y", "2")
            .await
            .unwrap();
        let removed = purge_examples_for_block(&pool, "/a.md", "req1")
            .await
            .unwrap();
        assert_eq!(removed, 1);
        assert!(list_examples(&pool, "/a.md", "req1")
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            list_examples(&pool, "/a.md", "req2").await.unwrap().len(),
            1
        );
    }
}
