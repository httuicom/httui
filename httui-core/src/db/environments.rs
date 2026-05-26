use super::keychain;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqlitePool;
use sqlx::Row;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVariable {
    pub id: String,
    pub environment_id: String,
    pub key: String,
    pub value: String,
    pub is_secret: bool,
    pub created_at: String,
}

impl EnvVariable {
    /// Return a masked copy for Tauri IPC — secret values replaced with empty string.
    /// Frontend uses this for display; actual secret values are resolved only during execution.
    pub fn to_masked(&self) -> EnvVariable {
        EnvVariable {
            value: if self.is_secret {
                String::new()
            } else {
                self.value.clone()
            },
            ..self.clone()
        }
    }
}

pub async fn list_env_variables_masked(
    pool: &SqlitePool,
    environment_id: &str,
) -> Result<Vec<EnvVariable>, String> {
    let vars = list_env_variables(pool, environment_id).await?;
    Ok(vars.into_iter().map(|v| v.to_masked()).collect())
}

fn row_to_environment(row: &sqlx::sqlite::SqliteRow) -> Environment {
    Environment {
        id: row.get("id"),
        name: row.get("name"),
        is_active: row.get::<i32, _>("is_active") != 0,
        created_at: row.get("created_at"),
    }
}

fn row_to_variable(row: &sqlx::sqlite::SqliteRow) -> EnvVariable {
    let environment_id: String = row.get("environment_id");
    let key: String = row.get("key");
    let db_value: String = row.get("value");
    let is_secret: bool = row.get::<i32, _>("is_secret") != 0;

    // Resolve keychain sentinel if needed — return empty string on failure, never the sentinel
    let value = if is_secret && db_value == keychain::KEYCHAIN_SENTINEL {
        keychain::resolve_value(&db_value, &keychain::env_var_key(&environment_id, &key))
            .unwrap_or_else(|_| String::new())
    } else {
        db_value
    };

    EnvVariable {
        id: row.get("id"),
        environment_id,
        key,
        value,
        is_secret,
        created_at: row.get("created_at"),
    }
}

/// Get the ID of the currently active environment, if any.
pub async fn get_active_environment_id(pool: &SqlitePool) -> Option<String> {
    sqlx::query_scalar("SELECT id FROM environments WHERE is_active = 1")
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

pub async fn list_environments(pool: &SqlitePool) -> Result<Vec<Environment>, String> {
    let rows = sqlx::query("SELECT * FROM environments ORDER BY created_at ASC")
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list environments: {e}"))?;

    Ok(rows.iter().map(row_to_environment).collect())
}

pub async fn create_environment(pool: &SqlitePool, name: String) -> Result<Environment, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Environment name is required".to_string());
    }

    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query("INSERT INTO environments (id, name, is_active) VALUES (?, ?, 0)")
        .bind(&id)
        .bind(&name)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to create environment: {e}"))?;

    let row = sqlx::query("SELECT * FROM environments WHERE id = ?")
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to fetch created environment: {e}"))?;

    Ok(row_to_environment(&row))
}

pub async fn delete_environment(pool: &SqlitePool, id: &str) -> Result<(), String> {
    let result = sqlx::query("DELETE FROM environments WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to delete environment: {e}"))?;

    if result.rows_affected() == 0 {
        return Err("Environment not found".to_string());
    }

    Ok(())
}

