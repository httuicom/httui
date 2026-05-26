use serde_json::json;
use sqlx::sqlite::SqlitePool;

pub async fn list_environments(pool: &SqlitePool) -> String {
    match httui_core::db::environments::list_environments(pool).await {
        Ok(envs) => json!({"environments": envs}).to_string(),
        Err(e) => json!({"error": e}).to_string(),
    }
}

pub async fn get_environment_variables(pool: &SqlitePool, environment_id: &str) -> String {
    match httui_core::db::environments::list_env_variables(pool, environment_id).await {
        Ok(vars) => {
            let masked: Vec<serde_json::Value> = vars
                .iter()
                .map(|v| {
                    json!({
                        "id": v.id,
                        "key": v.key,
                        "value": if v.is_secret { "***" } else { &v.value },
                        "is_secret": v.is_secret,
                    })
                })
                .collect();
            json!({"variables": masked}).to_string()
        }
        Err(e) => json!({"error": e}).to_string(),
    }
}

pub async fn set_active_environment(pool: &SqlitePool, id: Option<&str>) -> String {
    match httui_core::db::environments::set_active_environment(pool, id).await {
        Ok(()) => {
            let msg = match id {
                Some(id) => format!("Activated environment {id}"),
                None => "Deactivated all environments".to_string(),
            };
            json!({"success": msg}).to_string()
        }
        Err(e) => json!({"error": e}).to_string(),
    }
}
