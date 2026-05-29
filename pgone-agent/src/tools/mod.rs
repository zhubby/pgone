use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::provider::ToolDefinition;
use crate::{
    AgentContext, AgentError, AgentEvent, AgentToolCallSummary, AgentToolServices, AgentTurnStatus,
    Result,
};

mod complete_task;
mod get_table;
mod health_check;
mod introspect_database;
mod list_databases;
mod list_routines;
mod list_triggers;
mod list_types;
mod preview_sql;
mod render_dbml;
mod render_er;
mod render_sql_result;

use complete_task::CompleteTaskTool;
use get_table::GetTableTool;
use health_check::HealthCheckTool;
use introspect_database::IntrospectDatabaseTool;
use list_databases::ListDatabasesTool;
use list_routines::ListRoutinesTool;
use list_triggers::ListTriggersTool;
use list_types::ListTypesTool;
use preview_sql::PreviewSqlTool;
use render_dbml::RenderDbmlTool;
use render_er::RenderErTool;
use render_sql_result::RenderSqlResultTool;
pub use render_sql_result::validate_readonly_sql;

#[derive(Clone, Debug)]
pub struct ToolOutput {
    pub content: String,
    pub completion: Option<CompletionSignal>,
    pub ui_payload: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct CompletionSignal {
    pub status: AgentTurnStatus,
    pub summary: String,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Value;

    async fn execute(
        &self,
        args: Value,
        dbconfig_id: &str,
        context: &AgentContext,
        services: Arc<dyn AgentToolServices>,
    ) -> Result<ToolOutput>;

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_owned(),
            description: self.description().to_owned(),
            parameters: self.parameters(),
        }
    }
}

#[derive(Clone)]
pub struct ToolRegistry {
    tools: Vec<Arc<dyn Tool>>,
}

impl ToolRegistry {
    #[must_use]
    pub fn pgone_readonly() -> Self {
        Self {
            tools: vec![
                Arc::new(HealthCheckTool),
                Arc::new(ListDatabasesTool),
                Arc::new(IntrospectDatabaseTool),
                Arc::new(GetTableTool),
                Arc::new(ListTriggersTool),
                Arc::new(ListRoutinesTool),
                Arc::new(ListTypesTool),
                Arc::new(RenderSqlResultTool),
                Arc::new(PreviewSqlTool),
                Arc::new(RenderErTool),
                Arc::new(RenderDbmlTool),
                Arc::new(CompleteTaskTool),
            ],
        }
    }

    #[must_use]
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|tool| tool.definition()).collect()
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.iter().find(|tool| tool.name() == name).cloned()
    }
}

pub struct ToolExecutionRecord {
    pub summary: AgentToolCallSummary,
    pub event: AgentEvent,
    pub output: Option<ToolOutput>,
    pub ui_payload: Option<Value>,
}

impl ToolExecutionRecord {
    pub fn success(name: String, arguments: Value, result: String, output: ToolOutput) -> Self {
        Self {
            summary: AgentToolCallSummary {
                name: name.clone(),
                arguments,
                result: Some(result.clone()),
                error: None,
            },
            event: AgentEvent::ToolFinished { name, result },
            ui_payload: output.ui_payload.clone(),
            output: Some(output),
        }
    }

    pub fn failure(name: String, arguments: Value, error: String) -> Self {
        Self {
            summary: AgentToolCallSummary {
                name: name.clone(),
                arguments,
                result: None,
                error: Some(error.clone()),
            },
            event: AgentEvent::ToolFailed { name, error },
            output: None,
            ui_payload: None,
        }
    }
}

pub(super) fn parse_args<T>(args: Value) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(args)
        .map_err(|error| AgentError::Tool(format!("invalid agent tool arguments: {error}")))
}

pub(super) fn json_output<T>(value: &T) -> Result<ToolOutput>
where
    T: serde::Serialize,
{
    let content =
        serde_json::to_string_pretty(value).map_err(|error| AgentError::Tool(error.to_string()))?;
    Ok(ToolOutput {
        content,
        completion: None,
        ui_payload: None,
    })
}