pub async fn duplicate_environment(
    pool: &SqlitePool,
    source_id: &str,
    new_name: String,
) -> Result<Environment, String> {
    let new_name = new_name.trim().to_string();
    if new_name.is_empty() {
        return Err("Environment name is required".to_string());
    }

    let source = sqlx::query("SELECT * FROM environments WHERE id = ?")
        .bind(source_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to find source environment: {e}"))?
        .ok_or("Source environment not found")?;
    let _ = row_to_environment(&source);

    let new_env = create_environment(pool, new_name).await?;

    // Copy variables — read raw rows to avoid resolving secrets to plaintext
    let raw_rows = sqlx::query(
        "SELECT key, value, is_secret FROM env_variables WHERE environment_id = ? ORDER BY key ASC",
    )
    .bind(source_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to list source variables: {e}"))?;

    for row in &raw_rows {
        let key: String = row.get("key");
        let db_value: String = row.get("value");
        let is_secret: bool = row.get::<i32, _>("is_secret") != 0;

        if is_secret && db_value == keychain::KEYCHAIN_SENTINEL {
            // Copy secret directly keychain-to-keychain without resolving to plaintext
            let source_key = keychain::env_var_key(source_id, &key);
            let dest_key = keychain::env_var_key(&new_env.id, &key);
            let secret = keychain::get_secret(&source_key)
                .map_err(|e| format!("Failed to read secret for {key}: {e}"))?
                .ok_or_else(|| format!("Secret not found in keychain for {key}"))?;
            keychain::store_secret(&dest_key, &secret)
                .map_err(|e| format!("Failed to copy secret for {key}: {e}"))?;
            let var_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO env_variables (id, environment_id, key, value, is_secret) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&var_id)
            .bind(&new_env.id)
            .bind(&key)
            .bind(keychain::KEYCHAIN_SENTINEL)
            .bind(1)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to copy variable {key}: {e}"))?;
        } else {
            set_env_variable(pool, &new_env.id, key, db_value, is_secret).await?;
        }
    }

    Ok(new_env)
}

pub async fn set_active_environment(pool: &SqlitePool, id: Option<&str>) -> Result<(), String> {
    sqlx::query("UPDATE environments SET is_active = 0")
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to deactivate environments: {e}"))?;

    if let Some(id) = id {
        let result = sqlx::query("UPDATE environments SET is_active = 1 WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to activate environment: {e}"))?;

        if result.rows_affected() == 0 {
            return Err("Environment not found".to_string());
        }
    }

    Ok(())
}

pub async fn list_env_variables(
    pool: &SqlitePool,
    environment_id: &str,
) -> Result<Vec<EnvVariable>, String> {
    let rows = sqlx::query("SELECT * FROM env_variables WHERE environment_id = ? ORDER BY key ASC")
        .bind(environment_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Failed to list variables: {e}"))?;

    Ok(rows.iter().map(row_to_variable).collect())
}

pub async fn set_env_variable(
    pool: &SqlitePool,
    environment_id: &str,
    key: String,
    value: String,
    is_secret: bool,
) -> Result<EnvVariable, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Variable key is required".to_string());
    }

    // If secret, store in keychain and use sentinel in DB — fail on keychain error, no plaintext fallback
    let db_value = if is_secret && !value.is_empty() {
        keychain::store_secret(&keychain::env_var_key(environment_id, &key), &value)
            .map_err(|e| format!("Failed to store secret securely: {e}"))?;
        keychain::KEYCHAIN_SENTINEL.to_string()
    } else {
        value.clone()
    };

    // Upsert: try update first, then insert
    let existing = sqlx::query("SELECT id FROM env_variables WHERE environment_id = ? AND key = ?")
        .bind(environment_id)
        .bind(&key)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to check existing variable: {e}"))?;

    let var_id = if let Some(row) = existing {
        let id: String = row.get("id");
        sqlx::query("UPDATE env_variables SET value = ?, is_secret = ? WHERE id = ?")
            .bind(&db_value)
            .bind(is_secret as i32)
            .bind(&id)
            .execute(pool)
            .await
            .map_err(|e| format!("Failed to update variable: {e}"))?;
        id
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO env_variables (id, environment_id, key, value, is_secret) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(environment_id)
        .bind(&key)
        .bind(&db_value)
        .bind(is_secret as i32)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to insert variable: {e}"))?;
        id
    };

    let row = sqlx::query("SELECT * FROM env_variables WHERE id = ?")
        .bind(&var_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Failed to fetch variable: {e}"))?;

    Ok(row_to_variable(&row))
}

