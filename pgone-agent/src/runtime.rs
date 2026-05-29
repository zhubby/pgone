use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use serde_json::{Value, json};
use tokio::time::timeout;

use crate::provider::{
    ChatMessage, LlmProvider, ProviderChatRequest, ProviderChatResponse, ProviderChatStreamEvent,
};
use crate::tools::{ToolExecutionRecord, ToolRegistry};
use crate::{
    AgentError, AgentEvent, AgentMessage, AgentRole, AgentStreamEvent, AgentToolCallSummary,
    AgentToolServices, AgentTurnRequest, AgentTurnResponse, AgentTurnStatus, Result,
};

#[derive(Clone, Debug)]
pub struct RunLimits {
    pub max_tool_iterations: u32,
    pub max_tool_calls: u32,
    pub provider_timeout: Duration,
    pub tool_timeout: Duration,
}

impl Default for RunLimits {
    fn default() -> Self {
        Self {
            max_tool_iterations: 8,
            max_tool_calls: 24,
            provider_timeout: Duration::from_secs(60),
            tool_timeout: Duration::from_secs(20),
        }
    }
}

#[derive(Clone)]
pub struct AgentRuntime {
    provider: Arc<dyn LlmProvider>,
    tools: ToolRegistry,
    limits: RunLimits,
}

impl AgentRuntime {
    #[must_use]
    pub fn new(provider: Arc<dyn LlmProvider>) -> Self {
        Self {
            provider,
            tools: ToolRegistry::pgone_readonly(),
            limits: RunLimits::default(),
        }
    }

    #[must_use]
    pub fn with_limits(mut self, limits: RunLimits) -> Self {
        self.limits = limits;
        self
    }

    #[tracing::instrument(name = "agent.run_turn", skip_all, fields(session_id = %request.session_id))]
    pub async fn run_turn<S>(
        &self,
        request: AgentTurnRequest,
        services: Arc<S>,
    ) -> Result<AgentTurnResponse>
    where
        S: AgentToolServices + 'static,
    {
        let dbconfig_id = request.context.dbconfig_id.clone().ok_or_else(|| {
            AgentError::Config("agent context dbconfig_id is required".to_owned())
        })?;
        self.run_turn_inner(request, services, dbconfig_id, None)
            .await
    }

    pub async fn run_turn_stream<S>(
        &self,
        request: AgentTurnRequest,
        services: Arc<S>,
        mut sender: tokio::sync::mpsc::Sender<AgentStreamEvent>,
    ) -> Result<AgentTurnResponse>
    where
        S: AgentToolServices + 'static,
    {
        let dbconfig_id = request.context.dbconfig_id.clone().ok_or_else(|| {
            AgentError::Config("agent context dbconfig_id is required".to_owned())
        })?;
        let response = self
            .run_turn_inner(request, services, dbconfig_id, Some(&mut sender))
            .await;
        match &response {
            Ok(response) => {
                let _ = sender
                    .send(AgentStreamEvent::Completed {
                        response: response.clone(),
                    })
                    .await;
            }
            Err(error) => {
                let _ = sender
                    .send(AgentStreamEvent::Failed {
                        error: error.to_string(),
                    })
                    .await;
            }
        }
        response
    }

