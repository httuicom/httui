use httui_core::db::connections::PoolManager;
use httui_core::executor::ExecutorRegistry;
use rmcp::{
    handler::server::wrapper::Parameters, model::*, schemars, tool, tool_handler, tool_router,
    ServerHandler,
};
use sqlx::sqlite::SqlitePool;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::tools;

/// Max execute_block calls per minute.
const EXECUTE_RATE_LIMIT: usize = 30;
const EXECUTE_RATE_WINDOW_SECS: u64 = 60;

#[derive(Clone)]
pub struct NotesMcpServer {
    pub pool: SqlitePool,
    pub conn_manager: Arc<PoolManager>,
    pub registry: Arc<ExecutorRegistry>,
    pub vault_path: String,
    /// Timestamps of recent execute_block calls for rate limiting.
    execute_timestamps: Arc<Mutex<VecDeque<std::time::Instant>>>,
}

impl NotesMcpServer {
    pub fn new(
        pool: SqlitePool,
        conn_manager: Arc<PoolManager>,
        registry: Arc<ExecutorRegistry>,
        vault_path: String,
    ) -> Self {
        Self {
            pool,
            conn_manager,
            registry,
            vault_path,
            execute_timestamps: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Check and record an execute_block call. Returns `Err` if rate limit exceeded.
    async fn check_execute_rate_limit(&self) -> Result<(), String> {
        let mut timestamps = self.execute_timestamps.lock().await;
        let now = std::time::Instant::now();
        let window = std::time::Duration::from_secs(EXECUTE_RATE_WINDOW_SECS);

        while timestamps
            .front()
            .is_some_and(|t| now.duration_since(*t) > window)
        {
            timestamps.pop_front();
        }

        if timestamps.len() >= EXECUTE_RATE_LIMIT {
            return Err(format!(
                "Rate limit exceeded: max {EXECUTE_RATE_LIMIT} execute_block calls per {EXECUTE_RATE_WINDOW_SECS}s"
            ));
        }

        timestamps.push_back(now);
        Ok(())
    }
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListNotesInput {
    #[schemars(
        description = "Optional subdirectory path to list (relative to vault root). Lists all notes if omitted."
    )]
    pub path: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadNoteInput {
    #[schemars(
        description = "Path to the note file, relative to vault root (e.g. 'api/users.md')"
    )]
    pub path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateNoteInput {
    #[schemars(description = "Path for the new note, relative to vault root")]
    pub path: String,
    #[schemars(description = "Markdown content for the note")]
    pub content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct UpdateNoteInput {
    #[schemars(description = "Path to the note file, relative to vault root")]
    pub path: String,
    #[schemars(description = "New markdown content")]
    pub content: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchNotesInput {
    #[schemars(description = "Search query string")]
    pub query: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListBlocksInput {
    #[schemars(description = "Path to the note file, relative to vault root")]
    pub note_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteBlockInput {
    #[schemars(description = "Path to the note file, relative to vault root")]
    pub note_path: String,
    #[schemars(description = "Alias of the block to execute")]
    pub alias: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListEnvironmentsInput {}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetEnvironmentVariablesInput {
    #[schemars(description = "Environment ID")]
    pub environment_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetActiveEnvironmentInput {
    #[schemars(description = "Environment ID to activate, or null to deactivate all")]
    pub environment_id: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListConnectionsInput {}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetDbSchemaInput {
    #[schemars(description = "Connection ID")]
    pub connection_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TestConnectionInput {
    #[schemars(description = "Connection ID to test")]
    pub connection_id: String,
}

#[tool_router]
impl NotesMcpServer {
    #[tool(description = "List all notes in the vault. Returns file tree with paths and names.")]
    async fn list_notes(&self, Parameters(input): Parameters<ListNotesInput>) -> String {
        tools::notes::list_notes(&self.vault_path, input.path.as_deref())
    }

    #[tool(description = "Read the full markdown content of a note.")]
    async fn read_note(&self, Parameters(input): Parameters<ReadNoteInput>) -> String {
        tools::notes::read_note(&self.vault_path, &input.path)
    }

    #[tool(description = "Create a new note with the given markdown content.")]
    async fn create_note(&self, Parameters(input): Parameters<CreateNoteInput>) -> String {
        tools::notes::create_note(&self.vault_path, &input.path, &input.content)
    }

    #[tool(description = "Update an existing note's content.")]
    async fn update_note(&self, Parameters(input): Parameters<UpdateNoteInput>) -> String {
        tools::notes::update_note(&self.vault_path, &input.path, &input.content)
    }

    #[tool(description = "Full-text search across all notes in the vault.")]
    async fn search_notes(&self, Parameters(input): Parameters<SearchNotesInput>) -> String {
        tools::notes::search_notes(&self.pool, &input.query).await
    }

    #[tool(
        description = "List all executable blocks (HTTP, DB) in a note with their type, alias, and parameters."
    )]
    async fn list_blocks(&self, Parameters(input): Parameters<ListBlocksInput>) -> String {
        tools::blocks::list_blocks(&self.vault_path, &input.note_path)
    }

    #[tool(
        description = "Execute an executable block by its alias. Resolves dependencies and environment variables automatically."
    )]
    async fn execute_block(&self, Parameters(input): Parameters<ExecuteBlockInput>) -> String {
        if let Err(e) = self.check_execute_rate_limit().await {
            return serde_json::json!({"error": e}).to_string();
        }
        tools::blocks::execute_block(
            &self.vault_path,
            &input.note_path,
            &input.alias,
            &self.registry,
            &self.pool,
        )
        .await
    }

    #[tool(description = "List all environments.")]
    async fn list_environments(
        &self,
        Parameters(_input): Parameters<ListEnvironmentsInput>,
    ) -> String {
        tools::environments::list_environments(&self.pool).await
    }

    #[tool(description = "Get all variables for an environment.")]
    async fn get_environment_variables(
        &self,
        Parameters(input): Parameters<GetEnvironmentVariablesInput>,
    ) -> String {
        tools::environments::get_environment_variables(&self.pool, &input.environment_id).await
    }

    #[tool(description = "Set the active environment. Pass null to deactivate all.")]
    async fn set_active_environment(
        &self,
        Parameters(input): Parameters<SetActiveEnvironmentInput>,
    ) -> String {
        tools::environments::set_active_environment(&self.pool, input.environment_id.as_deref())
            .await
    }

    #[tool(description = "List all configured database connections.")]
    async fn list_connections(
        &self,
        Parameters(_input): Parameters<ListConnectionsInput>,
    ) -> String {
        tools::connections::list_connections(&self.pool).await
    }

    #[tool(description = "Get the database schema (tables and columns) for a connection.")]
    async fn get_db_schema(&self, Parameters(input): Parameters<GetDbSchemaInput>) -> String {
        tools::connections::get_db_schema(&self.pool, &self.conn_manager, &input.connection_id)
            .await
    }

    #[tool(description = "Test connectivity to a database connection.")]
    async fn test_connection(&self, Parameters(input): Parameters<TestConnectionInput>) -> String {
        tools::connections::test_connection(&self.conn_manager, &input.connection_id).await
    }
}

#[tool_handler]
impl ServerHandler for NotesMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("httui-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "MCP server for httui-notes. Interact with markdown notes containing \
                 executable blocks (HTTP requests, database queries, E2E tests). \
                 You can read, create, and edit notes, execute blocks, manage environments, \
                 and query database schemas."
                    .to_string(),
            )
    }
}