pub async fn delete_env_variable(pool: &SqlitePool, id: &str) -> Result<(), String> {
    // Fetch before deleting to clean up keychain if needed
    let row = sqlx::query("SELECT environment_id, key, is_secret FROM env_variables WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Failed to fetch variable: {e}"))?;

    let result = sqlx::query("DELETE FROM env_variables WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to delete variable: {e}"))?;

    if result.rows_affected() == 0 {
        return Err("Variable not found".to_string());
    }

    // Clean up keychain entry if it was a secret
    if let Some(row) = row {
        let is_secret: bool = row.get::<i32, _>("is_secret") != 0;
        if is_secret {
            let env_id: String = row.get("environment_id");
            let key: String = row.get("key");
            let _ = keychain::delete_secret(&keychain::env_var_key(&env_id, &key));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let tmp = TempDir::new().unwrap();
        let pool = init_db(tmp.path()).await.unwrap();
        (tmp, pool)
    }

    #[tokio::test]
    async fn test_create_and_list_environments() {
        let (_tmp, pool) = setup().await;

        let envs = list_environments(&pool).await.unwrap();
        assert!(envs.is_empty());

        let env = create_environment(&pool, "local".to_string())
            .await
            .unwrap();
        assert_eq!(env.name, "local");
        assert!(!env.is_active);

        create_environment(&pool, "staging".to_string())
            .await
            .unwrap();

        let envs = list_environments(&pool).await.unwrap();
        assert_eq!(envs.len(), 2);
    }

    #[tokio::test]
    async fn test_create_empty_name_fails() {
        let (_tmp, pool) = setup().await;
        let result = create_environment(&pool, "  ".to_string()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_set_active_environment() {
        let (_tmp, pool) = setup().await;

        let env1 = create_environment(&pool, "local".to_string())
            .await
            .unwrap();
        let env2 = create_environment(&pool, "staging".to_string())
            .await
            .unwrap();

        set_active_environment(&pool, Some(&env1.id)).await.unwrap();
        let envs = list_environments(&pool).await.unwrap();
        assert!(envs.iter().find(|e| e.id == env1.id).unwrap().is_active);
        assert!(!envs.iter().find(|e| e.id == env2.id).unwrap().is_active);

        set_active_environment(&pool, Some(&env2.id)).await.unwrap();
        let envs = list_environments(&pool).await.unwrap();
        assert!(!envs.iter().find(|e| e.id == env1.id).unwrap().is_active);
        assert!(envs.iter().find(|e| e.id == env2.id).unwrap().is_active);

        set_active_environment(&pool, None).await.unwrap();
        let envs = list_environments(&pool).await.unwrap();
        assert!(envs.iter().all(|e| !e.is_active));
    }

    #[tokio::test]
    async fn test_env_variables_crud() {
        let (_tmp, pool) = setup().await;

        let env = create_environment(&pool, "local".to_string())
            .await
            .unwrap();

        let var = set_env_variable(
            &pool,
            &env.id,
            "BASE_URL".to_string(),
            "http://localhost:3000".to_string(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(var.key, "BASE_URL");
        assert_eq!(var.value, "http://localhost:3000");

        let vars = list_env_variables(&pool, &env.id).await.unwrap();
        assert_eq!(vars.len(), 1);

        // Upsert (update existing key)
        let updated = set_env_variable(
            &pool,
            &env.id,
            "BASE_URL".to_string(),
            "http://localhost:8080".to_string(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(updated.id, var.id); // same id
        assert_eq!(updated.value, "http://localhost:8080");

        let vars = list_env_variables(&pool, &env.id).await.unwrap();
        assert_eq!(vars.len(), 1); // still 1, not 2

        delete_env_variable(&pool, &var.id).await.unwrap();
        let vars = list_env_variables(&pool, &env.id).await.unwrap();
        assert!(vars.is_empty());
    }

    #[tokio::test]
    async fn test_duplicate_environment() {
        let (_tmp, pool) = setup().await;

        let env = create_environment(&pool, "local".to_string())
            .await
            .unwrap();
        set_env_variable(
            &pool,
            &env.id,
            "API_KEY".to_string(),
            "secret123".to_string(),
            false,
        )
        .await
        .unwrap();
        set_env_variable(
            &pool,
            &env.id,
            "BASE_URL".to_string(),
            "http://localhost".to_string(),
            false,
        )
        .await
        .unwrap();

        let dup = duplicate_environment(&pool, &env.id, "staging".to_string())
            .await
            .unwrap();
        assert_eq!(dup.name, "staging");
        assert_ne!(dup.id, env.id);

        let dup_vars = list_env_variables(&pool, &dup.id).await.unwrap();
        assert_eq!(dup_vars.len(), 2);
        assert!(dup_vars
            .iter()
            .any(|v| v.key == "API_KEY" && v.value == "secret123"));
        assert!(dup_vars
            .iter()
            .any(|v| v.key == "BASE_URL" && v.value == "http://localhost"));
    }

    #[tokio::test]
    async fn test_delete_environment_cascades_variables() {
        let (_tmp, pool) = setup().await;

        let env = create_environment(&pool, "local".to_string())
            .await
            .unwrap();
        set_env_variable(&pool, &env.id, "KEY".to_string(), "val".to_string(), false)
            .await
            .unwrap();

        delete_environment(&pool, &env.id).await.unwrap();

        // Variables should be gone (CASCADE)
        let vars = list_env_variables(&pool, &env.id).await.unwrap();
        assert!(vars.is_empty());
    }
}