    async fn run_turn_inner<S>(
        &self,
        request: AgentTurnRequest,
        services: Arc<S>,
        dbconfig_id: String,
        mut stream_sender: Option<&mut tokio::sync::mpsc::Sender<AgentStreamEvent>>,
    ) -> Result<AgentTurnResponse>
    where
        S: AgentToolServices + 'static,
    {
        let session_id = request.session_id.clone();
        let mut messages = self.build_messages(&request);
        let mut events = Vec::new();
        let mut tool_calls = Vec::new();
        let mut tool_call_count = 0_u32;
        let services: Arc<dyn AgentToolServices> = services;

        for _ in 0..self.limits.max_tool_iterations {
            let request_for_provider = ProviderChatRequest {
                messages: messages.clone(),
                tools: self.tools.definitions(),
            };
            let response = if let Some(sender) = stream_sender.as_deref_mut() {
                self.provider_response_streaming(request_for_provider, sender)
                    .await?
            } else {
                timeout(
                    self.limits.provider_timeout,
                    self.provider.chat(request_for_provider),
                )
                .await
                .map_err(|_| AgentError::Provider("agent provider timed out".to_owned()))??
            };

            let assistant_message = response.message.clone();
            messages.push(assistant_message.clone());

            if assistant_message.tool_calls.is_empty() {
                let content = assistant_message.content.unwrap_or_default();
                return Ok(AgentTurnResponse {
                    session_id,
                    message: AgentMessage::assistant(content),
                    status: AgentTurnStatus::Completed,
                    tool_calls,
                    events,
                });
            }

            for tool_call in assistant_message.tool_calls {
                tool_call_count += 1;
                if tool_call_count > self.limits.max_tool_calls {
                    return Ok(limit_response(
                        session_id,
                        "Agent stopped because it reached the tool call limit.",
                        tool_calls,
                        events,
                    ));
                }

                let arguments =
                    parse_tool_arguments(&tool_call.function.arguments).unwrap_or_else(|error| {
                        json!({
                            "parse_error": error,
                            "raw_arguments": tool_call.function.arguments,
                        })
                    });
                let name = tool_call.function.name;
                events.push(AgentEvent::ToolStarted {
                    name: name.clone(),
                    arguments: arguments.clone(),
                });
                if let Some(sender) = stream_sender.as_deref_mut() {
                    let _ = sender
                        .send(AgentStreamEvent::Event {
                            event: AgentEvent::ToolStarted {
                                name: name.clone(),
                                arguments: arguments.clone(),
                            },
                        })
                        .await;
                }

                let record = self
                    .execute_tool(
                        name.clone(),
                        arguments.clone(),
                        &request.context,
                        &dbconfig_id,
                        services.clone(),
                    )
                    .await;
                events.push(record.event.clone());
                if let Some(sender) = stream_sender.as_deref_mut() {
                    let _ = sender
                        .send(AgentStreamEvent::Event {
                            event: record.event.clone(),
                        })
                        .await;
                }
                tool_calls.push(record.summary.clone());

                match record.output {
                    Some(output) => {
                        messages.push(ChatMessage::tool(tool_call.id, output.content.clone()));
                        if let Some(completion) = output.completion {
                            events.push(AgentEvent::Completed {
                                status: completion.status.clone(),
                                summary: completion.summary.clone(),
                            });
                            if let Some(sender) = stream_sender.as_deref_mut() {
                                let _ = sender
                                    .send(AgentStreamEvent::Event {
                                        event: AgentEvent::Completed {
                                            status: completion.status.clone(),
                                            summary: completion.summary.clone(),
                                        },
                                    })
                                    .await;
                            }
                            return Ok(AgentTurnResponse {
                                session_id,
                                message: AgentMessage::assistant(completion.summary),
                                status: completion.status,
                                tool_calls,
                                events,
                            });
                        }
                    }
                    None => {
                        let error = record
                            .summary
                            .error
                            .unwrap_or_else(|| "tool failed".to_owned());
                        messages.push(ChatMessage::tool(tool_call.id, error));
                    }
                }
            }
        }

        Ok(limit_response(
            session_id,
            "Agent stopped because it reached the tool iteration limit.",
            tool_calls,
            events,
        ))
    }