pub(super) fn target_database<'a>(
    argument_database: Option<&'a str>,
    context: &'a AgentContext,
) -> Option<&'a str> {
    argument_database
        .filter(|database| !database.trim().is_empty())
        .or(context.database_name.as_deref())
        .filter(|database| !database.trim().is_empty())
}

pub(super) fn default_true() -> bool {
    true
}

#[derive(Clone)]
pub(super) struct SchemaProperty {
    name: &'static str,
    schema: Value,
    required: bool,
}

impl SchemaProperty {
    pub(super) fn optional(mut self) -> Self {
        self.required = false;
        self
    }
}

pub(super) fn string_property(name: &'static str, description: &'static str) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({"type": "string", "description": description}),
        required: true,
    }
}

pub(super) fn database_name_property() -> SchemaProperty {
    string_property(
        "database_name",
        "Target database name on the selected PostgreSQL instance; omit to use the current UI database",
    )
}

pub(super) fn string_enum_property(
    name: &'static str,
    description: &'static str,
    values: &[&'static str],
) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({"type": "string", "enum": values, "description": description}),
        required: true,
    }
}

pub(super) fn integer_property(name: &'static str, description: &'static str) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({"type": "integer", "description": description}),
        required: true,
    }
}

pub(super) fn boolean_property(name: &'static str, description: &'static str) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({"type": "boolean", "description": description}),
        required: true,
    }
}

pub(super) fn array_string_property(
    name: &'static str,
    description: &'static str,
) -> SchemaProperty {
    SchemaProperty {
        name,
        schema: json!({
            "type": "array",
            "items": {"type": "string"},
            "description": description
        }),
        required: true,
    }
}

