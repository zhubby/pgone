mod provider;
mod runtime;
mod tools;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use pgone_mcp::adapter::SqlSessionIntrospector;
use pgone_mcp::core::models::{
    DatabaseSchema, IntrospectOptions, RoutineDetail, RoutineKind, TableDetail, TriggerDetail,
    TypeDetail, TypeKind,
};
use pgone_mcp::formatters::{dbml, mermaid};
use pgone_sql::Session;
use pgone_storage::service::StorageService;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub use pgone_llm::Config as LlmConfig;
pub use provider::{
    ChatMessage, ChatMessageDelta, LlmProvider, OpenAiCompatibleProvider, ProviderChatRequest,
    ProviderChatResponse, ProviderChatStream, ProviderChatStreamEvent, ProviderConfig,
    ToolCallDelta, ToolCallFunctionDelta, ToolDefinition,
};
pub use runtime::{AgentRuntime, RunLimits};
pub use tools::ToolRegistry;

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("agent configuration error: {0}")]
    Config(String),
    #[error("agent provider error: {0}")]
    Provider(String),
    #[error("agent tool error: {0}")]
    Tool(String),
}

pub type Result<T> = std::result::Result<T, AgentError>;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentContext {
    pub dbconfig_id: Option<String>,
    pub database_name: Option<String>,
    pub selected_schema: Option<String>,
    pub selected_table: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentTurnRequest {
    pub session_id: String,
    pub message: String,
    pub context: AgentContext,
    #[serde(default)]
    pub history: Vec<AgentMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentTurnResponse {
    pub session_id: String,
    pub message: AgentMessage,
    pub status: AgentTurnStatus,
    pub tool_calls: Vec<AgentToolCallSummary>,
    pub events: Vec<AgentEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTurnStatus {
    Completed,
    Partial,
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentMessage {
    pub role: AgentRole,
    pub content: String,
}

impl AgentMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: AgentRole::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: AgentRole::Assistant,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: AgentRole::System,
            content: content.into(),
        }
    }

    pub fn tool(content: impl Into<String>) -> Self {
        Self {
            role: AgentRole::Tool,
            content: content.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    ToolStarted {
        name: String,
        arguments: Value,
    },
    ToolFinished {
        name: String,
        result: String,
    },
    ToolFailed {
        name: String,
        error: String,
    },
    Completed {
        status: AgentTurnStatus,
        summary: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentToolCallSummary {
    pub name: String,
    pub arguments: Value,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub struct RenderedDiagram {
    pub content: String,
}

#[async_trait]
pub trait AgentToolServices: Send + Sync {
    async fn health_check(&self, dbconfig_id: &str) -> Result<Value>;

    async fn introspect_database(
        &self,
        dbconfig_id: &str,
        opts: IntrospectOptions,
    ) -> Result<DatabaseSchema>;

    async fn get_table(&self, dbconfig_id: &str, schema: &str, table: &str) -> Result<TableDetail>;

    async fn list_triggers(
        &self,
        dbconfig_id: &str,
        schema: Option<&str>,
    ) -> Result<Vec<TriggerDetail>>;

    async fn list_routines(
        &self,
        dbconfig_id: &str,
        schema: Option<&str>,
        kind: Option<RoutineKind>,
    ) -> Result<Vec<RoutineDetail>>;

    async fn list_types(
        &self,
        dbconfig_id: &str,
        schema: Option<&str>,
        kind: Option<TypeKind>,
    ) -> Result<Vec<TypeDetail>>;

    async fn render_er(
        &self,
        dbconfig_id: &str,
        schemas: Option<Vec<String>>,
    ) -> Result<RenderedDiagram>;

    async fn render_dbml(
        &self,
        dbconfig_id: &str,
        schemas: Option<Vec<String>>,
    ) -> Result<RenderedDiagram>;
}

#[derive(Clone, Debug)]
pub struct StorageBackedAgentToolServices {
    storage_path: PathBuf,
}

impl StorageBackedAgentToolServices {
    #[must_use]
    pub fn new() -> Self {
        Self {
            storage_path: pgone_storage::database_path(),
        }
    }

    #[must_use]
    pub fn with_path(path: impl AsRef<Path>) -> Self {
        Self {
            storage_path: path.as_ref().to_path_buf(),
        }
    }

    async fn session(&self, dbconfig_id: &str) -> Result<Session> {
        let storage = StorageService::open_local(
            self.storage_path
                .to_str()
                .ok_or_else(|| AgentError::Config("storage path is not valid UTF-8".to_owned()))?,
        )
        .await
        .map_err(|error| AgentError::Tool(error.to_string()))?;
        let config = storage
            .get_db_config(dbconfig_id)
            .await
            .map_err(|error| AgentError::Tool(error.to_string()))?
            .ok_or_else(|| {
                AgentError::Config(format!("database config not found: {dbconfig_id}"))
            })?;
        Session::new(&config.dsn)
            .await
            .map_err(|error| AgentError::Tool(error.to_string()))
    }

    async fn introspector(&self, dbconfig_id: &str) -> Result<SqlSessionIntrospector> {
        Ok(SqlSessionIntrospector::new(
            self.session(dbconfig_id).await?,
        ))
    }
}

impl Default for StorageBackedAgentToolServices {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentToolServices for StorageBackedAgentToolServices {
    async fn health_check(&self, dbconfig_id: &str) -> Result<Value> {
        let session = self.session(dbconfig_id).await?;
        let database = session
            .current_database()
            .await
            .map_err(|error| AgentError::Tool(error.to_string()))?;
        Ok(json!({"ok": true, "database": database}))
    }

    async fn introspect_database(
        &self,
        dbconfig_id: &str,
        opts: IntrospectOptions,
    ) -> Result<DatabaseSchema> {
        self.introspector(dbconfig_id)
            .await?
            .introspect_database(opts)
            .await
            .map_err(|error| AgentError::Tool(error.to_string()))
    }

    async fn get_table(&self, dbconfig_id: &str, schema: &str, table: &str) -> Result<TableDetail> {
        self.introspector(dbconfig_id)
            .await?
            .get_table(schema, table)
            .await
            .map_err(|error| AgentError::Tool(error.to_string()))
    }

    async fn list_triggers(
        &self,
        dbconfig_id: &str,
        schema: Option<&str>,
    ) -> Result<Vec<TriggerDetail>> {
        self.introspector(dbconfig_id)
            .await?
            .list_triggers(schema)
            .await
            .map_err(|error| AgentError::Tool(error.to_string()))
    }

    async fn list_routines(
        &self,
        dbconfig_id: &str,
        schema: Option<&str>,
        kind: Option<RoutineKind>,
    ) -> Result<Vec<RoutineDetail>> {
        self.introspector(dbconfig_id)
            .await?
            .list_routines(schema, kind)
            .await
            .map_err(|error| AgentError::Tool(error.to_string()))
    }

    async fn list_types(
        &self,
        dbconfig_id: &str,
        schema: Option<&str>,
        kind: Option<TypeKind>,
    ) -> Result<Vec<TypeDetail>> {
        self.introspector(dbconfig_id)
            .await?
            .list_types(schema, kind)
            .await
            .map_err(|error| AgentError::Tool(error.to_string()))
    }

    async fn render_er(
        &self,
        dbconfig_id: &str,
        schemas: Option<Vec<String>>,
    ) -> Result<RenderedDiagram> {
        let db = self
            .introspect_database(dbconfig_id, diagram_options(schemas))
            .await?;
        Ok(RenderedDiagram {
            content: mermaid::render_er(&db),
        })
    }

    async fn render_dbml(
        &self,
        dbconfig_id: &str,
        schemas: Option<Vec<String>>,
    ) -> Result<RenderedDiagram> {
        let db = self
            .introspect_database(dbconfig_id, diagram_options(schemas))
            .await?;
        Ok(RenderedDiagram {
            content: dbml::render_dbml(&db),
        })
    }
}

fn diagram_options(schemas: Option<Vec<String>>) -> IntrospectOptions {
    IntrospectOptions {
        schemas,
        with_indexes: false,
        with_routines: false,
        with_types: false,
        with_triggers: false,
        page: None,
        page_size: None,
    }
}

#[derive(Clone)]
pub struct PgOneAgentService<S> {
    runtime: AgentRuntime,
    services: Arc<S>,
}

impl<S> PgOneAgentService<S>
where
    S: AgentToolServices + 'static,
{
    #[must_use]
    pub fn new(provider: Arc<dyn LlmProvider>, services: Arc<S>) -> Self {
        Self {
            runtime: AgentRuntime::new(provider),
            services,
        }
    }

    #[must_use]
    pub fn with_limits(mut self, limits: RunLimits) -> Self {
        self.runtime = self.runtime.with_limits(limits);
        self
    }

    pub async fn run_agent_turn(&self, request: AgentTurnRequest) -> Result<AgentTurnResponse> {
        self.runtime.run_turn(request, self.services.clone()).await
    }
}