    fn build_messages(&self, request: &AgentTurnRequest) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage::system(system_prompt(request))];
        messages.extend(request.history.iter().filter_map(history_message));
        messages.push(ChatMessage::user(request.message.clone()));
        messages
    }

    async fn execute_tool(
        &self,
        name: String,
        arguments: Value,
        context: &crate::AgentContext,
        dbconfig_id: &str,
        services: Arc<dyn AgentToolServices>,
    ) -> ToolExecutionRecord {
        let Some(tool) = self.tools.get(&name) else {
            return ToolExecutionRecord::failure(
                name,
                arguments,
                "tool is not registered".to_owned(),
            );
        };

        match timeout(
            self.limits.tool_timeout,
            tool.execute(arguments.clone(), dbconfig_id, context, services),
        )
        .await
        {
            Ok(Ok(output)) => {
                let result = output.content.clone();
                ToolExecutionRecord::success(name, arguments, result, output)
            }
            Ok(Err(error)) => ToolExecutionRecord::failure(name, arguments, error.to_string()),
            Err(_) => ToolExecutionRecord::failure(name, arguments, "tool timed out".to_owned()),
        }
    }

    async fn provider_response_streaming(
        &self,
        request: ProviderChatRequest,
        sender: &mut tokio::sync::mpsc::Sender<AgentStreamEvent>,
    ) -> Result<ProviderChatResponse> {
        let mut stream = timeout(
            self.limits.provider_timeout,
            self.provider.chat_stream(request),
        )
        .await
        .map_err(|_| AgentError::Provider("agent provider timed out".to_owned()))??;
        while let Some(event) = timeout(self.limits.provider_timeout, stream.next())
            .await
            .map_err(|_| AgentError::Provider("agent provider timed out".to_owned()))?
        {
            match event? {
                ProviderChatStreamEvent::Delta(delta) => {
                    if let Some(content) = delta.content {
                        let _ = sender
                            .send(AgentStreamEvent::AssistantDelta { content })
                            .await;
                    }
                }
                ProviderChatStreamEvent::Completed(response) => return Ok(response),
            }
        }
        Err(AgentError::Provider(
            "LLM stream ended before a completed response".to_owned(),
        ))
    }
}

fn history_message(message: &AgentMessage) -> Option<ChatMessage> {
    match message.role {
        AgentRole::User => Some(ChatMessage::user(message.content.clone())),
        AgentRole::Assistant => Some(ChatMessage::assistant(message.content.clone())),
        AgentRole::System => Some(ChatMessage::system(message.content.clone())),
        AgentRole::Tool => None,
    }
}

fn system_prompt(request: &AgentTurnRequest) -> String {
    let context = &request.context;
    let dbconfig_id = context.dbconfig_id.as_deref().unwrap_or("missing");
    let database = context.database_name.as_deref().unwrap_or("unknown");
    let selected_schema = context.selected_schema.as_deref().unwrap_or("none");
    let selected_table = context.selected_table.as_deref().unwrap_or("none");

    format!(
        r#"You are PgOne's PostgreSQL database understanding agent.

Help the user inspect and understand PostgreSQL database metadata from inside PgOne.
Use tools to inspect real database metadata before making factual claims about schemas, tables, columns, indexes, triggers, routines, types, or relationships.

Current UI context:
- Database config id: {dbconfig_id} (PostgreSQL instance connection identity)
- Default target database name: {database}
- Selected schema: {selected_schema}
- Selected table: {selected_table}

Available behavior:
- Prefer read-only database metadata inspection tools.
- Metadata and query tools can target a database with database_name. When the user names a database, pass that name explicitly instead of relying only on the default target database.
- Use list_databases to discover available databases on the selected PostgreSQL instance when the target database is unclear.
- Use execute_readonly_sql only for safe read-only inspection queries.
- When the user asks you to generate SQL for review or execution, call preview_sql to send the SQL to the SQL panel.
- Mutating SQL such as CREATE, ALTER, DROP, INSERT, UPDATE, DELETE, TRUNCATE, GRANT, or REVOKE must be sent with preview_sql; do not claim you executed it.
- Do not claim you changed schema, data, permissions, or configuration; mutating execution tools are not available.
- When you have completed the user's request, call complete_task with a concise summary.
- If you are blocked, call complete_task with status "blocked" and explain what is missing.
"#
    )
}