pub(super) fn object_schema(properties: Vec<SchemaProperty>) -> Value {
    let required = properties
        .iter()
        .filter(|property| property.required)
        .map(|property| property.name)
        .collect::<Vec<_>>();
    let properties = properties
        .into_iter()
        .map(|property| (property.name.to_owned(), property.schema))
        .collect::<serde_json::Map<_, _>>();

    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

#[cfg(test)]
pub(super) mod test_support {
    use async_trait::async_trait;
    use pgone_mcp::core::models::{
        Column, DatabaseSchema, ForeignKey, Index, IntrospectOptions, PrimaryKey, RoutineDetail,
        RoutineKind, Schema, TableDetail, TriggerDetail, TypeDetail, TypeKind, ViewDetail,
    };
    use pgone_sql::models::DatabaseInfo;
    use serde_json::{Value, json};
    use tokio::sync::Mutex;

    use crate::{
        AgentContext, AgentToolServices, ReadonlySqlRequest, ReadonlySqlResult, RenderedDiagram,
        Result,
    };

    #[derive(Default)]
    pub(super) struct MockServices {
        seen_database_name: Mutex<Option<Option<String>>>,
    }

    impl MockServices {
        async fn record_database_name(&self, database_name: Option<&str>) {
            *self.seen_database_name.lock().await = Some(database_name.map(ToOwned::to_owned));
        }

        pub(super) async fn seen_database_name(&self) -> Option<Option<String>> {
            self.seen_database_name.lock().await.clone()
        }
    }

    #[async_trait]
    impl AgentToolServices for MockServices {
        async fn health_check(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
        ) -> Result<Value> {
            self.record_database_name(database_name).await;
            Ok(json!({"ok": true, "database": "app"}))
        }

        async fn list_databases(&self, _dbconfig_id: &str) -> Result<Vec<DatabaseInfo>> {
            Ok(vec![DatabaseInfo {
                name: "app".to_owned(),
                owner: "postgres".to_owned(),
                encoding: "UTF8".to_owned(),
                collate: None,
                ctype: None,
                size: None,
                description: None,
            }])
        }

        async fn introspect_database(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _opts: IntrospectOptions,
        ) -> Result<DatabaseSchema> {
            self.record_database_name(database_name).await;
            Ok(DatabaseSchema {
                database: "app".to_owned(),
                schemas: vec![Schema {
                    name: "public".to_owned(),
                    tables: vec![table_detail()],
                    views: Vec::<ViewDetail>::new(),
                }],
            })
        }

        async fn get_table(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            schema: &str,
            table: &str,
        ) -> Result<TableDetail> {
            self.record_database_name(database_name).await;
            assert_eq!(schema, "public");
            assert_eq!(table, "users");
            Ok(table_detail())
        }

        async fn list_triggers(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schema: Option<&str>,
        ) -> Result<Vec<TriggerDetail>> {
            self.record_database_name(database_name).await;
            Ok(Vec::new())
        }

        async fn list_routines(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schema: Option<&str>,
            _kind: Option<RoutineKind>,
        ) -> Result<Vec<RoutineDetail>> {
            self.record_database_name(database_name).await;
            Ok(Vec::new())
        }

        async fn list_types(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schema: Option<&str>,
            _kind: Option<TypeKind>,
        ) -> Result<Vec<TypeDetail>> {
            self.record_database_name(database_name).await;
            Ok(Vec::new())
        }

        async fn render_er(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schemas: Option<Vec<String>>,
        ) -> Result<RenderedDiagram> {
            self.record_database_name(database_name).await;
            Ok(RenderedDiagram {
                content: "erDiagram".to_owned(),
            })
        }

        async fn render_dbml(
            &self,
            _dbconfig_id: &str,
            database_name: Option<&str>,
            _schemas: Option<Vec<String>>,
        ) -> Result<RenderedDiagram> {
            self.record_database_name(database_name).await;
            Ok(RenderedDiagram {
                content: "Table public.users {}".to_owned(),
            })
        }

        async fn execute_readonly_sql(
            &self,
            _dbconfig_id: &str,
            request: ReadonlySqlRequest,
        ) -> Result<ReadonlySqlResult> {
            self.record_database_name(request.database_name.as_deref())
                .await;
            assert!(request.database_name.is_some());
            Ok(ReadonlySqlResult {
                columns: vec!["id".to_owned()],
                rows: vec![vec!["1".to_owned()]],
                row_count: 1,
                truncated: false,
                explain: Some("Result".to_owned()),
            })
        }
    }

    pub(super) fn table_detail() -> TableDetail {
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

    pub(super) fn context() -> AgentContext {
        AgentContext {
            dbconfig_id: Some("local".to_owned()),
            database_name: Some("app".to_owned()),
            selected_schema: Some("public".to_owned()),
            selected_table: Some("users".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn registry_exposes_database_metadata_tools() {
        let definitions = ToolRegistry::pgone_readonly()
            .definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();

        assert!(definitions.contains(&"health_check".to_owned()));
        assert!(definitions.contains(&"list_databases".to_owned()));
        assert!(definitions.contains(&"introspect_database".to_owned()));
        assert!(definitions.contains(&"get_table".to_owned()));
        assert!(definitions.contains(&"list_triggers".to_owned()));
        assert!(definitions.contains(&"list_routines".to_owned()));
        assert!(definitions.contains(&"list_types".to_owned()));
        assert!(definitions.contains(&"render_sql_result".to_owned()));
        assert!(!definitions.contains(&"execute_readonly_sql".to_owned()));
        assert!(definitions.contains(&"preview_sql".to_owned()));
        assert!(definitions.contains(&"render_er".to_owned()));
        assert!(definitions.contains(&"render_dbml".to_owned()));
        assert!(definitions.contains(&"complete_task".to_owned()));
    }

    #[test]
    fn object_schema_marks_required_properties_and_rejects_extra_properties() {
        let schema = object_schema(vec![
            string_property("required_name", "Required name"),
            string_property("optional_name", "Optional name").optional(),
        ]);

        assert_eq!(schema["required"], json!(["required_name"]));
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(
            schema["properties"]["required_name"]["type"],
            json!("string")
        );
    }
}