fn parse_tool_arguments(arguments: &str) -> std::result::Result<Value, String> {
    if arguments.trim().is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    serde_json::from_str(arguments).map_err(|error| error.to_string())
}

fn limit_response(
    session_id: String,
    content: &str,
    tool_calls: Vec<AgentToolCallSummary>,
    mut events: Vec<AgentEvent>,
) -> AgentTurnResponse {
    events.push(AgentEvent::Completed {
        status: AgentTurnStatus::Partial,
        summary: content.to_owned(),
    });
    AgentTurnResponse {
        session_id,
        message: AgentMessage::assistant(content),
        status: AgentTurnStatus::Partial,
        tool_calls,
        events,
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use pgone_mcp::core::models::{
        Column, DatabaseSchema, ForeignKey, Index, PrimaryKey, RoutineDetail, RoutineKind, Schema,
        TableDetail, TriggerDetail, TypeDetail, TypeKind, ViewDetail,
    };
    use tokio::sync::Mutex;

    use super::*;
    use crate::provider::{
        ChatMessageDelta, ProviderChatResponse, ProviderChatStream, ProviderChatStreamEvent,
        ToolCall, ToolCallFunction, ToolDefinition,
    };
    use crate::{RenderedDiagram, Result};

    struct MockProvider {
        responses: Mutex<Vec<ChatMessage>>,
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _request: ProviderChatRequest) -> Result<ProviderChatResponse> {
            Ok(ProviderChatResponse {
                message: self.responses.lock().await.remove(0),
            })
        }
    }

    struct MockStreamingProvider {
        responses: Mutex<Vec<Vec<ProviderChatStreamEvent>>>,
    }

    #[async_trait]
    impl LlmProvider for MockStreamingProvider {
        async fn chat(&self, _request: ProviderChatRequest) -> Result<ProviderChatResponse> {
            unreachable!("streaming tests use chat_stream")
        }

        async fn chat_stream(&self, _request: ProviderChatRequest) -> Result<ProviderChatStream> {
            let events = self.responses.lock().await.remove(0);
            Ok(Box::pin(futures::stream::iter(events.into_iter().map(Ok))))
        }
    }

    #[derive(Clone)]
    struct DummyServices;

    #[async_trait]
    impl AgentToolServices for DummyServices {
        async fn health_check(
            &self,
            _dbconfig_id: &str,
            _database_name: Option<&str>,
        ) -> Result<Value> {
            Ok(json!({"ok": true}))
        }

        async fn list_databases(
            &self,
            _dbconfig_id: &str,
        ) -> Result<Vec<pgone_sql::models::DatabaseInfo>> {
            Ok(Vec::new())
        }

        async fn introspect_database(
            &self,
            _dbconfig_id: &str,
            _database_name: Option<&str>,
            _opts: pgone_mcp::core::models::IntrospectOptions,
        ) -> Result<DatabaseSchema> {
            Ok(database_schema())
        }

        async fn get_table(
            &self,
            _dbconfig_id: &str,
            _database_name: Option<&str>,
            _schema: &str,
            _table: &str,
        ) -> Result<TableDetail> {
            Ok(table_detail())
        }

        async fn list_triggers(
            &self,
            _dbconfig_id: &str,
            _database_name: Option<&str>,
            _schema: Option<&str>,
        ) -> Result<Vec<TriggerDetail>> {
            Ok(Vec::new())
        }

        async fn list_routines(
            &self,
            _dbconfig_id: &str,
            _database_name: Option<&str>,
            _schema: Option<&str>,
            _kind: Option<RoutineKind>,
        ) -> Result<Vec<RoutineDetail>> {
            Ok(Vec::new())
        }

        async fn list_types(
            &self,
            _dbconfig_id: &str,
            _database_name: Option<&str>,
            _schema: Option<&str>,
            _kind: Option<TypeKind>,
        ) -> Result<Vec<TypeDetail>> {
            Ok(Vec::new())
        }

        async fn render_er(
            &self,
            _dbconfig_id: &str,
            _database_name: Option<&str>,
            _schemas: Option<Vec<String>>,
        ) -> Result<RenderedDiagram> {
            Ok(RenderedDiagram {
                content: "erDiagram".to_owned(),
            })
        }

        async fn render_dbml(
            &self,
            _dbconfig_id: &str,
            _database_name: Option<&str>,
            _schemas: Option<Vec<String>>,
        ) -> Result<RenderedDiagram> {
            Ok(RenderedDiagram {
                content: "Table public.users {}".to_owned(),
            })
        }

        async fn execute_readonly_sql(
            &self,
            _dbconfig_id: &str,
            _request: crate::ReadonlySqlRequest,
        ) -> Result<crate::ReadonlySqlResult> {
            Ok(crate::ReadonlySqlResult {
                columns: vec!["id".to_owned()],
                rows: vec![vec!["1".to_owned()]],
                row_count: 1,
                truncated: false,
                explain: Some("Result".to_owned()),
            })
        }
    }

    fn request() -> AgentTurnRequest {
        AgentTurnRequest {
            session_id: "agent-1".to_owned(),
            message: "List tables".to_owned(),
            context: crate::AgentContext {
                dbconfig_id: Some("local".to_owned()),
                database_name: Some("app".to_owned()),
                selected_schema: Some("public".to_owned()),
                selected_table: None,
            },
            history: Vec::new(),
        }
    }

    fn provider(responses: Vec<ChatMessage>) -> Arc<dyn LlmProvider> {
        Arc::new(MockProvider {
            responses: Mutex::new(responses),
        })
    }

    fn streaming_provider(responses: Vec<Vec<ProviderChatStreamEvent>>) -> Arc<dyn LlmProvider> {
        Arc::new(MockStreamingProvider {
            responses: Mutex::new(responses),
        })
    }

    fn table_detail() -> TableDetail {
        TableDetail {
            schema: "public".to_owned(),
            name: "users".to_owned(),
            comment: None,
            columns: vec![Column {
                name: "id".to_owned(),
                data_type: "integer".to_owned(),
                udt_name: None,
                nullable: false,
                default: None,
                character_maximum_length: None,
                numeric_precision: None,
                numeric_scale: None,
                comment: None,
            }],
            primary_key: Some(PrimaryKey {
                columns: vec!["id".to_owned()],
            }),
            foreign_keys: Vec::<ForeignKey>::new(),
            indexes: Vec::<Index>::new(),
        }
    }

    fn database_schema() -> DatabaseSchema {
        DatabaseSchema {
            database: "app".to_owned(),
            schemas: vec![Schema {
                name: "public".to_owned(),
                tables: vec![table_detail()],
                views: Vec::<ViewDetail>::new(),
            }],
        }
    }

    #[tokio::test]
    async fn returns_final_message_without_tools() {
        let runtime = AgentRuntime::new(provider(vec![ChatMessage::assistant("done")]));

        let response = runtime
            .run_turn(request(), Arc::new(DummyServices))
            .await
            .unwrap();

        assert_eq!(response.message.content, "done");
        assert_eq!(response.status, AgentTurnStatus::Completed);
        assert!(response.tool_calls.is_empty());
    }

    #[tokio::test]
    async fn stops_when_complete_task_is_called() {
        let runtime = AgentRuntime::new(provider(vec![ChatMessage {
            role: "assistant".to_owned(),
            content: None,
            tool_call_id: None,
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                r#type: "function".to_owned(),
                function: ToolCallFunction {
                    name: "complete_task".to_owned(),
                    arguments: r#"{"summary":"checked","status":"success"}"#.to_owned(),
                },
            }],
        }]));

        let response = runtime
            .run_turn(request(), Arc::new(DummyServices))
            .await
            .unwrap();

        assert_eq!(response.message.content, "checked");
        assert_eq!(response.status, AgentTurnStatus::Completed);
        assert_eq!(response.tool_calls[0].name, "complete_task");
    }

    #[tokio::test]
    async fn stops_when_preview_sql_is_called() {
        let runtime = AgentRuntime::new(provider(vec![ChatMessage {
            role: "assistant".to_owned(),
            content: None,
            tool_call_id: None,
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                r#type: "function".to_owned(),
                function: ToolCallFunction {
                    name: "preview_sql".to_owned(),
                    arguments: r#"{"title":"Create audit table","sql":"CREATE TABLE audit_log (id bigint);"}"#.to_owned(),
                },
            }],
        }]));

        let response = runtime
            .run_turn(request(), Arc::new(DummyServices))
            .await
            .unwrap();

        assert_eq!(
            response.message.content,
            "SQL has been sent to the SQL panel. Please review it before executing."
        );
        assert_eq!(response.status, AgentTurnStatus::Completed);
        assert_eq!(response.tool_calls[0].name, "preview_sql");
        assert!(
            response.tool_calls[0]
                .result
                .as_deref()
                .is_some_and(|result| result.contains("CREATE TABLE audit_log"))
        );
    }

    #[test]
    fn system_prompt_guides_generated_sql_to_preview_tool() {
        let prompt = system_prompt(&request());

        assert!(prompt.contains("preview_sql"));
        assert!(prompt.contains("execute_readonly_sql only for safe read-only"));
        assert!(prompt.contains("Mutating SQL"));
    }

    #[tokio::test]
    async fn unknown_tool_failure_is_returned_to_provider() {
        let runtime = AgentRuntime::new(provider(vec![
            ChatMessage {
                role: "assistant".to_owned(),
                content: None,
                tool_call_id: None,
                tool_calls: vec![ToolCall {
                    id: "call-1".to_owned(),
                    r#type: "function".to_owned(),
                    function: ToolCallFunction {
                        name: "missing_tool".to_owned(),
                        arguments: "{}".to_owned(),
                    },
                }],
            },
            ChatMessage::assistant("recovered"),
        ]));

        let response = runtime
            .run_turn(request(), Arc::new(DummyServices))
            .await
            .unwrap();

        assert_eq!(response.message.content, "recovered");
        assert_eq!(response.tool_calls[0].name, "missing_tool");
        assert_eq!(
            response.tool_calls[0].error.as_deref(),
            Some("tool is not registered")
        );
    }

    #[tokio::test]
    async fn stops_at_tool_call_limit() {
        let runtime = AgentRuntime::new(provider(vec![ChatMessage {
            role: "assistant".to_owned(),
            content: None,
            tool_call_id: None,
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                r#type: "function".to_owned(),
                function: ToolCallFunction {
                    name: "health_check".to_owned(),
                    arguments: "{}".to_owned(),
                },
            }],
        }]))
        .with_limits(RunLimits {
            max_tool_iterations: 8,
            max_tool_calls: 0,
            provider_timeout: Duration::from_secs(60),
            tool_timeout: Duration::from_secs(20),
        });

        let response = runtime
            .run_turn(request(), Arc::new(DummyServices))
            .await
            .unwrap();

        assert_eq!(response.status, AgentTurnStatus::Partial);
        assert!(response.message.content.contains("tool call limit"));
    }

    #[tokio::test]
    async fn stops_at_tool_iteration_limit() {
        let runtime = AgentRuntime::new(provider(vec![ChatMessage {
            role: "assistant".to_owned(),
            content: None,
            tool_call_id: None,
            tool_calls: vec![ToolCall {
                id: "call-1".to_owned(),
                r#type: "function".to_owned(),
                function: ToolCallFunction {
                    name: "health_check".to_owned(),
                    arguments: "{}".to_owned(),
                },
            }],
        }]))
        .with_limits(RunLimits {
            max_tool_iterations: 1,
            max_tool_calls: 24,
            provider_timeout: Duration::from_secs(60),
            tool_timeout: Duration::from_secs(20),
        });

        let response = runtime
            .run_turn(request(), Arc::new(DummyServices))
            .await
            .unwrap();

        assert_eq!(response.status, AgentTurnStatus::Partial);
        assert!(response.message.content.contains("tool iteration limit"));
        assert_eq!(response.tool_calls[0].name, "health_check");
    }

    #[tokio::test]
    async fn readonly_registry_exposes_complete_task() {
        let definitions = ToolRegistry::pgone_readonly()
            .definitions()
            .into_iter()
            .map(|definition: ToolDefinition| definition.name)
            .collect::<Vec<_>>();

        assert!(definitions.contains(&"introspect_database".to_owned()));
        assert!(definitions.contains(&"get_table".to_owned()));
        assert!(definitions.contains(&"complete_task".to_owned()));
    }

    #[tokio::test]
    async fn streams_assistant_deltas_and_completion() {
        let runtime = AgentRuntime::new(streaming_provider(vec![vec![
            ProviderChatStreamEvent::Delta(ChatMessageDelta {
                role: Some("assistant".to_owned()),
                content: Some("do".to_owned()),
                tool_calls: Vec::new(),
                finish_reason: None,
            }),
            ProviderChatStreamEvent::Delta(ChatMessageDelta {
                role: None,
                content: Some("ne".to_owned()),
                tool_calls: Vec::new(),
                finish_reason: None,
            }),
            ProviderChatStreamEvent::Completed(ProviderChatResponse {
                message: ChatMessage::assistant("done"),
            }),
        ]]));
        let (sender, mut receiver) = tokio::sync::mpsc::channel(8);

        let response = runtime
            .run_turn_stream(request(), Arc::new(DummyServices), sender)
            .await
            .unwrap();

        assert_eq!(response.message.content, "done");
        assert!(matches!(
            receiver.recv().await,
            Some(AgentStreamEvent::AssistantDelta { content }) if content == "do"
        ));
        assert!(matches!(
            receiver.recv().await,
            Some(AgentStreamEvent::AssistantDelta { content }) if content == "ne"
        ));
        assert!(matches!(
            receiver.recv().await,
            Some(AgentStreamEvent::Completed { .. })
        ));
    }

    #[tokio::test]
    async fn streams_tool_events_before_completion() {
        let runtime = AgentRuntime::new(streaming_provider(vec![
            vec![ProviderChatStreamEvent::Completed(ProviderChatResponse {
                message: ChatMessage {
                    role: "assistant".to_owned(),
                    content: None,
                    tool_call_id: None,
                    tool_calls: vec![ToolCall {
                        id: "call-1".to_owned(),
                        r#type: "function".to_owned(),
                        function: ToolCallFunction {
                            name: "health_check".to_owned(),
                            arguments: "{}".to_owned(),
                        },
                    }],
                },
            })],
            vec![ProviderChatStreamEvent::Completed(ProviderChatResponse {
                message: ChatMessage::assistant("checked"),
            })],
        ]));
        let (sender, mut receiver) = tokio::sync::mpsc::channel(8);

        let response = runtime
            .run_turn_stream(request(), Arc::new(DummyServices), sender)
            .await
            .unwrap();

        assert_eq!(response.message.content, "checked");
        assert!(matches!(
            receiver.recv().await,
            Some(AgentStreamEvent::Event {
                event: AgentEvent::ToolStarted { name, .. }
            }) if name == "health_check"
        ));
        assert!(matches!(
            receiver.recv().await,
            Some(AgentStreamEvent::Event {
                event: AgentEvent::ToolFinished { name, .. }
            }) if name == "health_check"
        ));
        assert!(matches!(
            receiver.recv().await,
            Some(AgentStreamEvent::Completed { .. })
        ));
    }
}
